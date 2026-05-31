//! # `u256`-related libfuncs

use super::{increment_builtin_counter_by, LibfuncHelper};
use crate::{error::Result, metadata::MetadataStorage, utils::ProgramRegistryExt};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        int::unsigned256::Uint256Concrete,
        lib_func::SignatureOnlyConcreteLibfunc,
        ConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{
        arith::{self, CmpiPredicate},
        cf, llvm, ods, scf,
    },
    helpers::BuiltinBlockExt,
    ir::{
        attribute::{DenseI64ArrayAttribute, IntegerAttribute},
        operation::OperationBuilder,
        r#type::IntegerType,
        Block, BlockLike, Location, Region, Value,
    },
    Context,
};

/// Select and call the correct libfunc builder function from the selector.
pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &Uint256Concrete,
) -> Result<()> {
    match selector {
        Uint256Concrete::Divmod(info) => {
            build_divmod(context, registry, entry, location, helper, metadata, info)
        }
        Uint256Concrete::IsZero(info) => {
            build_is_zero(context, registry, entry, location, helper, metadata, info)
        }
        Uint256Concrete::SquareRoot(info) => {
            build_square_root(context, registry, entry, location, helper, metadata, info)
        }
        Uint256Concrete::InvModN(info) => build_u256_guarantee_inv_mod_n(
            context, registry, entry, location, helper, metadata, info,
        ),
    }
}

