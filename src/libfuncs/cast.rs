//! # Casting libfuncs

use super::LibfuncHelper;
use crate::{
    error::Result,
    libfuncs::increment_builtin_counter,
    metadata::MetadataStorage,
    native_assert, native_panic,
    types::TypeBuilder,
    utils::{RangeExt, HALF_PRIME, PRIME},
};
use cairo_lang_sierra::{
    extensions::{
        casts::{CastConcreteLibfunc, DowncastConcreteLibfunc},
        core::{CoreLibfunc, CoreType},
        lib_func::SignatureOnlyConcreteLibfunc,
        utils::Range,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::arith::{self, CmpiPredicate},
    helpers::{ArithBlockExt, BuiltinBlockExt},
    ir::{r#type::IntegerType, Block, Location, Value, ValueLike},
    Context,
};
use num_bigint::{BigInt, Sign};
use num_traits::One;

/// Select and call the correct libfunc builder function from the selector.
pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &CastConcreteLibfunc,
) -> Result<()> {
    match selector {
        CastConcreteLibfunc::Downcast(info) => {
            build_downcast(context, registry, entry, location, helper, metadata, info)
        }
        CastConcreteLibfunc::Upcast(info) => {
            build_upcast(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `downcast` libfunc which converts from a
/// source type `T` to a target type `U`, where `U` might not fully include `T`.
/// This means that the operation can fail.
///
/// ## Signature
/// ```cairo
/// pub extern const fn downcast<FromType, ToType>(
///     x: FromType,
/// ) -> Option<ToType> implicits(RangeCheck) nopanic;
/// ```
pub fn build_downcast<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &DowncastConcreteLibfunc,
) -> Result<()> {
    let range_check = entry.arg(0)?;
    let src_value: Value = entry.arg(1)?;

    let src_ty = registry.get_type(&info.signature.param_signatures[1].ty)?;
    let dst_ty = registry.get_type(&info.signature.branch_signatures[0].vars[1].ty)?;

    let dst_range = dst_ty.integer_range(registry)?;
    let src_range = if src_ty.is_felt252(registry)? && dst_range.lower.sign() == Sign::Minus {
        if dst_range.upper.sign() != Sign::Plus {
            Range {
                lower: BigInt::from_biguint(Sign::Minus, PRIME.clone()) + 1,
                upper: BigInt::one(),
            }
        } else {
            Range {
                lower: BigInt::from_biguint(Sign::Minus, HALF_PRIME.clone()),
                upper: BigInt::from_biguint(Sign::Plus, HALF_PRIME.clone()) + BigInt::one(),
            }
        }
    } else {
        src_ty.integer_range(registry)?
    };

    // When the source type is the same as the target type, we just return the
    // value as it cannot fail. However, for backwards compatibility, we need to
    // increment the range check as if we were checking the upper bound. See:
    // - https://github.com/starkware-libs/cairo/tree/v2.12.3/crates/cairo-lang-sierra/src/extensions/modules/casts.rs#L67.
    // - https://github.com/starkware-libs/cairo/tree/v2.12.3/crates/cairo-lang-sierra-to-casm/src/invocations/casts.rs#L56.
    if info.signature.param_signatures[1].ty == info.signature.branch_signatures[0].vars[1].ty {
        let range_check = if src_range.lower == 0.into() {
            increment_builtin_counter(context, entry, location, range_check)?
        } else {
            range_check
        };
        let k1 = entry.const_int(context, location, 1, 1)?;
        return helper.cond_br(
            context,
            entry,
            k1,
            [0, 1],
            [&[range_check, src_value], &[range_check]],
            location,
        );
    }

    let src_width = src_range.repr_bit_width();
    let dst_width = dst_range.repr_bit_width();

    let compute_width = src_range
        .zero_based_bit_width()
        .max(dst_range.zero_based_bit_width());

    let is_signed = src_range.lower.sign() == Sign::Minus;

    // If the target type is wider than the source type, extend the value representation width.
    let src_value = if compute_width > src_width {
        if is_signed && !src_ty.is_bounded_int(registry)? && !src_ty.is_felt252(registry)? {
            entry.extsi(
                src_value,
                IntegerType::new(context, compute_width).into(),
                location,
            )?
        } else {
            entry.extui(
                src_value,
                IntegerType::new(context, compute_width).into(),
                location,
            )?
        }
    } else {
        src_value
    };

    // Correct the value representation accordingly.
    // 1. if it is a felt, then we need to convert the value from [0,P) to
    //    [-P/2, P/2].
    // 2. if it is a bounded_int, we need to offset the value to get the
    //    actual value.
    let src_value = if is_signed && src_ty.is_felt252(registry)? {
        if src_range.upper.is_one() {
            let adj_offset =
                entry.const_int_from_type(context, location, PRIME.clone(), src_value.r#type())?;
            entry.append_op_result(arith::subi(src_value, adj_offset, location))?
        } else {
            let adj_offset = entry.const_int_from_type(
                context,
                location,
                HALF_PRIME.clone(),
                src_value.r#type(),
            )?;
            let is_negative =
                entry.cmpi(context, CmpiPredicate::Ugt, src_value, adj_offset, location)?;

            let k_prime =
                entry.const_int_from_type(context, location, PRIME.clone(), src_value.r#type())?;
            let adj_value = entry.append_op_result(arith::subi(src_value, k_prime, location))?;

            entry.append_op_result(arith::select(is_negative, adj_value, src_value, location))?
        }
    } else if src_ty.is_bounded_int(registry)? && src_range.lower != BigInt::ZERO {
        let dst_offset = entry.const_int_from_type(
            context,
            location,
            src_range.lower.clone(),
            src_value.r#type(),
        )?;
        entry.addi(src_value, dst_offset, location)?
    } else {
        src_value
    };

    // Check if the source type is included in the target type. If it is not
    // then check if the value is in bounds. If the value is also not in
    // bounds then return an error.
    if dst_range.lower <= src_range.lower && dst_range.upper >= src_range.upper {
        let dst_value = if dst_ty.is_bounded_int(registry)? && dst_range.lower != BigInt::ZERO {
            let dst_offset = entry.const_int_from_type(
                context,
                location,
                dst_range.lower,
                src_value.r#type(),
            )?;
            entry.append_op_result(arith::subi(src_value, dst_offset, location))?
        } else {
            src_value
        };

        let dst_value = if dst_width < compute_width {
            entry.trunci(
                dst_value,
                IntegerType::new(context, dst_width).into(),
                location,
            )?
        } else {
            dst_value
        };

        let is_in_bounds = entry.const_int(context, location, 1, 1)?;

        helper.cond_br(
            context,
            entry,
            is_in_bounds,
            [0, 1],
            [&[range_check, dst_value], &[range_check]],
            location,
        )?;
    } else {
        // Check if the value is in bounds with respect to the lower bound.
        let lower_check = if dst_range.lower > src_range.lower {
            let dst_lower = entry.const_int_from_type(
                context,
                location,
                dst_range.lower.clone(),
                src_value.r#type(),
            )?;
            Some(entry.cmpi(
                context,
                if !is_signed {
                    CmpiPredicate::Uge
                } else {
                    CmpiPredicate::Sge
                },
                src_value,
                dst_lower,
                location,
            )?)
        } else {
            None
        };
        // Check if the value is in bounds with respect to the upper bound.
        let upper_check = if dst_range.upper < src_range.upper {
            let dst_upper = entry.const_int_from_type(
                context,
                location,
                dst_range.upper.clone(),
                src_value.r#type(),
            )?;
            Some(entry.cmpi(
                context,
                if !is_signed {
                    CmpiPredicate::Ult
                } else {
                    CmpiPredicate::Slt
                },
                src_value,
                dst_upper,
                location,
            )?)
        } else {
            None
        };

        let is_in_bounds = match (lower_check, upper_check) {
            (Some(lower_check), Some(upper_check)) => {
                entry.append_op_result(arith::andi(lower_check, upper_check, location))?
            }
            (Some(lower_check), None) => lower_check,
            (None, Some(upper_check)) => upper_check,
            // its always in bounds since dst is larger than src (i.e no bounds checks needed)
            (None, None) => {
                native_panic!("matched an unreachable: no bounds checks are being performed")
            }
        };

        // Incrementing the range check depends on whether the source range can hold a felt252 or not
        let range_check = if info.from_range.is_full_felt252_range() {
            let rc_size = BigInt::from(1) << 128;
            // If the range can contain a felt252, how the range check is increased depends on whether the value is in bounds or not:
            // * If it is in bounds, we check whether the destination range size is less than range check size. If it is, increment
            //   the range check builtin by 2. Otherwise, increment it by 1.
            //   https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/range_reduction.rs#L87
            // * If it is not in bounds, increment the range check builtin by 3.
            //   https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/range_reduction.rs#L79
            super::increment_builtin_counter_conditionally_by(
                context,
                entry,
                location,
                range_check,
                if dst_range.size() < rc_size { 2 } else { 1 },
                3,
                is_in_bounds,
            )?
        } else {
            match (lower_check, upper_check) {
                (Some(_), None) | (None, Some(_)) => {
                    // If either the lower or the upper bound was checked, increment the range check builtin by 1.
                    // * In case the lower bound was checked: https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/casts.rs#L135
                    // * In case the upper bound was checked: https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/casts.rs#L111
                    super::increment_builtin_counter_by(context, entry, location, range_check, 1)?
                }
                (Some(lower_check), Some(upper_check)) => {
                    let is_in_range =
                        entry.append_op_result(arith::andi(lower_check, upper_check, location))?;

                    // If the result is in range, increment the range check builtin by 2. Otherwise, increment it by 1.
                    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/casts.rs#L160
                    super::increment_builtin_counter_conditionally_by(
                        context,
                        entry,
                        location,
                        range_check,
                        2,
                        1,
                        is_in_range,
                    )?
                }
                (None, None) => range_check,
            }
        };

        let dst_value = if dst_ty.is_bounded_int(registry)? && dst_range.lower != BigInt::ZERO {
            let dst_offset = entry.const_int_from_type(
                context,
                location,
                dst_range.lower,
                src_value.r#type(),
            )?;
            entry.append_op_result(arith::subi(src_value, dst_offset, location))?
        } else {
            src_value
        };

        let dst_value = if dst_width < compute_width {
            entry.trunci(
                dst_value,
                IntegerType::new(context, dst_width).into(),
                location,
            )?
        } else {
            dst_value
        };

        helper.cond_br(
            context,
            entry,
            is_in_bounds,
            [0, 1],
            [&[range_check, dst_value], &[range_check]],
            location,
        )?;
    }

    Ok(())
}

/// Builds the `upcast` libfunc, which converts from a source type `T` to a
/// target type `U`, where `U` fully includes `T`. This means that the operation
/// cannot fail.
///
/// ## Signature
///
/// ```cairo
/// extern const fn upcast<FromType, ToType>(x: FromType) -> ToType nopanic;
/// ```
pub fn build_upcast<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let src_value = entry.arg(0)?;

    if info.signature.param_signatures[0].ty == info.signature.branch_signatures[0].vars[0].ty {
        return helper.br(entry, 0, &[src_value], location);
    }

    let src_ty = registry.get_type(&info.signature.param_signatures[0].ty)?;
    let dst_ty = registry.get_type(&info.signature.branch_signatures[0].vars[0].ty)?;

    let src_range = src_ty.integer_range(registry)?;
    let dst_range = dst_ty.integer_range(registry)?;

    // An upcast is infallible, so the target type should always contain the source type.
    {
        let dst_contains_src =
            dst_range.lower <= src_range.lower && dst_range.upper >= src_range.upper;

        // If the target type is a felt, then both [0; P) and [-P/2, P/2] ranges are valid.
        let dst_contains_src = if dst_ty.is_felt252(registry)? {
            let signed_dst_range = Range {
                lower: BigInt::from_biguint(Sign::Minus, HALF_PRIME.clone()),
                upper: BigInt::from_biguint(Sign::Plus, HALF_PRIME.clone()) + BigInt::one(),
            };
            let signed_dst_contains_src = signed_dst_range.lower <= src_range.lower
                && signed_dst_range.upper >= src_range.upper;
            dst_contains_src || signed_dst_contains_src
        } else {
            dst_contains_src
        };

        native_assert!(
            dst_contains_src,
            "cannot upcast `{:?}` into `{:?}`: target range doesn't contain source range",
            info.signature.param_signatures[0].ty,
            info.signature.branch_signatures[0].vars[0].ty
        );
    }

    let src_width = src_range.repr_bit_width();
    let dst_width = dst_range.repr_bit_width();

    // Extend value to target bit width.
    let dst_value = if dst_width > src_width {
        if src_ty.is_bounded_int(registry)? {
            // A bounded int is always represented as a positive integer,
            // because we store the offset to the lower bound.
            entry.extui(
                src_value,
                IntegerType::new(context, dst_width).into(),
                location,
            )?
        } else if src_range.lower.sign() == Sign::Minus {
            entry.extsi(
                src_value,
                IntegerType::new(context, dst_width).into(),
                location,
            )?
        } else {
            entry.extui(
                src_value,
                IntegerType::new(context, dst_width).into(),
                location,
            )?
        }
    } else {
        src_value
    };

    // When converting to/from bounded ints, we need to take into account the offset.
    let offset = if src_ty.is_bounded_int(registry)? && dst_ty.is_bounded_int(registry)? {
        &src_range.lower - &dst_range.lower
    } else if src_ty.is_bounded_int(registry)? {
        src_range.lower.clone()
    } else if dst_ty.is_bounded_int(registry)? {
        -dst_range.lower
    } else {
        BigInt::ZERO
    };
    let offset_value = entry.const_int_from_type(context, location, offset, dst_value.r#type())?;
    let dst_value = entry.addi(dst_value, offset_value, location)?;

    // When converting to a felt from a signed integer, we need to convert
    // the canonical signed integer representation, to the signed felt
    // representation: `negative = P - absolute`.
    let dst_value = if dst_ty.is_felt252(registry)? && src_range.lower.sign() == Sign::Minus {
        let k0 = entry.const_int(context, location, 0, 252)?;
        let is_negative = entry.cmpi(context, CmpiPredicate::Slt, dst_value, k0, location)?;

        let k_prime = entry.const_int(context, location, PRIME.clone(), 252)?;
        let adj_value = entry.addi(dst_value, k_prime, location)?;

        entry.append_op_result(arith::select(is_negative, adj_value, dst_value, location))?
    } else {
        dst_value
    };

    helper.br(entry, 0, &[dst_value], location)
}

#[cfg(test)]
mod test {
    use crate::{
        jit_enum, jit_struct,
        utils::testing::{get_compiled_program, run_program_assert_output},
        Value,
    };
    use starknet_types_core::felt::Felt;
    use test_case::test_case;

    #[test]
    fn downcast() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/cast_downcast");
        run_program_assert_output(
            &program,
            "run_test",
            &[
                u8::MAX.into(),
                u16::MAX.into(),
                u32::MAX.into(),
                u64::MAX.into(),
                u128::MAX.into(),
            ],
            jit_struct!(
                jit_struct!(
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(0, u8::MAX.into()),
                ),
                jit_struct!(
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(0, u16::MAX.into()),
                ),
                jit_struct!(
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(1, jit_struct!()),
                    jit_enum!(0, u32::MAX.into()),
                ),
                jit_struct!(jit_enum!(1, jit_struct!()), jit_enum!(0, u64::MAX.into())),
                jit_struct!(jit_enum!(0, u128::MAX.into())),
            ),
        );
    }

    #[test_case("b0x30_b0x30", 5.into())]
    #[test_case("bm31x30_b31x30", 5.into())]
    #[test_case("bm31x30_bm5x30", (-5).into())]
    #[test_case("bm31x30_b5x30", 30.into())]
    #[test_case("b5x30_b31x31", 31.into())]
    #[test_case("bm100x100_bm100xm1", (-90).into())]
    #[test_case("bm31xm31_bm31xm31", (-31).into())]
    #[test_case("b0x30_b5x40", 10.into())]
    #[test_case("b0x30_bm40x40", 10.into())]
    fn downcast_bounded_int(entry_point: &str, value: Felt) {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/cast_downcast_bounded_int");
        run_program_assert_output(
            &program,
            entry_point,
            &[Value::Felt252(value)],
            jit_enum!(0, jit_struct!(Value::Felt252(value))),
        );
    }

    #[test_case("felt252_i8", i8::MAX.into())]
    #[test_case("felt252_i8", i8::MIN.into())]
    #[test_case("felt252_i16", i16::MAX.into())]
    #[test_case("felt252_i16", i16::MIN.into())]
    #[test_case("felt252_i32", i32::MAX.into())]
    #[test_case("felt252_i32", i32::MIN.into())]
    #[test_case("felt252_i64", i64::MAX.into())]
    #[test_case("felt252_i64", i64::MIN.into())]
    fn downcast_felt(entry_point: &str, value: Felt) {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/cast_downcast_felt");
        run_program_assert_output(
            &program,
            entry_point,
            &[Value::Felt252(value)],
            jit_enum!(0, jit_struct!(Value::Felt252(value))),
        );
    }

    // u8 upcast test
    #[test_case("u8_u16", u8::MIN.into())]
    #[test_case("u8_u16", u8::MAX.into())]
    #[test_case("u8_u32", u8::MIN.into())]
    #[test_case("u8_u32", u8::MAX.into())]
    #[test_case("u8_u64", u8::MIN.into())]
    #[test_case("u8_u64", u8::MAX.into())]
    #[test_case("u8_u128", u8::MIN.into())]
    #[test_case("u8_u128", u8::MAX.into())]
    #[test_case("u8_felt252", u8::MIN.into())]
    #[test_case("u8_felt252", u8::MAX.into())]
    // u16 upcast test
    #[test_case("u16_u32", u16::MIN.into())]
    #[test_case("u16_u32", u16::MAX.into())]
    #[test_case("u16_u64", u16::MIN.into())]
    #[test_case("u16_u64", u16::MAX.into())]
    #[test_case("u16_u128", u16::MIN.into())]
    #[test_case("u16_u128", u16::MAX.into())]
    #[test_case("u16_felt252", u16::MIN.into())]
    #[test_case("u16_felt252", u16::MAX.into())]
    // u32 upcast test
    #[test_case("u32_u64", u32::MIN.into())]
    #[test_case("u32_u64", u32::MAX.into())]
    #[test_case("u32_u128", u32::MIN.into())]
    #[test_case("u32_u128", u32::MAX.into())]
    #[test_case("u32_felt252", u32::MIN.into())]
    #[test_case("u32_felt252", u32::MAX.into())]
    // u64 upcast test
    #[test_case("u64_u128", u64::MIN.into())]
    #[test_case("u64_u128", u64::MAX.into())]
    #[test_case("u64_felt252", u64::MIN.into())]
    #[test_case("u64_felt252", u64::MAX.into())]
    // u128 upcast test
    #[test_case("u128_felt252", u128::MIN.into())]
    #[test_case("u128_felt252", u128::MAX.into())]
    // i8 upcast test
    #[test_case("i8_i16", i8::MIN.into())]
    #[test_case("i8_i16", i8::MAX.into())]
    #[test_case("i8_i32", i8::MIN.into())]
    #[test_case("i8_i32", i8::MAX.into())]
    #[test_case("i8_i64", i8::MIN.into())]
    #[test_case("i8_i64", i8::MAX.into())]
    #[test_case("i8_i128", i8::MIN.into())]
    #[test_case("i8_i128", i8::MAX.into())]
    #[test_case("i8_felt252", i8::MIN.into())]
    #[test_case("i8_felt252", i8::MAX.into())]
    // i16 upcast test
    #[test_case("i16_i32", i16::MIN.into())]
    #[test_case("i16_i32", i16::MAX.into())]
    #[test_case("i16_i64", i16::MIN.into())]
    #[test_case("i16_i64", i16::MAX.into())]
    #[test_case("i16_i128", i16::MIN.into())]
    #[test_case("i16_i128", i16::MAX.into())]
    #[test_case("i16_felt252", i16::MIN.into())]
    #[test_case("i16_felt252", i16::MAX.into())]
    // i32 upcast test
    #[test_case("i32_i64", i32::MIN.into())]
    #[test_case("i32_i64", i32::MAX.into())]
    #[test_case("i32_i128", i32::MIN.into())]
    #[test_case("i32_i128", i32::MAX.into())]
    #[test_case("i32_felt252", i32::MIN.into())]
    #[test_case("i32_felt252", i32::MAX.into())]
    // i64 upcast test
    #[test_case("i64_i128", i64::MIN.into())]
    #[test_case("i64_i128", i64::MAX.into())]
    #[test_case("i64_felt252", i64::MIN.into())]
    #[test_case("i64_felt252", i64::MAX.into())]
    // i128 upcast test
    #[test_case("i128_felt252", i128::MIN.into())]
    #[test_case("i128_felt252", i128::MAX.into())]
    // bounded int test
    #[test_case("b0x5_b0x10", 0.into())]
    #[test_case("b0x5_b0x10", 5.into())]
    #[test_case("b2x5_b2x10", 2.into())]
    #[test_case("b2x5_b2x10", 5.into())]
    #[test_case("b2x5_b1x10", 2.into())]
    #[test_case("b2x5_b1x10", 5.into())]
    #[test_case("b0x5_bm10x10", 0.into())]
    #[test_case("b0x5_bm10x10", 5.into())]
    #[test_case("bm5x5_bm10x10", Felt::from(-5))]
    #[test_case("bm5x5_bm10x10", 5.into())]
    #[test_case("i8_bm200x200", Felt::from(-128))]
    #[test_case("i8_bm200x200", 127.into())]
    #[test_case("bm100x100_i8", Felt::from(-100))]
    #[test_case("bm100x100_i8", 100.into())]
    fn upcast(entry_point: &str, value: Felt) {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/cast_upcast");
        let arguments = &[value.into()];
        let expected_result = jit_enum!(0, jit_struct!(value.into(),));
        run_program_assert_output(&program, entry_point, arguments, expected_result);
    }
}
