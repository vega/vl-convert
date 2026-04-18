use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct BudgetTracker {
    per_ip_budget_ms: AtomicI64,
    global_budget_ms: AtomicI64,
    global_remaining: AtomicI64,
    hold_ms: AtomicI64,
    ip_entries: DashMap<IpAddr, IpBudgetEntry>,
    global_refill_remainder: std::sync::Mutex<i64>,
    per_ip_refill_remainder: std::sync::Mutex<i64>,
}

struct IpBudgetEntry {
    remaining: AtomicI64,
    last_seen: std::sync::Mutex<Instant>,
}

pub struct BudgetReservation {
    tracker: Arc<BudgetTracker>,
    ip: IpAddr,
    reserved_ms: i64,
    released: bool,
}

/// Summary of how a reservation was settled. Returned from
/// [`BudgetReservation::complete`] so callers (e.g. the request-logging
/// middleware) can record post-settlement state without re-querying the
/// tracker.
#[derive(Debug, Clone, Copy)]
pub struct BudgetSettlement {
    /// Budget actually consumed in ms. Matches `actual_ms` passed to
    /// `complete`.
    pub charged_ms: i64,
    /// Global remaining ms after settlement, or `None` if the global
    /// dimension is disabled.
    pub global_remaining_ms: Option<i64>,
    /// Per-IP remaining ms after settlement, or `None` if the per-IP
    /// dimension is disabled.
    pub ip_remaining_ms: Option<i64>,
}

impl BudgetReservation {
    pub fn complete(mut self, actual_ms: i64) -> BudgetSettlement {
        let (global_remaining_ms, ip_remaining_ms) = self
            .tracker
            .apply_adjustment_and_read(self.ip, self.reserved_ms - actual_ms);
        self.released = true;
        BudgetSettlement {
            charged_ms: actual_ms,
            global_remaining_ms,
            ip_remaining_ms,
        }
    }
}

impl Drop for BudgetReservation {
    fn drop(&mut self) {
        if !self.released {
            self.tracker.apply_adjustment(self.ip, self.reserved_ms);
            self.released = true;
        }
    }
}

impl BudgetTracker {
    pub fn new(per_ip_budget_ms: i64, global_budget_ms: i64, hold_ms: i64) -> Arc<Self> {
        Arc::new(Self {
            per_ip_budget_ms: AtomicI64::new(per_ip_budget_ms),
            global_budget_ms: AtomicI64::new(global_budget_ms),
            global_remaining: AtomicI64::new(global_budget_ms),
            hold_ms: AtomicI64::new(hold_ms),
            ip_entries: DashMap::new(),
            global_refill_remainder: std::sync::Mutex::new(0),
            per_ip_refill_remainder: std::sync::Mutex::new(0),
        })
    }

    pub fn hold_ms(&self) -> i64 {
        self.hold_ms.load(Ordering::Relaxed)
    }

    pub fn is_enabled(&self) -> bool {
        self.per_ip_budget_ms.load(Ordering::Relaxed) > 0
            || self.global_budget_ms.load(Ordering::Relaxed) > 0
    }

    /// Atomically reserve budget for a request. Returns Err if either the
    /// per-IP or global budget is exhausted.
    ///
    /// Note: there is a small race window between `fetch_sub` and the
    /// conditional `fetch_add` rollback. During this window, a concurrent
    /// request may observe a temporarily over-decremented budget and be
    /// rejected even though budget will be restored momentarily. This is
    /// acceptable for rate-limiting — it errs on the side of caution.
    pub fn reserve(self: &Arc<Self>, ip: IpAddr) -> Result<BudgetReservation, BudgetExhausted> {
        let estimate = self.hold_ms.load(Ordering::Relaxed);

        // Check global budget
        let global_limit = self.global_budget_ms.load(Ordering::Relaxed);
        if global_limit > 0 {
            let prev = self.global_remaining.fetch_sub(estimate, Ordering::AcqRel);
            if prev - estimate < 0 {
                self.global_remaining.fetch_add(estimate, Ordering::AcqRel);
                return Err(BudgetExhausted::Global);
            }
        }

        // Check per-IP budget
        let ip_limit = self.per_ip_budget_ms.load(Ordering::Relaxed);
        if ip_limit > 0 {
            let entry = self.ip_entries.entry(ip).or_insert_with(|| IpBudgetEntry {
                remaining: AtomicI64::new(ip_limit),
                last_seen: std::sync::Mutex::new(Instant::now()),
            });
            // Refresh activity timestamp
            if let Ok(mut last) = entry.last_seen.lock() {
                *last = Instant::now();
            }
            let prev = entry.remaining.fetch_sub(estimate, Ordering::AcqRel);
            if prev - estimate < 0 {
                entry.remaining.fetch_add(estimate, Ordering::AcqRel);
                // Roll back global reservation too
                if global_limit > 0 {
                    self.global_remaining.fetch_add(estimate, Ordering::AcqRel);
                }
                return Err(BudgetExhausted::PerIp);
            }
        }

        Ok(BudgetReservation {
            tracker: Arc::clone(self),
            ip,
            reserved_ms: estimate,
            released: false,
        })
    }

