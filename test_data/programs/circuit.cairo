use core::circuit::{
    CircuitData, CircuitElement, CircuitInput, circuit_add, circuit_sub, circuit_mul, circuit_inverse,
    EvalCircuitResult, EvalCircuitTrait, u384, CircuitOutputsTrait, CircuitModulus, into_u96_guarantee, U96Guarantee,
    CircuitInputs, AddInputResultTrait, AddInputResult, IntoCircuitInputValue, add_circuit_input,
    CircuitOutputs, U96LimbsLtGuarantee, u96, RangeCheck96,
};

#[feature("bounded-int-utils")]
use core::internal::bounded_int::BoundedInt;

fn test_guarantee_first_limb() {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let add = circuit_add(in1, in2);
    let inv = circuit_inverse(add);
    let sub = circuit_sub(inv, in2);
    let mul = circuit_mul(inv, sub);

    let modulus = TryInto::<_, CircuitModulus>::try_into([7, 0, 0, 0]).unwrap();
    let outputs = (mul,)
        .new_inputs()
        .next([3, 0, 0, 0])
        .next([6, 0, 0, 0])
        .done()
        .eval(modulus)
        .unwrap();

    assert!(outputs.get_output(mul) == u384 { limb0: 6, limb1: 0, limb2: 0, limb3: 0 });
}

fn test_guarantee_last_limb() {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let add = circuit_add(in1, in2);

    let modulus = TryInto::<_, CircuitModulus>::try_into([7, 0, 0, 1]).unwrap();
    let outputs = (add,)
        .new_inputs()
        .next([5, 0, 0, 0])
        .next([9, 0, 0, 0])
        .done()
        .eval(modulus)
        .unwrap();

    assert!(outputs.get_output(add) == u384 { limb0: 14, limb1: 0, limb2: 0, limb3: 0 });
}

fn test_guarantee_middle_limb() {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let add = circuit_add(in1, in2);

    let modulus = TryInto::<_, CircuitModulus>::try_into([7, 0, 1, 0]).unwrap();
    let outputs = (add,)
        .new_inputs()
        .next([5, 0, 0, 0])
        .next([9, 0, 0, 0])
        .done()
        .eval(modulus)
        .unwrap();

    assert!(outputs.get_output(add) == u384 { limb0: 14, limb1: 0, limb2: 0, limb3: 0 });
}

fn test_circuit_add() -> u384 {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let add = circuit_add(in1, in2);

    let modulus = TryInto::<_, CircuitModulus>::try_into([12, 12, 12, 12]).unwrap();

    let outputs = (add,)
        .new_inputs()
        .next([3, 3, 3, 3])
        .next([6, 6, 6, 6])
        .done()
        .eval(modulus)
        .unwrap();

    outputs.get_output(add)
}

fn test_circuit_sub() -> u384 {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let sub = circuit_sub(in1, in2);

    let modulus = TryInto::<_, CircuitModulus>::try_into([12, 12, 12, 12]).unwrap();

    let outputs = (sub,)
        .new_inputs()
        .next([6, 6, 6, 6])
        .next([3, 3, 3, 3])
        .done()
        .eval(modulus)
        .unwrap();

    outputs.get_output(sub)
}

fn test_circuit_mul() -> u384 {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let mul = circuit_mul(in1, in2);

    let modulus = TryInto::<_, CircuitModulus>::try_into([12, 12, 12, 12]).unwrap();

    let outputs = (mul,)
        .new_inputs()
        .next([3, 0, 0, 0])
        .next([3, 3, 3, 3])
        .done()
        .eval(modulus)
        .unwrap();

    outputs.get_output(mul)
}

fn test_circuit_inv() -> u384 {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let inv = circuit_inverse(in1);

    let modulus = TryInto::<_, CircuitModulus>::try_into([11, 0, 0, 0]).unwrap();

    let outputs = (inv,)
        .new_inputs()
        .next([2, 0, 0, 0])
        .done()
        .eval(modulus)
        .unwrap();

    outputs.get_output(inv)
}

fn test_circuit_full() -> u384 {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let add1 = circuit_add(in1, in2);
    let mul1 = circuit_mul(add1, in1);
    let mul2 = circuit_mul(mul1, add1);
    let inv1 = circuit_inverse(mul2);
    let sub1 = circuit_sub(inv1, in2);
    let sub2 = circuit_sub(sub1, mul2);
    let inv2 = circuit_inverse(sub2);
    let add2 = circuit_add(inv2, inv2);

    let modulus = TryInto::<_, CircuitModulus>::try_into([17, 14, 14, 14]).unwrap();

    let outputs = (add2,)
        .new_inputs()
        .next([9, 2, 9, 3])
        .next([5, 7, 0, 8])
        .done()
        .eval(modulus)
        .unwrap();

    outputs.get_output(add2)
}

fn test_circuit_fail() -> u384 {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let add = circuit_add(in1, in2);

    let modulus = TryInto::<_, CircuitModulus>::try_into([0, 0, 0, 0]).unwrap(); // Having this modulus makes eval panic

    let outputs = (add,)
        .new_inputs()
        .next([3, 3, 3, 3])
        .next([6, 6, 6, 6])
        .done()
        .eval(modulus)
        .unwrap();

    outputs.get_output(add)
}

fn test_into_u96_guarantee() -> (U96Guarantee, U96Guarantee, U96Guarantee) {
    (
        into_u96_guarantee::<BoundedInt<0, 79228162514264337593543950335>>(123),
        into_u96_guarantee::<BoundedInt<100, 1000>>(123),
        into_u96_guarantee::<u8>(123),
    )
}

extern fn get_circuit_output<C, Output>(
    outputs: CircuitOutputs<C>,
) -> (u384, U96LimbsLtGuarantee<4>) nopanic;

fn get_output_and_guarantee<C, Output>(
    outputs: CircuitOutputs<C>, _output: CircuitElement<Output>,
) -> (u384, U96LimbsLtGuarantee<4>) {
    get_circuit_output::<C, Output>(outputs)
}

/// Evaluates `5 + 9` modulo `modulus`, returning the output and its less-than-modulus guarantee.
fn eval_add_with_guarantee(modulus: [u96; 4]) -> (u384, U96LimbsLtGuarantee<4>) {
    let in1 = CircuitElement::<CircuitInput<0>> {};
    let in2 = CircuitElement::<CircuitInput<1>> {};
    let add = circuit_add(in1, in2);

    let modulus = TryInto::<_, CircuitModulus>::try_into(modulus).unwrap();
    let outputs = (add,)
        .new_inputs()
        .next([5, 0, 0, 0])
        .next([9, 0, 0, 0])
        .done()
        .eval(modulus)
        .unwrap();

    get_output_and_guarantee(outputs, add)
}

extern fn u96_limbs_less_than_guarantee_verify_v2<const LIMB_COUNT: usize>(
    guarantee: U96LimbsLtGuarantee<LIMB_COUNT>,
) implicits(RangeCheck96) nopanic;

fn test_guarantee_v2_last_limb() -> u384 {
    let (output, guarantee) = eval_add_with_guarantee([7, 0, 0, 1]);
    u96_limbs_less_than_guarantee_verify_v2(guarantee);
    output
}

fn test_guarantee_v2_first_limb() -> u384 {
    let (output, guarantee) = eval_add_with_guarantee([7, 0, 0, 0]);
    u96_limbs_less_than_guarantee_verify_v2(guarantee);
    output
}
