use super::EvalAction;
use crate::{
    utils::{get_numeric_args_as_bigints, get_value_from_integer},
    Value,
};
use cairo_lang_sierra::{
    extensions::{
        bounded_int::{
            BoundedIntConcreteLibfunc, BoundedIntConstrainConcreteLibfunc,
            BoundedIntDivRemConcreteLibfunc, BoundedIntGuaranteeVerifyConcreteLibfunc,
            BoundedIntTrimConcreteLibfunc,
        },
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        lib_func::SignatureOnlyConcreteLibfunc,
        ConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use num_bigint::BigInt;
use smallvec::smallvec;

pub fn eval(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    selector: &BoundedIntConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    match selector {
        BoundedIntConcreteLibfunc::Add(info) => eval_add(registry, info, args),
        BoundedIntConcreteLibfunc::Sub(info) => eval_sub(registry, info, args),
        BoundedIntConcreteLibfunc::Mul(info) => eval_mul(registry, info, args),
        BoundedIntConcreteLibfunc::DivRem(info) => eval_div_rem(registry, info, args),
        BoundedIntConcreteLibfunc::Constrain(info) => eval_constrain(registry, info, args),
        BoundedIntConcreteLibfunc::IsZero(info) => eval_is_zero(registry, info, args),
        BoundedIntConcreteLibfunc::WrapNonZero(info) => eval_wrap_non_zero(registry, info, args),
        BoundedIntConcreteLibfunc::TrimMin(info) | BoundedIntConcreteLibfunc::TrimMax(info) => {
            eval_trim(registry, info, args)
        }
        BoundedIntConcreteLibfunc::GuaranteeVerify(info) => {
            eval_guarantee_verify(registry, info, args)
        }
        BoundedIntConcreteLibfunc::U128ToU32Guarantees(info) => {
            eval_u128_to_u32_guarantees(registry, info, args)
        }
    }
}

pub fn eval_add(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args).try_into().unwrap();

    let range = match registry
        .get_type(&info.signature.branch_signatures[0].vars[0].ty)
        .unwrap()
    {
        CoreTypeConcrete::BoundedInt(info) => info.range.lower.clone()..info.range.upper.clone(),
        CoreTypeConcrete::NonZero(info) => match registry.get_type(&info.ty).unwrap() {
            CoreTypeConcrete::BoundedInt(info) => {
                info.range.lower.clone()..info.range.upper.clone()
            }
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };
    EvalAction::NormalBranch(
        0,
        smallvec![Value::BoundedInt {
            range,
            value: lhs + rhs,
        }],
    )
}

pub fn eval_sub(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args).try_into().unwrap();

    let range = match registry
        .get_type(&info.signature.branch_signatures[0].vars[0].ty)
        .unwrap()
    {
        CoreTypeConcrete::BoundedInt(info) => info.range.lower.clone()..info.range.upper.clone(),
        CoreTypeConcrete::NonZero(info) => match registry.get_type(&info.ty).unwrap() {
            CoreTypeConcrete::BoundedInt(info) => {
                info.range.lower.clone()..info.range.upper.clone()
            }
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };
    EvalAction::NormalBranch(
        0,
        smallvec![Value::BoundedInt {
            range,
            value: lhs - rhs,
        }],
    )
}

pub fn eval_mul(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args).try_into().unwrap();

    let range = match registry
        .get_type(&info.signature.branch_signatures[0].vars[0].ty)
        .unwrap()
    {
        CoreTypeConcrete::BoundedInt(info) => info.range.lower.clone()..info.range.upper.clone(),
        CoreTypeConcrete::NonZero(info) => match registry.get_type(&info.ty).unwrap() {
            CoreTypeConcrete::BoundedInt(info) => {
                info.range.lower.clone()..info.range.upper.clone()
            }
            _ => unreachable!(),
        },
        _ => unreachable!(),
    };
    EvalAction::NormalBranch(
        0,
        smallvec![Value::BoundedInt {
            range,
            value: lhs * rhs,
        }],
    )
}

pub fn eval_div_rem(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &BoundedIntDivRemConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let range_check @ Value::Unit: Value = args[0].clone() else {
        panic!()
    };
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args[1..]).try_into().unwrap();
    let quo = &lhs / &rhs;
    let rem = lhs % rhs;

    let quo_range = match registry
        .get_type(&info.branch_signatures()[0].vars[1].ty)
        .unwrap()
    {
        CoreTypeConcrete::BoundedInt(info) => info.range.lower.clone()..info.range.upper.clone(),
        _ => unreachable!(),
    };
    let rem_range = match registry
        .get_type(&info.branch_signatures()[0].vars[2].ty)
        .unwrap()
    {
        CoreTypeConcrete::BoundedInt(info) => info.range.lower.clone()..info.range.upper.clone(),
        _ => unreachable!(),
    };
    assert!(quo_range.contains(&quo));
    assert!(rem_range.contains(&rem));

    EvalAction::NormalBranch(
        0,
        smallvec![
            range_check,
            Value::BoundedInt {
                range: quo_range,
                value: quo,
            },
            Value::BoundedInt {
                range: rem_range,
                value: rem,
            },
        ],
    )
}

