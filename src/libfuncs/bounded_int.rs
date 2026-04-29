//! # Bounded int libfuncs

use super::LibfuncHelper;
use crate::{
    error::{panic::ToNativeAssertError, Result},
    execution_result::RANGE_CHECK_BUILTIN_SIZE,
    metadata::MetadataStorage,
    native_assert,
    types::TypeBuilder,
    utils::RangeExt,
};
use cairo_lang_sierra::{
    extensions::{
        bounded_int::{
            BoundedIntConcreteLibfunc, BoundedIntConstrainConcreteLibfunc,
            BoundedIntDivRemAlgorithm, BoundedIntDivRemConcreteLibfunc,
            BoundedIntTrimConcreteLibfunc,
        },
        core::{CoreLibfunc, CoreType},
        lib_func::SignatureOnlyConcreteLibfunc,
        utils::Range,
        ConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{
        arith::{self, CmpiPredicate},
        cf,
    },
    helpers::{ArithBlockExt, BuiltinBlockExt},
    ir::{r#type::IntegerType, Block, BlockLike, Location, Value, ValueLike},
    Context,
};
use num_bigint::{BigInt, Sign};

/// Select and call the correct libfunc builder function from the selector.
pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &BoundedIntConcreteLibfunc,
) -> Result<()> {
    match selector {
        BoundedIntConcreteLibfunc::Add(info) => {
            build_add(context, registry, entry, location, helper, metadata, info)
        }
        BoundedIntConcreteLibfunc::Sub(info) => {
            build_sub(context, registry, entry, location, helper, metadata, info)
        }
        BoundedIntConcreteLibfunc::Mul(info) => {
            build_mul(context, registry, entry, location, helper, metadata, info)
        }
        BoundedIntConcreteLibfunc::DivRem(info) => {
            build_div_rem(context, registry, entry, location, helper, metadata, info)
        }
        BoundedIntConcreteLibfunc::Constrain(info) => {
            build_constrain(context, registry, entry, location, helper, metadata, info)
        }
        BoundedIntConcreteLibfunc::TrimMin(info) | BoundedIntConcreteLibfunc::TrimMax(info) => {
            build_trim(context, registry, entry, location, helper, metadata, info)
        }
        BoundedIntConcreteLibfunc::IsZero(info) => {
            build_is_zero(context, registry, entry, location, helper, metadata, info)
        }
        BoundedIntConcreteLibfunc::WrapNonZero(info) => {
            build_wrap_non_zero(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `bounded_int_add` libfunc.
///
/// # Cairo Signature
///
/// ```cairo
/// extern fn bounded_int_add<Lhs, Rhs, impl H: AddHelper<Lhs, Rhs>>(
///    lhs: Lhs, rhs: Rhs,
/// ) -> H::Result nopanic;
/// ```
///
/// A number X as a `BoundedInt` is internally represented as an offset Xd from the lower bound Xo.
/// So X = Xo + Xd.
///
/// Since we want to get C = A + B, we can translate this to
/// Co + Cd = Ao + Ad + Bo + Bd. Where Ao, Bo and Co represent the lower bound
/// of the ranges in the `BoundedInt` and Ad, Bd and Cd represent the offsets. Since
/// we also know that Co = Ao + Bo we can simplify the equation to Cd = Ad + Bd.
#[allow(clippy::too_many_arguments)]
fn build_add<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let lhs_value = entry.arg(0)?;
    let rhs_value = entry.arg(1)?;

    // Extract the ranges for the operands.
    let lhs_ty = registry.get_type(&info.signature.param_signatures[0].ty)?;
    let rhs_ty = registry.get_type(&info.signature.param_signatures[1].ty)?;

    let lhs_range = lhs_ty.integer_range(registry)?;
    let rhs_range = rhs_ty.integer_range(registry)?;
    let dst_range = registry
        .get_type(&info.signature.branch_signatures[0].vars[0].ty)?
        .integer_range(registry)?;

    // Extract the bit width.
    let lhs_width = if lhs_ty.is_bounded_int(registry)? {
        lhs_range.offset_bit_width()
    } else {
        lhs_range.zero_based_bit_width()
    };
    let rhs_width = if rhs_ty.is_bounded_int(registry)? {
        rhs_range.offset_bit_width()
    } else {
        rhs_range.zero_based_bit_width()
    };
    let dst_width = dst_range.offset_bit_width();

    // Get the compute type so we can do the addition without problems
    let compute_width = lhs_width.max(rhs_width) + 1;
    let compute_ty = IntegerType::new(context, compute_width).into();

    // Get the operands on the same number of bits so we can operate with them
    let lhs_value = if compute_width > lhs_width {
        if lhs_range.lower.sign() != Sign::Minus || lhs_ty.is_bounded_int(registry)? {
            entry.extui(lhs_value, compute_ty, location)?
        } else {
            entry.extsi(lhs_value, compute_ty, location)?
        }
    } else {
        lhs_value
    };
    let rhs_value = if compute_width > rhs_width {
        if rhs_range.lower.sign() != Sign::Minus || rhs_ty.is_bounded_int(registry)? {
            entry.extui(rhs_value, compute_ty, location)?
        } else {
            entry.extsi(rhs_value, compute_ty, location)?
        }
    } else {
        rhs_value
    };

    // Addition and get the result value on the desired range
    let res_value = entry.addi(lhs_value, rhs_value, location)?;
    let res_value = if compute_width > dst_width {
        entry.trunci(
            res_value,
            IntegerType::new(context, dst_width).into(),
            location,
        )?
    } else if compute_width < dst_width {
        entry.extui(
            res_value,
            IntegerType::new(context, dst_width).into(),
            location,
        )?
    } else {
        res_value
    };

    helper.br(entry, 0, &[res_value], location)
}

/// Generate MLIR operations for the `bounded_int_sub` libfunc.
///
/// # Cairo Signature
/// ```cairo
/// extern fn bounded_int_sub<Lhs, Rhs, impl H: SubHelper<Lhs, Rhs>>(
///    lhs: Lhs, rhs: Rhs,
/// ) -> H::Result nopanic;
/// ```
///
/// A number X as a `BoundedInt` is internally represented as an offset Xd from the lower bound Xo.
/// So X = Xo + Xd.
///
/// Since we want to get C = A - B, we can translate this to
/// Co + Cd = (Ao + Ad) - (Bo + Bd). Where Ao, Bo and Co represent the lower bound
/// of the ranges in the `BoundedInt` and Ad, Bd and Cd represent the offsets.
#[allow(clippy::too_many_arguments)]
fn build_sub<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let lhs_value = entry.arg(0)?;
    let rhs_value = entry.arg(1)?;

    // Extract the ranges for the operands.
    let lhs_ty = registry.get_type(&info.signature.param_signatures[0].ty)?;
    let rhs_ty = registry.get_type(&info.signature.param_signatures[1].ty)?;

    let lhs_range = lhs_ty.integer_range(registry)?;
    let rhs_range = rhs_ty.integer_range(registry)?;
    let dst_range = registry
        .get_type(&info.signature.branch_signatures[0].vars[0].ty)?
        .integer_range(registry)?;

    // Extract the bit width.
    let lhs_width = if lhs_ty.is_bounded_int(registry)? {
        lhs_range.offset_bit_width()
    } else {
        lhs_range.zero_based_bit_width()
    };
    let rhs_width = if rhs_ty.is_bounded_int(registry)? {
        rhs_range.offset_bit_width()
    } else {
        rhs_range.zero_based_bit_width()
    };
    let dst_width = dst_range.offset_bit_width();

    // Get the compute type so we can do the subtraction without problems
    let compile_time_val = lhs_range.lower.clone() - rhs_range.lower.clone() - dst_range.lower;
    let compile_time_val_width = u32::try_from(compile_time_val.bits())?;

    let compute_width = lhs_width.max(rhs_width).max(compile_time_val_width) + 1;
    let compute_ty = IntegerType::new(context, compute_width).into();

    // Get the operands on the same number of bits so we can operate with them
    let lhs_value = if compute_width > lhs_width {
        if lhs_range.lower.sign() != Sign::Minus || lhs_ty.is_bounded_int(registry)? {
            entry.extui(lhs_value, compute_ty, location)?
        } else {
            entry.extsi(lhs_value, compute_ty, location)?
        }
    } else {
        lhs_value
    };
    let rhs_value = if compute_width > rhs_width {
        if rhs_range.lower.sign() != Sign::Minus || rhs_ty.is_bounded_int(registry)? {
            entry.extui(rhs_value, compute_ty, location)?
        } else {
            entry.extsi(rhs_value, compute_ty, location)?
        }
    } else {
        rhs_value
    };

    let compile_time_val =
        entry.const_int_from_type(context, location, compile_time_val, compute_ty)?;
    // First we do -> intermediate_res = Ad - Bd
    let res_value = entry.subi(lhs_value, rhs_value, location)?;
    // Then we do -> intermediate_res += (Ao - Bo - Co)
    let res_value = entry.addi(res_value, compile_time_val, location)?;
    // Get the result value on the desired range
    let res_value = if compute_width > dst_width {
        entry.trunci(
            res_value,
            IntegerType::new(context, dst_width).into(),
            location,
        )?
    } else if compute_width < dst_width {
        entry.extui(
            res_value,
            IntegerType::new(context, dst_width).into(),
            location,
        )?
    } else {
        res_value
    };

    helper.br(entry, 0, &[res_value], location)
}

/// Generate MLIR operations for the `bounded_int_mul` libfunc.
#[allow(clippy::too_many_arguments)]
fn build_mul<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let lhs_value = entry.arg(0)?;
    let rhs_value = entry.arg(1)?;

    // Extract the ranges for the operands and the result type.
    let lhs_ty = registry.get_type(&info.signature.param_signatures[0].ty)?;
    let rhs_ty = registry.get_type(&info.signature.param_signatures[1].ty)?;

    let lhs_range = lhs_ty.integer_range(registry)?;
    let rhs_range = rhs_ty.integer_range(registry)?;
    let dst_range = registry
        .get_type(&info.signature.branch_signatures[0].vars[0].ty)?
        .integer_range(registry)?;

    let lhs_width = if lhs_ty.is_bounded_int(registry)? {
        lhs_range.offset_bit_width()
    } else {
        lhs_range.zero_based_bit_width()
    };
    let rhs_width = if rhs_ty.is_bounded_int(registry)? {
        rhs_range.offset_bit_width()
    } else {
        rhs_range.zero_based_bit_width()
    };

    // Calculate the computation range.
    let compute_range = Range {
        lower: (&lhs_range.lower)
            .min(&rhs_range.lower)
            .min(&dst_range.lower)
            .min(&BigInt::ZERO)
            .clone(),
        upper: (&lhs_range.upper)
            .max(&rhs_range.upper)
            .max(&dst_range.upper)
            .clone(),
    };
    let compute_ty = IntegerType::new(context, compute_range.zero_based_bit_width()).into();

    // Zero-extend operands into the computation range.
    native_assert!(
        compute_range.offset_bit_width() >= lhs_width,
        "the lhs_range bit_width must be less or equal than the compute_range"
    );
    native_assert!(
        compute_range.offset_bit_width() >= rhs_width,
        "the rhs_range bit_width must be less or equal than the compute_range"
    );

    let lhs_value = if compute_range.zero_based_bit_width() > lhs_width {
        if lhs_range.lower.sign() != Sign::Minus || lhs_ty.is_bounded_int(registry)? {
            entry.extui(lhs_value, compute_ty, location)?
        } else {
            entry.extsi(lhs_value, compute_ty, location)?
        }
    } else {
        lhs_value
    };
    let rhs_value = if compute_range.zero_based_bit_width() > rhs_width {
        if rhs_range.lower.sign() != Sign::Minus || rhs_ty.is_bounded_int(registry)? {
            entry.extui(rhs_value, compute_ty, location)?
        } else {
            entry.extsi(rhs_value, compute_ty, location)?
        }
    } else {
        rhs_value
    };

    // Offset the operands so that they are compatible with the operation.
    let lhs_value = if lhs_ty.is_bounded_int(registry)? && lhs_range.lower != BigInt::ZERO {
        let lhs_offset =
            entry.const_int_from_type(context, location, lhs_range.lower, compute_ty)?;
        entry.addi(lhs_value, lhs_offset, location)?
    } else {
        lhs_value
    };
    let rhs_value = if rhs_ty.is_bounded_int(registry)? && rhs_range.lower != BigInt::ZERO {
        let rhs_offset =
            entry.const_int_from_type(context, location, rhs_range.lower, compute_ty)?;
        entry.addi(rhs_value, rhs_offset, location)?
    } else {
        rhs_value
    };

    // Compute the operation.
    let res_value = entry.muli(lhs_value, rhs_value, location)?;

    // Offset and truncate the result to the output type.
    let res_offset = dst_range.lower.clone();
    let res_value = if res_offset != BigInt::ZERO {
        let res_offset = entry.const_int_from_type(context, location, res_offset, compute_ty)?;
        entry.append_op_result(arith::subi(res_value, res_offset, location))?
    } else {
        res_value
    };

    let res_value = if dst_range.offset_bit_width() < compute_range.zero_based_bit_width() {
        entry.trunci(
            res_value,
            IntegerType::new(context, dst_range.offset_bit_width()).into(),
            location,
        )?
    } else {
        res_value
    };

    helper.br(entry, 0, &[res_value], location)
}

/// Builds the `bounded_int_div_rem` libfunc, which divides a non negative
/// integer by a positive integer (non zero), returning the quotient and
/// the remainder as bounded ints.
///
/// # Signature
///
/// ```cairo
/// extern fn bounded_int_div_rem<Lhs, Rhs, impl H: DivRemHelper<Lhs, Rhs>>(
///     lhs: Lhs, rhs: NonZero<Rhs>,
/// ) -> (H::DivT, H::RemT) implicits(RangeCheck) nopanic;
/// ```
///
/// The input arguments can be both regular integers or bounded ints.
fn build_div_rem<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &BoundedIntDivRemConcreteLibfunc,
) -> Result<()> {
    let lhs_value = entry.arg(1)?;
    let rhs_value = entry.arg(2)?;

    // Extract the ranges for the operands and the result type.
    let lhs_ty = registry.get_type(&info.param_signatures()[1].ty)?;
    let rhs_ty = registry.get_type(&info.param_signatures()[2].ty)?;

    let lhs_range = lhs_ty.integer_range(registry)?;
    let rhs_range = rhs_ty.integer_range(registry)?;
    let div_range = registry
        .get_type(&info.branch_signatures()[0].vars[1].ty)?
        .integer_range(registry)?;
    let rem_range = registry
        .get_type(&info.branch_signatures()[0].vars[2].ty)?
        .integer_range(registry)?;

    let lhs_width = if lhs_ty.is_bounded_int(registry)? {
        lhs_range.offset_bit_width()
    } else {
        lhs_range.zero_based_bit_width()
    };
    let rhs_width = if rhs_ty.is_bounded_int(registry)? {
        rhs_range.offset_bit_width()
    } else {
        rhs_range.zero_based_bit_width()
    };

    let div_rem_algorithm = BoundedIntDivRemAlgorithm::try_new(&lhs_range, &rhs_range)
        .to_native_assert_error(&format!(
            "div_rem of ranges: lhs = {:#?} and rhs= {:#?} is not supported yet",
            &lhs_range, &rhs_range
        ))?;

    // Calculate the computation range.
    let compute_range = Range {
        lower: BigInt::ZERO,
        upper: (&lhs_range.upper).max(&rhs_range.upper).clone(),
    };
    let compute_ty = IntegerType::new(context, compute_range.zero_based_bit_width()).into();

    // Zero-extend operands into the computation range.
    native_assert!(
        compute_range.offset_bit_width() >= lhs_width,
        "the lhs_range bit_width must be less or equal than the compute_range"
    );
    native_assert!(
        compute_range.offset_bit_width() >= rhs_width,
        "the rhs_range bit_width must be less or equal than the compute_range"
    );

    let lhs_value = if compute_range.zero_based_bit_width() > lhs_width {
        if lhs_range.lower.sign() != Sign::Minus || lhs_ty.is_bounded_int(registry)? {
            entry.extui(lhs_value, compute_ty, location)?
        } else {
            entry.extsi(lhs_value, compute_ty, location)?
        }
    } else {
        lhs_value
    };
    let rhs_value = if compute_range.zero_based_bit_width() > rhs_width {
        if rhs_range.lower.sign() != Sign::Minus || rhs_ty.is_bounded_int(registry)? {
            entry.extui(rhs_value, compute_ty, location)?
        } else {
            entry.extsi(rhs_value, compute_ty, location)?
        }
    } else {
        rhs_value
    };

    // Offset the operands so that they are compatible with the operation.
    let lhs_value = if lhs_ty.is_bounded_int(registry)? && lhs_range.lower != BigInt::ZERO {
        let lhs_offset =
            entry.const_int_from_type(context, location, lhs_range.lower, compute_ty)?;
        entry.addi(lhs_value, lhs_offset, location)?
    } else {
        lhs_value
    };
    let rhs_value = if rhs_ty.is_bounded_int(registry)? && rhs_range.lower != BigInt::ZERO {
        let rhs_offset =
            entry.const_int_from_type(context, location, rhs_range.lower, compute_ty)?;
        entry.addi(rhs_value, rhs_offset, location)?
    } else {
        rhs_value
    };

    // Compute the operation.
    let div_value = entry.append_op_result(arith::divui(lhs_value, rhs_value, location))?;
    let rem_value = entry.append_op_result(arith::remui(lhs_value, rhs_value, location))?;

    // Offset result to the output type.
    let div_value = if div_range.lower.clone() != BigInt::ZERO {
        let div_offset =
            entry.const_int_from_type(context, location, div_range.lower.clone(), compute_ty)?;
        entry.append_op_result(arith::subi(div_value, div_offset, location))?
    } else {
        div_value
    };

    native_assert!(
        rem_range.lower == BigInt::ZERO,
        "The remainder range lower bound should be zero"
    );

    // Truncate to the output type
    let div_value = if div_range.offset_bit_width() < compute_range.zero_based_bit_width() {
        entry.trunci(
            div_value,
            IntegerType::new(context, div_range.offset_bit_width()).into(),
            location,
        )?
    } else {
        div_value
    };
    let rem_value = if rem_range.offset_bit_width() < compute_range.zero_based_bit_width() {
        entry.trunci(
            rem_value,
            IntegerType::new(context, rem_range.offset_bit_width()).into(),
            location,
        )?
    } else {
        rem_value
    };

    // Increase range check builtin by 3, regardless of `div_rem_algorithm`:
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/int/bounded.rs#L100
    let range_check = match div_rem_algorithm {
        BoundedIntDivRemAlgorithm::KnownSmallRhs => crate::libfuncs::increment_builtin_counter_by(
            context,
            entry,
            location,
            entry.arg(0)?,
            3 * RANGE_CHECK_BUILTIN_SIZE,
        )?,
        BoundedIntDivRemAlgorithm::KnownSmallQuotient { .. }
        | BoundedIntDivRemAlgorithm::KnownSmallLhs { .. } => {
            // If `div_rem_algorithm` is `KnownSmallQuotient` or `KnownSmallLhs`, increase range check builtin by 1.
            //
            // Case KnownSmallQuotient: https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/int/bounded.rs#L129
            // Case KnownSmallLhs: https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/int/bounded.rs#L157
            crate::libfuncs::increment_builtin_counter_by(
                context,
                entry,
                location,
                entry.arg(0)?,
                4 * RANGE_CHECK_BUILTIN_SIZE,
            )?
        }
    };

    helper.br(entry, 0, &[range_check, div_value, rem_value], location)
}

/// Generate MLIR operations for the `bounded_int_constrain` libfunc.
fn build_constrain<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &BoundedIntConstrainConcreteLibfunc,
) -> Result<()> {
    let range_check = super::increment_builtin_counter(context, entry, location, entry.arg(0)?)?;
    let src_value: Value = entry.arg(1)?;

    let src_ty = registry.get_type(&info.param_signatures()[1].ty)?;
    let src_range = src_ty.integer_range(registry)?;

    let src_width = if src_ty.is_bounded_int(registry)? {
        src_range.offset_bit_width()
    } else {
        src_range.zero_based_bit_width()
    };

    let lower_range = registry
        .get_type(&info.branch_signatures()[0].vars[1].ty)?
        .integer_range(registry)?;
    let upper_range = registry
        .get_type(&info.branch_signatures()[1].vars[1].ty)?
        .integer_range(registry)?;

    let boundary = if src_ty.is_bounded_int(registry)? {
        entry.const_int_from_type(
            context,
            location,
            info.boundary.clone() - src_range.lower.clone(),
            src_value.r#type(),
        )?
    } else {
        entry.const_int_from_type(context, location, info.boundary.clone(), src_value.r#type())?
    };

    let cmpi_predicate =
        if src_ty.is_bounded_int(registry)? || src_range.lower.sign() != Sign::Minus {
            CmpiPredicate::Ult
        } else {
            CmpiPredicate::Slt
        };
    let is_lower = entry.cmpi(context, cmpi_predicate, src_value, boundary, location)?;

    let lower_block = helper.append_block(Block::new(&[]));
    let upper_block = helper.append_block(Block::new(&[]));
    entry.append_operation(cf::cond_br(
        context,
        is_lower,
        lower_block,
        upper_block,
        &[],
        &[],
        location,
    ));

    {
        let res_value = if src_range.lower != lower_range.lower {
            let lower_offset = &lower_range.lower - &src_range.lower;
            let lower_offset = lower_block.const_int_from_type(
                context,
                location,
                lower_offset,
                src_value.r#type(),
            )?;
            lower_block.append_op_result(arith::subi(src_value, lower_offset, location))?
        } else {
            src_value
        };

        let res_value = if src_width > lower_range.offset_bit_width() {
            lower_block.trunci(
                res_value,
                IntegerType::new(context, lower_range.offset_bit_width()).into(),
                location,
            )?
        } else {
            res_value
        };

        helper.br(lower_block, 0, &[range_check, res_value], location)?;
    }

    {
        let res_value = if src_range.lower != upper_range.lower {
            let upper_offset = &upper_range.lower - &src_range.lower;
            let upper_offset = upper_block.const_int_from_type(
                context,
                location,
                upper_offset,
                src_value.r#type(),
            )?;
            upper_block.append_op_result(arith::subi(src_value, upper_offset, location))?
        } else {
            src_value
        };

        let res_value = if src_width > upper_range.offset_bit_width() {
            upper_block.trunci(
                res_value,
                IntegerType::new(context, upper_range.offset_bit_width()).into(),
                location,
            )?
        } else {
            res_value
        };

        helper.br(upper_block, 1, &[range_check, res_value], location)?;
    }

    Ok(())
}

/// Makes a downcast of a type `T` to `BoundedInt<T::MIN, T::MAX - 1>`
/// or `BoundedInt<T::MIN + 1, T::MAX>` where `T` can be any type of signed
/// or unsigned integer.
///
/// ```cairo
/// extern fn bounded_int_trim<T, const TRIMMED_VALUE: felt252, impl H: TrimHelper<T, TRIMMED_VALUE>>(
///     value: T,
/// ) -> core::internal::OptionRev<H::Target> nopanic;
/// ```
fn build_trim<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &BoundedIntTrimConcreteLibfunc,
) -> Result<()> {
    let value: Value = entry.arg(0)?;

    let src_ty = registry.get_type(&info.param_signatures()[0].ty)?;
    let dst_ty = registry.get_type(&info.branch_signatures()[1].vars[0].ty)?;

    let trimmed_value = if src_ty.is_bounded_int(registry)? {
        entry.const_int_from_type(
            context,
            location,
            info.trimmed_value.clone() - src_ty.integer_range(registry)?.lower,
            value.r#type(),
        )?
    } else {
        entry.const_int_from_type(
            context,
            location,
            info.trimmed_value.clone(),
            value.r#type(),
        )?
    };
    let is_invalid = entry.cmpi(context, CmpiPredicate::Eq, value, trimmed_value, location)?;

    let offset = if src_ty.is_bounded_int(registry)? {
        dst_ty.integer_range(registry)?.lower - src_ty.integer_range(registry)?.lower
    } else {
        dst_ty.integer_range(registry)?.lower
    };
    let value = entry.append_op_result(arith::subi(
        value,
        entry.const_int_from_type(context, location, offset, value.r#type())?,
        location,
    ))?;
    let value = entry.trunci(
        value,
        dst_ty.build(
            context,
            helper,
            registry,
            metadata,
            &info.branch_signatures()[1].vars[0].ty,
        )?,
        location,
    )?;

    helper.cond_br(
        context,
        entry,
        is_invalid,
        [0, 1],
        [&[], &[value]],
        location,
    )
}

/// Generate MLIR operations for the `bounded_int_is_zero` libfunc.
fn build_is_zero<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let src_value: Value = entry.arg(0)?;

    let src_ty = registry.get_type(&info.signature.param_signatures[0].ty)?;
    let src_range = src_ty.integer_range(registry)?;

    native_assert!(
        src_range.lower <= BigInt::ZERO && BigInt::ZERO < src_range.upper,
        "value can never be zero"
    );

    let k0 = if src_ty.is_bounded_int(registry)? {
        // We can do the substraction since the lower bound of the bounded int will
        // always be less or equal than 0.
        entry.const_int_from_type(context, location, 0 - src_range.lower, src_value.r#type())?
    } else {
        entry.const_int_from_type(context, location, 0, src_value.r#type())?
    };
    let src_is_zero = entry.cmpi(context, CmpiPredicate::Eq, src_value, k0, location)?;

    helper.cond_br(
        context,
        entry,
        src_is_zero,
        [0, 1],
        [&[], &[src_value]],
        location,
    )
}

/// Generate MLIR operations for the `bounded_int_wrap_non_zero` libfunc.
fn build_wrap_non_zero<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let src_range = registry
        .get_type(&info.signature.param_signatures[0].ty)?
        .integer_range(registry)?;

    native_assert!(
        src_range.lower > BigInt::ZERO || BigInt::ZERO >= src_range.upper,
        "value must not be zero"
    );

    super::build_noop::<1, false>(
        context,
        registry,
        entry,
        location,
        helper,
        metadata,
        &info.signature.param_signatures,
    )
}

#[cfg(test)]
mod test {
    use starknet_types_core::felt::Felt as Felt252;
    use test_case::test_case;

    use crate::{
        jit_enum, jit_panic_byte_array, jit_struct,
        utils::testing::{get_compiled_program, run_program, run_program_assert_output},
        Value,
    };

    #[test_case("bi_m128x127_times_bi_m128x127", -128, -128, 16384)]
    #[test_case("bi_0x128_times_bi_0x128", 126, 128, 16128)]
    #[test_case("bi_1x31_times_bi_1x1", 31, 1, 31)]
    #[test_case("bi_m1x31_times_bi_m1xm1", 31, -1, -31)]
    #[test_case("bi_31x31_times_bi_1x1", 31, 1, 31)]
    #[test_case("bi_m100x0_times_bi_0x100", -100, 100, -10000)]
    #[test_case("bi_1x1_times_bi_1x1", 1, 1, 1)]
    #[test_case("bi_m5x5_times_ui_2", -3, 2, -6)]
    // `BoundedInt` ranges with non-power-of-two exclusive upper
    #[test_case("bi_m3x5_times_bi_m3x5", 5, 5, 25)]
    #[test_case("bi_m3x5_times_bi_m3x5", -3, 5, -15)]
    #[test_case("bi_m3x5_times_bi_m3x5", -3, -3, 9)]
    #[test_case("bi_m3x5_times_bi_m3x5", 5, -3, -15)]
    fn test_mul(entry_point: &str, lhs: i32, rhs: i32, expected_result: i32) {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/bounded_int_mul");
        let result = run_program(
            &program,
            entry_point,
            &[
                Value::Felt252(Felt252::from(lhs)),
                Value::Felt252(Felt252::from(rhs)),
            ],
        )
        .return_value;
        if let Value::Enum { value, .. } = result {
            if let Value::Struct { fields, .. } = *value {
                assert!(
                    matches!(fields[0], Value::BoundedInt { value, .. } if value == Felt252::from(expected_result))
                )
            } else {
                panic!("Test returned an unexpected value");
            }
        } else {
            panic!("Test didn't return an enum as expected");
        }
    }

    // test trim_min on i8
    #[test_case("test_i8_min", 0, None)]
    #[test_case("test_i8_min", 20, None)]
    #[test_case("test_i8_min", 127, None)]
    #[test_case("test_i8_min", -21, None)]
    #[test_case("test_i8_min", -128, Some("boundary"))]
    // test trim_max on i8
    #[test_case("test_i8_max", 0, None)]
    #[test_case("test_i8_max", 20, None)]
    #[test_case("test_i8_max", 127, Some("boundary"))]
    #[test_case("test_i8_max", -21, None)]
    #[test_case("test_i8_max", -128, None)]
    // test trim_min on u8
    #[test_case("test_u8_min", 0, Some("boundary"))]
    #[test_case("test_u8_min", 20, None)]
    #[test_case("test_u8_min", 255, None)]
    // test trim_max on u8
    #[test_case("test_u8_max", 20, None)]
    #[test_case("test_u8_max", 0, None)]
    #[test_case("test_u8_max", 255, Some("boundary"))]
    // test trim_min on BoundedInt<0, 100>
    #[test_case("test_0_100_min", 0, Some("boundary"))]
    #[test_case("test_0_100_min", 10, None)]
    #[test_case("test_0_100_min", 100, None)]
    // test trim_max on BoundedInt<0, 100>
    #[test_case("test_0_100_max", 0, None)]
    #[test_case("test_0_100_max", 10, None)]
    #[test_case("test_0_100_max", 100, Some("boundary"))]
    // test trim_min on BoundedInt<10, 100>
    #[test_case("test_10_100_min", 10, Some("boundary"))]
    #[test_case("test_10_100_min", 20, None)]
    #[test_case("test_10_100_min", 100, None)]
    // test trim_max on BoundedInt<10, 100>
    #[test_case("test_10_100_max", 10, None)]
    #[test_case("test_10_100_max", 20, None)]
    #[test_case("test_10_100_max", 100, Some("boundary"))]
    // test trim_min on BoundedInt<-100, 0>
    #[test_case("test_m100_0_min", 0, None)]
    #[test_case("test_m100_0_min", -10, None)]
    #[test_case("test_m100_0_min", -100, Some("boundary"))]
    // test trim_max on BoundedInt<-100, 0>
    #[test_case("test_m100_0_max", 0, Some("boundary"))]
    #[test_case("test_m100_0_max", -10, None)]
    #[test_case("test_m100_0_max", -100, None)]
    // test trim_min on BoundedInt<-100, -10>
    #[test_case("test_m100_m10_min", -10, None)]
    #[test_case("test_m100_m10_min", -50, None)]
    #[test_case("test_m100_m10_min", -100, Some("boundary"))]
    // test trim_max on BoundedInt<-100, -10>
    #[test_case("test_m100_m10_max", -10, Some("boundary"))]
    #[test_case("test_m100_m10_max", -50, None)]
    #[test_case("test_m100_m10_max", -100, None)]
    // test trim_min on BoundedInt<-100, 100>
    #[test_case("test_m100_100_min", -100, Some("boundary"))]
    #[test_case("test_m100_100_min", -51, None)]
    #[test_case("test_m100_100_min", 0, None)]
    #[test_case("test_m100_100_min", 50, None)]
    #[test_case("test_m100_100_min", 100, None)]
    // test trim_max on BoundedInt<-100, 100>
    #[test_case("test_m100_100_max", -100, None)]
    #[test_case("test_m100_100_max", -51, None)]
    #[test_case("test_m100_100_max", 0, None)]
    #[test_case("test_m100_100_max", 50, None)]
    #[test_case("test_m100_100_max", 100, Some("boundary"))]
    // test trim_min on BoundedInt<0, 8>
    #[test_case("test_0_8_min", 0, Some("boundary"))]
    #[test_case("test_0_8_min", 4, None)]
    #[test_case("test_0_8_min", 8, None)]
    // test trim_max on BoundedInt<0, 8>
    #[test_case("test_0_8_max", 0, None)]
    #[test_case("test_0_8_max", 4, None)]
    #[test_case("test_0_8_max", 8, Some("boundary"))]
    fn test_trim(entry_point: &str, argument: i32, expected_error: Option<&str>) {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/bounded_int_trim");
        let arguments = &[Felt252::from(argument).into()];
        let expected_result = match expected_error {
            Some(error_message) => jit_panic_byte_array!(error_message),
            None => jit_enum!(0, jit_struct!(jit_struct!())),
        };
        run_program_assert_output(&program, entry_point, arguments, expected_result);
    }

    #[test_case("bi_1x1_minus_bi_1x5", 1, 5, -4)]
    #[test_case("bi_1x1_minus_bi_1x1", 1, 1, 0)]
    #[test_case("bi_m3xm3_minus_bi_m3xm3", -3, -3, 0)]
    #[test_case("bi_m6xm3_minus_bi_1x3", -6, 3, -9)]
    #[test_case("bi_m6xm2_minus_bi_m20xm10", -2, -20, 18)]
    fn test_sub(entry_point: &str, lhs: i32, rhs: i32, expected_result: i32) {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/bounded_int_sub");
        let result = run_program(
            &program,
            entry_point,
            &[
                Value::Felt252(Felt252::from(lhs)),
                Value::Felt252(Felt252::from(rhs)),
            ],
        )
        .return_value;
        if let Value::Enum { value, .. } = result {
            if let Value::Struct { fields, .. } = *value {
                assert!(
                    matches!(fields[0], Value::BoundedInt { value, .. } if value == Felt252::from(expected_result))
                )
            } else {
                panic!("Test returned an unexpected value");
            }
        } else {
            panic!("Test didn't return an enum as expected");
        }
    }

    #[test_case("bi_1x31_plus_bi_1x1", 31, 1, 32)]
    #[test_case("bi_1x31_plus_bi_m1xm1", 31, -1, 30)]
    #[test_case("bi_0x30_plus_bi_0x10", 30, 10, 40)]
    #[test_case("bi_m20xm15_plus_bi_0x10", -15, 10, -5)]
    #[test_case("bi_m20xm15_plus_bi_0x10", -20, 10, -10)]
    #[test_case("bi_m5xm5_plus_bi_m5xm5", -5, -5, -10)]
    #[test_case("bi_m5xm5_plus_ui_m1", -5, -1, -6)]
    #[test_case("ui_m1_plus_bi_m5xm5", 1, -5, -4)]
    fn test_add(entry_point: &str, lhs: i32, rhs: i32, expected_result: i32) {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/bounded_int_add");
        let result = run_program(
            &program,
            entry_point,
            &[
                Value::Felt252(Felt252::from(lhs)),
                Value::Felt252(Felt252::from(rhs)),
            ],
        )
        .return_value;

        if let Value::Enum { value, .. } = result {
            if let Value::Struct { fields, .. } = *value {
                assert!(
                    matches!(fields[0], Value::BoundedInt { value, .. } if value == Felt252::from(expected_result))
                )
            } else {
                panic!("Test returned an unexpected value");
            }
        } else {
            panic!("Test didn't return an enum as expected");
        }
    }

    fn assert_bool_output(result: Value, expected_tag: usize) {
        if let Value::Enum { tag, value, .. } = result {
            assert_eq!(tag, 0);
            if let Value::Struct { fields, .. } = *value {
                if let Value::Enum { tag, .. } = fields[0] {
                    assert_eq!(tag, expected_tag)
                }
            }
        }
    }

    #[test]
    fn test_is_zero() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/bounded_int_is_zero");

        let result =
            run_program(&program, "run_test_1", &[Value::Felt252(Felt252::from(0))]).return_value;
        assert_bool_output(result, 1);

        let result =
            run_program(&program, "run_test_1", &[Value::Felt252(Felt252::from(5))]).return_value;
        assert_bool_output(result, 0);

        let result =
            run_program(&program, "run_test_2", &[Value::Felt252(Felt252::from(0))]).return_value;
        assert_bool_output(result, 1);

        let result =
            run_program(&program, "run_test_2", &[Value::Felt252(Felt252::from(-5))]).return_value;
        assert_bool_output(result, 0);
    }

    #[test_case("constrain_bi_m128_127_lt_0", -1, -1)]
    #[test_case("constrain_bi_m128_127_gt_0", 1, 1)]
    #[test_case("constrain_bi_m128_127_gt_0", 0, 0)]
    #[test_case("constrain_bi_0_15_lt_5", 0, 0)]
    #[test_case("constrain_bi_0_15_gt_5", 15, 15)]
    #[test_case("constrain_bi_m10_10_lt_0", -5, -5)]
    #[test_case("constrain_bi_m10_10_gt_0", 5, 5)]
    #[test_case("constrain_bi_1_61_lt_31", 30, 30)]
    #[test_case("constrain_bi_1_61_gt_31", 31, 31)]
    #[test_case("constrain_bi_m200_m100_lt_m150", -200, -200)]
    #[test_case("constrain_bi_m200_m100_gt_m150", -150, -150)]
    #[test_case("constrain_bi_30_100_gt_100", 100, 100)]
    #[test_case("constrain_bi_m30_31_lt_0", -5, -5)]
    #[test_case("constrain_bi_m30_31_gt_0", 5, 5)]
    fn test_constrain(entry_point: &str, input: i32, expected_result: i32) {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/bounded_int_constrain");
        let result = run_program(
            &program,
            entry_point,
            &[Value::Felt252(Felt252::from(input))],
        )
        .return_value;
        if let Value::Enum { value, .. } = result {
            if let Value::Struct { fields, .. } = *value {
                assert!(
                    matches!(fields[0], Value::BoundedInt { value, .. } if value == Felt252::from(expected_result))
                )
            } else {
                panic!("Test returned an unexpected value");
            }
        } else {
            panic!("Test didn't return an enum as expected");
        }
    }

    #[test_case("test_u8", 100, 30, 3, 10)]
    #[test_case("test_10_100_10_40", 100, 30, 3, 10)]
    #[test_case("test_50_100_20_40", 100, 30, 3, 10)]
    fn test_div_rem(entry_point: &str, a: i32, b: i32, expected_q: u32, expected_r: u32) {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/bounded_int_div_rem");
        let arguments = &[Felt252::from(a).into(), Felt252::from(b).into()];
        let expected_result = jit_enum!(
            0,
            jit_struct!(jit_struct!(
                Felt252::from(expected_q).into(),
                Felt252::from(expected_r).into(),
            ))
        );
        run_program_assert_output(&program, entry_point, arguments, expected_result);
    }
}
