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

use super::WithSelf;
use crate::{error::Result, metadata::MetadataStorage, utils::ProgramRegistryExt};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        types::InfoAndTypeConcreteType,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::llvm,
    helpers::{GepIndex, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, Location, Module, Type, Value},
    Context,
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
    // Ensure the element type is built.
    registry.build_type(context, module, metadata, &info.ty)?;

    let ptr_ty = llvm::r#type::pointer(context, 0);
    let len_ty = IntegerType::new(context, 32).into();

    Ok(llvm::r#type::r#struct(
        context,
        &[ptr_ty, len_ty, len_ty, len_ty],
        false,
    ))
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

/// Returns a pointer to the given `field` in the ArrayMetadata struct.
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

    #[test]
    fn test_dup_snapshots_of_dictionary_array() {
        let program = get_compiled_program(
            "test_data_artifacts/programs/types/dup_snapshots_of_dictionary_array",
        );
        let result = run_program(&program, "run_test", &[]).return_value;
        assert_eq!(result, Value::Felt252(6.into()));
    }
}
