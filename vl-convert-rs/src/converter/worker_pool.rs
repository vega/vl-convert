use deno_core::anyhow::{anyhow, bail};
use deno_core::error::AnyError;
use std::path::Path;
use std::sync::Arc;
use std::sync::Once;
use std::thread;
use std::thread::JoinHandle;

use std::sync::atomic::{AtomicUsize, Ordering};

use super::config::{
    parse_allowed_base_urls_from_config, ConverterContext, ResolvedPlugin, VlcConfig,
};
use super::InnerVlConverter;
use crate::text::get_font_baseline_snapshot;

/// Arguments are passed to V8 as JSON strings via Deno ops and parsed in JS.
/// Scenegraph results are returned as MessagePack byte buffers via ops,
/// avoiding JSON serialization overhead for large payloads.
pub(crate) struct WorkerPool {
    pub(super) senders: Vec<tokio::sync::mpsc::Sender<QueuedWork>>,
    // Per-worker count of requests that have been reserved for this worker but not yet
    // fully processed. This includes in-flight senders blocked on channel capacity and
    // commands currently queued/executing in the worker loop.
    pub(super) outstanding: Vec<Arc<AtomicUsize>>,
    pub(super) dispatch_cursor: AtomicUsize,
    pub(super) _handles: Vec<JoinHandle<()>>,
}

impl WorkerPool {
    pub(crate) fn next_sender(
        &self,
    ) -> Option<(tokio::sync::mpsc::Sender<QueuedWork>, OutstandingTicket)> {
        if self.senders.is_empty() {
            return None;
        }

        // Choose the worker with the smallest outstanding count. Rotate scan start so ties
        // are not biased to index 0.
        let start = self.dispatch_cursor.fetch_add(1, Ordering::Relaxed) % self.senders.len();
        let mut best_idx = None;
        let mut best_outstanding = usize::MAX;

        for offset in 0..self.senders.len() {
            let idx = (start + offset) % self.senders.len();
            if self.senders[idx].is_closed() {
                continue;
            }

            let outstanding = self.outstanding[idx].load(Ordering::Relaxed);
            if outstanding < best_outstanding {
                best_idx = Some(idx);
                best_outstanding = outstanding;
                if outstanding == 0 {
                    break;
                }
            }
        }

        let idx = best_idx?;
        let ticket = OutstandingTicket::new(self.outstanding[idx].clone());
        Some((self.senders[idx].clone(), ticket))
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.senders
            .iter()
            .all(tokio::sync::mpsc::Sender::is_closed)
    }
}

pub(crate) struct OutstandingTicket {
    counter: Arc<AtomicUsize>,
}

impl OutstandingTicket {
    pub(crate) fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self { counter }
    }
}

impl Drop for OutstandingTicket {
    fn drop(&mut self) {
        let prev = self.counter.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(prev > 0, "outstanding counter underflow");
    }
}

