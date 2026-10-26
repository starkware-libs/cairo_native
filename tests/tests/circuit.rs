use crate::common::{compare_outputs, get_run_result, load_cairo, DEFAULT_GAS};
use crate::common::{run_native_program, run_vm_program};
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::Value;
use starknet_types_core::felt::Felt;
use test_case::test_case;

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

/// Checks that the failure guarantee's limbs match the VM, via the length of its destruct walk.
#[test]
fn test_circuit_fail_inverse_guarantee_walk_length() {
    let program = &load_program_and_runner("programs/circuit_failure_guarantee");

    for (entry_point, expected_length) in [("walk_secp256k1", 2), ("walk_full", 4)] {
        let result_vm =
            run_vm_program(program, entry_point, vec![], Some(DEFAULT_GAS as usize)).unwrap();

        let result_native = run_native_program(
            program,
            entry_point,
            &[],
            Some(DEFAULT_GAS),
            Option::<DummySyscallHandler>::None,
        );

        assert_eq!(
            get_run_result(&result_vm.value),
            vec![Felt::from(expected_length).to_string()],
            "unexpected VM walk length for {entry_point}",
        );

        compare_outputs(
            &program.1,
            &program.2.find_function(entry_point).unwrap().id,
            &result_vm,
            &result_native,
        )
        .unwrap_or_else(|e| panic!("VM/native mismatch for {entry_point}: {e:?}"));
    }
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

#[test_case("test_guarantee_v2_last_limb")]
#[test_case("test_guarantee_v2_first_limb")]
fn test_circuit_guarantee_verify_v2(entry_point: &str) {
    let program = &load_program_and_runner("programs/circuit");

    let result_vm =
        run_vm_program(program, entry_point, vec![], Some(DEFAULT_GAS as usize)).unwrap();

    let result_native = run_native_program(
        program,
        entry_point,
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
        &program.2.find_function(entry_point).unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

/// `U96LimbsLtGuarantee<N>` is `2 * N` u96 limbs wide, but it used to be declared as a ZST
/// with a zero-sized layout. Boxing it then allocated zero bytes in the arena and the store
/// clobbered the previous allocation, which right after `eval_circuit` is the outputs array.
/// Boxing the first output's guarantee before reading the second output exposes this.
#[test]
fn test_circuit_guarantee_in_box_keeps_outputs() {
    let program = load_cairo! {
        use core::circuit::{
            CircuitElement, CircuitInput, CircuitOutputs, circuit_add, circuit_mul,
            EvalCircuitTrait, u384, CircuitModulus, AddInputResultTrait, CircuitInputs,
            get_circuit_output, U96LimbsLtGuarantee,
        };

        fn get_out<C, Output>(
            outputs: CircuitOutputs<C>, _o: CircuitElement<Output>,
        ) -> (u384, U96LimbsLtGuarantee<4>) nopanic {
            get_circuit_output::<C, Output>(outputs)
        }

        #[inline(never)]
        fn boxit(g: U96LimbsLtGuarantee<4>) -> Box<U96LimbsLtGuarantee<4>> {
            BoxTrait::new(g)
        }

        fn main() -> (u384, u384) {
            let in1 = CircuitElement::<CircuitInput<0>> {};
            let in2 = CircuitElement::<CircuitInput<1>> {};
            let add = circuit_add(in1, in2);
            let mul = circuit_mul(in1, in2);
            let modulus = TryInto::<_, CircuitModulus>::try_into([7, 0, 0, 0]).unwrap();
            let outputs = (add, mul)
                .new_inputs()
                .next([3, 0, 0, 0])
                .next([6, 0, 0, 0])
                .done()
                .eval(modulus)
                .unwrap();
            let (v1, g1) = get_out(outputs, add);
            let b = boxit(g1);
            let (v2, _g2) = get_out(outputs, mul);
            let _g1 = b.unbox();
            (v1, v2)
        }
    };

    let result_vm = run_vm_program(&program, "main", vec![], Some(DEFAULT_GAS as usize)).unwrap();
    let result_native = run_native_program(
        &program,
        "main",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("main").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}
