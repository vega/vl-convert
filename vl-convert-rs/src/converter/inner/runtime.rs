use super::near_heap_limit_callback;
use super::ConversionTimer;
use super::InnerVlConverter;
use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use deno_core::{serde_v8, v8};
use std::sync::Arc;

use super::super::types::{LogEntry, LogLevel};

impl InnerVlConverter {
    /// Returns `true` if the near-heap-limit callback has fired.
    fn heap_limit_was_hit(&self) -> bool {
        if let Some(ptr) = self.heap_limit_data {
            // SAFETY: ptr is a leaked Box<HeapLimitCallbackData> created via
            // Box::into_raw during worker init. It lives for the isolate's lifetime.
            let data = unsafe { &*ptr };
            data.heap_limit_hit
                .load(std::sync::atomic::Ordering::Acquire)
        } else {
            false
        }
    }

    /// If the near-heap-limit callback fired, reset the flag, restore the
    /// original heap limit, and re-register the callback so it fires again
    /// on the next OOM. This must be called after
    /// `cancel_terminate_execution()`.
    pub(crate) fn restore_heap_limit_if_needed(&mut self) {
        if let Some(ptr) = self.heap_limit_data {
            // SAFETY: ptr is a leaked Box<HeapLimitCallbackData> created via
            // Box::into_raw during worker init. It lives for the isolate's lifetime.
            let data = unsafe { &*ptr };
            if !data
                .heap_limit_hit
                .swap(false, std::sync::atomic::Ordering::AcqRel)
            {
                // Callback hasn't fired — nothing to restore.
                return;
            }

            // `max_v8_heap_size_mb` is `Some(..)` when `heap_limit_data` is
            // set (both initialized together in `try_new`).
            let max_bytes = self
                .ctx
                .config
                .max_v8_heap_size_mb
                .map(|n| n.get().saturating_mul(1024 * 1024))
                .unwrap_or(0);
            let isolate = self.worker.js_runtime.v8_isolate();

            // Remove the (already consumed) callback and restore the
            // original heap limit.
            isolate.remove_near_heap_limit_callback(near_heap_limit_callback, max_bytes);

            // GC to free garbage from the failed conversion before
            // re-registering the callback at the original limit.
            isolate.low_memory_notification();

            // Re-register so the next OOM is caught too.
            isolate.add_near_heap_limit_callback(
                near_heap_limit_callback,
                ptr as *const _ as *mut std::ffi::c_void,
            );

            // Clear plugin poisoning so the next request retries init.
            // Don't reset vega_initialized — the modules are still in V8's
            // cache and the GC above reclaimed enough headroom to continue.
            self.plugin_init_error = None;
        }
    }

    /// If the near-heap-limit callback fired, annotate the error with V8
    /// memory stats. Otherwise return the result unchanged.
    pub(crate) fn annotate_heap_limit_error<T>(
        &mut self,
        result: Result<T, AnyError>,
    ) -> Result<T, AnyError> {
        match result {
            Err(original) if self.heap_limit_was_hit() => {
                let stats = self.worker.js_runtime.v8_isolate().get_heap_statistics();
                let used_mb = stats.used_heap_size() as f64 / (1024.0 * 1024.0);
                let total_mb = stats.total_heap_size() as f64 / (1024.0 * 1024.0);
                let external_mb = stats.external_memory() as f64 / (1024.0 * 1024.0);
                let configured_mb = self
                    .ctx
                    .config
                    .max_v8_heap_size_mb
                    .map(|n| n.get())
                    .unwrap_or(0);
                Err(original.context(format!(
                    "V8 heap limit exceeded (configured: {configured_mb} MB). \
                     Worker memory: {used_mb:.1} MB used, {total_mb:.1} MB total, \
                     {external_mb:.1} MB external. \
                     Increase max_v8_heap_size_mb or omit for no limit.",
                )))
            }
            other => other,
        }
    }

    /// Start a conversion timeout timer that also watches for caller disconnect.
    /// Returns `None` if the timeout is disabled (0). The returned
    /// `ConversionTimer` must be cancelled after the conversion completes by
    /// calling `cancel_conversion_timer()`.
    pub(crate) fn start_conversion_timer(
        &mut self,
        caller_gone: Arc<std::sync::atomic::AtomicBool>,
    ) -> Option<ConversionTimer> {
        let duration = self
            .ctx
            .config
            .max_v8_execution_time_secs
            .map(|n| std::time::Duration::from_secs(n.get()))
            .unwrap_or_else(|| std::time::Duration::from_secs(0));
        self.start_conversion_timer_with_duration(duration, caller_gone)
    }