/// A boxed closure that executes work on a worker's InnerVlConverter.
pub(crate) type WorkFn = Box<
    dyn FnOnce(
            &mut InnerVlConverter,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + '_>>
        + Send,
>;

pub(crate) struct QueuedWork {
    pub(super) work: WorkFn,
    pub(super) ticket: OutstandingTicket,
    pub(super) caller_gone: Arc<std::sync::atomic::AtomicBool>,
}

impl QueuedWork {
    pub(crate) fn new(
        work: WorkFn,
        ticket: OutstandingTicket,
        caller_gone: Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            work,
            ticket,
            caller_gone,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        WorkFn,
        OutstandingTicket,
        Arc<std::sync::atomic::AtomicBool>,
    ) {
        (self.work, self.ticket, self.caller_gone)
    }
}

/// Drop guard that signals `caller_gone` when the caller's future is dropped
/// (e.g. by HTTP timeout). Call `disarm()` on normal completion to prevent
/// spurious signaling.
pub(crate) struct CallerGoneGuard {
    flag: Arc<std::sync::atomic::AtomicBool>,
    armed: bool,
}

impl CallerGoneGuard {
    pub(crate) fn new(flag: Arc<std::sync::atomic::AtomicBool>) -> Self {
        Self { flag, armed: true }
    }
    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CallerGoneGuard {
    fn drop(&mut self) {
        if self.armed {
            self.flag.store(true, std::sync::atomic::Ordering::Release);
        }
    }
}

pub(crate) fn ensure_v8_platform_initialized() {
    static V8_INIT: Once = Once::new();
    V8_INIT.call_once(|| deno_core::JsRuntime::init_platform(None));
}

pub(crate) fn worker_queue_capacity(num_workers: usize) -> usize {
    num_workers.saturating_mul(32).max(32)
}

/// Minimum `max_v8_heap_size_mb` in MB. Values below this cause V8 to
/// abort during isolate creation (unrecoverable).
pub(crate) const MIN_V8_HEAP_SIZE_MB: usize = 64;

/// Bundle a URL plugin using the URL as the deno_emit entry specifier.
pub(crate) async fn bundle_url_plugin(
    url: &str,
    allowed_domains: &[String],
) -> Result<String, AnyError> {
    use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
    use crate::module_loader::PluginBundleLoader;

    let entry = deno_core::url::Url::parse(url)?;
    let mut loader = PluginBundleLoader {
        entry_source: String::new(),
        entry_specifier: String::new(),
        allowed_domains: allowed_domains.to_vec(),
    };
    let bundled = bundle(
        entry,
        &mut loader,
        BundleOptions {
            bundle_type: BundleType::Module,
            transpile_options: Default::default(),
            emit_options: EmitOptions {
                source_map: SourceMapOption::None,
                ..Default::default()
            },
            emit_ignore_directives: false,
            minify: false,
        },
    )
    .await?;
    Ok(bundled.code)
}

/// Bundle a file/inline plugin into a self-contained ESM string.
pub(crate) async fn bundle_source_plugin(
    source: &str,
    allowed_domains: &[String],
) -> Result<String, AnyError> {
    use crate::deno_emit::{bundle, BundleOptions, BundleType, EmitOptions, SourceMapOption};
    use crate::module_loader::PluginBundleLoader;

    let entry =
        deno_core::resolve_path("vl-plugin-entry.js", Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let entry_str = entry.to_string();
    let mut loader = PluginBundleLoader {
        entry_source: source.to_string(),
        entry_specifier: entry_str,
        allowed_domains: allowed_domains.to_vec(),
    };
    let bundled = bundle(
        entry,
        &mut loader,
        BundleOptions {
            bundle_type: BundleType::Module,
            transpile_options: Default::default(),
            emit_options: EmitOptions {
                source_map: SourceMapOption::None,
                ..Default::default()
            },
            emit_ignore_directives: false,
            minify: false,
        },
    )
    .await?;
    Ok(bundled.code)
}

/// Resolve and bundle all plugins. Runs async on a dedicated thread.
/// Returns the resolved plugins (or None if no plugins configured).
/// Bundle a single plugin entry (URL or source) into a ResolvedPlugin.
pub(crate) async fn resolve_plugin(
    entry: &str,
    allowed_domains: &[String],
) -> Result<ResolvedPlugin, AnyError> {
    let is_url = entry.starts_with("http://") || entry.starts_with("https://");
    if is_url {
        let bundled = bundle_url_plugin(entry, allowed_domains).await?;
        Ok(ResolvedPlugin {
            original_url: Some(entry.to_string()),
            bundled_source: bundled,
        })
    } else {
        let bundled = bundle_source_plugin(entry, allowed_domains).await?;
        Ok(ResolvedPlugin {
            original_url: None,
            bundled_source: bundled,
        })
    }
}

pub(crate) async fn resolve_and_bundle_plugins(
    config: &VlcConfig,
) -> Result<Option<Vec<ResolvedPlugin>>, AnyError> {
    let Some(ref plugins) = config.vega_plugins else {
        return Ok(None);
    };
    let mut resolved = Vec::new();
    for (i, entry) in plugins.iter().enumerate() {
        let plugin = resolve_plugin(entry, &config.plugin_import_domains)
            .await
            .map_err(|e| anyhow!("Vega plugin {i} bundling failed: {e}"))?;
        resolved.push(plugin);
    }
    Ok(Some(resolved))
}

pub(crate) fn spawn_worker_pool(
    config: Arc<VlcConfig>,
) -> Result<(WorkerPool, Arc<ConverterContext>), AnyError> {
    let num_workers = config.num_workers;
    if num_workers < 1 {
        bail!("num_workers must be >= 1");
    }
    ensure_v8_platform_initialized();

    // Resolve plugins before spawning workers (needs async for HTTP + deno_emit).
    // Runs on a dedicated thread with its own tokio runtime to avoid
    // "nested runtime" panics when called from Python's async path.
    let resolved_plugins = if config.vega_plugins.is_some() {
        let config_ref = config.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| anyhow!("Failed to build plugin resolver runtime: {e}"))?;
            rt.block_on(resolve_and_bundle_plugins(&config_ref))
        })
        .join()
        .map_err(|_| anyhow!("Plugin resolver thread panicked"))??
    } else {
        None
    };

    let parsed_allowed_base_urls = parse_allowed_base_urls_from_config(&config)?;
    let ctx = Arc::new(ConverterContext {
        config: (*config).clone(),
        parsed_allowed_base_urls,
        resolved_plugins: resolved_plugins.clone(),
    });

    let initial_font_baseline = get_font_baseline_snapshot()?;

    let total_queue_capacity = worker_queue_capacity(num_workers);
    let per_worker_queue_capacity = (total_queue_capacity / num_workers).max(1);
    let mut handles = Vec::with_capacity(num_workers);
    let mut senders = Vec::with_capacity(num_workers);
    let mut startup_receivers = Vec::with_capacity(num_workers);

    for _ in 0..num_workers {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<QueuedWork>(per_worker_queue_capacity);
        senders.push(tx);
        let (startup_tx, startup_rx) = std::sync::mpsc::channel::<Result<(), String>>();
        let worker_ctx = ctx.clone();
        let worker_font_baseline = initial_font_baseline.clone();
        let handle = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    let _ = startup_tx.send(Err(format!("Failed to construct runtime: {err}")));
                    return;
                }
            };
            let local = tokio::task::LocalSet::new();
            local.block_on(&runtime, async move {
                let mut inner =
                    match InnerVlConverter::try_new(worker_ctx, worker_font_baseline).await {
                        Ok(inner) => {
                            let _ = startup_tx.send(Ok(()));
                            inner
                        }
                        Err(err) => {
                            let _ = startup_tx.send(Err(err.to_string()));
                            return;
                        }
                    };

                while let Some(queued_work) = rx.recv().await {
                    // Keep the ticket alive for the full loop iteration so outstanding
                    // covers the work execution (drop happens at iteration end).
                    let (work, _ticket, caller_gone) = queued_work.into_parts();

                    // Skip work if the caller already timed out or disconnected
                    if caller_gone.load(std::sync::atomic::Ordering::Acquire) {
                        continue;
                    }

                    let timer = inner.start_conversion_timer(caller_gone);
                    (work)(&mut inner).await;
                    inner.cancel_conversion_timer(timer);

                    // If V8 execution was terminated (e.g. by the near-heap-limit
                    // callback or the conversion timeout timer), clear the
                    // terminated state so the worker can process subsequent
                    // commands, then restore the original heap limit and
                    // re-register the callback.
                    inner
                        .worker
                        .js_runtime
                        .v8_isolate()
                        .cancel_terminate_execution();
                    inner.restore_heap_limit_if_needed();
                    inner.reset_timeout_if_needed();

                    if inner.ctx.config.gc_after_conversion {
                        inner
                            .worker
                            .js_runtime
                            .v8_isolate()
                            .low_memory_notification();
                    }
                }
            });
        });
        handles.push(handle);
        startup_receivers.push(startup_rx);
    }

    for startup_rx in startup_receivers {
        match startup_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(err)) => {
                drop(senders);
                for handle in handles {
                    let _ = handle.join();
                }
                bail!("Failed to initialize worker: {err}");
            }
            Err(err) => {
                drop(senders);
                for handle in handles {
                    let _ = handle.join();
                }
                bail!("Failed to receive worker startup status: {err}");
            }
        }
    }

    let num = senders.len();
    Ok((
        WorkerPool {
            senders,
            outstanding: (0..num).map(|_| Arc::new(AtomicUsize::new(0))).collect(),
            dispatch_cursor: AtomicUsize::new(0),
            _handles: handles,
        },
        ctx,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_work() -> WorkFn {
        Box::new(|_inner| Box::pin(async {}))
    }

    #[test]
    fn test_worker_pool_next_sender_balances_outstanding_reservations() {
        let mut senders = Vec::new();
        let mut _receivers = Vec::new();
        for _ in 0..3 {
            let (tx, rx) = tokio::sync::mpsc::channel::<QueuedWork>(1);
            senders.push(tx);
            _receivers.push(rx);
        }

        let pool = WorkerPool {
            senders,
            outstanding: (0..3)
                .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
                .collect(),
            dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
            _handles: Vec::new(),
        };

        let mut tickets = Vec::new();
        for _ in 0..30 {
            let (_, ticket) = pool
                .next_sender()
                .expect("pool with open senders should produce a sender");
            tickets.push(ticket);

            let loads: Vec<usize> = pool
                .outstanding
                .iter()
                .map(|outstanding| outstanding.load(std::sync::atomic::Ordering::Relaxed))
                .collect();
            let min = *loads.iter().min().expect("loads should not be empty");
            let max = *loads.iter().max().expect("loads should not be empty");
            assert!(
                max - min <= 1,
                "expected balanced outstanding counts, got {loads:?}"
            );
        }

        drop(tickets);
        for outstanding in &pool.outstanding {
            assert_eq!(outstanding.load(std::sync::atomic::Ordering::Relaxed), 0);
        }
    }

    #[test]
    fn test_worker_pool_next_sender_skips_closed_senders() {
        let (closed_sender, closed_receiver) = tokio::sync::mpsc::channel::<QueuedWork>(1);
        drop(closed_receiver);

        let (open_sender, mut open_receiver) = tokio::sync::mpsc::channel::<QueuedWork>(1);

        let pool = WorkerPool {
            senders: vec![closed_sender, open_sender],
            outstanding: (0..2)
                .map(|_| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)))
                .collect(),
            dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
            _handles: Vec::new(),
        };

        for _ in 0..4 {
            let (sender, ticket) = pool
                .next_sender()
                .expect("pool should return the open sender");
            sender
                .try_send(QueuedWork::new(
                    make_test_work(),
                    ticket,
                    Arc::new(std::sync::atomic::AtomicBool::new(false)),
                ))
                .expect("dispatch should use open sender, not closed sender");
            let queued = open_receiver
                .try_recv()
                .expect("open receiver should receive dispatched command");
            drop(queued);
        }
    }

    #[tokio::test]
    async fn test_worker_pool_cancellation_releases_outstanding_ticket() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<QueuedWork>(1);
        let pool = WorkerPool {
            senders: vec![sender],
            outstanding: vec![std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))],
            dispatch_cursor: std::sync::atomic::AtomicUsize::new(0),
            _handles: Vec::new(),
        };

        let (sender, ticket) = pool.next_sender().unwrap();
        sender
            .send(QueuedWork::new(
                make_test_work(),
                ticket,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ))
            .await
            .unwrap();
        assert_eq!(
            pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        let (sender, ticket) = pool.next_sender().unwrap();
        let blocked_send = tokio::spawn(async move {
            sender
                .send(QueuedWork::new(
                    make_test_work(),
                    ticket,
                    Arc::new(std::sync::atomic::AtomicBool::new(false)),
                ))
                .await
        });
        tokio::task::yield_now().await;

        assert_eq!(
            pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
            2
        );

        blocked_send.abort();
        let _ = blocked_send.await;
        tokio::task::yield_now().await;

        assert_eq!(
            pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        let queued = receiver
            .recv()
            .await
            .expect("first queued command should still be in the channel");
        drop(queued);

        assert_eq!(
            pool.outstanding[0].load(std::sync::atomic::Ordering::Relaxed),
            0
        );
    }
}
