use crate::common::{
    any_felt, compare_outputs, felt_to_arg, run_native_program, run_vm_program, DEFAULT_GAS,
};
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::felt252_str;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::Value;
use proptest::prelude::*;
use starknet_types_core::felt::Felt;

#[test]
fn fib() {
    let program = load_program_and_runner("test_data_artifacts/programs/fibonacci");
    let result_vm = run_vm_program(
        &program,
        "fibonacci",
        vec![felt_to_arg(Felt::from(10))],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        &program,
        "fibonacci",
        &[Value::Felt252(10.into())],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("fibonacci").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn logistic_map() {
    let program = load_program_and_runner("test_data_artifacts/programs/logistic_map");
    let result_vm = run_vm_program(
        &program,
        "run_test",
        vec![felt_to_arg(Felt::from(1000))],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        &program,
        "run_test",
        &[Value::Felt252(1000.into())],
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
fn pedersen() {
    let program = load_program_and_runner("test_data_artifacts/programs/pedersen");
    let result_vm = run_vm_program(
        &program,
        "run_test",
        vec![
            felt_to_arg(
                Felt::from_dec_str(
                    "2163739901324492107409690946633517860331020929182861814098856895601180685",
                )
                .unwrap(),
            ),
            felt_to_arg(
                Felt::from_dec_str(
                    "2392090257937917229310563411601744459500735555884672871108624696010915493156",
                )
                .unwrap(),
            ),
        ],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        &program,
        "run_test",
        &[
            Value::Felt252(felt252_str(
                "2163739901324492107409690946633517860331020929182861814098856895601180685",
            )),
            Value::Felt252(felt252_str(
                "2392090257937917229310563411601744459500735555884672871108624696010915493156",
            )),
        ],
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
fn factorial() {
    let program = load_program_and_runner("test_data_artifacts/programs/factorial");
    let result_vm = run_vm_program(
        &program,
        "run_test",
        vec![felt_to_arg(Felt::from(13))],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        &program,
        "run_test",
        &[Value::Felt252(13.into())],
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
    fn fib_proptest(n in 0..100i32) {
        let program = load_program_and_runner("test_data_artifacts/programs/fib");
        let result_vm = run_vm_program(
            &program,
            "run_test",
            vec![felt_to_arg(Felt::from(n))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
        let result_native = run_native_program(
            &program,
            "run_test",
            &[Value::Felt252(n.into())],
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
    fn logistic_map_proptest(n in 100..110i32) {
        let program = load_program_and_runner("test_data_artifacts/programs/logistic_map");
        let result_vm = run_vm_program(
            &program,
            "run_test",
            vec![felt_to_arg(Felt::from(n))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
        let result_native = run_native_program(
            &program,
            "run_test",
            &[Value::Felt252(n.into())],
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
    fn factorial_proptest(n in 1..100i32) {
        let program = load_program_and_runner("test_data_artifacts/programs/factorial");
        let result_vm = run_vm_program(
            &program,
            "run_test",
            vec![felt_to_arg(Felt::from(n))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();
        let result_native = run_native_program(
            &program,
            "run_test",
            &[Value::Felt252(n.into())],
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
    fn pedersen_proptest(a in any_felt(), b in any_felt()) {
        let program = load_program_and_runner("test_data_artifacts/programs/pedersen");
        let result_vm = run_vm_program(
            &program,
            "run_test",
            vec![felt_to_arg(Felt::from_bytes_be(&a.clone().to_bytes_be())), felt_to_arg(Felt::from_bytes_be(&b.clone().to_bytes_be()))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();

        let result_native = run_native_program(
            &program,
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
    fn poseidon_proptest(a in any_felt(), b in any_felt(), c in any_felt()) {
        let program = load_program_and_runner("test_data_artifacts/programs/poseidon");
        let result_vm = run_vm_program(
            &program,
            "run_test",
            vec![felt_to_arg(Felt::from_bytes_be(&a.clone().to_bytes_be())),
             felt_to_arg(Felt::from_bytes_be(&b.clone().to_bytes_be())),
            felt_to_arg(Felt::from_bytes_be(&c.clone().to_bytes_be()))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();

        let result_native = run_native_program(
            &program,
            "run_test",
            &[Value::Felt252(a), Value::Felt252(b), Value::Felt252(c)],
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
fn self_referencing_struct() {
    let program = load_program_and_runner("test_data_artifacts/programs/self_referencing");
    let result_vm =
        run_vm_program(&program, "run_test", vec![], Some(DEFAULT_GAS as usize)).unwrap();
    let result_native = run_native_program(
        &program,
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
fn no_op() {
    let program = load_program_and_runner("test_data_artifacts/programs/no_op");
    let result_vm =
        run_vm_program(&program, "run_test", vec![], Some(DEFAULT_GAS as usize)).unwrap();
    let result_native = run_native_program(
        &program,
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
