//! Admin reconfig coordination primitives.
//!
//! Drain semantics invariant: "drain complete" is signalled by
//! [`InflightGuard::drop`] decrementing [`ReconfigCoordinator::inflight`]
//! to zero while [`ReconfigCoordinator::gate_closed`] is `true`. It is
//! **not** inferred from any reference-count on shared state (e.g.
//! `Arc::strong_count`). Long-lived clones of `Arc<RuntimeSnapshot>`,
//! `Arc<AppState>`, or any middleware-internal state are therefore safe —
//! they do not block drain. In-flight requests are by definition those
//! that entered the gate middleware, were admitted, and have not yet
//! released their `InflightGuard`.
//!
//! The admission gate uses an **increment-first, recheck-after** handshake
//! so there is no race between a request checking `gate_closed` and the
//! drain loop observing `inflight == 0`. See [`ReconfigCoordinator::drain`]
//! for the detailed algorithm + race analysis.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio_util::sync::CancellationToken;
use vl_convert_rs::converter::VlcConfig;

use crate::health::ReadinessState;
use crate::types::{ConfigPatch, ConfigValidationError, FieldError, FieldErrorCode};

/// Error produced by [`ReconfigCoordinator::drain`] when the drain window
/// cannot complete cleanly.
#[derive(Debug)]
pub(crate) enum DrainError {
    /// Shutdown token fired before drain finished.
    Cancelled,
    /// Drain deadline exceeded; `inflight` is the count at the moment the
    /// timeout fired.
    Timeout { inflight: usize },
}

/// Coordinates the admin-reconfig lifecycle: admission gate, in-flight
/// counting, drain notification, shutdown integration, and serialization of
/// concurrent admin-mutating requests.
///
/// Held as `Arc<ReconfigCoordinator>` inside [`crate::config::AppState`] and
/// [`crate::admin::AdminState`]; the same `Arc` is shared between the gate
/// middleware (main listener) and the admin handlers (admin listener) so
/// both participate in the same drain domain.
pub(crate) struct ReconfigCoordinator {
    /// Set `true` while a reconfig is draining or rebuilding. Reads by the
    /// gate middleware and drain loop use `SeqCst` so admission and drain
    /// agree on one total order with `inflight`.
    gate_closed: AtomicBool,
    /// Number of admitted, not-yet-completed requests on the gated router.
    /// Incremented by gate middleware at admission, decremented by
    /// `InflightGuard::drop`.
    inflight: AtomicUsize,
    /// Woken on every `inflight` decrement so the drain loop does not have
    /// to poll.
    drained: Notify,
    /// Serializes all admin-mutating endpoints (PATCH, PUT, DELETE, POST
    /// /admin/config/fonts/directories). Last-writer-wins.
    reconfig_lock: Mutex<()>,
    /// Shared with `serve()` so SIGTERM / SIGINT / stdin-EOF aborts a
    /// reconfig mid-flight.
    shutdown_token: CancellationToken,
    /// Absolute deadline for a single drain call.
    drain_timeout: Duration,
}

