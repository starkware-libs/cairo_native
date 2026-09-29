use super::EvalAction;
use crate::Value;
use cairo_lang_sierra::{
    extensions::{
        circuit::{
            CircuitConcreteLibfunc, CircuitTypeConcrete, ConcreteGetOutputLibFunc,
            ConcreteU96LimbsLessThanGuaranteeVerifyLibfunc,
        },
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        lib_func::{SignatureAndTypeConcreteLibfunc, SignatureOnlyConcreteLibfunc},
    },
    program_registry::ProgramRegistry,
};
use num_bigint::{BigInt, BigUint, Sign, ToBigInt};
use num_integer::{ExtendedGcd, Integer};
use num_traits::{One, ToPrimitive, Zero};
use smallvec::smallvec;

fn u384_to_struct(num: BigUint) -> Value {
    let output_big = num.to_bigint().unwrap();

    let mask: BigInt = BigInt::from_bytes_be(Sign::Plus, &[255; 12]);

    let l0: BigInt = &output_big & &mask;
    let l1: BigInt = (&output_big >> 96) & &mask;
    let l2: BigInt = (&output_big >> 192) & &mask;
    let l3: BigInt = (output_big >> 288) & &mask;

    let range = BigInt::ZERO..(BigInt::from(1) << 96);
    Value::Struct(vec![
        Value::BoundedInt {
            range: range.clone(),
            value: l0,
        },
        Value::BoundedInt {
            range: range.clone(),
            value: l1,
        },
        Value::BoundedInt {
            range: range.clone(),
            value: l2,
        },
        Value::BoundedInt { range, value: l3 },
    ])
}

fn struct_to_u384(struct_members: Vec<Value>) -> BigUint {
    let [Value::U128(l0), Value::U128(l1), Value::U128(l2), Value::U128(l3)]: [Value; 4] =
        struct_members.try_into().unwrap()
    else {
        panic!()
    };

    let l0 = l0.to_le_bytes();
    let l1 = l1.to_le_bytes();
    let l2 = l2.to_le_bytes();
    let l3 = l3.to_le_bytes();

    BigUint::from_bytes_le(&[
        l0[0], l0[1], l0[2], l0[3], l0[4], l0[5], l0[6], l0[7], l0[8], l0[9], l0[10],
        l0[11], //
        l1[0], l1[1], l1[2], l1[3], l1[4], l1[5], l1[6], l1[7], l1[8], l1[9], l1[10],
        l1[11], //
        l2[0], l2[1], l2[2], l2[3], l2[4], l2[5], l2[6], l2[7], l2[8], l2[9], l2[10],
        l2[11], //
        l3[0], l3[1], l3[2], l3[3], l3[4], l3[5], l3[6], l3[7], l3[8], l3[9], l3[10],
        l3[11], //
    ])
}

/// If the value is non-invertible a nullifier is returned instead. A nullifier
/// is value which satisfies this following equation: num * nullifier ≡ 0(modulus).
fn find_nullifier(num: &BigUint, modulus: &BigUint) -> BigUint {
    let ExtendedGcd { gcd, .. } = num
        .to_bigint()
        .unwrap()
        .extended_gcd(&modulus.to_bigint().unwrap());
    let gcd = gcd.to_biguint().unwrap();

    modulus / gcd
}

pub fn eval(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    selector: &CircuitConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    match selector {
        CircuitConcreteLibfunc::AddInput(info) => eval_add_input(registry, info, args),
        CircuitConcreteLibfunc::Eval(info) => eval_eval(registry, info, args),
        CircuitConcreteLibfunc::GetDescriptor(info) => eval_get_descriptor(registry, info, args),
        CircuitConcreteLibfunc::InitCircuitData(info) => {
            eval_init_circuit_data(registry, info, args)
        }
        CircuitConcreteLibfunc::GetOutput(info) => eval_get_output(registry, info, args),
        CircuitConcreteLibfunc::TryIntoCircuitModulus(info) => {
            eval_try_into_circuit_modulus(registry, info, args)
        }
        CircuitConcreteLibfunc::FailureGuaranteeVerify(info) => {
            eval_failure_guarantee_verify(registry, info, args)
        }
        CircuitConcreteLibfunc::IntoU96Guarantee(info) => {
            eval_into_u96_guarantee(registry, info, args)
        }
        CircuitConcreteLibfunc::U96GuaranteeVerify(info) => {
            eval_u96_guarantee_verify(registry, info, args)
        }
        CircuitConcreteLibfunc::U96LimbsLessThanGuaranteeVerify(info) => {
            eval_u96_limbs_less_than_guarantee_verify(registry, info, args)
        }
        CircuitConcreteLibfunc::U96SingleLimbLessThanGuaranteeVerify(info) => {
            eval_u96_single_limb_less_than_guarantee_verify(registry, info, args)
        }
        CircuitConcreteLibfunc::U96LimbsLessThanGuaranteeVerifyV2(_) => {
            todo!("u96_limbs_less_than_guarantee_verify_v2")
        }
    }
}

