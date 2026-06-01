use crate::common::{compare_outputs, run_native_program, run_vm_program, DEFAULT_GAS};
use cairo_lang_runner::Arg;
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::Value;
use starknet_types_core::felt::Felt;

#[test]
fn felt252_to_i8_downcast_neg_one_matches_vm() {
    let program = &load_program_and_runner("programs/cast_downcast_felt_neg");
    let neg_one = Felt::from(-1);

    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![Arg::Value(neg_one)],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[Value::Felt252(neg_one)],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("run_test").unwrap().id,
        &result_vm,
        &result_native,
    )
    .expect("felt252 -> i8 downcast of -1 must agree between VM and native");
}

#[test]
fn nonneg_to_i8_downcast_255_is_none_and_matches_vm() {
    let program = &load_program_and_runner("programs/cast_downcast_nonneg_i8");

    for value in [255u16, 100u16] {
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![Arg::Value(Felt::from(value))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Uint16(value)],
            Some(DEFAULT_GAS),
            Option::<DummySyscallHandler>::None,
        );

        compare_outputs(
            &program.1,
            &program.2.find_function("run_test").unwrap().id,
            &result_vm,
            &result_native,
        )
        .unwrap_or_else(|e| {
            panic!("u16 -> i8 downcast of {value} diverges between VM and native: {e:?}")
        });
    }
}
