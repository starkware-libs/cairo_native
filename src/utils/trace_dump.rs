#![cfg(feature = "with-trace-dump")]
//! The trace dump feature is used to generate the execution trace of a sierra program.
//!
//! Take, for example, the following sierra code:
//!
//! ```sierra
//! const_as_immediate<Const<felt252, 10>>() -> ([0]);
//! const_as_immediate<Const<felt252, 20>>() -> ([1]);
//! store_temp<felt252>([0]) -> ([0]);
//! felt252_add([0], [1]) -> ([2]);
//! store_temp<felt252>([2]) -> ([2]);
//! return([2]);
//! ```
//!
//! The compiler will call `build_state_snapshot` right before each statement.
//! Iterating every variable on the current scope and saving its value on a global
//! static variable.
//!
//! At the end of the execution, the full trace dump can be retrieved, which
//! looks something like this:
//!
//! ```json
//! {
//!   "states": [
//!     {
//!       "statementIdx": 0,
//!       "preStateDump": {}
//!     },
//!     {
//!       "statementIdx": 1,
//!       "preStateDump": {
//!         "0": { "Felt": "0xa" }
//!       }
//!     },
//!     {
//!       "statementIdx": 2,
//!       "preStateDump": {
//!         "0": { "Felt": "0xa" },
//!         "1": { "Felt": "0x14" }
//!       }
//!     },
//!     ...
//!   ]
//! }
//! ```
//!
//! To support this feature even on the context of starknet contracts, then we
//! must support building a trace dump for multiple programs at the same time, as
//! starknet contracts can themselves call another contracts. To achieve this, we
//! need two important elements.
//!
//! 1. The global static variable must me able to store multiple trace dumps at the
//!    same time. We have a global static hashmap from trace id to trace dump content.
//!    See `TRACE_DUMP`.
//!
//! 2. We must store somewhere the ID of the current trace dump, and update it
//!    acordingly when switching between contract executors
//!
//! Both these elements must be properly setup before running the executor. See
//! `cairo-native-run` for an example on how to do it. You can also check on
//! this file's integration tests.
//!
//! When executing starknet contracts, the trace id must be set right
//! before each execution, restoring the previous trace id after the execution.

use std::collections::HashMap;

use cairo_lang_sierra::{
    extensions::core::{CoreLibfunc, CoreType},
    ids::{ConcreteTypeId, VarId},
    program::StatementIdx,
    program_registry::ProgramRegistry,
};
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use melior::{
    helpers::LlvmBlockExt,
    ir::{BlockRef, Location, Module, Value, ValueLike},
    Context,
};

use crate::{
    metadata::{trace_dump::TraceDumpMeta, MetadataStorage},
    types::TypeBuilder,
};

#[allow(clippy::too_many_arguments)]
pub fn build_state_snapshot(
    context: &Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    module: &Module,
    block: &BlockRef,
    location: Location,
    metadata: &mut MetadataStorage,
    statement_idx: StatementIdx,
    state: &OrderedHashMap<VarId, Value>,
    var_types: &HashMap<VarId, ConcreteTypeId>,
) {
    let trace_dump = metadata.get_or_insert_with(TraceDumpMeta::default);

    for (var_id, value) in state.iter() {
        let value_type_id = var_types.get(var_id).unwrap();
        let value_type = registry.get_type(value_type_id).unwrap();

        let layout = value_type.layout(registry).unwrap();

        let value_ptr = block
            .alloca1(context, location, value.r#type(), layout.align())
            .unwrap();
        block.store(context, location, value_ptr, *value).unwrap();

        trace_dump
            .build_state(
                context,
                module,
                block,
                var_id,
                value_type_id,
                value_ptr,
                location,
            )
            .unwrap();
    }

    trace_dump
        .build_push(context, module, block, statement_idx, location)
        .unwrap();
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cairo_lang_sierra::{program::Program, program_registry::ProgramRegistry};
    use pretty_assertions_sorted::assert_eq_sorted;
    use rstest::{fixture, rstest};
    use sierra_emu::{starknet::StubSyscallHandler, VirtualMachine};

    use crate::{
        context::NativeContext,
        executor::AotNativeExecutor,
        metadata::trace_dump::{
            trace_dump_runtime::{TraceDump, TRACE_DUMP},
            TraceBinding,
        },
        utils::testing::load_program,
        OptLevel,
    };

    #[fixture]
    fn program() -> Program {
        load_program("programs/fib_program")
    }

    #[rstest]
    fn test_program(program: Program) {
        let entrypoint_function = &program
            .funcs
            .iter()
            .find(|x| {
                x.id.debug_name
                    .as_ref()
                    .map(|x| x.contains("main"))
                    .unwrap_or_default()
            })
            .unwrap()
            .clone();

        let native_context = NativeContext::new();
        let module = native_context
            .compile(&program, false, Some(Default::default()), None)
            .expect("failed to compile context");
        let executor = AotNativeExecutor::from_native_module(module, OptLevel::default()).unwrap();

        if let Some(trace_id) = executor.find_symbol_ptr(TraceBinding::TraceId.symbol()) {
            let trace_id = trace_id.cast::<u64>();
            unsafe { *trace_id = 0 };
        }

        TRACE_DUMP
            .lock()
            .unwrap()
            .insert(0, TraceDump::new(ProgramRegistry::new(&program).unwrap()));

        executor
            .invoke_dynamic(&entrypoint_function.id, &[], Some(u64::MAX))
            .unwrap();

        let native_trace = TRACE_DUMP
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .trace
            .clone();

        let mut vm = VirtualMachine::new(Arc::new(program));

        let initial_gas = u64::MAX;
        let args = [];
        vm.call_program(entrypoint_function, initial_gas, args.into_iter());

        let syscall_handler = &mut StubSyscallHandler::default();
        let emu_trace = vm.run_with_trace(syscall_handler);

        assert_eq_sorted!(emu_trace, native_trace);
    }
}
