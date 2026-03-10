//! # Struct-related libfuncs

use super::LibfuncHelper;
use crate::{
    error::Result,
    libfuncs::r#box::{into_box, unbox},
    metadata::MetadataStorage,
    native_panic,
    types::TypeBuilder,
    utils::ProgramRegistryExt,
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        lib_func::SignatureOnlyConcreteLibfunc,
        structure::{ConcreteStructBoxedDeconstructLibfunc, StructConcreteLibfunc},
        ConcreteLibfunc,
    },
    ids::ConcreteTypeId,
    program_registry::ProgramRegistry,
};
use itertools::Itertools;
use melior::{
    dialect::llvm::{self},
    helpers::{BuiltinBlockExt, LlvmBlockExt},
    ir::{Block, BlockLike, Location, Value},
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
    selector: &StructConcreteLibfunc,
) -> Result<()> {
    match selector {
        StructConcreteLibfunc::Construct(info) => {
            build_construct(context, registry, entry, location, helper, metadata, info)
        }
        StructConcreteLibfunc::Deconstruct(info)
        | StructConcreteLibfunc::SnapshotDeconstruct(info) => {
            build_deconstruct(context, registry, entry, location, helper, metadata, info)
        }
        StructConcreteLibfunc::BoxedDeconstruct(info) => {
            build_boxed_deconstruct(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `struct_construct` libfunc.
pub fn build_construct<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let mut fields = Vec::new();

    for (i, _) in info.param_signatures().iter().enumerate() {
        fields.push(entry.argument(i)?.into());
    }

    let value = build_struct_value(
        context,
        registry,
        entry,
        location,
        helper,
        metadata,
        &info.branch_signatures()[0].vars[0].ty,
        &fields,
    )?;

    helper.br(entry, 0, &[value], location)
}

/// Generate MLIR operations for the `struct_construct` libfunc.
#[allow(clippy::too_many_arguments)]
pub fn build_struct_value<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    struct_type: &ConcreteTypeId,
    fields: &[Value<'ctx, 'this>],
) -> Result<Value<'ctx, 'this>> {
    let struct_ty = registry.build_type(context, helper, metadata, struct_type)?;

    let struct_type = registry.get_type(struct_type)?;

    // LLVM fails when inserting zero-sized types into a struct.
    // See: https://github.com/llvm/llvm-project/issues/107198.
    // We will manually skip ZST fields, as inserting a ZST is a noop, and there
    // is no point on building that operation.
    let zst_fields = match struct_type {
        CoreTypeConcrete::Struct(info) => {
            // If the type is a struct, we check for each member if its a ZST.
            info.members
                .iter()
                .map(|member| registry.get_type(member)?.is_zst(registry))
                .try_collect()?
        }
        _ => {
            // There are many Sierra types represented as an LLVM struct, but
            // are not of Sierra struct type. In these cases we assume that
            // their members are not ZST.
            vec![false; fields.len()]
        }
    };

    let mut accumulator = entry.append_op_result(llvm::undef(struct_ty, location))?;
    for (idx, field) in fields.iter().enumerate() {
        if zst_fields[idx] {
            continue;
        }

        accumulator = entry.insert_value(context, location, accumulator, *field, idx)?
    }

    Ok(accumulator)
}

/// Generate MLIR operations for the `struct_deconstruct` libfunc.
pub fn build_deconstruct<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let container = entry.arg(0)?;

    let mut fields = Vec::<Value>::with_capacity(info.branch_signatures()[0].vars.len());
    for (i, var_info) in info.branch_signatures()[0].vars.iter().enumerate() {
        let type_info = registry.get_type(&var_info.ty)?;
        let field_ty = type_info.build(context, helper, registry, metadata, &var_info.ty)?;

        let value = entry.extract_value(context, location, container, field_ty, i)?;

        fields.push(value);
    }

    helper.br(entry, 0, &fields, location)
}

/// Generate MLIR operations for the `struct_boxed_deconstruct` libfunc.
///
/// Receives a `Struct` inside a `Box` and returns a tuple containing each member
/// of the `Struct` wrapped inside a `Box`.
///
/// # Signature
///
/// ```cairo
/// struct MyStruct {
///     x: u8,
///     y: felt252,
/// }
///
/// extern fn struct_boxed_deconstruct<T>(
///     value: Box<T>
/// ) -> (Box<u8>, Box<felt252>) nopanic;
/// ```
pub fn build_boxed_deconstruct<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &ConcreteStructBoxedDeconstructLibfunc,
) -> Result<()> {

    // Unbox the container
    let CoreTypeConcrete::Box(box_info) = registry.get_type(&info.param_signatures()[0].ty)? else {
        native_panic!("Should receibe a Box type as argument");
    };
    let container = unbox(
        context,
        registry,
        entry,
        location,
        helper,
        metadata,
        &box_info.ty,
    )?;

    let mut fields = Vec::<Value>::with_capacity(info.members.len());
    for (i, member_type_id) in info.members.iter().enumerate() {
        let type_info = registry.get_type(member_type_id)?;
        let field_ty = type_info.build(context, helper, registry, metadata, member_type_id)?;

        let member = entry.extract_value(context, location, container, field_ty, i)?;
        let (_, member_layout) =
            registry.build_type_with_layout(context, helper, metadata, member_type_id)?;
        // Box the member
        let member = into_box(context, helper.module, entry, location, member, member_layout, metadata)?;

        fields.push(member);
    }

    helper.br(entry, 0, &fields, location)
}

#[cfg(test)]
mod test {
    use crate::{
        jit_struct,
        utils::testing::{get_compiled_program, run_program_assert_output},
        Value,
    };

    #[test]
    fn boxed_deconstruct_3_fields() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/struct_boxed_deconstruct");
        run_program_assert_output(
            &program,
            "deconstruct_struct_3_fields",
            &[],
            jit_struct!(Value::Felt252(2.into()), Value::Uint8(2), Value::Uint128(2)),
        );
    }

    #[test]
    fn boxed_deconstruct_1_field() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/struct_boxed_deconstruct");
        run_program_assert_output(
            &program,
            "deconstruct_struct_1_field",
            &[],
            jit_struct!(Value::Uint8(2)),
        );
    }

    #[test]
    fn boxed_deconstruct_empty_struct() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/struct_boxed_deconstruct");
        run_program_assert_output(&program, "deconstruct_empty_struct", &[], jit_struct!());
    }

    #[test]
    fn boxed_deconstruct_struct_snapshot() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/struct_boxed_deconstruct");
        run_program_assert_output(
            &program,
            "deconstruct_struct_snapshot",
            &[],
            jit_struct!(Value::Felt252(2.into()), Value::Uint8(2), Value::Uint128(2)),
        );
    }
}