fn eval_add_input(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureAndTypeConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::Circuit(mut values), Value::Struct(members)]: [Value; 2] = args.try_into().unwrap()
    else {
        panic!()
    };

    let n_inputs = match registry.get_type(&info.ty).unwrap() {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) => info.circuit_info.n_inputs,
        _ => panic!(),
    };

    values.push(struct_to_u384(members));

    EvalAction::NormalBranch(
        (values.len() != n_inputs) as usize,
        smallvec![Value::Circuit(values)],
    )
}

fn eval_eval(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureAndTypeConcreteLibfunc,
    _args: Vec<Value>,
) -> EvalAction {
    let [add_mod @ Value::Unit, mul_mod @ Value::Unit, _descripctor @ Value::Unit, Value::Circuit(inputs), Value::CircuitModulus(modulus), _, _]: [Value; 7] = _args.try_into().unwrap()
    else {
        panic!()
    };
    let circ_info = match _registry.get_type(&_info.ty).unwrap() {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) => &info.circuit_info,
        _ => todo!(),
    };
    let mut outputs = vec![None; 1 + circ_info.n_inputs + circ_info.values.len()];
    let mut add_gates = circ_info.add_offsets.iter().peekable();
    let mut mul_gates = circ_info.mul_offsets.iter();

    outputs[0] = Some(BigUint::from(1_u8));

    for (i, input) in inputs.iter().enumerate() {
        outputs[i + 1] = Some(input.to_owned());
    }

    let success = loop {
        while let Some(add_gate) = add_gates.peek() {
            let lhs = outputs[add_gate.lhs].to_owned();
            let rhs = outputs[add_gate.rhs].to_owned();

            match (lhs, rhs) {
                (Some(l), Some(r)) => {
                    outputs[add_gate.output] = Some((l + r) % &modulus);
                }
                (None, Some(r)) => {
                    let res = match outputs[add_gate.output].to_owned() {
                        Some(res) => res,
                        None => break,
                    };
                    // if it is a sub_gate the output index is store in lhs
                    outputs[add_gate.lhs] = Some((res + &modulus - r) % &modulus);
                }
                // there aren't enough gates computed for add_gate to compute
                // the next gate so we need to compute a mul_gate
                _ => break,
            };

            add_gates.next();
        }

        match mul_gates.next() {
            Some(mul_gate) => {
                let lhs = outputs[mul_gate.lhs].to_owned();
                let rhs = outputs[mul_gate.rhs].to_owned();

                match (lhs, rhs) {
                    (Some(l), Some(r)) => {
                        outputs[mul_gate.output] = Some((l * r) % &modulus);
                    }
                    (None, Some(r)) => {
                        match r.modinv(&modulus) {
                            // if it is a inv_gate the output index is store in lhs
                            Some(r) => outputs[mul_gate.lhs] = Some(r),
                            // Since we don't calculate CircuitPartialOutputs
                            // perform an early break
                            None => {
                                outputs[mul_gate.lhs] = Some(find_nullifier(&r, &modulus));
                                break false;
                            }
                        }
                    }
                    // this state should not be reached since it would mean that
                    // not all the circuit's inputs where filled
                    _ => unreachable!(),
                }
            }
            None => break true,
        }
    };

    if success {
        let values = outputs
            .into_iter()
            .skip(1 + circ_info.n_inputs)
            .collect::<Option<Vec<BigUint>>>()
            .expect("The circuit cannot be calculated");

        EvalAction::NormalBranch(
            0,
            smallvec![
                add_mod,
                mul_mod,
                Value::CircuitOutputs {
                    circuits: values,
                    modulus
                }
            ],
        )
    } else {
        EvalAction::NormalBranch(1, smallvec![add_mod, mul_mod, Value::Unit, Value::Unit])
    }
}

fn eval_get_output(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &ConcreteGetOutputLibFunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::CircuitOutputs {
        circuits: outputs,
        modulus,
    }]: [Value; 1] = args.try_into().unwrap()
    else {
        panic!()
    };
    let circuit_info = match _registry.get_type(&_info.circuit_ty).unwrap() {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) => &info.circuit_info,
        _ => todo!(),
    };
    let gate_offset = *circuit_info.values.get(&_info.output_ty).unwrap();
    let output_idx = gate_offset - 1 - circuit_info.n_inputs;
    let output = outputs[output_idx].to_owned();

    let output_struct = u384_to_struct(output);
    let modulus = u384_to_struct(modulus);

    EvalAction::NormalBranch(
        0,
        smallvec![
            output_struct.clone(),
            Value::Struct(vec![output_struct, modulus]),
        ],
    )
}

