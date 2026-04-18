//! # Enum-related libfuncs
//!
//! Check out [the enum type](crate::types::enum) for more information on enum layouts.

use super::LibfuncHelper;
use crate::{
    error::{panic::ToNativeAssertError, Error, Result},
    libfuncs::r#box::into_box,
    metadata::{enum_snapshot_variants::EnumSnapshotVariantsMeta, MetadataStorage},
    native_assert, native_panic,
    types::TypeBuilder,
    utils::ProgramRegistryExt,
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        enm::{
            EnumBoxedMatchConcreteLibfunc, EnumConcreteLibfunc, EnumFromBoundedIntConcreteLibfunc,
            EnumInitConcreteLibfunc,
        },
        lib_func::SignatureOnlyConcreteLibfunc,
        ConcreteLibfunc,
    },
    ids::ConcreteTypeId,
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{arith, cf, llvm, ods},
    helpers::{ArithBlockExt, BuiltinBlockExt, GepIndex, LlvmBlockExt},
    ir::{
        attribute::{DenseI64ArrayAttribute, IntegerAttribute},
        r#type::IntegerType,
        Block, BlockLike, Location, Value,
    },
    Context,
};
use std::num::TryFromIntError;

/// Select and call the correct libfunc builder function from the selector.
pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &EnumConcreteLibfunc,
) -> Result<()> {
    match selector {
        EnumConcreteLibfunc::Init(info) => {
            build_init(context, registry, entry, location, helper, metadata, info)
        }
        EnumConcreteLibfunc::Match(info) => {
            build_match(context, registry, entry, location, helper, metadata, info)
        }
        EnumConcreteLibfunc::SnapshotMatch(info) => {
            build_snapshot_match(context, registry, entry, location, helper, metadata, info)
        }
        EnumConcreteLibfunc::FromBoundedInt(info) => {
            build_from_bounded_int(context, registry, entry, location, helper, metadata, info)
        }
        EnumConcreteLibfunc::BoxedMatch(info) => {
            build_boxed_match(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `enum_init` libfunc.
pub fn build_init<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &EnumInitConcreteLibfunc,
) -> Result<()> {
    #[cfg(feature = "with-debug-utils")]
    if let Some(auto_breakpoint) =
        metadata.get::<crate::metadata::auto_breakpoint::AutoBreakpoint>()
    {
        auto_breakpoint.maybe_breakpoint(
            entry,
            location,
            metadata,
            &crate::metadata::auto_breakpoint::BreakpointEvent::EnumInit {
                type_id: info.signature.branch_signatures[0].vars[0].ty.clone(),
                variant_idx: info.index,
            },
        )?;
    }

    let val = build_enum_value(
        context,
        registry,
        entry,
        location,
        helper,
        metadata,
        entry.arg(0)?,
        &info.branch_signatures()[0].vars[0].ty,
        &info.signature.param_signatures[0].ty,
        info.index,
    )?;

    helper.br(entry, 0, &[val], location)
}

#[allow(clippy::too_many_arguments)]
pub fn build_enum_value<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    payload_value: Value<'ctx, 'this>,
    enum_type: &ConcreteTypeId,
    variant_type: &ConcreteTypeId,
    variant_index: usize,
) -> Result<Value<'ctx, 'this>> {
    let type_info = registry.get_type(enum_type)?;
    let payload_type_info = registry.get_type(variant_type)?;

    let (layout, (tag_ty, _), variant_tys) = crate::types::r#enum::get_type_for_variants(
        context,
        helper,
        registry,
        metadata,
        type_info
            .variants()
            .to_native_assert_error("found non-enum type where an enum is required")?,
    )?;

    Ok(match variant_tys.len() {
        0 => native_panic!("attempt to initialize a zero-variant enum"),
        1 => payload_value,
        _ => {
            let enum_ty = llvm::r#type::r#struct(
                context,
                &[
                    tag_ty,
                    if payload_type_info.is_zst(registry)? {
                        llvm::r#type::array(IntegerType::new(context, 8).into(), 0)
                    } else {
                        variant_tys[variant_index].0
                    },
                ],
                false,
            );

            let tag_val = entry
                .append_operation(arith::constant(
                    context,
                    IntegerAttribute::new(tag_ty, variant_index.try_into()?).into(),
                    location,
                ))
                .result(0)?
                .into();

            let val = entry.append_op_result(llvm::undef(enum_ty, location))?;
            let val = entry.insert_value(context, location, val, tag_val, 0)?;

            let mut val = if payload_type_info.is_zst(registry)? {
                val
            } else {
                entry.insert_value(context, location, val, payload_value, 1)?
            };

            if type_info.is_memory_allocated(registry)? {
                let stack_ptr = helper.init_block().alloca1(
                    context,
                    location,
                    type_info.build(context, helper, registry, metadata, enum_type)?,
                    layout.align(),
                )?;

                // Convert the enum from the concrete variant to the internal representation.
                entry.store(context, location, stack_ptr, val)?;
                val = entry.load(
                    context,
                    location,
                    stack_ptr,
                    type_info.build(context, helper, registry, metadata, enum_type)?,
                )?;
            };

            val
        }
    })
}

