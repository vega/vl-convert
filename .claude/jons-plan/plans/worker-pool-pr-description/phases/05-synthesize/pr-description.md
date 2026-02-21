# Parallel Worker Pool for VlConverter + V8 Platform Init Safety

## Summary

Introduces a configurable parallel worker pool for `VlConverter`, Python worker-count APIs (`set_num_workers` / `get_num_workers`), and a proactive V8 platform initialization guard that prevents a class of sporadic `SIGSEGV` crashes when spawning multiple Deno isolates from a multithreaded process. Default behavior is unchanged (1 worker).

## Motivation

### Background: The #206 segfault and the #237 workaround

[Issue #206](https://github.com/vega/vl-convert/issues/206) reported a segfault when creating multiple `VlConverter` instances — each instance was spawning its own OS thread with its own V8 isolate, and the second isolate crashed inside `JsRuntime::new_inner`. [PR #237](https://github.com/vega/vl-convert/pull/237) fixed the crash by collapsing to a single shared global worker thread (`VL_CONVERTER_RUNTIME`), so only one V8 isolate ever existed in the process. That fixed the segfault — but it made all conversions from all instances serialize through the same thread, with no path to concurrency.

### What was actually causing the crash

Since V8 11.6, V8 enforces W^X on JIT pages using Intel/AMD [Memory Protection Keys for Userspace (PKU/MPK)](https://www.kernel.org/doc/html/latest/core-api/protection-keys.html). PKU state is stored in the per-thread `PKRU` register, which is **inherited from parent to child at `pthread_create()` time**. V8 writes the correct `PKRU` state during platform initialization. Any thread **not descended** from the platform-initializing thread gets wrong `PKRU` state for V8's JIT pages — causing `SIGSEGV` not at startup, but **sporadically later when the JIT compiler activates**.

The original multi-instance code created new worker threads from arbitrary calling threads (not the platform-initializing thread), so V8's JIT pages were inaccessible from those workers. The global-singleton workaround (#237) was immune because only one worker thread ever existed, but it gave up concurrency to get there.

### This PR: the proper fix enables multiple workers

By calling `init_platform` once on the parent thread (via `ensure_v8_platform_initialized`) **before** spawning any worker threads, all workers are descendants of the platform-initializing thread and inherit correct `PKRU` state. This is a proactive workaround, not runtime detection — it unconditionally calls `init_platform` once via a `std::sync::Once` guard. With this guard in place, N workers can safely coexist, which removes the architectural bottleneck from #237 and enables genuine parallel execution.

References:
- [`deno_core::JsRuntime` docs](https://docs.rs/deno_core/latest/deno_core/struct.JsRuntime.html): *"Since V8 11.6, all runtimes must have a common parent thread that initialized the V8 platform."*
- [`deno_core` PR #471](https://github.com/denoland/deno_core/pull/471): Segfault fix on PKU-enabled CPUs
- [`rusty_v8` issue #1381](https://github.com/denoland/rusty_v8/issues/1381): Direct SIGSEGV report on Intel 13th-gen hardware
- [`deno` issue #20495](https://github.com/denoland/deno/issues/20495): *"Unless V8 platform is initialized on main thread the segfaults start appearing once JIT kicks in."*

## What Changed

### Rust — `vl-convert-rs/src/converter.rs`

**`VlConverter` is now `Clone`** via `Arc<VlConverterInner>`. All conversion methods take `&self` instead of `&mut self`.

New API:
- `VlConverter::with_num_workers(n: usize) -> Result<Self, AnyError>` — construct with a specific worker count (validated ≥ 1)
- `VlConverter::num_workers(&self) -> usize` — query configured worker count
- `VlConverter::new()` — unchanged, defaults to 1 worker

**Clone semantics:** `#[derive(Clone)]` on `VlConverter` clones the `Arc<VlConverterInner>` — it increments the reference count, not the pool. The clone and the original share the same worker pool, bundle cache, and configuration. The pool is torn down only when all clones are dropped. To get an **independent** pool (separate workers, separate memory), construct a new `VlConverter::with_num_workers(n)` rather than cloning.

**Per-instance pools (behavioral change from #237):** Previously all `VlConverter` instances shared a single global `VL_CONVERTER_RUNTIME` thread. Now each instance owns its own worker pool — `new()` calls no longer share state. Cloning is the way to get multiple handles to one pool.

**Worker pool model:** One dedicated OS thread per worker, each with a `new_current_thread()` Tokio runtime + `LocalSet` (required because Deno's `MainWorker` uses `!Send` types). Per-worker bounded channels (capacity 32) provide backpressure; round-robin dispatch via lock-free `AtomicUsize` balances load. The round-robin design (vs. a single shared queue) is required because each Deno runtime is stateful and cannot have work redistributed mid-execution. Pool spawns lazily on first request with startup handshake for error propagation.

**V8 init guard:**
```rust
fn ensure_v8_platform_initialized() {
    static V8_INIT: Once = Once::new();
    // V8 11.6+ PKU requirement: all worker threads must descend from the thread
    // that initialized the platform. PKRU register is inherited at pthread_create()
    // time; threads not in the lineage get wrong JIT page permissions → SIGSEGV.
    V8_INIT.call_once(|| deno_core::JsRuntime::init_platform(None, false));
}
```

Called at the start of `spawn_worker_pool`, before any worker thread is created.

**Send retry:** If a worker's channel is closed (worker died), `send_command_with_retry` resets the pool and retries once. If the retry also fails, the error is returned to the caller.

**Worker-local transfer state:** JSON args / MessagePack scenegraph payloads are now stored per worker in Deno `OpState` (`WorkerTransferState`), removing process-wide contention on `JSON_ARGS` / `MSGPACK_RESULTS` / `NEXT_ID`. `JsonArgGuard` and `MsgpackResultGuard` remain RAII-based, but now clean up worker-local state on all error paths.

### Python — `vl-convert-python/src/lib.rs` + `vl_convert.pyi`

The global converter state changed from `Mutex<VlConverterRs>` to `RwLock<Arc<VlConverterRs>>`. The read lock is held only briefly to clone the `Arc`; conversion runs after the lock is released, so concurrent conversion calls don't block each other. `set_num_workers` write-locks briefly to swap in a new `Arc`; in-flight callers hold the old `Arc` and complete normally.

New public API:
```python
vl_convert.set_num_workers(num_workers: int) -> None
vl_convert.get_num_workers() -> int
```

**GIL release:** Vega-Lite and Vega conversion functions (those backed by Deno/Tokio) now call `py.allow_threads(|| ...)` to release the Python GIL during the blocking Rust execution, enabling true concurrency when called from Python threads. (Note: `svg_to_png`, `svg_to_jpeg`, `svg_to_pdf` are synchronous and not affected.)

**Not asyncio-compatible.** These functions call `block_on` internally. Calling from an asyncio event loop without an executor will stall the loop. Use `loop.run_in_executor(None, ...)` from async contexts.

### Tests and Docs

New Rust tests: `test_with_num_workers_rejects_zero`, `test_num_workers_reports_configured_value`, `test_parallel_conversions_with_shared_converter`.

New Python tests (`test_workers.py`): default count, zero-worker rejection, 16 parallel conversions with 4 workers, `set_num_workers` during concurrent submissions.

README: added "Configure Worker Parallelism" section.

## User-Facing Impact

- **No breaking changes.** All existing Python and CLI APIs work identically.
- **Default is unchanged:** 1 worker, same behavior as before.
- **Rust API:** Conversion methods changed from `&mut self` to `&self` (backwards-compatible; enables shared access without exclusive borrow).
- **Python API:** `set_num_workers` / `get_num_workers` are additive.
- **No CLI flag changes.** The CLI creates one converter per invocation.

**Memory note:** Each worker is a full Deno/V8 runtime with Vega-Lite loaded (~100–200 MB, rough estimate). Pool state is **per `VlConverter` instance** — reuse or clone one converter handle rather than constructing multiple instances unless independent pools are intentional. See the README's "Parallel Workers" section for usage guidance.

## Known Limitations

**First-request latency.** The pool spawns lazily on first use while holding the pool mutex. With N workers, concurrent callers queue behind the mutex for the duration of N V8 bootstrap cycles. Subsequent requests are unaffected. Eager initialization is a possible follow-up.

## Review Tour

### Tier 1: Core Change

**`vl-convert-rs/src/converter.rs`**

New types near top: `WorkerPool` (~line 60), `ensure_v8_platform_initialized` (~line 89). `VlConverterInner`/`VlConverter` are further down (~lines 1739–1750). Worker pool implementation: `spawn_worker_pool`, `get_or_spawn_sender`, `send_command_with_retry`, `request`. RAII guards (`JsonArgGuard`, `MsgpackResultGuard`) at top of file.

Key review questions:
- Is the `Mutex<Option<WorkerPool>>` locking strategy sound? (Note: held during pool spawn — blocks concurrent first-callers for the full bootstrap duration)
- Does `send_command_with_retry` correctly handle the race between detecting a closed channel and resetting the pool?
- Is `WorkerTransferState` lifecycle in `OpState` correct (initialized once per worker and available to all ops)?
- Are transfer-state borrows and RAII cleanup (`JsonArgGuard` / `MsgpackResultGuard`) correct for early-return and error paths?
- Is round-robin dispatch via `AtomicUsize::Relaxed` correct here?

### Tier 2: Python Integration

**`vl-convert-python/src/lib.rs`**

`VL_CONVERTER` declaration (global `RwLock<Arc<>>`), `converter_read_handle`, `run_converter_future` (clones the `Arc`, releases GIL, drives the future), `set_num_workers` / `get_num_workers`.

Key review questions:
- Is the `RwLock<Arc<>>` short-lock-and-release pattern correct for concurrent access?
- Does `run_converter_future` correctly release the GIL and propagate errors from async paths?
- Which functions still don't call `allow_threads` (svg_to_* paths)?

### Tier 3: Tests and Types

**`vl-convert-python/tests/test_workers.py`** — New file. Four focused tests for the worker API.

**`vl-convert-python/vl_convert.pyi`** — Type stubs for `set_num_workers` and `get_num_workers`.

### Tier 4: Mechanical (Skim or Skip)

Minor `let mut` → `let` changes in `vl-convert/src/main.rs`, `test_specs.rs`, `test_themes.rs`, and docs-only addition to `README.md`.
