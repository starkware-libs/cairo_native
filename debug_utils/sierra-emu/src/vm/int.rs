use std::fmt::Debug;

use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        int::{
            signed::{SintConcrete, SintTraits},
            signed128::Sint128Concrete,
            unsigned::{UintConcrete, UintTraits},
            unsigned128::Uint128Concrete,
            IntConstConcreteLibfunc, IntMulTraits, IntOperationConcreteLibfunc, IntOperator,
            IntTraits,
        },
        is_zero::IsZeroTraits,
        lib_func::SignatureOnlyConcreteLibfunc,
        ConcreteLibfunc,
    },
    ids::ConcreteTypeId,
    program_registry::ProgramRegistry,
};
use num_bigint::{BigInt, BigUint, ToBigInt};
use num_traits::ops::overflowing::{OverflowingAdd, OverflowingSub};
use smallvec::smallvec;
use starknet_crypto::Felt;
use starknet_types_core::felt::NonZeroFelt;

use crate::{
    utils::{get_numeric_args_as_bigints, get_value_from_integer, integer_range},
    Value,
};

use super::EvalAction;

fn apply_overflowing_op_for_type(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ty: &ConcreteTypeId,
    lhs: BigInt,
    rhs: BigInt,
    op: IntOperator,
) -> (BigInt, bool) {
    fn overflowing_op<T>(lhs: BigInt, rhs: BigInt, op: IntOperator) -> (BigInt, bool)
    where
        T: OverflowingAdd + OverflowingSub + Into<BigInt> + TryFrom<BigInt>,
        <T as TryFrom<BigInt>>::Error: Debug,
    {
        let lhs: T = lhs.try_into().unwrap();
        let rhs: T = rhs.try_into().unwrap();

        let (result, had_overflow) = match op {
            IntOperator::OverflowingAdd => OverflowingAdd::overflowing_add(&lhs, &rhs),
            IntOperator::OverflowingSub => OverflowingSub::overflowing_sub(&lhs, &rhs),
        };

        (result.into(), had_overflow)
    }

    let ty = registry.get_type(ty).unwrap();

    match ty {
        CoreTypeConcrete::Sint8(_) => overflowing_op::<i8>(lhs, rhs, op),
        CoreTypeConcrete::Sint16(_) => overflowing_op::<i16>(lhs, rhs, op),
        CoreTypeConcrete::Sint32(_) => overflowing_op::<i32>(lhs, rhs, op),
        CoreTypeConcrete::Sint64(_) => overflowing_op::<i64>(lhs, rhs, op),
        CoreTypeConcrete::Sint128(_) => overflowing_op::<i128>(lhs, rhs, op),
        CoreTypeConcrete::Uint8(_) => overflowing_op::<u8>(lhs, rhs, op),
        CoreTypeConcrete::Uint16(_) => overflowing_op::<u16>(lhs, rhs, op),
        CoreTypeConcrete::Uint32(_) => overflowing_op::<u32>(lhs, rhs, op),
        CoreTypeConcrete::Uint64(_) => overflowing_op::<u64>(lhs, rhs, op),
        CoreTypeConcrete::Uint128(_) => overflowing_op::<u128>(lhs, rhs, op),
        _ => panic!("cannot apply integer operation to non-integer type"),
    }
}

pub fn eval_signed<T>(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    selector: &SintConcrete<T>,
    args: Vec<Value>,
) -> EvalAction
where
    T: IntMulTraits + SintTraits,
{
    match selector {
        SintConcrete::Const(info) => eval_const(registry, info, args),
        SintConcrete::Diff(info) => eval_diff(registry, info, args),
        SintConcrete::Equal(info) => eval_equal(registry, info, args),
        SintConcrete::FromFelt252(info) => eval_from_felt(registry, info, args),
        SintConcrete::Operation(info) => eval_operation(registry, info, args),
        SintConcrete::ToFelt252(info) => eval_to_felt(registry, info, args),
        SintConcrete::WideMul(info) => eval_widemul(registry, info, args),
    }
}