impl ReconfigCoordinator {
    /// Construct a fresh coordinator. Clone the returned `Arc` into any
    /// state container that needs it.
    pub(crate) fn new(shutdown_token: CancellationToken, drain_timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            gate_closed: AtomicBool::new(false),
            inflight: AtomicUsize::new(0),
            drained: Notify::new(),
            reconfig_lock: Mutex::new(()),
            shutdown_token,
            drain_timeout,
        })
    }

    /// Acquire the reconfig lock. Returned guard serializes against every
    /// other admin-mutating request. Last-writer-wins.
    pub(crate) async fn lock(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.reconfig_lock.lock().await
    }

    /// Read the coordinator's shutdown token (cheap `Clone`).
    pub(crate) fn shutdown_token(&self) -> CancellationToken {
        self.shutdown_token.clone()
    }

    /// Close the admission gate. New requests hitting the gate middleware
    /// will be rejected with 503 until [`Self::open_gate`] fires.
    pub(crate) fn close_gate(&self) {
        self.gate_closed.store(true, Ordering::SeqCst);
    }

    /// Reopen the admission gate. Called by [`ReconfigScopeGuard::drop`]
    /// on any exit path (success, error, admin-caller disconnect).
    pub(crate) fn open_gate(&self) {
        self.gate_closed.store(false, Ordering::SeqCst);
    }

    pub(crate) fn is_gate_closed(&self) -> bool {
        self.gate_closed.load(Ordering::SeqCst)
    }

    pub(crate) fn inflight(&self) -> usize {
        self.inflight.load(Ordering::SeqCst)
    }

    /// Admission handshake used by the gate middleware.
    ///
    /// Increments `inflight` first, then rechecks `gate_closed`. If the
    /// gate closed between the bump and this load, decrements and returns
    /// `Err(())` so the middleware can reject the request with 503.
    ///
    /// On success returns an [`InflightGuard`] that decrements and wakes
    /// the drain loop on drop.
    pub(crate) fn try_admit(self: &Arc<Self>) -> Result<InflightGuard, ()> {
        self.inflight.fetch_add(1, Ordering::SeqCst);
        if self.gate_closed.load(Ordering::SeqCst) {
            self.inflight.fetch_sub(1, Ordering::SeqCst);
            self.drained.notify_waiters();
            return Err(());
        }
        Ok(InflightGuard {
            coord: self.clone(),
        })
    }

    /// Close the gate and wait for all in-flight admitted requests to
    /// finish, or for an error condition.
    ///
    /// **Algorithm** (race-free per design §2.2):
    ///
    /// 1. Store `true` to `gate_closed` (SeqCst). From this point on the
    ///    middleware's recheck-after-increment will fail any new admit.
    /// 2. Compute a single absolute deadline.
    /// 3. Loop:
    ///    - Register the next `Notify::notified()` future **before**
    ///      loading the counter. Any decrement that happens after this
    ///      registration is guaranteed to wake us.
    ///    - If `inflight == 0`, success.
    ///    - Otherwise `select!` biased over shutdown / notify / deadline.
    ///      Shutdown → `Cancelled`. Notify → re-check. Deadline →
    ///      `Timeout { inflight: <current count> }`.
    ///
    /// On `Ok`, the gate remains closed; the caller is responsible for
    /// reopening it (typically via `ReconfigScopeGuard::drop`). On `Err`,
    /// the gate also remains closed — the caller unwinds (guard drop
    /// reopens).
    pub(crate) async fn drain(&self) -> Result<(), DrainError> {
        self.close_gate();
        let deadline = tokio::time::Instant::now() + self.drain_timeout;

        loop {
            // Registering `notified` BEFORE the inflight load is critical —
            // otherwise a decrement between the load and the notify
            // registration is missed and we hang until the deadline.
            let notified = self.drained.notified();
            tokio::pin!(notified);

            if self.inflight.load(Ordering::SeqCst) == 0 {
                return Ok(());
            }

            tokio::select! {
                biased;
                _ = self.shutdown_token.cancelled() => return Err(DrainError::Cancelled),
                _ = notified => continue,
                _ = tokio::time::sleep_until(deadline) => {
                    return Err(DrainError::Timeout {
                        inflight: self.inflight.load(Ordering::SeqCst),
                    });
                }
            }
        }
    }
}

/// Drop-guard that decrements [`ReconfigCoordinator::inflight`] and wakes
/// the drain loop. Install on the request task after admission so every
/// exit path — normal return, handler panic caught by `CatchPanicLayer`,
/// client disconnect, `TimeoutLayer` cancellation — decrements.
pub(crate) struct InflightGuard {
    coord: Arc<ReconfigCoordinator>,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.coord.inflight.fetch_sub(1, Ordering::SeqCst);
        self.coord.drained.notify_waiters();
    }
}

/// Drop-safe scope guard for the admin reconfig handler.
///
/// Installed after the handler acquires the reconfig lock. On any exit
/// path (explicit return, `?`, handler panic, admin-caller disconnect)
/// the guard's drop:
///
/// 1. Reopens the admission gate (if the handler closed it).
/// 2. Clears `readiness.reconfig_in_progress`.
/// 3. Runs the rollback closure (if one is armed).
///
/// Rollback closures are armed by the handler *after* it mutates process-
/// global state (e.g. `set_font_directories`, `apply_hot_font_cache`), and
/// disarmed once the handler successfully commits a new snapshot. This
/// guarantees that a mid-rebuild cancellation or warm-up failure always
/// restores globals to their pre-reconfig values before the gate reopens.
pub(crate) struct ReconfigScopeGuard<'a> {
    coord: &'a Arc<ReconfigCoordinator>,
    readiness: &'a Arc<ReadinessState>,
    /// Whether the guard should reopen the gate + clear readiness on drop.
    /// The handler sets this to `true` when it calls `close_gate()` (at
    /// the start of the drain/rebuild path) and leaves it `false` on the
    /// hot-apply / identity-patch paths that never closed the gate.
    gate_was_closed: bool,
    /// Optional rollback closure. Armed after mutating globals and
    /// disarmed (set to `None`) after a successful commit.
    rollback: Option<Box<dyn FnOnce() + Send + 'a>>,
}

