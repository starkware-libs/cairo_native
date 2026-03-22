//! # Array type
//!
//! An array type is a dynamically allocated list of items.
//!
//! ## Layout
//!
//! Being dynamically allocated, we just need to keep the pointer to the metadata, its length and
//! its capacity:
//!
//! | Index | Type        | Description                   |
//! | ----- | ----------- | ----------------------------- |
//! |   0   | `!llvm.ptr` | Pointer to ArrayMetadata[^1]. |
//! |   1   | `i32`       | Array start offset[^2].       |
//! |   2   | `i32`       | Array end offset[^2].         |
//! |   3   | `i32`       | Allocated capacity[^2].       |
//!
//! The ArrayMetadata struct contains:
//!   1. Array max length (`u32`).
//!   2. Pointer to array data (`*mut u8`).
//!
//! [^1]: This pointer is null when the array has not yet been allocated (i.e., initially).
//! [^2]: Those numbers are number of items, **not bytes**.

use super::{TypeBuilder, WithSelf};
use crate::{
    error::Result,
    metadata::{
        drop_overrides::DropOverridesMeta, dup_overrides::DupOverridesMeta,
        runtime_bindings::RuntimeBindingsMeta, MetadataStorage,
    },
    utils::ProgramRegistryExt,
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        types::InfoAndTypeConcreteType,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{func, llvm, ods, scf},
    ir::{
        attribute::IntegerAttribute, r#type::IntegerType, Block, BlockLike, Location, Module, Type,
        Value,
    },
    Context,
};
use melior::{
    helpers::{ArithBlockExt, BuiltinBlockExt, GepIndex, LlvmBlockExt},
    ir::Region,
};

/// Build the MLIR type.
///
/// Check out [the module](self) for more info.
pub fn build<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoAndTypeConcreteType>,
) -> Result<Type<'ctx>> {
    DupOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            // There's no need to build the type here because it'll always be built within
            // `build_dup`.

            Ok(Some(build_dup(context, module, registry, metadata, &info)?))
        },
    )?;
    DropOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            // There's no need to build the type here because it'll always be built within
            // `build_drop`.

            Ok(Some(build_drop(
                context, module, registry, metadata, &info,
            )?))
        },
    )?;

    let ptr_ty = llvm::r#type::pointer(context, 0);
    let len_ty = IntegerType::new(context, 32).into();

    Ok(llvm::r#type::r#struct(
        context,
        &[ptr_ty, len_ty, len_ty, len_ty],
        false,
    ))
}

