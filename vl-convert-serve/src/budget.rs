use dashmap::DashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Instant;

pub struct BudgetTracker {
    per_ip_budget_ms: AtomicI64,
    global_budget_ms: AtomicI64,
    global_remaining: AtomicI64,
    estimate_ms: AtomicI64,
    ip_entries: DashMap<IpAddr, IpBudgetEntry>,
}

struct IpBudgetEntry {
    remaining: AtomicI64,
    last_seen: std::sync::Mutex<Instant>,
}

impl BudgetTracker {
    pub fn new(per_ip_budget_ms: i64, global_budget_ms: i64, estimate_ms: i64) -> Arc<Self> {
        Arc::new(Self {
            per_ip_budget_ms: AtomicI64::new(per_ip_budget_ms),
            global_budget_ms: AtomicI64::new(global_budget_ms),
            global_remaining: AtomicI64::new(global_budget_ms),
            estimate_ms: AtomicI64::new(estimate_ms),
            ip_entries: DashMap::new(),
        })
    }

    pub fn estimate_ms(&self) -> i64 {
        self.estimate_ms.load(Ordering::Relaxed)
    }

    pub fn is_enabled(&self) -> bool {
        self.per_ip_budget_ms.load(Ordering::Relaxed) > 0
            || self.global_budget_ms.load(Ordering::Relaxed) > 0
    }

    /// Atomically reserve budget for a request. Returns Err if either the
    /// per-IP or global budget is exhausted.
    pub fn reserve(&self, ip: IpAddr) -> Result<(), BudgetExhausted> {
        let estimate = self.estimate_ms.load(Ordering::Relaxed);

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

        Ok(())
    }

    /// Adjust the reservation after conversion completes. Returns the
    /// difference (estimate - actual) back to the budgets.
    pub fn adjust(&self, ip: IpAddr, actual_ms: i64) {
        let estimate = self.estimate_ms.load(Ordering::Relaxed);
        let diff = estimate - actual_ms;

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

    /// Update budget configuration dynamically. Existing balances are clamped
    /// to the new maximums.
    pub fn update_config(&self, per_ip: Option<i64>, global: Option<i64>) {
        if let Some(new_ip) = per_ip {
            let old = self.per_ip_budget_ms.swap(new_ip, Ordering::AcqRel);
            // Clamp existing IP balances to new max
            if new_ip < old {
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

    /// Refill budgets. Called every second by the background task.
    pub fn refill(&self) {
        let ip_limit = self.per_ip_budget_ms.load(Ordering::Relaxed);
        let global_limit = self.global_budget_ms.load(Ordering::Relaxed);
        // Round refill to avoid truncation; guarantee at least 1ms/sec when budget > 0
        let ip_refill = if ip_limit > 0 {
            (ip_limit as f64 / 60.0).round().max(1.0) as i64
        } else {
            0
        };
        let global_refill = if global_limit > 0 {
            (global_limit as f64 / 60.0).round().max(1.0) as i64
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

    /// Get current status for the admin API.
    pub fn status(&self) -> BudgetStatus {
        BudgetStatus {
            per_ip_budget_ms: self.per_ip_budget_ms.load(Ordering::Relaxed),
            global_budget_ms: self.global_budget_ms.load(Ordering::Relaxed),
            global_remaining_ms: self.global_remaining.load(Ordering::Relaxed),
            estimate_ms: self.estimate_ms.load(Ordering::Relaxed),
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
    pub estimate_ms: i64,
    pub active_ips: usize,
}