    fn apply_adjustment(&self, ip: IpAddr, diff: i64) {
        if diff == 0 {
            return;
        }

        let global_limit = self.global_budget_ms.load(Ordering::Relaxed);
        if global_limit > 0 {
            let max = global_limit;
            let prev = self.global_remaining.fetch_add(diff, Ordering::AcqRel);
            // Clamp to max
            if prev + diff > max {
                self.global_remaining.store(max, Ordering::Release);
            }
        }

        let ip_limit = self.per_ip_budget_ms.load(Ordering::Relaxed);
        if ip_limit > 0 {
            if let Some(entry) = self.ip_entries.get(&ip) {
                let prev = entry.remaining.fetch_add(diff, Ordering::AcqRel);
                if prev + diff > ip_limit {
                    entry.remaining.store(ip_limit, Ordering::Release);
                }
            }
        }
    }

    fn apply_adjustment_and_read(&self, ip: IpAddr, diff: i64) -> (Option<i64>, Option<i64>) {
        let global_limit = self.global_budget_ms.load(Ordering::Relaxed);
        let global_remaining = if global_limit > 0 {
            if diff != 0 {
                let prev = self.global_remaining.fetch_add(diff, Ordering::AcqRel);
                if prev + diff > global_limit {
                    self.global_remaining.store(global_limit, Ordering::Release);
                }
            }
            Some(self.global_remaining.load(Ordering::Relaxed))
        } else {
            None
        };

        let ip_limit = self.per_ip_budget_ms.load(Ordering::Relaxed);
        let ip_remaining = if ip_limit > 0 {
            self.ip_entries.get(&ip).map(|entry| {
                if diff != 0 {
                    let prev = entry.remaining.fetch_add(diff, Ordering::AcqRel);
                    if prev + diff > ip_limit {
                        entry.remaining.store(ip_limit, Ordering::Release);
                    }
                }
                entry.remaining.load(Ordering::Relaxed)
            })
        } else {
            None
        };

        (global_remaining, ip_remaining)
    }

    /// Current per-IP remaining budget in ms, or `None` if the per-IP
    /// dimension is disabled or no entry exists for `ip` yet.
    pub fn ip_remaining_ms(&self, ip: IpAddr) -> Option<i64> {
        if self.per_ip_budget_ms.load(Ordering::Relaxed) == 0 {
            return None;
        }
        self.ip_entries
            .get(&ip)
            .map(|entry| entry.remaining.load(Ordering::Relaxed))
    }

    /// Update budget configuration dynamically. Existing balances are clamped
    /// to the new maximums.
    pub fn update_config(&self, per_ip: Option<i64>, global: Option<i64>) {
        if let Some(new_ip) = per_ip {
            let old = self.per_ip_budget_ms.swap(new_ip, Ordering::AcqRel);
            *self
                .per_ip_refill_remainder
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = 0;
            if old == 0 && new_ip > 0 {
                // Enabling from disabled — reset all IP balances to the new limit
                for entry in self.ip_entries.iter_mut() {
                    entry.remaining.store(new_ip, Ordering::Release);
                }
            } else if new_ip < old {
                // Clamp existing IP balances to new max
                for entry in self.ip_entries.iter_mut() {
                    let current = entry.remaining.load(Ordering::Relaxed);
                    if current > new_ip {
                        entry.remaining.store(new_ip, Ordering::Release);
                    }
                }
            }
        }
        if let Some(new_global) = global {
            let old = self.global_budget_ms.swap(new_global, Ordering::AcqRel);
            *self
                .global_refill_remainder
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = 0;
            let current = self.global_remaining.load(Ordering::Relaxed);
            if current > new_global {
                // Clamp down
                self.global_remaining.store(new_global, Ordering::Release);
            } else if old == 0 && new_global > 0 {
                // Enabling from disabled — initialize remaining to the new limit
                self.global_remaining.store(new_global, Ordering::Release);
            }
        }
    }

    /// Update the pessimistic per-request reservation.
    pub fn update_estimate(&self, hold_ms: i64) {
        self.hold_ms.store(hold_ms, Ordering::Release);
    }

