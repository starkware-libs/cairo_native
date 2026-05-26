use crate::common::{compare_outputs, DEFAULT_GAS};
use crate::common::{run_native_program, run_vm_program};
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::Value;

#[test]
fn test_circuit_guarantee_first_limb() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_guarantee_first_limb",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_guarantee_first_limb",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    assert!(matches!(
        result_native.return_value,
        Value::Enum { tag: 0, .. }
    ));

    compare_outputs(
        &program.1,
        &program
            .2
            .find_function("test_guarantee_first_limb")
            .unwrap()
            .id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_guarantee_last_limb() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_guarantee_last_limb",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_guarantee_last_limb",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    assert!(matches!(
        result_native.return_value,
        Value::Enum { tag: 0, .. }
    ));

    compare_outputs(
        &program.1,
        &program
            .2
            .find_function("test_guarantee_last_limb")
            .unwrap()
            .id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_guarantee_middle_limb() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_guarantee_middle_limb",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_guarantee_middle_limb",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    assert!(matches!(
        result_native.return_value,
        Value::Enum { tag: 0, .. }
    ));

    compare_outputs(
        &program.1,
        &program
            .2
            .find_function("test_guarantee_middle_limb")
            .unwrap()
            .id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_add() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_circuit_add",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_circuit_add",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("test_circuit_add").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_sub() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_circuit_sub",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_circuit_sub",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("test_circuit_sub").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_mul() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_circuit_mul",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_circuit_mul",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("test_circuit_mul").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_inv() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_circuit_inv",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_circuit_inv",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("test_circuit_inv").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_full() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_circuit_full",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_circuit_full",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("test_circuit_full").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_fail() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_circuit_fail",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_circuit_fail",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("test_circuit_fail").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_into_u96_guarantee() {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm = run_vm_program(
        program,
        "test_into_u96_guarantee",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "test_into_u96_guarantee",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program
            .2
            .find_function("test_into_u96_guarantee")
            .unwrap()
            .id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_y_inv_x_neg_over_y_bn254() {
    let program = &load_program_and_runner("programs/garaga_circuits");

    let result_vm = run_vm_program(
        program,
        "compute_yInvXnegOverY_BN254",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "compute_yInvXnegOverY_BN254",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program
            .2
            .find_function("compute_yInvXnegOverY_BN254")
            .unwrap()
            .id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_batch_3_mod_bn254() {
    let program = &load_program_and_runner("programs/garaga_circuits");

    let result_vm = run_vm_program(
        program,
        "batch_3_mod_bn254",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "batch_3_mod_bn254",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("batch_3_mod_bn254").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_add_ec_points_g2() {
    let program = &load_program_and_runner("programs/garaga_circuits");

    let result_vm = run_vm_program(
        program,
        "run_ADD_EC_POINTS_G2_circuit",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "run_ADD_EC_POINTS_G2_circuit",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program
            .2
            .find_function("run_ADD_EC_POINTS_G2_circuit")
            .unwrap()
            .id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

// NOTE: Since Cairo 2.14.0-dev.1, the BIG_CIRCUIT program takes forever to
// compile to Sierra. Enable this test once fixed.
#[test]
#[ignore]
fn test_circuit_clear_cofactor_bls12_381() {
    let program = &load_program_and_runner("programs/big_circuit");

    let result_vm = run_vm_program(
        program,
        "run_CLEAR_COFACTOR_BLS12_381_circuit",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "run_CLEAR_COFACTOR_BLS12_381_circuit",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program
            .2
            .find_function("run_CLEAR_COFACTOR_BLS12_381_circuit")
            .unwrap()
            .id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn test_circuit_add_ec_point_unchecked() {
    let program = &load_program_and_runner("programs/kakarot_circuit");

    let result_vm = run_vm_program(
        program,
        "add_ec_point_unchecked",
        vec![],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "add_ec_point_unchecked",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program
            .2
            .find_function("add_ec_point_unchecked")
            .unwrap()
            .id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}
