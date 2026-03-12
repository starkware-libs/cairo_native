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
        drop_overrides::DropOverridesMeta, dup_overrides::DupOverridesMeta, MetadataStorage,
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
    ir::{attribute::IntegerAttribute, r#type::IntegerType, Block, BlockLike, Location, Module, Type},
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

/// True noop dup — returns `(arg0, arg0)`.
///
/// With arena allocation and Cairo's write-once memory model, array dup is a
/// true noop: both SSA values share the same metadata + data pointers.
/// Snapshots are immutable so no writer conflicts can occur.
pub fn build_dup<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: &WithSelf<InfoAndTypeConcreteType>,
) -> Result<Region<'ctx>> {
    let location = Location::unknown(context);
    let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;

    let region = Region::new();
    let entry = region.append_block(Block::new(&[(value_ty, location)]));

    entry.append_operation(func::r#return(
        &[entry.argument(0)?.into(), entry.argument(0)?.into()],
        location,
    ));
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

                // Load max_len from metadata (field index 0)
                let max_len_ptr = block.gep(
                    context,
                    location,
                    metadata_ptr,
                    &[GepIndex::Const(0), GepIndex::Const(0)],
                    get_metadata_llvm_type(context),
                )?;
                let max_len = block.load(
                    context,
                    location,
                    max_len_ptr,
                    IntegerType::new(context, 32).into(),
                )?;
                let max_len =
                    block.extui(max_len, IntegerType::new(context, 64).into(), location)?;
                let offset_end = block.muli(max_len, elem_stride_val, location)?;

                // Load data_ptr from metadata (field index 1)
                let data_ptr_ptr = block.gep(
                    context,
                    location,
                    metadata_ptr,
                    &[GepIndex::Const(0), GepIndex::Const(1)],
                    get_metadata_llvm_type(context),
                )?;
                let data_ptr = block.load(
                    context,
                    location,
                    data_ptr_ptr,
                    llvm::r#type::pointer(context, 0),
                )?;

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
/// Note: capacity stays in the array struct, not here!
///
/// Refcount has been removed — all array metadata is arena-allocated and freed
/// at invocation end. Cairo's write-once memory model means COW is unnecessary.
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
}
