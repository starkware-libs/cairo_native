//! Profiling-instrumented run wrapper around [`AotContractExecutor::run`].
//!
//! Available under the `with-libfunc-profiling` feature (gated at the `mod`
//! declaration in `src/executor.rs`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use cairo_lang_sierra::program::Program;
use starknet_types_core::felt::Felt;

use crate::error::Result;
use crate::execution_result::ContractExecutionResult;
use crate::executor::AotContractExecutor;
use crate::metadata::profiler::{Profile, ProfilerBinding, ProfilerImpl, LIBFUNC_PROFILE};
use crate::starknet::StarknetSyscallHandler;
use crate::utils::BuiltinCosts;

impl AotContractExecutor {
    /// Run the entrypoint with libfunc-level profiling instrumentation.
    ///
    /// Wraps [`AotContractExecutor::run`] with the bookkeeping the
    /// `with-libfunc-profiling` runtime needs:
    ///
    /// 1. Allocates a unique trace ID and inserts an empty `ProfilerImpl` slot in
    ///    [`LIBFUNC_PROFILE`].
    /// 2. Points the executor's `cairo_native__profiler__profile_id` symbol at the new
    ///    trace ID, saving the previous value.
    /// 3. Calls `run`. Per-statement samples accumulate in the slot via the runtime
    ///    `push_stmt` callback.
    /// 4. Drains the slot, calls [`ProfilerImpl::get_profile`] with `program`, and hands
    ///    the resulting [`Profile`] to `on_profile`.
    /// 5. A [`ProfilerGuard`] restores the previous trace ID — and removes the slot if
    ///    the success path didn't — on both success and unwind paths.
    ///
    /// `program` must be the Sierra program this executor was compiled from; it's used
    /// by `get_profile` to map runtime libfunc IDs back to declarations.
    ///
    /// Profiling is intended to run single-threaded; concurrent calls would race on the
    /// global `trace_id` symbol.
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
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);

        LIBFUNC_PROFILE
            .lock()
            .unwrap()
            .insert(counter, ProfilerImpl::new());

        // The pointer targets a global symbol in the executor's shared library; it lives
        // for the executor's lifetime. Single-threaded profiling means no concurrent writer.
        let trace_id_ptr = self
            .find_symbol_ptr(ProfilerBinding::ProfileId.symbol())
            .unwrap()
            .cast::<u64>();
        // SAFETY: see above. Read/write to a non-null, properly-aligned `*mut u64`.
        let old_trace_id = unsafe { *trace_id_ptr };
        unsafe {
            *trace_id_ptr = counter;
        }

        // Restore on the success path AND on unwind. On success the caller drains the
        // slot below; the guard's `remove` is then a no-op.
        let _guard = ProfilerGuard {
            trace_id_ptr,
            old_trace_id,
            counter,
        };

        let result = self.run(selector, args, gas, builtin_costs, syscall_handler);

        let profiler = LIBFUNC_PROFILE.lock().unwrap().remove(&counter).unwrap();
        on_profile(profiler.get_profile(program));

        result
    }
}

/// RAII cleanup for the profiler globals. Restores `*trace_id_ptr` and drops the
/// `LIBFUNC_PROFILE` slot at `counter` if it's still occupied.
struct ProfilerGuard {
    trace_id_ptr: *mut u64,
    old_trace_id: u64,
    counter: u64,
}

impl Drop for ProfilerGuard {
    fn drop(&mut self) {
        // SAFETY: same provenance as the construction site; single-threaded use.
        unsafe {
            *self.trace_id_ptr = self.old_trace_id;
        }
        // Tolerate a poisoned mutex silently — Drop must not panic.
        if let Ok(mut profile) = LIBFUNC_PROFILE.lock() {
            profile.remove(&self.counter);
        }
    }
}