/// Array dup.
///
/// When the inner element type has a drop or dup override we must deep-copy
/// the data buffer (and invoke the element dup override if one exists) so
/// that each copy can be dropped independently. Otherwise the noop dup
/// (returning the same value twice) is safe because drop is also a noop.
pub fn build_dup<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: &WithSelf<InfoAndTypeConcreteType>,
) -> Result<Region<'ctx>> {
    let location = Location::unknown(context);
    let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;

    let has_drop = DropOverridesMeta::is_overriden(metadata, &info.ty);
    let has_dup = DupOverridesMeta::is_overriden(metadata, &info.ty);

    let region = Region::new();
    let entry = region.append_block(Block::new(&[(value_ty, location)]));

    let arr = entry.argument(0)?.into();

    if !has_drop && !has_dup {
        // Inner type is trivial — noop dup is safe.
        entry.append_operation(func::r#return(&[arr, arr], location));
        return Ok(region);
    }

    // --- Deep-copy path ---

    let ptr_ty = llvm::r#type::pointer(context, 0);
    let i64_ty = IntegerType::new(context, 64).into();
    let i8_ty = IntegerType::new(context, 8).into();

    let elem_info = registry.get_type(&info.ty)?;
    let elem_stride = elem_info.layout(registry)?.pad_to_align().size();
    let elem_align = elem_info.layout(registry)?.align();
    let elem_ty = elem_info.build(context, module, registry, metadata, &info.ty)?;

    let metadata_ptr = entry.extract_value(context, location, arr, ptr_ty, 0)?;

    let null_ptr = entry.append_op_result(llvm::zero(ptr_ty, location))?;
    let is_null = entry.append_op_result(
        ods::llvm::icmp(
            context,
            IntegerType::new(context, 1).into(),
            metadata_ptr,
            null_ptr,
            IntegerAttribute::new(IntegerType::new(context, 64).into(), 0).into(),
            location,
        )
        .into(),
    )?;

    let new_arr = entry.append_op_result(scf::r#if(
        is_null,
        &[value_ty],
        {
            // Null metadata → empty array, noop.
            let region = Region::new();
            let block = region.append_block(Block::new(&[]));
            block.append_operation(scf::r#yield(&[arr], location));
            region
        },
        {
            let region = Region::new();
            let block = region.append_block(Block::new(&[]));

            let elem_stride_val = block.const_int(context, location, elem_stride, 64)?;
            let k0 = block.const_int(context, location, 0, 64)?;

            // Load max_len and data_ptr from old metadata.
            let max_len = load_max_len(context, &block, location, metadata_ptr)?;
            let max_len_64 = block.extui(max_len, i64_ty, location)?;
            let data_ptr = load_data_ptr(context, &block, location, metadata_ptr)?;

            // Allocate new data buffer from arena and copy old contents.
            let data_size = block.muli(max_len_64, elem_stride_val, location)?;
            let data_align_val = block.const_int(context, location, elem_align, 64)?;
            let rtb = metadata.get_or_insert_with(RuntimeBindingsMeta::default);

            let new_data_ptr =
                rtb.arena_alloc(context, module, &block, location, data_size, data_align_val)?;
            block.memcpy(context, location, data_ptr, new_data_ptr, data_size);

            // If inner type has a dup override, iterate elements and invoke it
            // on each pair so that ref-counted / owned inner values are properly
            // duplicated.
            if has_dup {
                let offset_end = block.muli(max_len_64, elem_stride_val, location)?;

                block.append_operation(scf::r#for(
                    k0,
                    offset_end,
                    elem_stride_val,
                    {
                        let region = Region::new();
                        let inner = region.append_block(Block::new(&[(i64_ty, location)]));

                        let offset = inner.argument(0)?.into();

                        // Pointer to element in old buffer.
                        let src_ptr = inner.gep(
                            context,
                            location,
                            data_ptr,
                            &[GepIndex::Value(offset)],
                            i8_ty,
                        )?;
                        let val = inner.load(context, location, src_ptr, elem_ty)?;

                        let (orig, copy) = DupOverridesMeta::invoke_override(
                            context, registry, module, &inner, &inner, location, metadata,
                            &info.ty, val,
                        )?;

                        // Write back to the original buffer (dup may update
                        // the value, e.g. Rc refcount).
                        inner.store(context, location, src_ptr, orig)?;

                        // Write copy to new buffer.
                        let dst_ptr = inner.gep(
                            context,
                            location,
                            new_data_ptr,
                            &[GepIndex::Value(offset)],
                            i8_ty,
                        )?;
                        inner.store(context, location, dst_ptr, copy)?;

                        inner.append_operation(scf::r#yield(&[], location));
                        region
                    },
                    location,
                ));
            }

            // Allocate new metadata and populate it.
            let meta_size = block.const_int(context, location, calc_metadata_size(), 64)?;
            let meta_align = block.const_int(context, location, calc_metadata_align(), 64)?;
            let rtb = metadata.get_or_insert_with(RuntimeBindingsMeta::default);

            let new_meta =
                rtb.arena_alloc(context, module, &block, location, meta_size, meta_align)?;

            store_max_len(context, &block, location, new_meta, max_len)?;
            store_data_ptr(context, &block, location, new_meta, new_data_ptr)?;

            // Build new array struct with the new metadata pointer and
            // capacity equal to max_len (matches the size of the new buffer
            // allocated above;
            let new_arr = block.insert_value(context, location, arr, new_meta, 0)?;
            let new_arr = block.insert_value(context, location, new_arr, max_len, 3)?;

            block.append_operation(scf::r#yield(&[new_arr], location));
            region
        },
        location,
    ))?;

    entry.append_operation(func::r#return(&[arr, new_arr], location));
    Ok(region)
}

/// Memory noop drop — arena owns all memory.
///
/// If the inner element type has a drop override, we iterate over `0..max_len`
/// elements and invoke the element drop. Otherwise this is a pure noop.
pub fn build_drop<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: &WithSelf<InfoAndTypeConcreteType>,
) -> Result<Region<'ctx>> {
    let location = Location::unknown(context);

    let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;

    let elem_ty = registry.get_type(&info.ty)?;
    let elem_stride = elem_ty.layout(registry)?.pad_to_align().size();
    let elem_ty = elem_ty.build(context, module, registry, metadata, &info.ty)?;

    let region = Region::new();
    let entry = region.append_block(Block::new(&[(value_ty, location)]));

    if DropOverridesMeta::is_overriden(metadata, &info.ty) {
        let metadata_ptr = entry.extract_value(
            context,
            location,
            entry.argument(0)?.into(),
            llvm::r#type::pointer(context, 0),
            0,
        )?;

        let null_ptr =
            entry.append_op_result(llvm::zero(llvm::r#type::pointer(context, 0), location))?;
        let is_null = entry.append_op_result(
            ods::llvm::icmp(
                context,
                IntegerType::new(context, 1).into(),
                metadata_ptr,
                null_ptr,
                IntegerAttribute::new(IntegerType::new(context, 64).into(), 0).into(),
                location,
            )
            .into(),
        )?;

        entry.append_operation(scf::r#if(
            is_null,
            &[],
            {
                let region = Region::new();
                let block = region.append_block(Block::new(&[]));
                block.append_operation(scf::r#yield(&[], location));
                region
            },
            {
                let region = Region::new();
                let block = region.append_block(Block::new(&[]));

                let k0 = block.const_int(context, location, 0, 64)?;
                let elem_stride_val = block.const_int(context, location, elem_stride, 64)?;

                let max_len = load_max_len(context, &block, location, metadata_ptr)?;
                let max_len =
                    block.extui(max_len, IntegerType::new(context, 64).into(), location)?;
                let offset_end = block.muli(max_len, elem_stride_val, location)?;

                let data_ptr = load_data_ptr(context, &block, location, metadata_ptr)?;

                // Drop each element in the array.
                block.append_operation(scf::r#for(
                    k0,
                    offset_end,
                    elem_stride_val,
                    {
                        let region = Region::new();
                        let block = region.append_block(Block::new(&[(
                            IntegerType::new(context, 64).into(),
                            location,
                        )]));

                        let elem_offset = block.argument(0)?.into();
                        let elem_ptr = block.gep(
                            context,
                            location,
                            data_ptr,
                            &[GepIndex::Value(elem_offset)],
                            IntegerType::new(context, 8).into(),
                        )?;
                        let elem_val = block.load(context, location, elem_ptr, elem_ty)?;

                        DropOverridesMeta::invoke_override(
                            context, registry, module, &block, &block, location, metadata,
                            &info.ty, elem_val,
                        )?;

                        block.append_operation(scf::r#yield(&[], location));
                        region
                    },
                    location,
                ));

                block.append_operation(scf::r#yield(&[], location));
                region
            },
            location,
        ));
    }

    entry.append_operation(func::r#return(&[], location));
    Ok(region)
}

/// Metadata struct definition for arrays (maxlen, data_ptr).
#[repr(C)]
pub struct ArrayMetadata {
    pub max_len: u32,
    pub data_ptr: *mut u8,
}

/// Returns the metadata struct layout size.
pub fn calc_metadata_size() -> usize {
    std::mem::size_of::<ArrayMetadata>()
}

/// Returns the metadata struct alignment.
pub fn calc_metadata_align() -> usize {
    std::mem::align_of::<ArrayMetadata>()
}

/// Get the LLVM struct type for ArrayMetadata: { max_len: i32, data_ptr: ptr }
///
/// Field indices:
/// - 0: max_len (u32)
/// - 1: data_ptr (*mut u8)
pub fn get_metadata_llvm_type(context: &Context) -> Type<'_> {
    llvm::r#type::r#struct(
        context,
        &[
            IntegerType::new(context, 32).into(), // max_len: u32
            llvm::r#type::pointer(context, 0),    // data_ptr: *mut u8
        ],
        false,
    )
}

#[repr(i32)]
enum MetadataField {
    MaxLen = 0,
    DataPtr = 1,
}

fn metadata_field_ptr<'ctx, 'this>(
    context: &'ctx Context,
    block: &'this Block<'ctx>,
    location: Location<'ctx>,
    metadata_ptr: Value<'ctx, 'this>,
    field: MetadataField,
) -> Result<Value<'ctx, 'this>> {
    Ok(block.gep(
        context,
        location,
        metadata_ptr,
        &[GepIndex::Const(0), GepIndex::Const(field as i32)],
        get_metadata_llvm_type(context),
    )?)
}

/// Load `max_len` (i32) from the ArrayMetadata struct.
pub fn load_max_len<'ctx, 'this>(
    context: &'ctx Context,
    block: &'this Block<'ctx>,
    location: Location<'ctx>,
    metadata_ptr: Value<'ctx, 'this>,
) -> Result<Value<'ctx, 'this>> {
    let field_ptr = metadata_field_ptr(
        context,
        block,
        location,
        metadata_ptr,
        MetadataField::MaxLen,
    )?;
    Ok(block.load(
        context,
        location,
        field_ptr,
        IntegerType::new(context, 32).into(),
    )?)
}

/// Load `data_ptr` from the ArrayMetadata struct.
pub fn load_data_ptr<'ctx, 'this>(
    context: &'ctx Context,
    block: &'this Block<'ctx>,
    location: Location<'ctx>,
    metadata_ptr: Value<'ctx, 'this>,
) -> Result<Value<'ctx, 'this>> {
    let field_ptr = metadata_field_ptr(
        context,
        block,
        location,
        metadata_ptr,
        MetadataField::DataPtr,
    )?;
    Ok(block.load(
        context,
        location,
        field_ptr,
        llvm::r#type::pointer(context, 0),
    )?)
}

/// Store `max_len` (i32) into the ArrayMetadata struct.
pub fn store_max_len<'ctx, 'this>(
    context: &'ctx Context,
    block: &'this Block<'ctx>,
    location: Location<'ctx>,
    metadata_ptr: Value<'ctx, 'this>,
    max_len: Value<'ctx, 'this>,
) -> Result<()> {
    let field_ptr = metadata_field_ptr(
        context,
        block,
        location,
        metadata_ptr,
        MetadataField::MaxLen,
    )?;
    block.store(context, location, field_ptr, max_len)?;
    Ok(())
}

/// Store `data_ptr` into the ArrayMetadata struct.
pub fn store_data_ptr<'ctx, 'this>(
    context: &'ctx Context,
    block: &'this Block<'ctx>,
    location: Location<'ctx>,
    metadata_ptr: Value<'ctx, 'this>,
    data_ptr: Value<'ctx, 'this>,
) -> Result<()> {
    let field_ptr = metadata_field_ptr(
        context,
        block,
        location,
        metadata_ptr,
        MetadataField::DataPtr,
    )?;
    block.store(context, location, field_ptr, data_ptr)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{
        utils::testing::{get_compiled_program, run_program},
        values::Value,
    };
    use pretty_assertions_sorted::assert_eq;

    #[test]
    fn test_array_snapshot_deep_clone() {
        let program = get_compiled_program("test_data_artifacts/programs/types/nested_arrays");
        let result = run_program(&program, "run_test", &[]).return_value;

        assert_eq!(
            result,
            Value::Array(vec![
                Value::Array(vec![
                    Value::Felt252(1.into()),
                    Value::Felt252(2.into()),
                    Value::Felt252(3.into()),
                ]),
                Value::Array(vec![
                    Value::Felt252(4.into()),
                    Value::Felt252(5.into()),
                    Value::Felt252(6.into()),
                ]),
            ]),
        );
    }

    /// Noop array dup + element type with destructive drop = double-free.
    /// SquashedFelt252Dict's drop calls Rc::from_raw → dealloc.
    /// With buggy noop dup this SIGABRTs; with correct dup it returns 6.
    #[test]
    fn test_array_dup_no_double_free() {
        let program =
            get_compiled_program("test_data_artifacts/programs/types/array_dup_double_free");
        let result = run_program(&program, "run_test", &[]).return_value;
        assert_eq!(result, Value::Felt252(6.into()));
    }
}
