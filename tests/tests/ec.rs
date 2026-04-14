use crate::common::{
    any_felt, compare_outputs, felt_to_arg, run_native_program, run_vm_program, DEFAULT_GAS,
};
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::{starknet::DummySyscallHandler, Value};
use num_bigint::BigUint;
use proptest::prelude::*;
use starknet_types_core::felt::Felt;
use std::str::FromStr;

#[test]
fn ec_point_zero() {
    let program = &load_program_and_runner("test_data_artifacts/programs/ec_point_zero");
    let result_vm =
        run_vm_program(program, "run_test", vec![], Some(DEFAULT_GAS as usize)).unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[],
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
fn ec_point_from_x_big() {
    let x = Felt::from(
        BigUint::from_str(
            "10503791839462130483045092717244804953879649418761481950933471772092536173",
        )
        .unwrap(),
    );
    let program = &load_program_and_runner("test_data_artifacts/programs/ec_point_from_x");
    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![felt_to_arg(x)],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[Value::Felt252(Felt::from_bytes_be(&x.to_bytes_be()))],
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
fn ec_point_from_x_small() {
    let x = Felt::from(BigUint::from_str("1234").unwrap());
    let program = &load_program_and_runner("test_data_artifacts/programs/ec_point_from_x");
    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![felt_to_arg(x)],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[Value::Felt252(Felt::from_bytes_be(&x.to_bytes_be()))],
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
    fn ec_point_try_new_proptest(a in any_felt(), b in any_felt()) {
        let program = &load_program_and_runner("test_data_artifacts/programs/ec_point_try_new");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(Felt::from_bytes_be(&a.clone().to_bytes_be())), felt_to_arg(Felt::from_bytes_be(&b.clone().to_bytes_be()))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Felt252(a), Value::Felt252(b)],
            Some(DEFAULT_GAS),
            Option::<DummySyscallHandler>::None,
        );

        compare_outputs(
            &program.1,
            &program.2.find_function("run_test").unwrap().id,
            &result_vm,
            &result_native,
        )?;
    }

    #[test]
    fn ec_point_from_x_proptest(a in any_felt()) {
    let program = &load_program_and_runner("test_data_artifacts/programs/ec_point_from_x");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(Felt::from_bytes_be(&a.clone().to_bytes_be()))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Felt252(a)],
            Some(DEFAULT_GAS),
            Option::<DummySyscallHandler>::None,
        );

        compare_outputs(
            &program.1,
            &program.2.find_function("run_test").unwrap().id,
            &result_vm,
            &result_native,
        )?;
    }
}