pub fn eval_i128(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    selector: &Sint128Concrete,
    args: Vec<Value>,
) -> EvalAction {
    match selector {
        Sint128Concrete::Const(info) => eval_const(registry, info, args),
        Sint128Concrete::Diff(info) => eval_diff(registry, info, args),
        Sint128Concrete::Equal(info) => eval_equal(registry, info, args),
        Sint128Concrete::FromFelt252(info) => eval_from_felt(registry, info, args),
        Sint128Concrete::Operation(info) => eval_operation(registry, info, args),
        Sint128Concrete::ToFelt252(info) => eval_to_felt(registry, info, args),
    }
}

pub fn eval_unsigned<T>(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    selector: &UintConcrete<T>,
    args: Vec<Value>,
) -> EvalAction
where
    T: IntMulTraits + IsZeroTraits + UintTraits,
{
    match selector {
        UintConcrete::Const(info) => eval_const(registry, info, args),
        UintConcrete::Bitwise(info) => eval_bitwise(registry, info, args),
        UintConcrete::Divmod(info) => eval_divmod(registry, info, args),
        UintConcrete::Equal(info) => eval_equal(registry, info, args),
        UintConcrete::FromFelt252(info) => eval_from_felt(registry, info, args),
        UintConcrete::Operation(info) => eval_operation(registry, info, args),
        UintConcrete::IsZero(info) => eval_is_zero(registry, info, args),
        UintConcrete::SquareRoot(info) => eval_square_root(registry, info, args),
        UintConcrete::ToFelt252(info) => eval_to_felt(registry, info, args),
        UintConcrete::WideMul(info) => eval_widemul(registry, info, args),
    }
}

pub fn eval_uint128(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    selector: &Uint128Concrete,
    args: Vec<Value>,
) -> EvalAction {
    match selector {
        Uint128Concrete::Const(info) => eval_const(registry, info, args),
        Uint128Concrete::Operation(info) => eval_operation(registry, info, args),
        Uint128Concrete::SquareRoot(info) => eval_square_root(registry, info, args),
        Uint128Concrete::Equal(info) => eval_equal(registry, info, args),
        Uint128Concrete::ToFelt252(info) => eval_to_felt(registry, info, args),
        Uint128Concrete::FromFelt252(info) => eval_u128_from_felt(registry, info, args),
        Uint128Concrete::IsZero(info) => eval_is_zero(registry, info, args),
        Uint128Concrete::Divmod(info) => eval_divmod(registry, info, args),
        Uint128Concrete::Bitwise(info) => eval_bitwise(registry, info, args),
        Uint128Concrete::GuaranteeMul(info) => eval_guarantee_mul(registry, info, args),
        Uint128Concrete::MulGuaranteeVerify(info) => eval_guarantee_verify(registry, info, args),
        Uint128Concrete::ByteReverse(info) => eval_byte_reverse(registry, info, args),
    }
}

fn eval_const<T: IntTraits>(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &IntConstConcreteLibfunc<T>,
    _args: Vec<Value>,
) -> EvalAction {
    let int_ty = &info.signature.branch_signatures[0].vars[0].ty;

    EvalAction::NormalBranch(
        0,
        smallvec![get_value_from_integer(registry, int_ty, info.c.into())],
    )
}

fn eval_bitwise(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let bitwise @ Value::Unit: Value = args[0].clone() else {
        panic!()
    };
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args[1..]).try_into().unwrap();

    let int_ty = &info.signature.branch_signatures[0].vars[1].ty;

    let and = get_value_from_integer(registry, int_ty, &lhs & &rhs);
    let or = get_value_from_integer(registry, int_ty, &lhs | &rhs);
    let xor = get_value_from_integer(registry, int_ty, &lhs ^ &rhs);

    EvalAction::NormalBranch(0, smallvec![bitwise, and, xor, or])
}

