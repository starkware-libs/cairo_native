use crate::common::{any_felt, compare_outputs, run_native_program, run_vm_program, DEFAULT_GAS};
use cairo_lang_runner::Arg;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::{starknet::DummySyscallHandler, Value};
use proptest::prelude::*;
use starknet_types_core::felt::Felt;

proptest! {
    #[test]
    fn dict_get_insert_proptest(a in any_felt(), b in any_felt()) {
        let program = &load_program_and_runner("programs/dict_get_insert");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![Arg::Value(Felt::from_bytes_be(&a.clone().to_bytes_be())), Arg::Value(Felt::from_bytes_be(&b.clone().to_bytes_be()))],
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
}

#[test]
fn dict_snapshot_loop() {
    let program = &load_program_and_runner("programs/snapshot_loop");
    run_native_program(
        program,
        "run_test",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );
}