    /// Refill budgets. Called every second by the background task.
    pub fn refill(&self) {
        let ip_limit = self.per_ip_budget_ms.load(Ordering::Relaxed);
        let global_limit = self.global_budget_ms.load(Ordering::Relaxed);
        let ip_refill = if ip_limit > 0 {
            Self::compute_refill_amount(ip_limit, &self.per_ip_refill_remainder)
        } else {
            0
        };
        let global_refill = if global_limit > 0 {
            Self::compute_refill_amount(global_limit, &self.global_refill_remainder)
        } else {
            0
        };

        // Refill global
        if global_limit > 0 {
            let prev = self
                .global_remaining
                .fetch_add(global_refill, Ordering::AcqRel);
            if prev + global_refill > global_limit {
                self.global_remaining.store(global_limit, Ordering::Release);
            }
        }

        // Refill per-IP and prune stale entries
        let now = Instant::now();
        let prune_threshold = std::time::Duration::from_secs(120);

        self.ip_entries.retain(|_ip, entry| {
            let last = entry.last_seen.lock().map(|l| *l).unwrap_or(now);
            if now.duration_since(last) > prune_threshold {
                return false; // prune
            }
            if ip_refill > 0 {
                let prev = entry.remaining.fetch_add(ip_refill, Ordering::AcqRel);
                if prev + ip_refill > ip_limit {
                    entry.remaining.store(ip_limit, Ordering::Release);
                }
            }
            true
        });
    }

    fn compute_refill_amount(limit: i64, remainder: &std::sync::Mutex<i64>) -> i64 {
        let mut remainder = remainder
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *remainder += limit;
        let refill = *remainder / 60;
        *remainder %= 60;
        refill
    }

    /// Get current status for the admin API.
    pub fn status(&self) -> BudgetStatus {
        BudgetStatus {
            per_ip_budget_ms: self.per_ip_budget_ms.load(Ordering::Relaxed),
            global_budget_ms: self.global_budget_ms.load(Ordering::Relaxed),
            global_remaining_ms: self.global_remaining.load(Ordering::Relaxed),
            hold_ms: self.hold_ms.load(Ordering::Relaxed),
            active_ips: self.ip_entries.len(),
        }
    }
}

#[derive(Debug)]
pub enum BudgetExhausted {
    PerIp,
    Global,
}

impl std::fmt::Display for BudgetExhausted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetExhausted::PerIp => write!(f, "per-IP compute budget exhausted"),
            BudgetExhausted::Global => write!(f, "global compute budget exhausted"),
        }
    }
}

#[derive(serde::Serialize)]
pub struct BudgetStatus {
    pub per_ip_budget_ms: i64,
    pub global_budget_ms: i64,
    pub global_remaining_ms: i64,
    pub hold_ms: i64,
    pub active_ips: usize,
}