fn eval_diff(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let range_check @ Value::Unit: Value = args[0].clone() else {
        panic!()
    };
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args[1..]).try_into().unwrap();

    let int_ty = &info.branch_signatures()[0].vars[1].ty;

    let res = lhs - rhs;

    // Since this libfunc returns an unsigned value, If lhs >= rhs then just
    // return Ok(lhs - rhs). Otherwise, we need to wrap around the value, returning
    // Err(2**n + lhs - rhs), where n is the amount of bits for that integer type.
    if res < BigInt::ZERO {
        let max_integer = integer_range(int_ty, registry).upper;
        EvalAction::NormalBranch(
            1,
            smallvec![
                range_check,
                get_value_from_integer(registry, int_ty, max_integer + res)
            ],
        )
    } else {
        EvalAction::NormalBranch(
            0,
            smallvec![range_check, get_value_from_integer(registry, int_ty, res)],
        )
    }
}

fn eval_divmod(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let range_check @ Value::Unit: Value = args[0].clone() else {
        panic!()
    };
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args[1..]).try_into().unwrap();

    let int_ty = &info.signature.branch_signatures[0].vars[1].ty;

    let res = &lhs / &rhs;
    let rem = lhs % rhs;

    let res = get_value_from_integer(registry, int_ty, res);
    let rem = get_value_from_integer(registry, int_ty, rem);

    EvalAction::NormalBranch(0, smallvec![range_check, res, rem])
}

fn eval_equal(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [lhs, rhs] = args.try_into().unwrap();

    EvalAction::NormalBranch((lhs == rhs) as usize, smallvec![])
}

fn eval_from_felt(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [range_check @ Value::Unit, Value::Felt(value_felt)]: [Value; 2] = args.try_into().unwrap()
    else {
        panic!()
    };

    let prime = Felt::prime();
    let half_prime = &prime / BigUint::from(2u8);

    let int_ty = &info.signature.branch_signatures[0].vars[1].ty;

    let range = integer_range(int_ty, registry);

    let value = {
        let value_bigint = value_felt.to_biguint();
        if value_bigint > half_prime {
            (prime - value_bigint).to_bigint().unwrap() * BigInt::from(-1)
        } else {
            value_felt.to_bigint()
        }
    };
    if value >= range.lower && value < range.upper {
        let value = get_value_from_integer(registry, int_ty, value);
        EvalAction::NormalBranch(0, smallvec![range_check, value])
    } else {
        EvalAction::NormalBranch(1, smallvec![range_check])
    }
}

pub fn eval_u128_from_felt(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [range_check @ Value::Unit, Value::Felt(value)]: [Value; 2] = args.try_into().unwrap()
    else {
        panic!()
    };

    let bound = Felt::from(u128::MAX) + 1;

    if value < bound {
        let value: u128 = value.to_biguint().try_into().unwrap();
        EvalAction::NormalBranch(0, smallvec![range_check, Value::U128(value)])
    } else {
        let (new_value, overflow) = value.div_rem(&NonZeroFelt::try_from(bound).unwrap());

        let overflow: u128 = overflow.to_biguint().try_into().unwrap();
        let new_value: u128 = new_value.to_biguint().try_into().unwrap();
        EvalAction::NormalBranch(
            1,
            smallvec![range_check, Value::U128(new_value), Value::U128(overflow)],
        )
    }
}

fn eval_is_zero(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [value]: [BigInt; 1] = get_numeric_args_as_bigints(&args).try_into().unwrap();

    let int_ty = &info.signature.branch_signatures[1].vars[0].ty;

    if value == 0.into() {
        EvalAction::NormalBranch(0, smallvec![])
    } else {
        EvalAction::NormalBranch(
            1,
            smallvec![get_value_from_integer(registry, int_ty, value)],
        )
    }
}