/// Generate MLIR operations for the `enum_from_bounded_int` libfunc.
///
/// # Constraints
///
/// - The target `Enum` must contain the same number of empty variants as the number
///   of possible values in the `BoundedInt` range.
/// - The range of the `BoundedInt` must start from **0**.
///
/// # Signature
///
/// ```cairo
/// fn enum_from_bounded_int<T, U>(index: U) -> T nopanic
/// ```
pub fn build_from_bounded_int<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &EnumFromBoundedIntConcreteLibfunc,
) -> Result<()> {
    let inp_ty = registry.get_type(&info.param_signatures()[0].ty)?;
    let varaint_selector_type: IntegerType = inp_ty
        .build(
            context,
            helper,
            registry,
            metadata,
            &info.param_signatures()[0].ty,
        )?
        .try_into()?;
    let enum_type = registry.get_type(&info.branch_signatures()[0].vars[0].ty)?;

    let enum_ty = enum_type.build(
        context,
        helper,
        registry,
        metadata,
        &info.branch_signatures()[0].vars[0].ty,
    )?;

    let tag_bits = info.n_variants.next_power_of_two().trailing_zeros();
    let tag_type = IntegerType::new(context, tag_bits);

    let mut tag_value: Value = entry.arg(0)?;

    match tag_type.width().cmp(&varaint_selector_type.width()) {
        std::cmp::Ordering::Less => {
            tag_value = entry.append_op_result(
                ods::llvm::trunc(context, tag_type.into(), tag_value, location).into(),
            )?;
        }
        std::cmp::Ordering::Equal => {}
        std::cmp::Ordering::Greater => {
            tag_value = entry.append_op_result(
                ods::llvm::zext(context, tag_type.into(), tag_value, location).into(),
            )?;
        }
    };

    let mut value = entry.append_op_result(llvm::undef(enum_ty, location))?;
    if info.n_variants > 1 {
        value = entry.insert_value(context, location, value, tag_value, 0)?;
    }

    helper.br(entry, 0, &[value], location)
}

