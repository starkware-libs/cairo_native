use crate::common::{any_felt, felt_to_arg, run_native_program, run_vm_program};
use crate::common::{compare_outputs, DEFAULT_GAS};
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::Value;
use proptest::prelude::*;
use starknet_types_core::felt::Felt;

#[test]
fn array_get_test() {
    let program = &load_program_and_runner("test_data_artifacts/programs/array_get");
    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![felt_to_arg(Felt::from(10)), felt_to_arg(Felt::from(5))],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[Value::Felt252(10.into()), Value::Felt252(5.into())],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("run_test").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

proptest! {
    #[test]
    fn array_get_test_proptest(value in any_felt(), idx in 0u32..26) {
        let program = &load_program_and_runner("test_data_artifacts/programs/array_get");
        let result_vm = run_vm_program(program, "run_test", vec![
            felt_to_arg(Felt::from_bytes_be(&value.to_bytes_be())),
            felt_to_arg(Felt::from(idx))
        ], Some(DEFAULT_GAS as usize)).unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Felt252(value), Value::Felt252(idx.into())],
            Some(DEFAULT_GAS),
            Option::<DummySyscallHandler>::None,
        );

        compare_outputs(
            &program.1,
            &program.2.find_function("run_test").unwrap().id,
            &result_vm,
            &result_native,
        )
        .unwrap();
    }
}
