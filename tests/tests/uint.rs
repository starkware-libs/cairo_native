use crate::common::{compare_outputs, felt_to_arg, run_native_program, run_vm_program, DEFAULT_GAS};
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::{starknet::DummySyscallHandler, Value};
use proptest::prelude::*;

proptest! {
    #[test]
    fn u8_overflowing_add_proptest(a in 0..u8::MAX, b in 0..u8::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u8_overflowing_add");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u8_overflowing_sub_proptest(a in 0..u8::MAX, b in 0..u8::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u8_overflowing_sub");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u8_safe_divmod_proptest(a in 0..u8::MAX, b in 0..u8::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u8_safe_divmod");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u8_equal_proptest(a in 0..u8::MAX, b in 0..u8::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u8_equal");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u8_is_zero_proptest(a in 0..u8::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u8_is_zero");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    #[test]
    fn u8_sqrt_proptest(a in 0..u8::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u8_sqrt");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    // u16

    #[test]
    fn u16_overflowing_add_proptest(a in 0..u16::MAX, b in 0..u16::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u16_overflowing_add");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u16_overflowing_sub_proptest(a in 0..u16::MAX, b in 0..u16::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u16_overflowing_sub");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u16_safe_divmod_proptest(a in 0..u16::MAX, b in 0..u16::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u16_safe_divmod");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u16_equal_proptest(a in 0..u16::MAX, b in 0..u16::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u16_equal");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u16_is_zero_proptest(a in 0..u16::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u16_is_zero");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    #[test]
    fn u16_sqrt_proptest(a in 0..u16::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u16_sqrt");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    // u32

    #[test]
    fn u32_overflowing_add_proptest(a in 0..u32::MAX, b in 0..u32::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u32_overflowing_add");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u32_overflowing_sub_proptest(a in 0..u32::MAX, b in 0..u32::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u32_overflowing_sub");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u32_safe_divmod_proptest(a in 0..u32::MAX, b in 0..u32::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u32_safe_divmod");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u32_equal_proptest(a in 0..u32::MAX, b in 0..u32::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u32_equal");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u32_is_zero_proptest(a in 0..u32::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u32_is_zero");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    #[test]
    fn u32_sqrt_proptest(a in 0..u32::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u32_sqrt");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    // u64

    #[test]
    fn u64_overflowing_add_proptest(a in 0..u64::MAX, b in 0..u64::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u64_overflowing_add");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u64_overflowing_sub_proptest(a in 0..u64::MAX, b in 0..u64::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u64_overflowing_sub");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u64_safe_divmod_proptest(a in 0..u64::MAX, b in 0..u64::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u64_safe_divmod");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u64_equal_proptest(a in 0..u64::MAX, b in 0..u64::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u64_equal");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u64_is_zero_proptest(a in 0..u64::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u64_is_zero");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    #[test]
    fn u64_sqrt_proptest(a in 0..u64::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u64_sqrt");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    // u128

    #[test]
    fn u128_overflowing_add_proptest(a in 0..u128::MAX, b in 0..u128::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u128_overflowing_add");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u128_overflowing_sub_proptest(a in 0..u128::MAX, b in 0..u128::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u128_overflowing_sub");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u128_safe_divmod_proptest(a in 0..u128::MAX, b in 0..u128::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u128_safe_divmod");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u128_equal_proptest(a in 0..u128::MAX, b in 0..u128::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u128_equal");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into()), felt_to_arg(b.into())],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
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
        )?;
    }

    #[test]
    fn u128_is_zero_proptest(a in 0..u128::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u128_is_zero");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }

    #[test]
    fn u128_sqrt_proptest(a in 0..u128::MAX) {
        let program = &load_program_and_runner("test_data_artifacts/programs/u128_sqrt");
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![felt_to_arg(a.into())],
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
        )?;
    }
}