    /// Start a conversion timeout timer with an explicit duration. Also monitors
    /// `caller_gone` so V8 can be terminated early if the caller disconnects.
    pub(crate) fn start_conversion_timer_with_duration(
        &mut self,
        duration: std::time::Duration,
        caller_gone: Arc<std::sync::atomic::AtomicBool>,
    ) -> Option<ConversionTimer> {
        if duration.is_zero() {
            return None;
        }
        let timeout_hit = self.timeout_hit.clone();
        let handle = self.worker.js_runtime.v8_isolate().thread_safe_handle();
        let (cancel_tx, cancel_rx) = std::sync::mpsc::channel::<()>();
        let poll_interval = std::time::Duration::from_millis(50);
        let thread = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + duration;
            loop {
                // Check if the conversion finished normally (Ok = explicit cancel,
                // Disconnected = cancel_tx dropped in cancel_conversion_timer)
                match cancel_rx.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
                // Check if the caller disconnected
                if caller_gone.load(std::sync::atomic::Ordering::Acquire) {
                    timeout_hit.store(true, std::sync::atomic::Ordering::Release);
                    handle.terminate_execution();
                    return;
                }
                // Check if the deadline expired
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    timeout_hit.store(true, std::sync::atomic::Ordering::Release);
                    handle.terminate_execution();
                    return;
                }
                std::thread::sleep(poll_interval.min(remaining));
            }
        });
        Some(ConversionTimer { cancel_tx, thread })
    }

    /// Cancel a running conversion timer and join its thread. Safe to call
    /// even if the timer already fired.
    pub(crate) fn cancel_conversion_timer(&self, timer: Option<ConversionTimer>) {
        if let Some(timer) = timer {
            drop(timer.cancel_tx);
            let _ = timer.thread.join();
        }
    }

    /// Returns `true` if the conversion timeout timer has fired.
    fn timeout_was_hit(&self) -> bool {
        self.timeout_hit.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Reset the timeout flag after a timed-out conversion so the worker
    /// can process subsequent requests. Also clears plugin poisoning and
    /// vega init state so the next request retries initialization.
    pub(crate) fn reset_timeout_if_needed(&mut self) {
        if self
            .timeout_hit
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            self.plugin_init_error = None;
            self.vega_initialized = false;
        }
    }

    /// If the timeout timer fired, annotate the error with timeout details.
    /// Otherwise return the result unchanged.
    pub(crate) fn annotate_timeout_error<T>(
        &self,
        result: Result<T, AnyError>,
    ) -> Result<T, AnyError> {
        match result {
            Err(original) if self.timeout_was_hit() => {
                let configured = self
                    .ctx
                    .config
                    .max_v8_execution_time_secs
                    .map(|n| n.get())
                    .unwrap_or(0);
                Err(original.context(format!(
                    "Conversion timed out (configured: {configured} seconds). \
                     Increase max_v8_execution_time_secs or omit for no limit.",
                )))
            }
            other => other,
        }
    }

    pub(crate) async fn execute_script_to_json(
        &mut self,
        script: &str,
    ) -> Result<serde_json::Value, AnyError> {
        let code = script.to_string();
        let res = self.worker.js_runtime.execute_script("ext:<anon>", code)?;

        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        deno_core::scope!(scope, self.worker.js_runtime);
        let local = v8::Local::new(scope, res);

        // Deserialize a `v8` object into a Rust type using `serde_v8`,
        // in this case deserialize to a JSON `Value`.
        let deserialized_value = serde_v8::from_v8::<serde_json::Value>(scope, local);
        deserialized_value.map_err(|err| anyhow!("Failed to deserialize JavaScript value: {err}"))
    }

    pub(crate) async fn execute_script_to_string(
        &mut self,
        script: &str,
    ) -> Result<String, AnyError> {
        let code = script.to_string();
        let res = self.worker.js_runtime.execute_script("ext:<anon>", code)?;

        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        deno_core::scope!(scope, self.worker.js_runtime);
        let local = v8::Local::new(scope, res);

        // Deserialize a `v8` object into a Rust type using `serde_v8`,
        // in this case deserialize to a JSON `Value`.
        let deserialized_value = serde_v8::from_v8::<serde_json::Value>(scope, local);

        let value = match deserialized_value {
            Ok(value) => {
                let value = value.as_str();
                value.unwrap().to_string()
            }
            Err(err) => bail!("{err}"),
        };

        Ok(value)
    }

    pub(crate) async fn execute_script_to_bytes(
        &mut self,
        script: &str,
    ) -> Result<Vec<u8>, AnyError> {
        let code = script.to_string();
        let res = self.worker.js_runtime.execute_script("ext:<anon>", code)?;

        self.worker
            .js_runtime
            .run_event_loop(Default::default())
            .await?;

        deno_core::scope!(scope, self.worker.js_runtime);
        let local = v8::Local::new(scope, res);

        // Deserialize typed-array data to bytes directly.
        let bytes = serde_v8::from_v8::<serde_v8::JsBuffer>(scope, local)?;
        Ok(bytes.to_vec())
    }

    pub(super) async fn emit_js_log_messages(&mut self) {
        self.last_log_entries.clear();
        let json = match self
            .execute_script_to_string("_collapsedLogMessages()")
            .await
        {
            Ok(json) => json,
            Err(e) => {
                vl_debug!("Failed to retrieve JS log messages: {e}");
                return;
            }
        };
        if json.is_empty() {
            return;
        }
        let entries: Vec<serde_json::Value> = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(e) => {
                vl_debug!("Failed to parse JS log messages: {e}");
                return;
            }
        };
        for entry in &entries {
            let level = entry.get("level").and_then(|v| v.as_str()).unwrap_or("");
            let msg = entry.get("msg").and_then(|v| v.as_str()).unwrap_or("");
            match level {
                "error" => {
                    vl_error!("{}", msg);
                    self.last_log_entries.push(LogEntry {
                        level: LogLevel::Error,
                        message: msg.to_string(),
                    });
                }
                "warn" => {
                    vl_warn!("{}", msg);
                    self.last_log_entries.push(LogEntry {
                        level: LogLevel::Warn,
                        message: msg.to_string(),
                    });
                }
                "info" => {
                    vl_info!("{}", msg);
                    self.last_log_entries.push(LogEntry {
                        level: LogLevel::Info,
                        message: msg.to_string(),
                    });
                }
                "debug" => {
                    vl_debug!("{}", msg);
                    self.last_log_entries.push(LogEntry {
                        level: LogLevel::Debug,
                        message: msg.to_string(),
                    });
                }
                _ => {}
            }
        }
    }
}
