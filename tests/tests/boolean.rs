use crate::common::{compare_outputs, felt_to_arg, run_native_program, run_vm_program, DEFAULT_GAS};
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::{starknet::DummySyscallHandler, Value};
use proptest::prelude::*;
use starknet_types_core::felt::Felt;

#[test]
fn felt252_to_bool_bug() {
    let program = &load_program_and_runner("test_data_artifacts/programs/felt252_to_bool");
    let a = true;
    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![felt_to_arg(Felt::from(a))],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[Value::Felt252(a.into())],
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

    let a = false;
    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![felt_to_arg(Felt::from(a))],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[Value::Felt252(a.into())],
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
    fn bool_to_felt252_proptest(a: bool) {
        let program = &load_program_and_runner("test_data_artifacts/programs/bool_to_felt252");
        let result_vm = run_vm_program(program, "run_test", vec![
            felt_to_arg(Felt::from(a)),
        ], Some(DEFAULT_GAS as usize)).unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Felt252(a.into())],
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

    #[test]
    fn bool_not_proptest(a: bool) {
        let program = &load_program_and_runner("test_data_artifacts/programs/bool_not");
        let result_vm = run_vm_program(program, "run_test", vec![
            felt_to_arg(Felt::from(a)),
        ], Some(DEFAULT_GAS as usize)).unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Felt252(a.into())],
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

    #[test]
    fn bool_and_proptest(a: bool, b: bool) {
        let program = &load_program_and_runner("test_data_artifacts/programs/bool_and");
        let result_vm = run_vm_program(program, "run_test", vec![
            felt_to_arg(Felt::from(a)),
            felt_to_arg(Felt::from(b))
        ], Some(DEFAULT_GAS as usize)).unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Felt252(a.into()), Value::Felt252(b.into())],
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

    #[test]
    fn bool_or_proptest(a: bool, b: bool) {
        let program = &load_program_and_runner("test_data_artifacts/programs/bool_or");
        let result_vm = run_vm_program(program, "run_test", vec![
            felt_to_arg(Felt::from(a)),
            felt_to_arg(Felt::from(b))
        ], Some(DEFAULT_GAS as usize)).unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Felt252(a.into()), Value::Felt252(b.into())],
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

    #[test]
    fn bool_xor_proptest(a: bool, b: bool) {
        let program = &load_program_and_runner("test_data_artifacts/programs/bool_xor");
        let result_vm = run_vm_program(program, "run_test", vec![
            felt_to_arg(Felt::from(a)),
            felt_to_arg(Felt::from(b))
        ], Some(DEFAULT_GAS as usize)).unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Felt252(a.into()), Value::Felt252(b.into())],
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
