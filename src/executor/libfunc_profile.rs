//! Profiling-instrumented run wrapper around [`AotContractExecutor::run`].
//!
//! Available under the `with-libfunc-profiling` feature (gated at the `mod`
//! declaration in `src/executor.rs`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use cairo_lang_sierra::program::Program;
use starknet_types_core::felt::Felt;

use crate::error::{Error, Result};
use crate::execution_result::ContractExecutionResult;
use crate::executor::AotContractExecutor;
use crate::metadata::profiler::{Profile, ProfilerBinding, ProfilerImpl, LIBFUNC_PROFILE};
use crate::starknet::StarknetSyscallHandler;
use crate::utils::BuiltinCosts;

/// Process-wide lock that serializes calls into [`AotContractExecutor::run_with_libfunc_profile`].
/// The profiler hot-swaps a process-global symbol (`cairo_native__profiler__profile_id`);
/// concurrent callers would race on that write and on the [`LIBFUNC_PROFILE`] slot bookkeeping.
static PROFILE_LOCK: Mutex<()> = Mutex::new(());

impl AotContractExecutor {
    /// Run the entrypoint with libfunc-level profiling instrumentation.
    ///
    /// Wraps [`AotContractExecutor::run`] with the bookkeeping the
    /// `with-libfunc-profiling` runtime needs:
    ///
    /// 1. Acquires [`PROFILE_LOCK`] so concurrent profile calls serialize on the
    ///    global trace-id symbol. The lock is recovered if poisoned.
    /// 2. Looks up the executor's `cairo_native__profiler__profile_id` symbol. If
    ///    absent (the .so was compiled without profiling instrumentation) the call
    ///    returns an error before touching any global state.
    /// 3. Allocates a unique trace ID and inserts an empty `ProfilerImpl` slot in
    ///    [`LIBFUNC_PROFILE`]; points the profile-id symbol at the new ID, saving
    ///    the previous value.
    /// 4. Calls `run`. Per-statement samples accumulate in the slot via the runtime
    ///    `push_stmt` callback.
    /// 5. Drains the slot. On success (and only on success) hands the resulting
    ///    [`Profile`] to `on_profile`; on failure the callback is not invoked
    ///    (partial profiles aren't meaningful).
    /// 6. A [`ProfilerGuard`] restores the previous trace ID and clears the slot on
    ///    both the success and unwind paths.
    ///
    /// `program` must be the Sierra program this executor was compiled from; it's used
    /// by `get_profile` to map runtime libfunc IDs back to declarations.
    #[allow(clippy::too_many_arguments)]
    pub fn run_with_libfunc_profile<H, F>(
        &self,
        program: &Arc<Program>,
        selector: Felt,
        args: &[Felt],
        gas: u64,
        builtin_costs: Option<BuiltinCosts>,
        syscall_handler: H,
        on_profile: F,
    ) -> Result<ContractExecutionResult>
    where
        H: StarknetSyscallHandler,
        F: FnOnce(Profile),
    {
        // Serialize against concurrent profile calls. Recover from a poisoned lock —
        // we don't have invariants on the protected state itself; the lock only gates
        // access to the global trace-id symbol.
        let _profile_lock = PROFILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        // Look up the profile-id symbol before touching any global state. If the
        // executor wasn't compiled with libfunc-profiling instrumentation, the
        // symbol is absent — return a typed error rather than panicking.
        let trace_id_ptr = self
            .find_symbol_ptr(ProfilerBinding::ProfileId.symbol())
            .ok_or_else(|| {
                Error::UnexpectedValue(format!(
                    "AOT executor missing libfunc-profiling symbol `{}`; \
                     was the program compiled with libfunc-profiling enabled?",
                    ProfilerBinding::ProfileId.symbol()
                ))
            })?
            .cast::<u64>();

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);

        LIBFUNC_PROFILE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(counter, ProfilerImpl::new());

        // SAFETY: the pointer targets a memref-global emitted into the executor's
        // shared library; the executor outlives the call. `PROFILE_LOCK` serializes
        // us against any other writer, and the JIT/AOT code reads through the same
        // address. Reads/writes are aligned `u64`s.
        let old_trace_id = unsafe { *trace_id_ptr };
        unsafe {
            *trace_id_ptr = counter;
        }

        let _guard = ProfilerGuard {
            trace_id_ptr,
            old_trace_id,
            counter,
        };

        let result = self.run(selector, args, gas, builtin_costs, syscall_handler);

        // Drain the slot. `ProfilerGuard::drop` would also remove it; doing it here
        // means we hold the lock for the shortest time and can hand the profile to
        // the callback. Tolerate a poisoned mutex (we'd lose the profile, not state).
        let drained = LIBFUNC_PROFILE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&counter);

        // Only call the user's callback when `run` succeeded — a partial profile
        // captured against an aborted execution wouldn't be meaningful.
        if let (Some(profiler), Ok(_)) = (drained, &result) {
            on_profile(profiler.get_profile(program));
        }

        result
    }
}

/// RAII cleanup for the profiler globals. Restores `*trace_id_ptr` on success or
/// unwind. The [`LIBFUNC_PROFILE`] slot at `counter` is normally drained on the
/// success path; this guard removes it if it's still occupied (panic case).
struct ProfilerGuard {
    trace_id_ptr: *mut u64,
    old_trace_id: u64,
    counter: u64,
}

impl Drop for ProfilerGuard {
    fn drop(&mut self) {
        // SAFETY: same provenance as the construction site. `PROFILE_LOCK` is held
        // by the enclosing scope (still in flight while we drop) so no other thread
        // races us.
        unsafe {
            *self.trace_id_ptr = self.old_trace_id;
        }
        // Tolerate a poisoned mutex silently — Drop must not panic. Slot leak on
        // poison is intentional and matches the behavior of other Drop impls in
        // this crate; the alternative (panic in Drop) is worse.
        if let Ok(mut profile) = LIBFUNC_PROFILE.lock() {
            profile.remove(&self.counter);
        }
    }
}