/// Generate MLIR operations for the `u256_safe_divmod` libfunc.
pub fn build_divmod<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    // The sierra-to-casm compiler uses the range check builtin a total of 6 times.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/int/unsigned256.rs?plain=1#L47
    let range_check =
        super::increment_builtin_counter_by(context, entry, location, entry.arg(0)?, 6)?;

    let i128_ty = IntegerType::new(context, 128).into();
    let i256_ty = IntegerType::new(context, 256).into();

    let guarantee_type = registry.build_type(
        context,
        helper,
        metadata,
        info.output_types().next().unwrap().nth(3).unwrap(),
    )?;

    let lhs_struct: Value = entry.arg(1)?;
    let rhs_struct: Value = entry.arg(2)?;

    let lhs_lo = entry
        .append_operation(llvm::extract_value(
            context,
            lhs_struct,
            DenseI64ArrayAttribute::new(context, &[0]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();
    let lhs_hi = entry
        .append_operation(llvm::extract_value(
            context,
            lhs_struct,
            DenseI64ArrayAttribute::new(context, &[1]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();
    let rhs_lo = entry
        .append_operation(llvm::extract_value(
            context,
            rhs_struct,
            DenseI64ArrayAttribute::new(context, &[0]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();
    let rhs_hi = entry
        .append_operation(llvm::extract_value(
            context,
            rhs_struct,
            DenseI64ArrayAttribute::new(context, &[1]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();

    let lhs_lo = entry
        .append_operation(arith::extui(lhs_lo, i256_ty, location))
        .result(0)?
        .into();
    let lhs_hi = entry
        .append_operation(arith::extui(lhs_hi, i256_ty, location))
        .result(0)?
        .into();
    let rhs_lo = entry
        .append_operation(arith::extui(rhs_lo, i256_ty, location))
        .result(0)?
        .into();
    let rhs_hi = entry
        .append_operation(arith::extui(rhs_hi, i256_ty, location))
        .result(0)?
        .into();

    let k128 = entry
        .append_operation(arith::constant(
            context,
            IntegerAttribute::new(i256_ty, 128).into(),
            location,
        ))
        .result(0)?
        .into();
    let lhs_hi = entry
        .append_operation(arith::shli(lhs_hi, k128, location))
        .result(0)?
        .into();
    let rhs_hi = entry
        .append_operation(arith::shli(rhs_hi, k128, location))
        .result(0)?
        .into();

    let lhs = entry
        .append_operation(arith::ori(lhs_hi, lhs_lo, location))
        .result(0)?
        .into();
    let rhs = entry
        .append_operation(arith::ori(rhs_hi, rhs_lo, location))
        .result(0)?
        .into();

    let result_div = entry
        .append_operation(arith::divui(lhs, rhs, location))
        .result(0)?
        .into();
    let result_rem = entry
        .append_operation(arith::remui(lhs, rhs, location))
        .result(0)?
        .into();

    let result_div_lo = entry
        .append_operation(arith::trunci(result_div, i128_ty, location))
        .result(0)?
        .into();
    let result_div_hi = entry
        .append_operation(arith::shrui(result_div, k128, location))
        .result(0)?
        .into();
    let result_div_hi = entry
        .append_operation(arith::trunci(result_div_hi, i128_ty, location))
        .result(0)?
        .into();

    let result_rem_lo = entry
        .append_operation(arith::trunci(result_rem, i128_ty, location))
        .result(0)?
        .into();
    let result_rem_hi = entry
        .append_operation(arith::shrui(result_rem, k128, location))
        .result(0)?
        .into();
    let result_rem_hi = entry
        .append_operation(arith::trunci(result_rem_hi, i128_ty, location))
        .result(0)?
        .into();

    let result_div = entry
        .append_operation(llvm::undef(
            llvm::r#type::r#struct(context, &[i128_ty, i128_ty], false),
            location,
        ))
        .result(0)?
        .into();
    let result_div = entry
        .append_operation(llvm::insert_value(
            context,
            result_div,
            DenseI64ArrayAttribute::new(context, &[0]),
            result_div_lo,
            location,
        ))
        .result(0)?
        .into();
    let result_div = entry
        .append_operation(llvm::insert_value(
            context,
            result_div,
            DenseI64ArrayAttribute::new(context, &[1]),
            result_div_hi,
            location,
        ))
        .result(0)?
        .into();

    let result_rem = entry
        .append_operation(llvm::undef(
            llvm::r#type::r#struct(context, &[i128_ty, i128_ty], false),
            location,
        ))
        .result(0)?
        .into();
    let result_rem = entry
        .append_operation(llvm::insert_value(
            context,
            result_rem,
            DenseI64ArrayAttribute::new(context, &[0]),
            result_rem_lo,
            location,
        ))
        .result(0)?
        .into();
    let result_rem = entry
        .append_operation(llvm::insert_value(
            context,
            result_rem,
            DenseI64ArrayAttribute::new(context, &[1]),
            result_rem_hi,
            location,
        ))
        .result(0)?
        .into();

    let op = entry.append_operation(llvm::undef(guarantee_type, location));
    let guarantee = op.result(0)?.into();

    helper.br(
        entry,
        0,
        &[range_check, result_div, result_rem, guarantee],
        location,
    )
}

/// Generate MLIR operations for the `u256_is_zero` libfunc.
pub fn build_is_zero<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let i128_ty = IntegerType::new(context, 128).into();

    let val_struct = entry.arg(0)?;
    let val_lo = entry
        .append_operation(llvm::extract_value(
            context,
            val_struct,
            DenseI64ArrayAttribute::new(context, &[0]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();
    let val_hi = entry
        .append_operation(llvm::extract_value(
            context,
            val_struct,
            DenseI64ArrayAttribute::new(context, &[1]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();

    let k0 = entry
        .append_operation(arith::constant(
            context,
            IntegerAttribute::new(i128_ty, 0).into(),
            location,
        ))
        .result(0)?
        .into();
    let val_lo_is_zero = entry
        .append_operation(arith::cmpi(
            context,
            CmpiPredicate::Eq,
            val_lo,
            k0,
            location,
        ))
        .result(0)?
        .into();
    let val_hi_is_zero = entry
        .append_operation(arith::cmpi(
            context,
            CmpiPredicate::Eq,
            val_hi,
            k0,
            location,
        ))
        .result(0)?
        .into();

    let val_is_zero = entry
        .append_operation(arith::andi(val_lo_is_zero, val_hi_is_zero, location))
        .result(0)?
        .into();

    helper.cond_br(
        context,
        entry,
        val_is_zero,
        [0, 1],
        [&[], &[val_struct]],
        location,
    )
}

/// Generate MLIR operations for the `u256_sqrt` libfunc.
pub fn build_square_root<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    // The sierra-to-casm compiler uses the range check builtin a total of 7 times.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/int/unsigned256.rs?plain=1#L189
    let range_check =
        super::increment_builtin_counter_by(context, entry, location, entry.arg(0)?, 7)?;

    let i128_ty = IntegerType::new(context, 128).into();
    let i256_ty = IntegerType::new(context, 256).into();

    let arg_struct = entry.arg(1)?;
    let arg_lo = entry
        .append_operation(llvm::extract_value(
            context,
            arg_struct,
            DenseI64ArrayAttribute::new(context, &[0]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();
    let arg_hi = entry
        .append_operation(llvm::extract_value(
            context,
            arg_struct,
            DenseI64ArrayAttribute::new(context, &[1]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();

    let arg_lo = entry
        .append_operation(arith::extui(arg_lo, i256_ty, location))
        .result(0)?
        .into();
    let arg_hi = entry
        .append_operation(arith::extui(arg_hi, i256_ty, location))
        .result(0)?
        .into();

    let k128 = entry
        .append_operation(arith::constant(
            context,
            IntegerAttribute::new(i256_ty, 128).into(),
            location,
        ))
        .result(0)?
        .into();
    let arg_hi = entry
        .append_operation(arith::shli(arg_hi, k128, location))
        .result(0)?
        .into();

    let arg_value = entry
        .append_operation(arith::ori(arg_hi, arg_lo, location))
        .result(0)?
        .into();

    let k1 = entry
        .append_operation(arith::constant(
            context,
            IntegerAttribute::new(i256_ty, 1).into(),
            location,
        ))
        .result(0)?
        .into();

    let is_small = entry
        .append_operation(arith::cmpi(
            context,
            CmpiPredicate::Ule,
            arg_value,
            k1,
            location,
        ))
        .result(0)?
        .into();

    let result = entry
        .append_operation(scf::r#if(
            is_small,
            &[i256_ty],
            {
                let region = Region::new();
                let block = region.append_block(Block::new(&[]));

                block.append_operation(scf::r#yield(&[arg_value], location));

                region
            },
            {
                let region = Region::new();
                let block = region.append_block(Block::new(&[]));

                let k128 = entry
                    .append_operation(arith::constant(
                        context,
                        IntegerAttribute::new(i256_ty, 256).into(),
                        location,
                    ))
                    .result(0)?
                    .into();

                let leading_zeros = block
                    .append_operation(
                        ods::llvm::intr_ctlz(
                            context,
                            i256_ty,
                            arg_value,
                            IntegerAttribute::new(IntegerType::new(context, 1).into(), 1),
                            location,
                        )
                        .into(),
                    )
                    .result(0)?
                    .into();

                let num_bits = block
                    .append_operation(arith::subi(k128, leading_zeros, location))
                    .result(0)?
                    .into();

                let shift_amount = block
                    .append_operation(arith::addi(num_bits, k1, location))
                    .result(0)?
                    .into();

                let parity_mask = block
                    .append_operation(arith::constant(
                        context,
                        IntegerAttribute::new(i256_ty, -2).into(),
                        location,
                    ))
                    .result(0)?
                    .into();
                let shift_amount = block
                    .append_operation(arith::andi(shift_amount, parity_mask, location))
                    .result(0)?
                    .into();

                let k0 = block
                    .append_operation(arith::constant(
                        context,
                        IntegerAttribute::new(i256_ty, 0).into(),
                        location,
                    ))
                    .result(0)?
                    .into();
                let result = block
                    .append_operation(scf::r#while(
                        &[k0, shift_amount],
                        &[i256_ty, i256_ty],
                        {
                            let region = Region::new();
                            let block = region.append_block(Block::new(&[
                                (i256_ty, location),
                                (i256_ty, location),
                            ]));

                            let result = block
                                .append_operation(arith::shli(block.arg(0)?, k1, location))
                                .result(0)?
                                .into();
                            let large_candidate = block
                                .append_operation(arith::xori(result, k1, location))
                                .result(0)?
                                .into();

                            let large_candidate_squared = block
                                .append_operation(arith::muli(
                                    large_candidate,
                                    large_candidate,
                                    location,
                                ))
                                .result(0)?
                                .into();

                            let threshold = block
                                .append_operation(arith::shrui(arg_value, block.arg(1)?, location))
                                .result(0)?
                                .into();
                            let threshold_is_poison = block
                                .append_operation(arith::cmpi(
                                    context,
                                    CmpiPredicate::Eq,
                                    block.arg(1)?,
                                    k128,
                                    location,
                                ))
                                .result(0)?
                                .into();
                            let threshold = block
                                .append_operation(
                                    OperationBuilder::new("arith.select", location)
                                        .add_operands(&[threshold_is_poison, k0, threshold])
                                        .add_results(&[i256_ty])
                                        .build()?,
                                )
                                .result(0)?
                                .into();

                            let is_in_range = block
                                .append_operation(arith::cmpi(
                                    context,
                                    CmpiPredicate::Ule,
                                    large_candidate_squared,
                                    threshold,
                                    location,
                                ))
                                .result(0)?
                                .into();

                            let result = block
                                .append_operation(
                                    OperationBuilder::new("arith.select", location)
                                        .add_operands(&[is_in_range, large_candidate, result])
                                        .add_results(&[i256_ty])
                                        .build()?,
                                )
                                .result(0)?
                                .into();

                            let k2 = block
                                .append_operation(arith::constant(
                                    context,
                                    IntegerAttribute::new(i256_ty, 2).into(),
                                    location,
                                ))
                                .result(0)?
                                .into();

                            let shift_amount = block
                                .append_operation(arith::subi(block.arg(1)?, k2, location))
                                .result(0)?
                                .into();

                            let should_continue = block
                                .append_operation(arith::cmpi(
                                    context,
                                    CmpiPredicate::Sge,
                                    shift_amount,
                                    k0,
                                    location,
                                ))
                                .result(0)?
                                .into();
                            block.append_operation(scf::condition(
                                should_continue,
                                &[result, shift_amount],
                                location,
                            ));

                            region
                        },
                        {
                            let region = Region::new();
                            let block = region.append_block(Block::new(&[
                                (i256_ty, location),
                                (i256_ty, location),
                            ]));

                            block.append_operation(scf::r#yield(
                                &[block.arg(0)?, block.argument(1)?.into()],
                                location,
                            ));

                            region
                        },
                        location,
                    ))
                    .result(0)?
                    .into();

                block.append_operation(scf::r#yield(&[result], location));

                region
            },
            location,
        ))
        .result(0)?
        .into();

    let result = entry
        .append_operation(arith::trunci(result, i128_ty, location))
        .result(0)?
        .into();

    helper.br(entry, 0, &[range_check, result], location)
}

/// Generate MLIR operations for the `u256_guarantee_inv_mod_n` libfunc.
pub fn build_u256_guarantee_inv_mod_n<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let i128_ty = IntegerType::new(context, 128).into();
    let i256_ty = IntegerType::new(context, 256).into();

    let lhs_struct = entry.arg(1)?;
    let lhs_lo = entry
        .append_operation(llvm::extract_value(
            context,
            lhs_struct,
            DenseI64ArrayAttribute::new(context, &[0]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();
    let lhs_hi = entry
        .append_operation(llvm::extract_value(
            context,
            lhs_struct,
            DenseI64ArrayAttribute::new(context, &[1]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();

    let rhs_struct = entry.arg(2)?;
    let rhs_lo = entry
        .append_operation(llvm::extract_value(
            context,
            rhs_struct,
            DenseI64ArrayAttribute::new(context, &[0]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();
    let rhs_hi = entry
        .append_operation(llvm::extract_value(
            context,
            rhs_struct,
            DenseI64ArrayAttribute::new(context, &[1]),
            i128_ty,
            location,
        ))
        .result(0)?
        .into();

    let lhs_lo = entry
        .append_operation(arith::extui(lhs_lo, i256_ty, location))
        .result(0)?
        .into();
    let lhs_hi = entry
        .append_operation(arith::extui(lhs_hi, i256_ty, location))
        .result(0)?
        .into();

    let rhs_lo = entry
        .append_operation(arith::extui(rhs_lo, i256_ty, location))
        .result(0)?
        .into();
    let rhs_hi = entry
        .append_operation(arith::extui(rhs_hi, i256_ty, location))
        .result(0)?
        .into();

    let k128 = entry
        .append_operation(arith::constant(
            context,
            IntegerAttribute::new(i256_ty, 128).into(),
            location,
        ))
        .result(0)?
        .into();
    let lhs_hi = entry
        .append_operation(arith::shli(lhs_hi, k128, location))
        .result(0)?
        .into();
    let rhs_hi = entry
        .append_operation(arith::shli(rhs_hi, k128, location))
        .result(0)?
        .into();

    let lhs = entry
        .append_operation(arith::ori(lhs_hi, lhs_lo, location))
        .result(0)?
        .into();
    let rhs = entry
        .append_operation(arith::ori(rhs_hi, rhs_lo, location))
        .result(0)?
        .into();

    let k0 = entry
        .append_operation(arith::constant(
            context,
            IntegerAttribute::new(i256_ty, 0).into(),
            location,
        ))
        .result(0)?
        .into();
    let k1 = entry
        .append_operation(arith::constant(
            context,
            IntegerAttribute::new(i256_ty, 1).into(),
            location,
        ))
        .result(0)?
        .into();

    // `a` (= lhs) is not constrained to be non-zero by the signature
    // `u256_guarantee_inv_mod_n(a, n: NonZero<u256>)`, and the extended Euclidean loop
    // below divides by the current divisor (initially `a`) on its first iteration, so
    // `a == 0` would be a udiv-by-zero (UB at the LLVM IR level). Since gcd(0, n) = n has
    // no inverse, short-circuit the whole calculation when `a == 0` and branch straight to
    // the "no inverse" output, matching casm_vm's U256InvModN hint. The range check builtin
    // is incremented 7 times on this path, same as the regular not-invertible branch.
    let guarantee_type = registry.build_type(
        context,
        helper,
        metadata,
        info.output_types().next().unwrap().nth(2).unwrap(),
    )?;
    let range_check = entry.arg(0)?;

    // The "no inverse" branch (output index 1) is reached from two places: the `a == 0`
    // short-circuit below and the regular `gcd != 1` exit of the Euclidean loop. The
    // LibfuncHelper reuses the values passed to `br`/`cond_br` directly in the shared
    // landing block, so they must dominate it. We therefore build them here in the entry
    // block (a common dominator of both predecessors): an `undef` guarantee and the range
    // check incremented 7 times.
    let guarantee = entry
        .append_operation(llvm::undef(guarantee_type, location))
        .result(0)?
        .into();
    let range_check_no_inverse =
        increment_builtin_counter_by(context, entry, location, range_check, 7)?;

    let lhs_is_zero = entry
        .append_operation(arith::cmpi(context, CmpiPredicate::Eq, lhs, k0, location))
        .result(0)?
        .into();
    let no_inverse_block = helper.append_block(Block::new(&[]));
    let compute_block = helper.append_block(Block::new(&[]));
    entry.append_operation(cf::cond_br(
        context,
        lhs_is_zero,
        no_inverse_block,
        compute_block,
        &[],
        &[],
        location,
    ));

    helper.br(
        no_inverse_block,
        1,
        &[range_check_no_inverse, guarantee, guarantee],
        location,
    )?;

    // `a != 0`: the inverse may exist, so run the extended Euclidean algorithm. From here
    // on everything is emitted into `compute_block` instead of the original entry block.
    let entry = compute_block;

    let result = entry.append_operation(scf::r#while(
        &[lhs, rhs, k1, k0],
        &[i256_ty, i256_ty, i256_ty, i256_ty],
        {
            let region = Region::new();
            let block = region.append_block(Block::new(&[
                (i256_ty, location),
                (i256_ty, location),
                (i256_ty, location),
                (i256_ty, location),
            ]));

            let q = block
                .append_operation(arith::divui(block.arg(1)?, block.arg(0)?, location))
                .result(0)?
                .into();

            let q_c = block
                .append_operation(arith::muli(q, block.arg(0)?, location))
                .result(0)?
                .into();
            let c = block
                .append_operation(arith::subi(block.arg(1)?, q_c, location))
                .result(0)?
                .into();

            let q_uc = block
                .append_operation(arith::muli(q, block.arg(2)?, location))
                .result(0)?
                .into();
            let u_c = block
                .append_operation(arith::subi(block.arg(3)?, q_uc, location))
                .result(0)?
                .into();

            let should_continue = block
                .append_operation(arith::cmpi(context, CmpiPredicate::Ne, c, k0, location))
                .result(0)?
                .into();
            block.append_operation(scf::condition(
                should_continue,
                &[c, block.arg(0)?, u_c, block.argument(2)?.into()],
                location,
            ));

            region
        },
        {
            let region = Region::new();
            let block = region.append_block(Block::new(&[
                (i256_ty, location),
                (i256_ty, location),
                (i256_ty, location),
                (i256_ty, location),
            ]));

            block.append_operation(scf::r#yield(
                &[block.arg(0)?, block.arg(1)?, block.arg(2)?, block.arg(3)?],
                location,
            ));

            region
        },
        location,
    ));

    let inv = result.result(3)?.into();

    let inv = entry
        .append_operation(scf::r#if(
            entry
                .append_operation(arith::cmpi(context, CmpiPredicate::Slt, inv, k0, location))
                .result(0)?
                .into(),
            &[i256_ty],
            {
                let region = Region::new();
                let block = region.append_block(Block::new(&[]));

                block.append_operation(scf::r#yield(
                    &[entry
                        .append_operation(arith::addi(inv, rhs, location))
                        .result(0)?
                        .into()],
                    location,
                ));

                region
            },
            {
                let region = Region::new();
                let block = region.append_block(Block::new(&[]));

                block.append_operation(scf::r#yield(
                    &[entry
                        .append_operation(arith::remui(inv, rhs, location))
                        .result(0)?
                        .into()],
                    location,
                ));

                region
            },
            location,
        ))
        .result(0)?
        .into();

    let inv_lo = entry
        .append_operation(arith::trunci(inv, i128_ty, location))
        .result(0)?
        .into();
    let inv_hi = entry
        .append_operation(arith::shrui(inv, k128, location))
        .result(0)?
        .into();
    let inv_hi = entry
        .append_operation(arith::trunci(inv_hi, i128_ty, location))
        .result(0)?
        .into();

    let return_ty = registry.build_type(
        context,
        helper,
        metadata,
        info.output_types().next().unwrap().nth(1).unwrap(),
    )?;
    let result_inv = entry
        .append_operation(llvm::undef(return_ty, location))
        .result(0)?
        .into();
    let result_inv = entry
        .append_operation(llvm::insert_value(
            context,
            result_inv,
            DenseI64ArrayAttribute::new(context, &[0]),
            inv_lo,
            location,
        ))
        .result(0)?
        .into();
    let result_inv = entry
        .append_operation(llvm::insert_value(
            context,
            result_inv,
            DenseI64ArrayAttribute::new(context, &[1]),
            inv_hi,
            location,
        ))
        .result(0)?
        .into();

    let lhs_is_invertible = entry
        .append_operation(arith::cmpi(
            context,
            CmpiPredicate::Eq,
            result.result(1)?.into(),
            k1,
            location,
        ))
        .result(0)?
        .into();
    let inv_not_zero = entry
        .append_operation(arith::cmpi(context, CmpiPredicate::Ne, inv, k0, location))
        .result(0)?
        .into();
    let inverse_exists_and_is_not_zero = entry
        .append_operation(arith::andi(lhs_is_invertible, inv_not_zero, location))
        .result(0)?
        .into();

    // The sierra-to-casm compiler uses the range check builtin a total of 9 times if the inverse is
    // not equal to 0 and lhs is invertible. Otherwise it will be used 7 times (handled by
    // `range_check_no_inverse`, computed in the entry block).
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/int/unsigned256.rs#L21
    let range_check_inverse =
        increment_builtin_counter_by(context, entry, location, range_check, 9)?;

    helper.cond_br(
        context,
        entry,
        inverse_exists_and_is_not_zero,
        [0, 1],
        [
            &[
                range_check_inverse,
                result_inv,
                guarantee,
                guarantee,
                guarantee,
                guarantee,
                guarantee,
                guarantee,
                guarantee,
                guarantee,
            ],
            &[range_check_no_inverse, guarantee, guarantee],
        ],
        location,
    )
}

#[cfg(test)]
mod test {
    use crate::{
        jit_enum, jit_panic, jit_struct,
        utils::testing::{get_compiled_program, run_program_assert_output},
        values::Value,
    };
    use num_bigint::BigUint;
    use num_traits::One;
    use starknet_types_core::felt::Felt;
    use std::ops::Shl;

    fn u256(value: BigUint) -> Value {
        assert!(value.bits() <= 256);
        jit_struct!(
            Value::Uint128((&value & &u128::MAX.into()).try_into().unwrap()),
            Value::Uint128(((&value >> 128u32) & &u128::MAX.into()).try_into().unwrap()),
        )
    }

    #[test]
    fn u256_is_zero() {
        let program = get_compiled_program("programs/libfuncs/u256_is_zero");
        run_program_assert_output(
            &program,
            "run_test",
            &[u256(0u32.into())],
            jit_enum!(1, jit_struct!()),
        );
        run_program_assert_output(
            &program,
            "run_test",
            &[u256(1u32.into())],
            jit_enum!(0, jit_struct!()),
        );
        run_program_assert_output(
            &program,
            "run_test",
            &[u256(BigUint::one() << 128u32)],
            jit_enum!(0, jit_struct!()),
        );
        run_program_assert_output(
            &program,
            "run_test",
            &[u256((BigUint::one() << 128u32) + 1u32)],
            jit_enum!(0, jit_struct!()),
        );
    }

    #[test]
    fn u256_safe_divmod() {
        let program = get_compiled_program("programs/libfuncs/u256_safe_divmod");
        #[track_caller]
        fn run(
            program: &(String, cairo_lang_sierra::program::Program),
            lhs: (u128, u128),
            rhs: (u128, u128),
            result: Value,
        ) {
            run_program_assert_output(
                program,
                "run_test",
                &[
                    jit_struct!(lhs.1.into(), lhs.0.into()),
                    jit_struct!(rhs.1.into(), rhs.0.into()),
                ],
                result,
            )
        }

        let u256_is_zero = Felt::from_bytes_be_slice(b"Division by 0");
        let max_value = 0xFFFFFFFF_FFFFFFFF_FFFFFFFF_FFFFFFFFu128;

        run(&program, (0, 0), (0, 0), jit_panic!(u256_is_zero));
        run(
            &program,
            (0, 0),
            (0, 1),
            jit_enum!(
                0,
                jit_struct!(jit_struct!(
                    jit_struct!(0u128.into(), 0u128.into()),
                    jit_struct!(0u128.into(), 0u128.into()),
                ))
            ),
        );
        run(
            &program,
            (0, 0),
            (max_value, max_value),
            jit_enum!(
                0,
                jit_struct!(jit_struct!(
                    jit_struct!(0u128.into(), 0u128.into()),
                    jit_struct!(0u128.into(), 0u128.into()),
                ))
            ),
        );

        run(&program, (0, 1), (0, 0), jit_panic!(u256_is_zero));
        run(
            &program,
            (0, 1),
            (0, 1),
            jit_enum!(
                0,
                jit_struct!(jit_struct!(
                    jit_struct!(1u128.into(), 0u128.into()),
                    jit_struct!(0u128.into(), 0u128.into()),
                ))
            ),
        );
        run(
            &program,
            (0, 1),
            (max_value, max_value),
            jit_enum!(
                0,
                jit_struct!(jit_struct!(
                    jit_struct!(0u128.into(), 0u128.into()),
                    jit_struct!(1u128.into(), 0u128.into()),
                ))
            ),
        );
        run(
            &program,
            (max_value, max_value),
            (0, 0),
            jit_panic!(u256_is_zero),
        );

        run(
            &program,
            (max_value, max_value),
            (0, 1),
            jit_enum!(
                0,
                jit_struct!(jit_struct!(
                    jit_struct!(max_value.into(), max_value.into()),
                    jit_struct!(0u128.into(), 0u128.into()),
                ))
            ),
        );
        run(
            &program,
            (max_value, max_value),
            (max_value, max_value),
            jit_enum!(
                0,
                jit_struct!(jit_struct!(
                    jit_struct!(1u128.into(), 0u128.into()),
                    jit_struct!(0u128.into(), 0u128.into()),
                ))
            ),
        );
    }

    #[test]
    fn u256_sqrt() {
        let program = get_compiled_program("programs/libfuncs/u256_sqrt");
        #[track_caller]
        fn run(
            program: &(String, cairo_lang_sierra::program::Program),
            value: (u128, u128),
            result: Value,
        ) {
            run_program_assert_output(
                program,
                "run_test",
                &[jit_struct!(value.1.into(), value.0.into())],
                result,
            )
        }

        run(&program, (0u128, 0u128), 0u128.into());
        run(&program, (0u128, 1u128), 1u128.into());
        run(&program, (u128::MAX, u128::MAX), u128::MAX.into());

        for i in 0..u128::BITS {
            let x = 1u128 << i;
            let y: u128 = BigUint::from(x)
                .sqrt()
                .try_into()
                .expect("should always fit into a u128");

            run(&program, (0, x), y.into());
        }

        for i in 0..u128::BITS {
            let x = 1u128 << i;
            let y: u128 = BigUint::from(x)
                .shl(128usize)
                .sqrt()
                .try_into()
                .expect("should always fit into a u128");

            run(&program, (x, 0), y.into());
        }
    }

    #[test]
    fn u256_inv_mod_n() {
        let program = get_compiled_program("programs/libfuncs/u256_inv_mod_n");
        #[track_caller]
        fn run(
            program: &(String, cairo_lang_sierra::program::Program),
            a: (u128, u128),
            n: (u128, u128),
            result: Value,
        ) {
            run_program_assert_output(
                program,
                "run_test",
                &[
                    jit_struct!(a.0.into(), a.1.into()),
                    jit_struct!(n.0.into(), n.1.into()),
                ],
                result,
            )
        }

        let none = jit_enum!(1, jit_struct!());

        // Not invertible.
        run(&program, (0, 0), (0, 0), none.clone());
        run(&program, (1, 0), (1, 0), none.clone());
        run(&program, (0, 0), (1, 0), none.clone());
        run(&program, (0, 0), (7, 0), none.clone());
        run(&program, (3, 0), (6, 0), none.clone());
        run(&program, (4, 0), (6, 0), none.clone());
        run(&program, (8, 0), (4, 0), none.clone());
        run(&program, (8, 0), (24, 0), none.clone());
        run(
            &program,
            (
                112713230461650448610759614893138283713,
                311795268193434200766998031144865279193,
            ),
            (
                214442144331145623175443765631916854552,
                85683151001472364977354294776284843870,
            ),
            none.clone(),
        );
        run(
            &program,
            (
                138560372230216185616572678448146427468,
                178030013799389090502578959553486954963,
            ),
            (
                299456334380503763038201670272353657683,
                285941620966047830312853638602560712796,
            ),
            none,
        );

        // Invertible.
        run(
            &program,
            (5, 0),
            (24, 0),
            jit_enum!(0, jit_struct!(5u128.into(), 0u128.into())),
        );
        run(
            &program,
            (29, 0),
            (24, 0),
            jit_enum!(0, jit_struct!(5u128.into(), 0u128.into())),
        );
        run(
            &program,
            (1, 0),
            (24, 0),
            jit_enum!(0, jit_struct!(1u128.into(), 0u128.into())),
        );
        run(
            &program,
            (1, 0),
            (5, 0),
            jit_enum!(0, jit_struct!(1u128.into(), 0u128.into())),
        );
        run(
            &program,
            (2, 0),
            (5, 0),
            jit_enum!(0, jit_struct!(3u128.into(), 0u128.into())),
        );
    }
}
