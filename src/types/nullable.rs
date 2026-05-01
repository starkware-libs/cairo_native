//! # Nullable type
//!
//! Nullable is represented as a pointer, usually the null value will point to a alloca in the stack.
//!
//! A nullable is functionally equivalent to Rust's `Option<Box<T>>`. Since it's always paired with
//! `Box<T>` we can reuse its pointer, just leaving it null when there's no value.

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
    dialect::{cf, func},
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{BlockLike, Region},
};
use melior::{
    dialect::{llvm, ods},
    ir::{attribute::IntegerAttribute, r#type::IntegerType, Block, Location, Module, Type},
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
    DupOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            registry.build_type(context, module, metadata, &info.ty)?;
            if DupOverridesMeta::is_overriden(metadata, &info.ty) {
                Ok(Some(build_dup(context, module, registry, metadata, &info)?))
            } else {
                Ok(None)
            }
        },
    )?;
    DropOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            registry.build_type(context, module, metadata, &info.ty)?;
            if DropOverridesMeta::is_overriden(metadata, &info.ty) {
                Ok(Some(build_drop(
                    context, module, registry, metadata, &info,
                )?))
            } else {
                Ok(None)
            }
        },
    )?;

    // A nullable is represented by a pointer (equivalent to a box). A null value means no value.
    Ok(llvm::r#type::pointer(context, 0))
}

fn build_dup<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: &WithSelf<InfoAndTypeConcreteType>,
) -> Result<Region<'ctx>> {
    let location = Location::unknown(context);

    let inner_ty = registry.get_type(&info.ty)?;
    let inner_layout = inner_ty.layout(registry)?;
    let inner_len = inner_layout.size();
    let inner_align = inner_layout.align();
    let inner_ty = inner_ty.build(context, module, registry, metadata, &info.ty)?;

    let region = Region::new();
    let entry = region.append_block(Block::new(&[(llvm::r#type::pointer(context, 0), location)]));

    let null_ptr =
        entry.append_op_result(llvm::zero(llvm::r#type::pointer(context, 0), location))?;

    let src_value = entry.arg(0)?;
    let src_is_null = entry.append_op_result(
        ods::llvm::icmp(
            context,
            IntegerType::new(context, 1).into(),
            src_value,
            null_ptr,
            IntegerAttribute::new(IntegerType::new(context, 64).into(), 0).into(),
            location,
        )
        .into(),
    )?;

    let block_realloc = region.append_block(Block::new(&[]));
    let block_finish =
        region.append_block(Block::new(&[(llvm::r#type::pointer(context, 0), location)]));
    entry.append_operation(cf::cond_br(
        context,
        src_is_null,
        &block_finish,
        &block_realloc,
        &[null_ptr],
        &[],
        location,
    ));

    {
        // build_dup is only registered when the inner type has a dup override.
        let size_val = block_realloc.const_int(context, location, inner_len, 64)?;
        let align_val = block_realloc.const_int(context, location, inner_align, 64)?;
        let rtb = metadata.get_or_insert_with(RuntimeBindingsMeta::default);
        let dst_value = rtb.box_alloc(
            context,
            module,
            &block_realloc,
            location,
            size_val,
            align_val,
        )?;

        let value = block_realloc.load(context, location, src_value, inner_ty)?;
        let values = DupOverridesMeta::invoke_override(
            context,
            registry,
            module,
            &block_realloc,
            &block_realloc,
            location,
            metadata,
            &info.ty,
            value,
        )?;
        block_realloc.store(context, location, src_value, values.0)?;
        block_realloc.store(context, location, dst_value, values.1)?;

        block_realloc.append_operation(cf::br(&block_finish, &[dst_value], location));
    }

    block_finish.append_operation(func::r#return(&[src_value, block_finish.arg(0)?], location));
    Ok(region)
}

fn build_drop<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: &WithSelf<InfoAndTypeConcreteType>,
) -> Result<Region<'ctx>> {
    let location = Location::unknown(context);

    // build_drop is only registered when the inner type has a drop override.
    let inner_ty = registry.build_type(context, module, metadata, &info.ty)?;

    let region = Region::new();
    let entry = region.append_block(Block::new(&[(llvm::r#type::pointer(context, 0), location)]));

    let null_ptr =
        entry.append_op_result(llvm::zero(llvm::r#type::pointer(context, 0), location))?;

    let value = entry.arg(0)?;
    let is_null = entry.append_op_result(
        ods::llvm::icmp(
            context,
            IntegerType::new(context, 1).into(),
            value,
            null_ptr,
            IntegerAttribute::new(IntegerType::new(context, 64).into(), 0).into(),
            location,
        )
        .into(),
    )?;

    let block_drop = region.append_block(Block::new(&[]));
    let block_finish = region.append_block(Block::new(&[]));
    entry.append_operation(cf::cond_br(
        context,
        is_null,
        &block_finish,
        &block_drop,
        &[],
        &[],
        location,
    ));

    {
        // No free: the pointer lives in the arena and is reclaimed at invocation end.
        let inner_value = block_drop.load(context, location, value, inner_ty)?;
        DropOverridesMeta::invoke_override(
            context,
            registry,
            module,
            &block_drop,
            &block_drop,
            location,
            metadata,
            &info.ty,
            inner_value,
        )?;
        block_drop.append_operation(cf::br(&block_finish, &[], location));
    }

    block_finish.append_operation(func::r#return(&[], location));
    Ok(region)
}

#[cfg(test)]
mod test {
    use crate::{
        jit_enum, jit_struct,
        utils::testing::{get_compiled_program, run_program},
        values::Value,
    };
    use pretty_assertions_sorted::assert_eq;

    #[test]
    fn test_nullable_deep_clone() {
        let program =
            get_compiled_program("test_data_artifacts/programs/types/nullable_deep_clone");
        let result = run_program(&program, "run_test", &[]).return_value;

        assert_eq!(
            result,
            jit_enum!(
                0,
                jit_struct!(Value::Array(vec![
                    Value::Felt252(1.into()),
                    Value::Felt252(2.into()),
                    Value::Felt252(3.into()),
                ]))
            ),
        );
    }
}