impl<'a> ReconfigScopeGuard<'a> {
    /// Create a new scope guard. Call at the start of the admin handler.
    pub(crate) fn new(
        coord: &'a Arc<ReconfigCoordinator>,
        readiness: &'a Arc<ReadinessState>,
    ) -> Self {
        Self {
            coord,
            readiness,
            gate_was_closed: false,
            rollback: None,
        }
    }

    /// Record that the handler closed the admission gate and marked
    /// `reconfig_in_progress`. Drop will undo both.
    pub(crate) fn mark_gate_closed(&mut self) {
        self.gate_was_closed = true;
        self.readiness
            .reconfig_in_progress
            .store(true, Ordering::Release);
    }

    /// Arm the rollback closure. Fires on drop unless `disarm_rollback`
    /// clears it first. Use after mutating process-global state that a
    /// subsequent `with_config` / `warm_up` failure would strand.
    pub(crate) fn arm_rollback<F>(&mut self, rollback: F)
    where
        F: FnOnce() + Send + 'a,
    {
        self.rollback = Some(Box::new(rollback));
    }

    /// Clear the rollback closure. Call once the commit has succeeded.
    pub(crate) fn disarm_rollback(&mut self) {
        self.rollback = None;
    }
}

impl<'a> Drop for ReconfigScopeGuard<'a> {
    fn drop(&mut self) {
        // CRITICAL ORDERING: fire the rollback BEFORE reopening the gate or
        // clearing `reconfig_in_progress`. If we reopened the gate first,
        // a request admitted in the window between `open_gate` and the
        // rollback closure firing would observe the pre-rollback
        // process-globals (a failed `with_config` that mutated
        // `FONT_CONFIG` / Google Fonts cache) combined with the still-old
        // `RuntimeSnapshot` — effectively serving traffic against a config
        // that was never committed. Running rollback first guarantees:
        // when the gate reopens, globals are either committed (success
        // path, rollback disarmed) or restored (failure path, rollback
        // fired).
        if let Some(rollback) = self.rollback.take() {
            rollback();
        }
        if self.gate_was_closed {
            self.coord.open_gate();
            self.readiness
                .reconfig_in_progress
                .store(false, Ordering::Release);
        }
    }
}

/// A patch rejection that must be surfaced to the admin caller. Variants
/// determine the HTTP status code: `NonNullable` → 400, `Invalid` → 422.
#[derive(Debug)]
pub(crate) enum PatchRejection {
    /// One or more non-nullable fields received an explicit `null` in the
    /// patch body. Parse-level rejection: the wire shape is illegal.
    NonNullable(ConfigValidationError),
    /// Semantic validation failure (cross-field invariant, etc.). Reserved
    /// for future use; `apply_patch` itself never returns this variant —
    /// `normalize_converter_config` is the only current producer.
    #[allow(dead_code)]
    Invalid(ConfigValidationError),
}