fn eval_operation(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &IntOperationConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let range_check @ Value::Unit: Value = args[0].clone() else {
        panic!()
    };
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args[1..]).try_into().unwrap();
    let int_ty = &info.signature.param_signatures[1].ty;

    let (res, had_overflow) =
        apply_overflowing_op_for_type(registry, int_ty, lhs, rhs, info.operator);
    let res = get_value_from_integer(registry, int_ty, res);

    EvalAction::NormalBranch(had_overflow as usize, smallvec![range_check, res])
}

fn eval_square_root(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let range_check @ Value::Unit: Value = args[0].clone() else {
        panic!()
    };
    let [value]: [BigInt; 1] = get_numeric_args_as_bigints(&args[1..]).try_into().unwrap();

    let int_ty = &info.signature.branch_signatures[0].vars[1].ty;

    let res = value.sqrt();

    EvalAction::NormalBranch(
        0,
        smallvec![range_check, get_value_from_integer(registry, int_ty, res)],
    )
}

fn eval_to_felt(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [val]: [BigInt; 1] = get_numeric_args_as_bigints(&args).try_into().unwrap();

    EvalAction::NormalBranch(0, smallvec![Value::Felt(Felt::from(val))])
}

fn eval_widemul(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [lhs, rhs]: [BigInt; 2] = get_numeric_args_as_bigints(&args).try_into().unwrap();

    let int_ty = &info.signature.branch_signatures[0].vars[0].ty;

    let res = lhs * rhs;

    EvalAction::NormalBranch(0, smallvec![get_value_from_integer(registry, int_ty, res)])
}

fn eval_guarantee_mul(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::U128(lhs), Value::U128(rhs)]: [Value; 2] = args.try_into().unwrap() else {
        panic!()
    };

    let mask128 = BigUint::from(u128::MAX);
    let result = BigUint::from(lhs) * BigUint::from(rhs);
    let high = Value::U128((&result >> 128u32).try_into().unwrap());
    let low = Value::U128((result & mask128).try_into().unwrap());

    EvalAction::NormalBranch(0, smallvec![high, low, Value::Unit])
}

fn eval_guarantee_verify(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [range_check @ Value::Unit, _verify @ Value::Unit]: [Value; 2] = args.try_into().unwrap()
    else {
        panic!()
    };

    EvalAction::NormalBranch(0, smallvec![range_check])
}

fn eval_byte_reverse(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [bitwise @ Value::Unit, Value::U128(value)]: [Value; 2] = args.try_into().unwrap() else {
        panic!()
    };

    let value = value.swap_bytes();

    EvalAction::NormalBranch(0, smallvec![bitwise, Value::U128(value)])
}

#[cfg(test)]
mod test {
    use crate::{
        test_utils::{load_program, run_test_program},
        Value,
    };

    #[test]
    fn test_diff_14_m2() {
        let program = load_program("programs/debug_utils/int_diff_14_m2");

        let result = run_test_program(program);
        let result = result.last().unwrap();

        let Value::Enum { payload, .. } = result else {
            panic!()
        };

        assert_eq!(**payload, Value::U8(16))
    }

    #[test]
    fn test_diff_m14_m2() {
        let program = load_program("programs/debug_utils/int_diff_m14_m2");

        let result = run_test_program(program);
        let result = result.last().unwrap();

        let Value::Enum { payload, .. } = result else {
            panic!()
        };

        assert_eq!(**payload, Value::U8(244))
    }

    #[test]
    fn test_diff_m2_0() {
        let program = load_program("programs/debug_utils/int_diff_m2_0");

        let result = run_test_program(program);
        let result = result.last().unwrap();

        let Value::Enum { payload, .. } = result else {
            panic!()
        };

        assert_eq!(**payload, Value::U8(254))
    }

    #[test]
    fn test_diff_2_10() {
        let program = load_program("programs/debug_utils/int_diff_2_10");

        let result = run_test_program(program);
        let result = result.last().unwrap();

        let Value::Enum { payload, .. } = result else {
            panic!()
        };

        assert_eq!(**payload, Value::U8(248))
    }
}
