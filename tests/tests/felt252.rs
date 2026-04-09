use crate::common::{
    any_felt, compare_outputs, felt_to_arg, nonzero_felt, run_native_program, run_vm_program,
    DEFAULT_GAS,
};
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::{starknet::DummySyscallHandler, Value};
use proptest::prelude::*;
use starknet_types_core::felt::Felt;

proptest! {
    #[test]
    fn felt_add_proptest(a in any_felt(), b in any_felt()) {
        let program = &load_program_and_runner("test_data_artifacts/programs/felt252_add");
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
    fn felt_sub_proptest(a in any_felt(), b in any_felt()) {
        let program = &load_program_and_runner("test_data_artifacts/programs/felt252_sub");
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
    fn felt_mul_proptest(a in any_felt(), b in any_felt()) {
        let program = &load_program_and_runner("test_data_artifacts/programs/felt252_mul");
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
    fn felt_div_proptest(a in any_felt(), b in nonzero_felt()) {
        let program = &load_program_and_runner("test_data_artifacts/programs/felt252_div");
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
}