/// Merge a [`ConfigPatch`] onto a current [`VlcConfig`] snapshot.
///
/// Semantics per design §2.5 ("natural JSON ↔ Option"):
/// * Field absent from the patch (outer `None`) → preserve current value.
/// * Field present (`Some(inner)`) → replace current with `inner`.
///
/// For VlcConfig fields whose library type is `Option<T>`, the inner
/// `Option` is stored as-is (so `null` → `None`). For non-optional
/// VlcConfig fields, `null` on the wire is illegal: the corresponding
/// `Option<Option<T>>` arrives as `Some(None)` and we reject with a
/// `PatchRejection::NonNullable` (the admin handler maps it to 400).
///
/// Cross-field invariants (e.g. `allow_google_fonts` requires
/// `auto_google_fonts`) are *not* checked here — that's the job of
/// `normalize_converter_config` further down the pipeline, which returns
/// a 422 `ConfigValidationError`.
pub(crate) fn apply_patch(
    current: &VlcConfig,
    patch: &ConfigPatch,
) -> Result<VlcConfig, PatchRejection> {
    let mut new = current.clone();
    let mut null_fields: Vec<FieldError> = Vec::new();

    // Optional<T> VlcConfig fields (null → None).
    if let Some(v) = patch.max_v8_heap_size_mb.as_ref() {
        new.max_v8_heap_size_mb = *v;
    }
    if let Some(v) = patch.max_v8_execution_time_secs.as_ref() {
        new.max_v8_execution_time_secs = *v;
    }
    if let Some(v) = patch.max_ephemeral_workers.as_ref() {
        new.max_ephemeral_workers = *v;
    }
    if let Some(v) = patch.default_theme.as_ref() {
        new.default_theme = v.clone();
    }
    if let Some(v) = patch.default_format_locale.as_ref() {
        new.default_format_locale = v.clone();
    }
    if let Some(v) = patch.default_time_format_locale.as_ref() {
        new.default_time_format_locale = v.clone();
    }
    if let Some(v) = patch.google_fonts_cache_size_mb.as_ref() {
        new.google_fonts_cache_size_mb = *v;
    }

    // Non-optional VlcConfig fields. Outer Option distinguishes
    // "absent" from "present"; inner None = explicit wire null → reject.
    macro_rules! apply_non_nullable {
        ($field:ident, $apply:expr) => {
            match patch.$field.as_ref() {
                None => {}
                Some(None) => null_fields.push(FieldError {
                    path: stringify!($field).to_string(),
                    code: FieldErrorCode::NonNullable,
                    message: format!(
                        "field '{}' is not nullable",
                        stringify!($field),
                    ),
                }),
                Some(Some(v)) => $apply(&mut new, v),
            }
        };
    }

    apply_non_nullable!(num_workers, |n: &mut VlcConfig, v: &_| n.num_workers = *v);
    apply_non_nullable!(base_url, |n: &mut VlcConfig,
                                   v: &vl_convert_rs::converter::BaseUrlSetting| {
        n.base_url = v.clone();
    });
    apply_non_nullable!(allowed_base_urls, |n: &mut VlcConfig, v: &Vec<String>| {
        n.allowed_base_urls = v.clone();
    });
    apply_non_nullable!(auto_google_fonts, |n: &mut VlcConfig, v: &bool| {
        n.auto_google_fonts = *v;
    });
    apply_non_nullable!(embed_local_fonts, |n: &mut VlcConfig, v: &bool| {
        n.embed_local_fonts = *v;
    });
    apply_non_nullable!(subset_fonts, |n: &mut VlcConfig, v: &bool| n.subset_fonts =
        *v);
    apply_non_nullable!(missing_fonts, |n: &mut VlcConfig, v: &_| {
        n.missing_fonts = *v;
    });
    apply_non_nullable!(google_fonts, |n: &mut VlcConfig, v: &Vec<_>| {
        n.google_fonts = v.clone();
    });
    apply_non_nullable!(gc_after_conversion, |n: &mut VlcConfig, v: &bool| {
        n.gc_after_conversion = *v;
    });
    apply_non_nullable!(vega_plugins, |n: &mut VlcConfig, v: &Vec<String>| {
        n.vega_plugins = v.clone();
    });
    apply_non_nullable!(plugin_import_domains, |n: &mut VlcConfig,
                                                v: &Vec<String>| {
        n.plugin_import_domains = v.clone();
    });
    apply_non_nullable!(allow_per_request_plugins, |n: &mut VlcConfig, v: &bool| {
        n.allow_per_request_plugins = *v;
    });
    apply_non_nullable!(allow_google_fonts, |n: &mut VlcConfig, v: &bool| {
        n.allow_google_fonts = *v;
    });
    apply_non_nullable!(per_request_plugin_import_domains, |n: &mut VlcConfig,
                                                            v: &Vec<String>| {
        n.per_request_plugin_import_domains = v.clone();
    });
    apply_non_nullable!(themes, |n: &mut VlcConfig,
                                 v: &std::collections::HashMap<
        String,
        serde_json::Value,
    >| {
        n.themes = v.clone();
    });
    apply_non_nullable!(font_directories, |n: &mut VlcConfig,
                                           v: &Vec<std::path::PathBuf>| {
        n.font_directories = v.clone();
    });

    if !null_fields.is_empty() {
        return Err(PatchRejection::NonNullable(ConfigValidationError {
            error: "null received on non-nullable field(s)".to_string(),
            field_errors: null_fields,
        }));
    }

    Ok(new)
}

