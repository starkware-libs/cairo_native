
use core::circuit::{
    AddInputResultTrait, CircuitElement, CircuitInput, CircuitInputs, CircuitModulus,
    EvalCircuitTrait, circuit_inverse, u96,
    CircuitFailureGuarantee, NextU96LessThanGuarantee, U96LimbsLtGuarantee,
    circuit_failure_guarantee_verify, u96_guarantee_verify,
    u96_limbs_less_than_guarantee_verify, u96_single_limb_less_than_guarantee_verify,
};

fn walk_length(guarantee: CircuitFailureGuarantee) -> felt252 {
    let g4: U96LimbsLtGuarantee<4> = circuit_failure_guarantee_verify(guarantee, 0, 1);
    let g3 = match u96_limbs_less_than_guarantee_verify(g4) {
        NextU96LessThanGuarantee::Next(next) => next,
        NextU96LessThanGuarantee::Final(guarantee) => {
            u96_guarantee_verify(guarantee);
            return 1;
        },
    };
    let g2 = match u96_limbs_less_than_guarantee_verify(g3) {
        NextU96LessThanGuarantee::Next(next) => next,
        NextU96LessThanGuarantee::Final(guarantee) => {
            u96_guarantee_verify(guarantee);
            return 2;
        },
    };
    let g1 = match u96_limbs_less_than_guarantee_verify(g2) {
        NextU96LessThanGuarantee::Next(next) => next,
        NextU96LessThanGuarantee::Final(guarantee) => {
            u96_guarantee_verify(guarantee);
            return 3;
        },
    };
    u96_guarantee_verify(u96_single_limb_less_than_guarantee_verify(g1));
    4
}

fn eval_fail_walk_length(input: [u96; 4], modulus: [u96; 4]) -> felt252 {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let inv = circuit_inverse(in1);

    let modulus = TryInto::<_, CircuitModulus>::try_into(modulus).unwrap();

    match (inv,).new_inputs().next(input).done().eval(modulus) {
        Ok(_) => panic!("expected the evaluation to fail"),
        Err((_partial, guarantee)) => walk_length(guarantee),
    }
}

// The secp256k1 prime: its most significant limb is 0 like the nullifier's
// (1), and they differ at the next limb.
fn walk_secp256k1() -> felt252 {
    eval_fail_walk_length(
        [0, 0, 0, 0],
        [0xfffffffffffffffefffffc2f, 0xffffffffffffffffffffffff, 0xffffffffffffffff, 0],
    )
}

// The nullifier (1) and the modulus only differ at the least significant
// limb: the walk visits every limb pair.
fn walk_full() -> felt252 {
    eval_fail_walk_length([0, 0, 0, 0], [7, 0, 0, 0])
}