/// Axum middleware that reserves budget for each request, refunds the
/// difference between reservation and actual handler time, and records
/// the outcome (`budget.*` fields) on the current tracing span.
pub(crate) async fn middleware(
    tracker: Arc<BudgetTracker>,
    opaque_errors: bool,
    trust_proxy: bool,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::http::StatusCode;

    if !tracker.is_enabled() {
        return next.run(req).await;
    }

    let ip = crate::middleware::extract_client_ip(&req, trust_proxy)
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

    let span = tracing::Span::current();
    span.record("budget_client_ip", tracing::field::display(&ip));

    let reservation = match tracker.reserve(ip) {
        Ok(reservation) => reservation,
        Err(e) => {
            let outcome = match e {
                BudgetExhausted::PerIp => "rejected_per_ip",
                BudgetExhausted::Global => "rejected_global",
            };
            span.record("budget_outcome", outcome);
            span.record("budget_charged_ms", 0_i64);
            let status = tracker.status();
            if status.global_budget_ms > 0 {
                span.record("budget_global_remaining_ms", status.global_remaining_ms);
            }
            if let Some(ip_rem) = tracker.ip_remaining_ms(ip) {
                span.record("budget_ip_remaining_ms", ip_rem);
            }
            return crate::util::error_response(
                StatusCode::TOO_MANY_REQUESTS,
                &format!("{e}"),
                opaque_errors,
            );
        }
    };

    // Optimistic pre-record: if the inner future is cancelled (request
    // timeout, handler panic, client disconnect) we never reach the
    // post-await overwrite, and `reservation`'s Drop refunds the full
    // reservation. These values stay on the span and appear on the
    // TraceLayer response log line as the signal of abnormal termination.
    let hold_ms = tracker.hold_ms();
    span.record("budget_outcome", "refunded_on_drop");
    span.record("budget_charged_ms", hold_ms);

    let start = Instant::now();
    let response = next.run(req).await;
    let actual_ms = start.elapsed().as_millis() as i64;

    let settlement = reservation.complete(actual_ms);
    span.record("budget_outcome", "accepted");
    span.record("budget_charged_ms", settlement.charged_ms);
    if let Some(g) = settlement.global_remaining_ms {
        span.record("budget_global_remaining_ms", g);
    }
    if let Some(p) = settlement.ip_remaining_ms {
        span.record("budget_ip_remaining_ms", p);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_middleware_stack;
    use crate::test_support::{
        capture_json_subscriber, default_serve_config, find_response_event, run_budget_request,
        BufferWriter,
    };
    use axum::http::StatusCode;
    use axum::routing::get;
    use axum::Router;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;
    use tower::Service;

    fn test_ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    }

    #[test]
    fn test_budget_reservation_drop_refunds_reserved_budget() {
        let tracker = BudgetTracker::new(100, 100, 25);
        let ip = test_ip();

        let reservation = tracker.reserve(ip).unwrap();
        assert_eq!(tracker.global_remaining.load(Ordering::Relaxed), 75);
        assert_eq!(
            tracker
                .ip_entries
                .get(&ip)
                .unwrap()
                .remaining
                .load(Ordering::Relaxed),
            75
        );

        drop(reservation);

        assert_eq!(tracker.global_remaining.load(Ordering::Relaxed), 100);
        assert_eq!(
            tracker
                .ip_entries
                .get(&ip)
                .unwrap()
                .remaining
                .load(Ordering::Relaxed),
            100
        );
    }

    #[test]
    fn test_budget_reservation_complete_charges_actual_elapsed_time() {
        let tracker = BudgetTracker::new(100, 100, 40);
        let ip = test_ip();

        let reservation = tracker.reserve(ip).unwrap();
        tracker.update_estimate(10);
        let settlement = reservation.complete(30);

        assert_eq!(settlement.charged_ms, 30);
        assert_eq!(settlement.global_remaining_ms, Some(70));
        assert_eq!(settlement.ip_remaining_ms, Some(70));
        assert_eq!(tracker.global_remaining.load(Ordering::Relaxed), 70);
        assert_eq!(
            tracker
                .ip_entries
                .get(&ip)
                .unwrap()
                .remaining
                .load(Ordering::Relaxed),
            70
        );
    }

    #[test]
    fn test_global_refill_exact_for_low_budget() {
        let tracker = BudgetTracker::new(0, 1, 1);
        tracker.global_remaining.store(0, Ordering::Release);

        for _ in 0..59 {
            tracker.refill();
        }
        assert_eq!(tracker.global_remaining.load(Ordering::Relaxed), 0);

        tracker.refill();
        assert_eq!(tracker.global_remaining.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_per_ip_refill_exact_for_non_divisible_budget() {
        let tracker = BudgetTracker::new(90, 0, 1);
        let ip = test_ip();
        tracker.ip_entries.insert(
            ip,
            IpBudgetEntry {
                remaining: AtomicI64::new(0),
                last_seen: std::sync::Mutex::new(Instant::now()),
            },
        );

        for _ in 0..60 {
            tracker.refill();
        }

        assert_eq!(
            tracker
                .ip_entries
                .get(&ip)
                .unwrap()
                .remaining
                .load(Ordering::Relaxed),
            90
        );
    }

    #[tokio::test]
    async fn test_budget_timeout_refunds_reservation() {
        async fn slow_handler() -> &'static str {
            tokio::time::sleep(Duration::from_millis(1100)).await;
            "slow"
        }

        async fn fast_handler() -> &'static str {
            "fast"
        }

        let tracker = BudgetTracker::new(100, 0, 100);
        let router = Router::new()
            .route("/slow", get(slow_handler))
            .route("/fast", get(fast_handler))
            .layer(axum::middleware::from_fn(
                move |req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| {
                    let tracker = tracker.clone();
                    async move { super::middleware(tracker, false, false, req, next).await }
                },
            ));

        let mut serve_config = default_serve_config();
        serve_config.request_timeout_secs = 1;
        serve_config.budget_hold_ms = 100;

        let mut app = build_middleware_stack(router, &serve_config);

        let slow_response = Service::call(
            &mut app,
            axum::http::Request::builder()
                .method("GET")
                .uri("/slow")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(slow_response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let fast_response = Service::call(
            &mut app,
            axum::http::Request::builder()
                .method("GET")
                .uri("/fast")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(fast_response.status(), StatusCode::OK);
    }

    #[test]
    fn test_budget_logging_accepted() {
        let tracker = BudgetTracker::new(1_000, 10_000, 50);
        let (buf, response) = run_budget_request(
            tracker,
            |cfg| {
                cfg.budget_hold_ms = 50;
            },
            "/t",
        );
        assert_eq!(response.status(), StatusCode::OK);

        let event = find_response_event(&buf);
        assert_eq!(event["budget.outcome"], "accepted");
        let charged = event["budget.charged_ms"]
            .as_i64()
            .expect("budget.charged_ms is i64");
        assert!(
            (0..=50).contains(&charged),
            "charged_ms out of range: {charged} captured: {}",
            buf.snapshot()
        );
        assert!(event["budget.global_remaining_ms"].as_i64().is_some());
        assert!(event["budget.ip_remaining_ms"].as_i64().is_some());
        assert!(event["budget.client_ip"].as_str().is_some());
    }

    #[test]
    fn test_budget_logging_rejected_per_ip() {
        // Tiny per-IP budget, global disabled, huge hold → reserve() fails on per-IP.
        let tracker = BudgetTracker::new(1, 0, 10_000);
        let (buf, response) = run_budget_request(
            tracker,
            |cfg| {
                cfg.budget_hold_ms = 10_000;
            },
            "/t",
        );
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let event = find_response_event(&buf);
        assert_eq!(event["budget.outcome"], "rejected_per_ip");
        assert_eq!(event["budget.charged_ms"].as_i64(), Some(0));
        assert!(
            event.get("budget.global_remaining_ms").is_none(),
            "global field should be absent when dimension disabled"
        );
        assert!(event["budget.ip_remaining_ms"].as_i64().is_some());
        assert!(event["budget.client_ip"].as_str().is_some());
    }

    #[test]
    fn test_budget_logging_rejected_global() {
        // Global tiny, per-IP disabled → reserve() fails on global.
        let tracker = BudgetTracker::new(0, 1, 10_000);
        let (buf, response) = run_budget_request(
            tracker,
            |cfg| {
                cfg.budget_hold_ms = 10_000;
            },
            "/t",
        );
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        let event = find_response_event(&buf);
        assert_eq!(event["budget.outcome"], "rejected_global");
        assert_eq!(event["budget.charged_ms"].as_i64(), Some(0));
        assert!(event["budget.global_remaining_ms"].as_i64().is_some());
        assert!(
            event.get("budget.ip_remaining_ms").is_none(),
            "ip field should be absent when dimension disabled"
        );
        assert!(event["budget.client_ip"].as_str().is_some());
    }

    #[test]
    fn test_budget_logging_override_semantics() {
        // Guards the optimistic pre-record pattern: the middleware records
        // "refunded_on_drop" before .await, then overwrites with "accepted"
        // after. This test proves the last Span::record wins in the final
        // formatted JSON. If tracing or JsonFields ever flips to first-wins
        // (or emit-both), this test fails immediately.
        let buf = BufferWriter::default();
        let subscriber = capture_json_subscriber(buf.clone());
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(
                "request",
                budget_outcome = tracing::field::Empty,
                budget_charged_ms = tracing::field::Empty,
            );
            let _entered = span.enter();
            tracing::Span::current().record("budget_outcome", "refunded_on_drop");
            tracing::Span::current().record("budget_charged_ms", 100_i64);
            tracing::Span::current().record("budget_outcome", "accepted");
            tracing::Span::current().record("budget_charged_ms", 42_i64);
            tracing::info!("response");
        });

        let event = find_response_event(&buf);
        assert_eq!(
            event["budget.outcome"], "accepted",
            "last-recorded outcome should win"
        );
        assert_eq!(event["budget.charged_ms"].as_i64(), Some(42));
    }

    #[test]
    fn test_json_response_event_has_response_time_ms() {
        let tracker = BudgetTracker::new(1_000, 10_000, 50);
        let (buf, response) = run_budget_request(
            tracker,
            |cfg| {
                cfg.budget_hold_ms = 50;
            },
            "/t",
        );
        assert_eq!(response.status(), StatusCode::OK);

        let event = find_response_event(&buf);
        assert!(
            event["response_time_ms"].as_f64().is_some_and(|v| v >= 0.0),
            "response_time_ms should be present as f64 >= 0. captured: {}",
            buf.snapshot()
        );
        assert!(
            event["duration"].as_i64().is_some_and(|v| v >= 0),
            "duration (ns) should still be present for back-compat"
        );
    }
}