/// Generate MLIR operations for the `enum_match` libfunc.
pub fn build_match<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let type_info = registry.get_type(&info.param_signatures()[0].ty)?;

    let variant_ids = type_info
        .variants()
        .to_native_assert_error("found non-enum type where an enum is required")?;
    match variant_ids.len() {
        0 => {
            // The Cairo compiler will generate an enum match for enums without variants, so this
            // case cannot be a compile-time error. We're assuming that even though it's been
            // generated, it's just dead code and can be made into an assertion that always fails.

            let k0 = entry.const_int(context, location, 0, 1)?;
            entry.append_operation(cf::assert(
                context,
                k0,
                "attempt to match a zero-variant enum",
                location,
            ));

            entry.append_operation(llvm::unreachable(location));
        }
        1 => {
            helper.br(entry, 0, &[entry.arg(0)?], location)?;
        }
        _ => {
            let (layout, (tag_ty, _), variant_tys) = crate::types::r#enum::get_type_for_variants(
                context,
                helper,
                registry,
                metadata,
                variant_ids,
            )?;

            let (stack_ptr, tag_val) = if type_info.is_memory_allocated(registry)? {
                let stack_ptr = helper.init_block().alloca1(
                    context,
                    location,
                    type_info.build(
                        context,
                        helper,
                        registry,
                        metadata,
                        &info.param_signatures()[0].ty,
                    )?,
                    layout.align(),
                )?;
                entry.store(context, location, stack_ptr, entry.arg(0)?)?;
                let tag_val = entry.load(context, location, stack_ptr, tag_ty)?;

                (Some(stack_ptr), tag_val)
            } else {
                let tag_val = entry
                    .append_operation(llvm::extract_value(
                        context,
                        entry.arg(0)?,
                        DenseI64ArrayAttribute::new(context, &[0]),
                        tag_ty,
                        location,
                    ))
                    .result(0)?
                    .into();

                (None, tag_val)
            };

            let default_block = helper.append_block(Block::new(&[]));
            let variant_blocks = variant_tys
                .iter()
                .map(|_| helper.append_block(Block::new(&[])))
                .collect::<Vec<_>>();

            let case_values = (0..variant_tys.len())
                .map(i64::try_from)
                .collect::<std::result::Result<Vec<_>, TryFromIntError>>()?;

            entry.append_operation(cf::switch(
                context,
                &case_values,
                tag_val,
                tag_ty,
                (default_block, &[]),
                &variant_blocks
                    .iter()
                    .copied()
                    .map(|block| (block, [].as_slice()))
                    .collect::<Vec<_>>(),
                location,
            )?);

            // Default block.
            {
                let val = default_block
                    .append_operation(arith::constant(
                        context,
                        IntegerAttribute::new(IntegerType::new(context, 1).into(), 0).into(),
                        location,
                    ))
                    .result(0)?
                    .into();

                default_block.append_operation(cf::assert(
                    context,
                    val,
                    "Invalid enum tag.",
                    location,
                ));
                default_block.append_operation(llvm::unreachable(location));
            }

            // Enum variants.
            for (i, (block, (payload_ty, _))) in
                variant_blocks.into_iter().zip(variant_tys).enumerate()
            {
                let enum_ty = llvm::r#type::r#struct(context, &[tag_ty, payload_ty], false);

                let payload_val = match stack_ptr {
                    Some(stack_ptr) => {
                        let val = block.load(context, location, stack_ptr, enum_ty)?;
                        block.extract_value(context, location, val, payload_ty, 1)?
                    }
                    None => {
                        // If the enum is not memory-allocated it means that:
                        //   - Either it's a C-style enum and all payloads have the same type.
                        //   - Or the enum only has a single non-memory-allocated variant.
                        if variant_ids.len() == 1 {
                            entry.arg(0)?
                        } else {
                            native_assert!(
                                registry.get_type(&variant_ids[i])?.is_zst(registry)?,
                                "should be zero sized"
                            );
                            block
                                .append_operation(llvm::undef(payload_ty, location))
                                .result(0)?
                                .into()
                        }
                    }
                };

                helper.br(block, i, &[payload_val], location)?;
            }
        }
    }

    Ok(())
}

