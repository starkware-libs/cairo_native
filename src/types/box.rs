//! # Box type
//!
//! The type box for a given type `T`.
//!
//! ## Layout
//!
//! Its layout is that of whatever it wraps. In other words, if it was Rust it would be equivalent
//! to the following:
//!
//! ```
//! #[repr(transparent)]
//! pub struct Box<T>(pub T);
//! ```

use super::WithSelf;
use crate::{
    error::Result,
    metadata::{
        drop_overrides::DropOverridesMeta, dup_overrides::DupOverridesMeta,
        runtime_bindings::RuntimeBindingsMeta, MetadataStorage,
    },
    types::TypeBuilder,
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
    dialect::{func, llvm},
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{Block, BlockLike, Location, Module, Region, Type},
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

    let size_val = entry.const_int(context, location, inner_len, 64)?;
    let align_val = entry.const_int(context, location, inner_align, 64)?;

    let src_value = entry.arg(0)?;
    // build_dup is only registered when the inner type has a dup override.
    let rtb = metadata.get_or_insert_with(RuntimeBindingsMeta::default);
    let dst_value = rtb.arena_alloc(context, module, &entry, location, size_val, align_val)?;

    let value = entry.load(context, location, src_value, inner_ty)?;
    let values = DupOverridesMeta::invoke_override(
        context, registry, module, &entry, &entry, location, metadata, &info.ty, value,
    )?;
    entry.store(context, location, src_value, values.0)?;
    entry.store(context, location, dst_value, values.1)?;

    entry.append_operation(func::r#return(&[src_value, dst_value], location));
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

    let inner_ty = registry.build_type(context, module, metadata, &info.ty)?;

    let region = Region::new();
    let entry = region.append_block(Block::new(&[(llvm::r#type::pointer(context, 0), location)]));

    // build_drop is only registered when the inner type has a drop override.
    let value = entry.arg(0)?;
    let value = entry.load(context, location, value, inner_ty)?;
    DropOverridesMeta::invoke_override(
        context, registry, module, &entry, &entry, location, metadata, &info.ty, value,
    )?;

    entry.append_operation(func::r#return(&[], location));
    Ok(region)
}