fn eval_u96_limbs_less_than_guarantee_verify(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &ConcreteU96LimbsLessThanGuaranteeVerifyLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::Struct(guarantee)]: [Value; 1] = args.try_into().unwrap() else {
        panic!()
    };
    let limb_count = info.limb_count;
    let Value::Struct(gate) = &guarantee[0] else {
        panic!();
    };
    let Value::Struct(modulus) = &guarantee[1] else {
        panic!();
    };
    let Value::BoundedInt {
        value: gate_last_limb,
        ..
    } = &gate[limb_count - 1]
    else {
        panic!();
    };
    let Value::BoundedInt {
        value: modulus_last_limb,
        ..
    } = &modulus[limb_count - 1]
    else {
        panic!();
    };
    let diff = (modulus_last_limb - gate_last_limb).to_u128().unwrap();

    if diff != 0 {
        EvalAction::NormalBranch(1, smallvec![Value::U128(diff)])
    } else {
        // if there is no diff, build a new guarantee, skipping the last limb
        let new_gate = Value::Struct(gate[0..limb_count].to_vec());
        let new_modulus = Value::Struct(modulus[0..limb_count].to_vec());

        EvalAction::NormalBranch(0, smallvec![Value::Struct(vec![new_gate, new_modulus])])
    }
}

fn eval_u96_single_limb_less_than_guarantee_verify(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [_guarantee]: [Value; 1] = args.try_into().unwrap();

    EvalAction::NormalBranch(0, smallvec![Value::U128(0)])
}

fn eval_u96_guarantee_verify(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [range_check_96 @ Value::Unit, _]: [Value; 2] = args.try_into().unwrap() else {
        panic!();
    };

    EvalAction::NormalBranch(0, smallvec![range_check_96])
}

fn eval_failure_guarantee_verify(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    _args: Vec<Value>,
) -> EvalAction {
    let [rc96 @ Value::Unit, mul_mod @ Value::Unit, _, _, _]: [Value; 5] =
        _args.try_into().unwrap()
    else {
        panic!()
    };

    let limbs_count = match registry
        .get_type(&info.signature.branch_signatures[0].vars[2].ty)
        .unwrap()
    {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::U96LimbsLessThanGuarantee(info)) => {
            info.limb_count
        }
        _ => panic!(),
    };

    let zero_u96 = Value::BoundedInt {
        range: BigInt::zero()..BigInt::one() << 96,
        value: 0.into(),
    };
    // This should be changed with it correct value when we implement this libfunc in native
    let limbs_struct = Value::Struct(vec![zero_u96; limbs_count]);

    EvalAction::NormalBranch(
        0,
        smallvec![
            rc96,
            mul_mod,
            Value::Struct(vec![limbs_struct.clone(), limbs_struct])
        ],
    )
}

fn eval_get_descriptor(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureAndTypeConcreteLibfunc,
    _args: Vec<Value>,
) -> EvalAction {
    EvalAction::NormalBranch(0, smallvec![Value::Unit])
}

fn eval_init_circuit_data(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureAndTypeConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [range_check_96 @ Value::Unit]: [Value; 1] = args.try_into().unwrap() else {
        panic!()
    };

    let num_inputs = match _registry.get_type(&info.ty).unwrap() {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) => info.circuit_info.n_inputs,
        _ => todo!("{}", info.ty),
    };

    EvalAction::NormalBranch(
        0,
        smallvec![
            range_check_96,
            Value::Circuit(Vec::with_capacity(num_inputs)),
        ],
    )
}

fn eval_try_into_circuit_modulus(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::Struct(members)]: [Value; 1] = args.try_into().unwrap() else {
        panic!()
    };

    let [Value::BoundedInt {
        range: r0,
        value: l0,
    }, Value::BoundedInt {
        range: r1,
        value: l1,
    }, Value::BoundedInt {
        range: r2,
        value: l2,
    }, Value::BoundedInt {
        range: r3,
        value: l3,
    }]: [Value; 4] = members.try_into().unwrap()
    else {
        panic!()
    };
    assert_eq!(r0, BigInt::ZERO..(BigInt::from(1) << 96));
    assert_eq!(r1, BigInt::ZERO..(BigInt::from(1) << 96));
    assert_eq!(r2, BigInt::ZERO..(BigInt::from(1) << 96));
    assert_eq!(r3, BigInt::ZERO..(BigInt::from(1) << 96));

    let l0 = l0.to_biguint().unwrap();
    let l1 = l1.to_biguint().unwrap();
    let l2 = l2.to_biguint().unwrap();
    let l3 = l3.to_biguint().unwrap();

    let value = l0 | (l1 << 96) | (l2 << 192) | (l3 << 288);

    if value > BigUint::one() {
        EvalAction::NormalBranch(0, smallvec![Value::CircuitModulus(value)])
    } else {
        EvalAction::NormalBranch(1, smallvec![])
    }
}

fn eval_into_u96_guarantee(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureAndTypeConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::BoundedInt { range, value }]: [Value; 1] = args.try_into().unwrap() else {
        panic!()
    };
    assert_eq!(range, BigInt::ZERO..(BigInt::from(1) << 96));

    EvalAction::NormalBranch(0, smallvec![Value::U128(value.try_into().unwrap())])
}