/// Generate MLIR operations for the `enum_snapshot_match` libfunc.
pub fn build_snapshot_match<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let type_info = registry.get_type(&info.param_signatures()[0].ty)?;

    // This libfunc's implementation is identical to `enum_match` aside from fetching the snapshotted enum's variants from the metadata:
    let variant_ids = metadata
        .get::<EnumSnapshotVariantsMeta>()
        .ok_or(Error::MissingMetadata)?
        .get_variants(&info.param_signatures()[0].ty)
        .to_native_assert_error("enum should always have variants")?
        .clone();
    match variant_ids.len() {
        0 => {
            // The Cairo compiler will generate an enum match for enums without variants, so this
            // case cannot be a compile-time error. We're assuming that even though it's been
            // generated, it's just dead code and can be made into an assertion that always fails.

            let k0 = entry.const_int(context, location, 0, 1)?;
            entry.append_operation(cf::assert(
                context,
                k0,
                "attempt to match a zero-variant enum",
                location,
            ));

            entry.append_operation(llvm::unreachable(location));
        }
        1 => {
            helper.br(entry, 0, &[entry.arg(0)?], location)?;
        }
        _ => {
            let (layout, (tag_ty, _), variant_tys) = crate::types::r#enum::get_type_for_variants(
                context,
                helper,
                registry,
                metadata,
                &variant_ids,
            )?;

            let (stack_ptr, tag_val) = if type_info.is_memory_allocated(registry)? {
                let stack_ptr = helper.init_block().alloca1(
                    context,
                    location,
                    type_info.build(
                        context,
                        helper,
                        registry,
                        metadata,
                        &info.param_signatures()[0].ty,
                    )?,
                    layout.align(),
                )?;
                entry.store(context, location, stack_ptr, entry.arg(0)?)?;
                let tag_val = entry.load(context, location, stack_ptr, tag_ty)?;

                (Some(stack_ptr), tag_val)
            } else {
                let tag_val = entry.extract_value(context, location, entry.arg(0)?, tag_ty, 0)?;

                (None, tag_val)
            };

            let default_block = helper.append_block(Block::new(&[]));
            let variant_blocks = variant_tys
                .iter()
                .map(|_| helper.append_block(Block::new(&[])))
                .collect::<Vec<_>>();

            let case_values = (0..variant_tys.len())
                .map(i64::try_from)
                .collect::<std::result::Result<Vec<_>, TryFromIntError>>()?;

            entry.append_operation(cf::switch(
                context,
                &case_values,
                tag_val,
                tag_ty,
                (default_block, &[]),
                &variant_blocks
                    .iter()
                    .copied()
                    .map(|block| (block, [].as_slice()))
                    .collect::<Vec<_>>(),
                location,
            )?);

            // Default block.
            {
                let val = default_block.const_int(context, location, 0, 1)?;

                default_block.append_operation(cf::assert(
                    context,
                    val,
                    "Invalid enum tag.",
                    location,
                ));
                default_block.append_operation(llvm::unreachable(location));
            }

            // Enum variants.
            for (i, (block, (payload_ty, _))) in
                variant_blocks.into_iter().zip(variant_tys).enumerate()
            {
                let enum_ty = llvm::r#type::r#struct(context, &[tag_ty, payload_ty], false);

                let payload_val = match stack_ptr {
                    Some(stack_ptr) => {
                        let val = block.load(context, location, stack_ptr, enum_ty)?;
                        block.extract_value(context, location, val, payload_ty, 1)?
                    }
                    None => {
                        // If the enum is not memory-allocated it means that:
                        //   - Either it's a C-style enum and all payloads have the same type.
                        //   - Or the enum only has a single non-memory-allocated variant.
                        if variant_ids.len() == 1 {
                            entry.arg(0)?
                        } else {
                            native_assert!(
                                registry.get_type(&variant_ids[i])?.is_zst(registry)?,
                                "should be zero sized"
                            );
                            block.append_op_result(llvm::undef(payload_ty, location))?
                        }
                    }
                };

                helper.br(block, i, &[payload_val], location)?;
            }
        }
    }

    Ok(())
}

