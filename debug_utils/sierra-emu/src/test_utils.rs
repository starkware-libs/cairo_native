#![cfg(test)]

use cairo_lang_sierra::program::{Program, VersionedProgram};
use std::{fs, sync::Arc};

use crate::{starknet::StubSyscallHandler, Value, VirtualMachine};

pub fn run_test_program(sierra_program: Program) -> Vec<Value> {
    let function = sierra_program
        .funcs
        .iter()
        .find(|f| {
            f.id.debug_name
                .as_ref()
                .map(|name| name.as_str().contains("main"))
                .unwrap_or_default()
        })
        .unwrap();

    let mut vm = VirtualMachine::new(Arc::new(sierra_program.clone()));

    let initial_gas = 1000000;

    let args: &[Value] = &[];
    vm.call_program(function, initial_gas, args.iter().cloned());

    let syscall_handler = &mut StubSyscallHandler::default();
    let trace = vm.run_with_trace(syscall_handler);

    trace
        .states
        .last()
        .unwrap()
        .items
        .values()
        .cloned()
        .collect()
}

pub fn load_program(path: &str) -> Program {
    let program_path = format!(
        "{}/../../test_data_artifacts/{path}.sierra.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let versioned_program =
        serde_json::from_str::<VersionedProgram>(&fs::read_to_string(program_path).unwrap())
            .unwrap();
    versioned_program.into_v1().unwrap().program
}