pub fn eval_constrain(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &BoundedIntConstrainConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let range_check @ Value::Unit: Value = args[0].clone() else {
        panic!()
    };
    let [value]: [BigInt; 1] = get_numeric_args_as_bigints(&args[1..]).try_into().unwrap();

    if value < info.boundary {
        let range = match registry
            .get_type(&info.branch_signatures()[0].vars[1].ty)
            .unwrap()
        {
            CoreTypeConcrete::BoundedInt(info) => {
                info.range.lower.clone()..info.range.upper.clone()
            }
            CoreTypeConcrete::NonZero(info) => match registry.get_type(&info.ty).unwrap() {
                CoreTypeConcrete::BoundedInt(info) => {
                    info.range.lower.clone()..info.range.upper.clone()
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        EvalAction::NormalBranch(
            0,
            smallvec![range_check, Value::BoundedInt { range, value }],
        )
    } else {
        let range = match registry
            .get_type(&info.branch_signatures()[1].vars[1].ty)
            .unwrap()
        {
            CoreTypeConcrete::BoundedInt(info) => {
                info.range.lower.clone()..info.range.upper.clone()
            }
            CoreTypeConcrete::NonZero(info) => match registry.get_type(&info.ty).unwrap() {
                CoreTypeConcrete::BoundedInt(info) => {
                    info.range.lower.clone()..info.range.upper.clone()
                }
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        EvalAction::NormalBranch(
            1,
            smallvec![range_check, Value::BoundedInt { range, value }],
        )
    }
}

pub fn eval_is_zero(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [value] = get_numeric_args_as_bigints(&args).try_into().unwrap();
    let is_zero = value == 0.into();

    let int_ty = &info.branch_signatures()[1].vars[0].ty;

    if is_zero {
        EvalAction::NormalBranch(0, smallvec![])
    } else {
        let value = get_value_from_integer(registry, int_ty, value);
        EvalAction::NormalBranch(1, smallvec![value])
    }
}

pub fn eval_wrap_non_zero(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [value] = args.try_into().unwrap();

    EvalAction::NormalBranch(0, smallvec![value])
}

pub fn eval_trim(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &BoundedIntTrimConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [value] = args.try_into().unwrap();
    let value = match value {
        Value::I8(v) => BigInt::from(v),
        Value::I16(v) => BigInt::from(v),
        Value::I32(v) => BigInt::from(v),
        Value::I64(v) => BigInt::from(v),
        Value::I128(v) => BigInt::from(v),
        Value::U8(v) => BigInt::from(v),
        Value::U16(v) => BigInt::from(v),
        Value::U32(v) => BigInt::from(v),
        Value::U64(v) => BigInt::from(v),
        Value::U128(v) => BigInt::from(v),
        Value::BoundedInt { value, .. } => value,
        _ => panic!("Not a valid integer type"),
    };
    let is_invalid = value == info.trimmed_value;
    let int_range = match registry
        .get_type(&info.branch_signatures()[1].vars[0].ty)
        .unwrap()
    {
        CoreTypeConcrete::BoundedInt(info) => info.range.clone(),
        _ => panic!("should be bounded int"),
    };

    if !is_invalid {
        let range = int_range.lower.clone()..int_range.upper.clone();
        EvalAction::NormalBranch(1, smallvec![Value::BoundedInt { range, value }])
    } else {
        EvalAction::NormalBranch(0, smallvec![])
    }
}

/// Evaluates `bounded_int_guarantee_verify`.
///
/// Consumes a `BoundedIntGuarantee` along with the `RangeCheck` builtin and
/// returns the builtin back.
pub fn eval_guarantee_verify(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &BoundedIntGuaranteeVerifyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let range_check @ Value::Unit = args[0].clone() else {
        panic!()
    };
    EvalAction::NormalBranch(0, smallvec![range_check])
}

/// Evaluates `u128_to_u32_guarantees`.
///
/// Splits a `u128` into four little-endian `u32` limbs (lowest 32 bits first,
/// highest last), each returned as a `BoundedInt<0, u32::MAX>`
pub fn eval_u128_to_u32_guarantees(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [value]: [Value; 1] = args.try_into().unwrap();
    let Value::U128(value) = value else {
        panic!("Expected u128")
    };
    let range = BigInt::from(0u32)..BigInt::from(u32::MAX) + 1;
    let limb = |shift: u32| Value::BoundedInt {
        range: range.clone(),
        value: BigInt::from((value >> shift) as u32),
    };
    EvalAction::NormalBranch(0, smallvec![limb(0), limb(32), limb(64), limb(96)])
}

#[cfg(test)]
mod tests {

    use super::Value;
    use crate::test_utils::{load_program, run_test_program};
    use num_bigint::BigInt;

    #[test]
    fn test_bounded_int_sub() {
        let program = load_program("test_data_artifacts/programs/debug_utils/bounded_int_sub");

        run_test_program(program);
    }

    #[test]
    fn test_trim_i8() {
        let program = load_program("test_data_artifacts/programs/debug_utils/bounded_int_trim_i8");

        let result = run_test_program(program);
        let result = result.last().unwrap();
        let expected = Value::BoundedInt {
            range: BigInt::from(-127)..BigInt::from(128),
            value: BigInt::from(1u8),
        };

        assert_eq!(*result, expected);
    }

    #[test]
    fn test_trim_u32() {
        let program = load_program("test_data_artifacts/programs/debug_utils/bounded_int_trim_u32");

        let result = run_test_program(program);
        let result = result.last().unwrap();
        let expected = Value::BoundedInt {
            range: BigInt::from(0)..BigInt::from(4294967295u32),
            value: BigInt::from(0xfffffffeu32),
        };

        assert_eq!(*result, expected);
    }

    #[test]
    fn test_trim_none() {
        let program =
            load_program("test_data_artifacts/programs/debug_utils/bounded_int_trim_none");

        let result = run_test_program(program);
        let result = result.last().unwrap();
        let expected = Value::BoundedInt {
            range: BigInt::from(-32767)..BigInt::from(32768),
            value: BigInt::from(0),
        };

        assert_eq!(*result, expected);
    }
}