/// Generate MLIR operations for the `enum_boxed_match` libfunc.
///
/// Receives an `Enum` inside a `Box` and branches based on the variant,
/// returning a `Box` containing the variant's payload.
///
/// # Signature
///
/// ```cairo
/// enum MyEnum {
///     A: felt252,
///     B: u128,
/// }
///
/// enum BoxedMyEnum {
///     A: Box<felt252>,
///     B: Box<u128>,
/// }
///
/// extern fn enum_boxed_match<MyEnum>(
///     value: Box<MyEnum>
/// ) -> BoxedMyEnum nopanic;
/// ```
pub fn build_boxed_match<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &EnumBoxedMatchConcreteLibfunc,
) -> Result<()> {
    let variant_ids = &info.variants;

    match variant_ids.len() {
        0 => {
            // The Cairo compiler will generate an enum match for enums without variants, so this
            // case cannot be a compile-time error. We're assuming that even though it's been
            // generated, it's just dead code and can be made into an assertion that always fails.
            let k0 = entry.const_int(context, location, 0, 1)?;
            entry.append_operation(cf::assert(
                context,
                k0,
                "attempt to match a zero-variant enum",
                location,
            ));
            entry.append_operation(llvm::unreachable(location));
        }
        1 => {
            // For single-variant enums, the enum type IS the payload type (no tag),
            // so Box<Enum> is already identical to Box<Payload> — just forward the pointer.
            helper.br(entry, 0, &[entry.arg(0)?], location)?;
        }
        _ => {
            let (_layout, (tag_ty, tag_layout), variant_tys) =
                crate::types::r#enum::get_type_for_variants(
                    context,
                    helper,
                    registry,
                    metadata,
                    variant_ids,
                )?;

            let tag_val = entry.load(context, location, entry.arg(0)?, tag_ty)?;

            let default_block = helper.append_block(Block::new(&[]));
            let variant_blocks = variant_tys
                .iter()
                .map(|_| helper.append_block(Block::new(&[])))
                .collect::<Vec<_>>();

            let case_values = (0..variant_tys.len())
                .map(i64::try_from)
                .collect::<std::result::Result<Vec<_>, TryFromIntError>>()?;

            entry.append_operation(cf::switch(
                context,
                &case_values,
                tag_val,
                tag_ty,
                (default_block, &[]),
                &variant_blocks
                    .iter()
                    .copied()
                    .map(|block| (block, [].as_slice()))
                    .collect::<Vec<_>>(),
                location,
            )?);

            // Default block (invalid tag).
            {
                let val = default_block.const_int(context, location, 0, 1)?;

                default_block.append_operation(cf::assert(
                    context,
                    val,
                    "Invalid enum tag.",
                    location,
                ));
                default_block.append_operation(llvm::unreachable(location));
            }

            for (i, (block, (payload_ty, payload_layout))) in
                variant_blocks.into_iter().zip(variant_tys).enumerate()
            {
                // Extract the payload from the enum
                let payload_val = {
                    let payload_offset = tag_layout.extend(payload_layout)?.1;
                    let ptr = block.gep(
                        context,
                        location,
                        entry.arg(0)?,
                        &[GepIndex::Const(payload_offset.try_into()?)],
                        IntegerType::new(context, 8).into(),
                    )?;
                    block.load(context, location, ptr, payload_ty)?
                };

                // Get the output variant type layout for boxing
                let output_variant_type_id = &info.branch_signatures()[i].vars[0].ty;
                let CoreTypeConcrete::Box(output_box_info) =
                    registry.get_type(output_variant_type_id)?
                else {
                    native_panic!("Output should be a Box type");
                };
                let (_, output_layout) = registry.build_type_with_layout(
                    context,
                    helper,
                    metadata,
                    &output_box_info.ty,
                )?;

                let boxed_payload = into_box(
                    context,
                    helper.module,
                    block,
                    location,
                    payload_val,
                    output_layout,
                    metadata,
                )?;

                helper.br(block, i, &[boxed_payload], location)?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{
        context::NativeContext,
        jit_enum, jit_struct, load_cairo,
        utils::testing::{get_compiled_program, run_program_assert_output},
        Value,
    };
    use starknet_types_core::felt::Felt;

    #[test]
    fn enum_init() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/enum_init");
        run_program_assert_output(
            &program,
            "run_test",
            &[],
            jit_struct!(
                jit_enum!(0, Felt::from(-1).into()),
                jit_enum!(0, Felt::from(5678).into()),
                jit_enum!(1, 90u8.into()),
                jit_enum!(2, 9012u16.into()),
                jit_enum!(3, 34567890u32.into()),
                jit_enum!(4, 1234567890123456u64.into()),
            ),
        );
    }

    #[test]
    fn enum_match() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/enum_match");
        run_program_assert_output(&program, "match_a", &[], Felt::from(5).into());
        run_program_assert_output(&program, "match_b", &[], 5u8.into());
    }

    #[test]
    fn compile_enum_match_without_variants() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/enum_match_no_variants");

        let native_context = NativeContext::new();
        native_context
            .compile(&program.1, false, Some(Default::default()), None)
            .unwrap();
    }

    #[test]
    fn create_enum_from_bounded_int() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/enum_from_bounded_int");

        run_program_assert_output(
            &program,
            "test_1_variants",
            &[Value::Felt252(0.into())],
            jit_enum!(0, jit_struct!(jit_enum!(0, jit_struct!()))),
        );

        run_program_assert_output(
            &program,
            "test_5_variants",
            &[Value::Felt252(0.into())],
            jit_enum!(0, jit_struct!(jit_enum!(0, jit_struct!()))),
        );

        run_program_assert_output(
            &program,
            "test_5_variants",
            &[Value::Felt252(4.into())],
            jit_enum!(0, jit_struct!(jit_enum!(4, jit_struct!()))),
        );
    }

    #[test]
    fn enum_boxed_match() {
        let program = load_cairo! {
            use core::box::BoxTrait;

            enum BoxedOption {
                Some: Box<felt252>,
                None: Box<()>,
            }

            extern fn enum_boxed_match<T>(e: Box<T>) -> BoxedOption nopanic;

            fn test_some() -> felt252 {
                let boxed = identity(BoxTrait::new(Option::Some(42_felt252)));
                match enum_boxed_match(boxed) {
                    BoxedOption::Some(v) => BoxTrait::unbox(v),
                    BoxedOption::None(_) => 0,
                }
            }

            fn test_none() -> felt252 {
                let opt: Option<felt252> = Option::None;
                let boxed = identity(BoxTrait::new(opt));
                match enum_boxed_match(boxed) {
                    BoxedOption::Some(_) => 1,
                    BoxedOption::None(_) => 0,
                }
            }


            #[inline(never)]
            // Prevent the compiler from optimizing enum variants.
            fn identity<T>(x: T) -> T {
                x
            }
        };

        run_program_assert_output(&program, "test_some", &[], Felt::from(42).into());
        run_program_assert_output(&program, "test_none", &[], Felt::from(0).into());
    }

    #[test]
    fn compile_enum_boxed_match_zero_variants() {
        // Zero-variant enums can never be constructed, so this is a compile-only test
        // to verify the zero-variant boxed match path doesn't crash the compiler.
        let program = load_cairo! {
            enum Never {}

            extern fn enum_boxed_match<T>(e: Box<T>) -> Never nopanic;

            fn test_never(value: Box<Never>) -> Never {
                enum_boxed_match(value)
            }
        };

        let native_context = NativeContext::new();
        native_context
            .compile(&program.1, false, Some(Default::default()), None)
            .unwrap();
    }

    #[test]
    fn enum_boxed_match_single_variant() {
        let program = load_cairo! {
            use core::box::BoxTrait;

            enum Wrapper {
                Value: felt252,
            }

            enum BoxedWrapper {
                Value: Box<felt252>,
            }

            extern fn enum_boxed_match<T>(e: Box<T>) -> BoxedWrapper nopanic;

            fn test_single() -> felt252 {
                let val = Wrapper::Value(99);
                let boxed = identity(BoxTrait::new(val));
                match enum_boxed_match(boxed) {
                    BoxedWrapper::Value(v) => BoxTrait::unbox(v),
                }
            }

            #[inline(never)]
            // Prevent the compiler from optimizing enum variants.
            fn identity<T>(x: T) -> T {
                x
            }
        };

        run_program_assert_output(&program, "test_single", &[], Felt::from(99).into());
    }

    #[test]
    fn enum_boxed_match_snapshot() {
        let program = load_cairo! {
            use core::box::BoxTrait;

            #[derive(Drop)]
            enum MyEnum {
                A: felt252,
                B: u128,
            }

            enum BoxedSnapMyEnum {
                A: Box<@felt252>,
                B: Box<@u128>,
            }

            extern fn enum_boxed_match<T>(e: Box<T>) -> BoxedSnapMyEnum nopanic;

            fn test_snapshot_a() -> felt252 {
                let val = MyEnum::A(42);
                let snap: @MyEnum = @val;
                let boxed: Box<@MyEnum> = identity(BoxTrait::new(snap));
                match enum_boxed_match(boxed) {
                    BoxedSnapMyEnum::A(v) => *BoxTrait::unbox(v),
                    BoxedSnapMyEnum::B(_) => 0,
                }
            }

            fn test_snapshot_b() -> u128 {
                let val = MyEnum::B(123_u128);
                let snap: @MyEnum = @val;
                let boxed: Box<@MyEnum> = identity(BoxTrait::new(snap));
                match enum_boxed_match(boxed) {
                    BoxedSnapMyEnum::A(_) => 0_u128,
                    BoxedSnapMyEnum::B(v) => *BoxTrait::unbox(v),
                }
            }

            #[inline(never)]
            // Prevent the compiler from optimizing enum variants.
            fn identity<T>(x: T) -> T {
                x
            }
        };

        run_program_assert_output(&program, "test_snapshot_a", &[], Felt::from(42).into());
        run_program_assert_output(&program, "test_snapshot_b", &[], 123u128.into());
    }

    #[test]
    fn enum_boxed_match_different_padding() {
        // Variants with different sizes/alignments exercise different payload
        // offsets in the GEP-based extraction: u8 (1 byte), u128 (16 bytes),
        // and (felt252, u128) (a 48-byte struct with 16-byte alignment).
        let program = load_cairo! {
            use core::box::BoxTrait;

            enum Mixed {
                Small: u8,
                Medium: u128,
                Large: (felt252, u128),
            }

            enum BoxedMixed {
                Small: Box<u8>,
                Medium: Box<u128>,
                Large: Box<(felt252, u128)>,
            }

            extern fn enum_boxed_match<T>(e: Box<T>) -> BoxedMixed nopanic;

            fn test_small() -> u8 {
                let boxed = identity(BoxTrait::new(Mixed::Small(7_u8)));
                match enum_boxed_match(boxed) {
                    BoxedMixed::Small(v) => BoxTrait::unbox(v),
                    BoxedMixed::Medium(_) => 0_u8,
                    BoxedMixed::Large(_) => 0_u8,
                }
            }

            fn test_medium() -> u128 {
                let boxed = identity(BoxTrait::new(Mixed::Medium(999_u128)));
                match enum_boxed_match(boxed) {
                    BoxedMixed::Small(_) => 0_u128,
                    BoxedMixed::Medium(v) => BoxTrait::unbox(v),
                    BoxedMixed::Large(_) => 0_u128,
                }
            }

            fn test_large() -> (felt252, u128) {
                let boxed = identity(BoxTrait::new(Mixed::Large((42, 123_u128))));
                match enum_boxed_match(boxed) {
                    BoxedMixed::Small(_) => (0, 0_u128),
                    BoxedMixed::Medium(_) => (0, 0_u128),
                    BoxedMixed::Large(v) => BoxTrait::unbox(v),
                }
            }

            #[inline(never)]
            // Prevent the compiler from optimizing enum variants.
            fn identity<T>(x: T) -> T {
                x
            }
        };

        run_program_assert_output(&program, "test_small", &[], 7u8.into());
        run_program_assert_output(&program, "test_medium", &[], 999u128.into());
        run_program_assert_output(
            &program,
            "test_large",
            &[],
            jit_struct!(Felt::from(42).into(), 123u128.into()),
        );
    }
}