/// Decide whether the new config can be committed via the hot-apply path
/// (swap snapshot, call `apply_hot_font_cache` / `set_font_directories`)
/// or requires the full drain + rebuild pipeline.
///
/// Returns `false` iff the *only* fields that differ are
/// `google_fonts_cache_size_mb` and/or `font_directories` — both of which
/// are safe to mutate at runtime without rebuilding the converter workers.
/// Any other field difference requires a rebuild because its value is
/// baked into worker state (V8 flags, plugin modules, resolved locales,
/// etc.).
pub(crate) fn requires_rebuild(cur: &VlcConfig, new: &VlcConfig) -> bool {
    if cur == new {
        return false;
    }
    // Project both configs to "all fields except the hot-apply ones" and
    // compare. If those projections agree, every actual difference must be
    // on a hot-apply field, so no rebuild is needed.
    let mut cur_for_compare = cur.clone();
    let mut new_for_compare = new.clone();
    cur_for_compare.google_fonts_cache_size_mb = None;
    new_for_compare.google_fonts_cache_size_mb = None;
    cur_for_compare.font_directories = Vec::new();
    new_for_compare.font_directories = Vec::new();
    cur_for_compare != new_for_compare
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    fn coord_with_timeout(ms: u64) -> Arc<ReconfigCoordinator> {
        ReconfigCoordinator::new(CancellationToken::new(), Duration::from_millis(ms))
    }

    #[tokio::test]
    async fn test_drain_returns_immediately_when_no_inflight() {
        let coord = coord_with_timeout(5_000);
        assert!(matches!(coord.drain().await, Ok(())));
        assert!(coord.is_gate_closed());
    }

    #[tokio::test]
    async fn test_drain_waits_for_guards_to_drop() {
        let coord = coord_with_timeout(5_000);
        let guard = coord
            .try_admit()
            .expect("gate should be open before drain");
        assert_eq!(coord.inflight(), 1);

        // Spawn the drain; it must block on the inflight guard.
        let c = coord.clone();
        let drain_handle = tokio::spawn(async move { c.drain().await });

        // Give the drain a chance to enter the wait loop.
        tokio::time::sleep(Duration::from_millis(20)).await;
        // try_admit should now fail because gate_closed is set.
        assert!(coord.try_admit().is_err());

        // Drop the guard — drain should complete Ok.
        drop(guard);
        assert!(matches!(drain_handle.await.unwrap(), Ok(())));
        assert_eq!(coord.inflight(), 0);
    }

    #[tokio::test]
    async fn test_drain_aborts_on_shutdown_cancel() {
        let shutdown = CancellationToken::new();
        let coord = Arc::new(ReconfigCoordinator {
            gate_closed: AtomicBool::new(false),
            inflight: AtomicUsize::new(0),
            drained: Notify::new(),
            reconfig_lock: Mutex::new(()),
            shutdown_token: shutdown.clone(),
            drain_timeout: Duration::from_secs(60),
        });

        // Hold a guard so drain blocks.
        let _guard = coord.try_admit().unwrap();

        let c = coord.clone();
        let drain_handle = tokio::spawn(async move { c.drain().await });

        tokio::time::sleep(Duration::from_millis(20)).await;
        shutdown.cancel();

        let result = drain_handle.await.unwrap();
        assert!(matches!(result, Err(DrainError::Cancelled)));
    }

    #[tokio::test]
    async fn test_drain_returns_timeout_error_when_bounded_time_exceeded() {
        let coord = coord_with_timeout(50);
        let _guard = coord.try_admit().unwrap();

        let start = std::time::Instant::now();
        let result = coord.drain().await;
        let elapsed = start.elapsed();

        match result {
            Err(DrainError::Timeout { inflight }) => {
                assert_eq!(inflight, 1);
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
        assert!(
            elapsed >= Duration::from_millis(40) && elapsed < Duration::from_millis(500),
            "drain returned in {elapsed:?}; expected ~50ms"
        );
    }

    /// Stress-tests the admission handshake against a racing `close_gate`.
    /// N concurrent tasks call `try_admit()`. A drainer closes the gate.
    /// Invariant: every successful admit must also be counted in the drain
    /// loop's inflight read at some point (or rejected cleanly). Neither
    /// case should deadlock or leak a count.
    #[tokio::test]
    async fn test_admission_race_regression() {
        const TASKS: usize = 200;
        let coord = coord_with_timeout(2_000);
        let accepted = Arc::new(AtomicU32::new(0));
        let rejected = Arc::new(AtomicU32::new(0));

        let mut handles = Vec::with_capacity(TASKS);
        for _ in 0..TASKS {
            let c = coord.clone();
            let a = accepted.clone();
            let r = rejected.clone();
            handles.push(tokio::spawn(async move {
                // Sleep a variable tiny amount so admits don't all fire together.
                tokio::task::yield_now().await;
                match c.try_admit() {
                    Ok(_guard) => {
                        a.fetch_add(1, Ordering::SeqCst);
                        // Hold briefly — this is a released-by-drop guard.
                    }
                    Err(()) => {
                        r.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }));
        }

        // Drain in parallel — picks an arbitrary moment to close the gate.
        let drain_handle = {
            let c = coord.clone();
            tokio::spawn(async move { c.drain().await })
        };

        for h in handles {
            h.await.unwrap();
        }
        let drain_result = drain_handle.await.unwrap();

        // Drain must succeed (all admits' guards were dropped at task exit).
        assert!(
            matches!(drain_result, Ok(()) | Err(DrainError::Timeout { .. })),
            "unexpected drain result: {drain_result:?}"
        );

        // Every task must have either admitted or been rejected — no lost tasks.
        let total = accepted.load(Ordering::SeqCst) + rejected.load(Ordering::SeqCst);
        assert_eq!(total as usize, TASKS, "lost admit/reject accounting");

        // After all tasks complete, inflight must be 0 (drop guards fired).
        assert_eq!(coord.inflight(), 0);
    }

    // --- apply_patch / requires_rebuild ---------------------------------

    use std::num::NonZeroU64;
    use vl_convert_rs::converter::MissingFontsPolicy;

    #[test]
    fn apply_patch_empty_preserves_current() {
        let cur = VlcConfig::default();
        let patch = ConfigPatch::default();
        let new = apply_patch(&cur, &patch).unwrap();
        assert_eq!(new, cur);
    }

    #[test]
    fn apply_patch_sets_non_optional_field() {
        let cur = VlcConfig::default();
        let patch = ConfigPatch {
            auto_google_fonts: Some(Some(true)),
            ..Default::default()
        };
        let new = apply_patch(&cur, &patch).unwrap();
        assert!(new.auto_google_fonts);
        // Other fields unchanged.
        assert_eq!(new.num_workers, cur.num_workers);
    }

    #[test]
    fn apply_patch_sets_option_field_to_some() {
        let cur = VlcConfig::default();
        let patch = ConfigPatch {
            default_theme: Some(Some("dark".to_string())),
            ..Default::default()
        };
        let new = apply_patch(&cur, &patch).unwrap();
        assert_eq!(new.default_theme, Some("dark".to_string()));
    }

    #[test]
    fn apply_patch_clears_option_field_to_none_on_null() {
        // Start from a state with default_theme set, patch with null to clear.
        let cur = VlcConfig {
            default_theme: Some("dark".to_string()),
            ..Default::default()
        };
        let patch = ConfigPatch {
            default_theme: Some(None),
            ..Default::default()
        };
        let new = apply_patch(&cur, &patch).unwrap();
        assert_eq!(new.default_theme, None);
    }

    #[test]
    fn apply_patch_missing_fonts_enum() {
        let cur = VlcConfig::default();
        let patch = ConfigPatch {
            missing_fonts: Some(Some(MissingFontsPolicy::Warn)),
            ..Default::default()
        };
        let new = apply_patch(&cur, &patch).unwrap();
        assert_eq!(new.missing_fonts, MissingFontsPolicy::Warn);
    }

    #[test]
    fn apply_patch_font_directories_replaces() {
        let cur = VlcConfig::default();
        let patch = ConfigPatch {
            font_directories: Some(Some(vec![
                std::path::PathBuf::from("/a"),
                std::path::PathBuf::from("/b"),
            ])),
            ..Default::default()
        };
        let new = apply_patch(&cur, &patch).unwrap();
        assert_eq!(new.font_directories.len(), 2);
    }

    #[test]
    fn requires_rebuild_identity_is_false() {
        let cur = VlcConfig::default();
        let new = cur.clone();
        assert!(!requires_rebuild(&cur, &new));
    }

    #[test]
    fn requires_rebuild_font_directories_only_is_false() {
        let cur = VlcConfig::default();
        let mut new = cur.clone();
        new.font_directories = vec![std::path::PathBuf::from("/tmp/fonts")];
        assert!(!requires_rebuild(&cur, &new));
    }

    #[test]
    fn requires_rebuild_cache_size_only_is_false() {
        let cur = VlcConfig::default();
        let mut new = cur.clone();
        new.google_fonts_cache_size_mb = NonZeroU64::new(128);
        assert!(!requires_rebuild(&cur, &new));
    }

    #[test]
    fn requires_rebuild_both_hot_apply_fields_is_false() {
        let cur = VlcConfig::default();
        let mut new = cur.clone();
        new.google_fonts_cache_size_mb = NonZeroU64::new(256);
        new.font_directories = vec![std::path::PathBuf::from("/tmp/x")];
        assert!(!requires_rebuild(&cur, &new));
    }

    #[test]
    fn requires_rebuild_non_hot_apply_field_is_true() {
        let cur = VlcConfig::default();
        let mut new = cur.clone();
        new.default_theme = Some("dark".to_string());
        assert!(requires_rebuild(&cur, &new));
    }

    #[test]
    fn requires_rebuild_mix_hot_and_non_hot_is_true() {
        let cur = VlcConfig::default();
        let mut new = cur.clone();
        new.font_directories = vec![std::path::PathBuf::from("/tmp/x")];
        new.auto_google_fonts = !cur.auto_google_fonts;
        assert!(requires_rebuild(&cur, &new));
    }

    #[test]
    fn apply_patch_hot_apply_field_then_requires_rebuild_false() {
        let cur = VlcConfig::default();
        let patch = ConfigPatch {
            google_fonts_cache_size_mb: Some(NonZeroU64::new(64)),
            ..Default::default()
        };
        let new = apply_patch(&cur, &patch).unwrap();
        assert!(!requires_rebuild(&cur, &new));
        assert_eq!(new.google_fonts_cache_size_mb, NonZeroU64::new(64));
    }

    #[test]
    fn apply_patch_non_hot_apply_field_then_requires_rebuild_true() {
        let cur = VlcConfig::default();
        let patch = ConfigPatch {
            subset_fonts: Some(Some(!cur.subset_fonts)),
            ..Default::default()
        };
        let new = apply_patch(&cur, &patch).unwrap();
        assert!(requires_rebuild(&cur, &new));
    }

    #[test]
    fn apply_patch_null_on_non_nullable_is_rejected() {
        let cur = VlcConfig::default();
        // `allowed_base_urls: null` — library field is `Vec<String>`, so
        // null is illegal.
        let patch = ConfigPatch {
            allowed_base_urls: Some(None),
            ..Default::default()
        };
        let err = apply_patch(&cur, &patch).unwrap_err();
        match err {
            PatchRejection::NonNullable(e) => {
                assert_eq!(e.field_errors.len(), 1);
                assert_eq!(e.field_errors[0].path, "allowed_base_urls");
                assert_eq!(e.field_errors[0].code, FieldErrorCode::NonNullable);
            }
            PatchRejection::Invalid(_) => panic!("expected NonNullable, got Invalid"),
        }
    }

    #[test]
    fn apply_patch_multiple_nulls_on_non_nullables_collected() {
        let cur = VlcConfig::default();
        let patch = ConfigPatch {
            allowed_base_urls: Some(None),
            subset_fonts: Some(None),
            themes: Some(None),
            ..Default::default()
        };
        let err = apply_patch(&cur, &patch).unwrap_err();
        match err {
            PatchRejection::NonNullable(e) => {
                assert_eq!(e.field_errors.len(), 3);
                let paths: Vec<&str> =
                    e.field_errors.iter().map(|fe| fe.path.as_str()).collect();
                assert!(paths.contains(&"allowed_base_urls"));
                assert!(paths.contains(&"subset_fonts"));
                assert!(paths.contains(&"themes"));
            }
            PatchRejection::Invalid(_) => panic!("expected NonNullable"),
        }
    }
}
