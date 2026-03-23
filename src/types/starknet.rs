//! # Starknet types
//!
//! ## ClassHash
//! Type for Starknet class hash, a value in the range [0, 2 ** 251).
//!
//! ## ContractAddress
//! Type for Starknet contract address, a value in the range [0, 2 ** 251).
//!
//! ## StorageBaseAddress
//! Type for Starknet storage base address, a value in the range [0, 2 ** 251 - 256).
//!
//! ## StorageAddress
//! Type for Starknet storage base address, a value in the range [0, 2 ** 251).
//!
//! ## System
//! Type for Starknet system object.
//! Used to make system calls.
//!
//! ## Secp256Point
//! TODO

// TODO: Maybe the types used here can be i251 instead of i252. See https://github.com/starkware-libs/cairo_native/issues/1226

use super::WithSelf;
use crate::{
    error::Result,
    metadata::{
        drop_overrides::DropOverridesMeta, dup_overrides::DupOverridesMeta,
        realloc_bindings::ReallocBindingsMeta, MetadataStorage,
    },
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        starknet::{secp256::Secp256PointTypeConcrete, StarknetTypeConcrete},
        types::InfoOnlyConcreteType,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{func, llvm, ods},
    helpers::{ArithBlockExt, BuiltinBlockExt},
    ir::{
        attribute::IntegerAttribute, r#type::IntegerType, Block, BlockLike, Location, Module,
        Region, Type,
    },
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
    selector: WithSelf<StarknetTypeConcrete>,
) -> Result<Type<'ctx>> {
    match &*selector {
        StarknetTypeConcrete::ClassHash(info) => build_class_hash(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        StarknetTypeConcrete::ContractAddress(info) => build_contract_address(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        StarknetTypeConcrete::StorageBaseAddress(info) => build_storage_base_address(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        StarknetTypeConcrete::StorageAddress(info) => build_storage_address(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        StarknetTypeConcrete::System(info) => build_system(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        StarknetTypeConcrete::Secp256Point(info) => build_secp256_point(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        StarknetTypeConcrete::Sha256StateHandle(info) => build_sha256_state_handle(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
    }
}

pub fn build_class_hash<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    // it's a felt252 value
    super::felt252::build(context, module, registry, metadata, info)
}

pub fn build_contract_address<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    // it's a felt252 value
    super::felt252::build(context, module, registry, metadata, info)
}

pub fn build_storage_base_address<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    // it's a felt252 value
    super::felt252::build(context, module, registry, metadata, info)
}

pub fn build_storage_address<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    // it's a felt252 value
    super::felt252::build(context, module, registry, metadata, info)
}

pub fn build_system<'ctx>(
    context: &'ctx Context,
    _module: &Module<'ctx>,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _metadata: &mut MetadataStorage,
    _info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    Ok(llvm::r#type::pointer(context, 0))
}

pub fn build_secp256_point<'ctx>(
    context: &'ctx Context,
    _module: &Module<'ctx>,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _metadata: &mut MetadataStorage,
    _info: WithSelf<Secp256PointTypeConcrete>,
) -> Result<Type<'ctx>> {
    Ok(llvm::r#type::r#struct(
        context,
        &[
            llvm::r#type::r#struct(
                context,
                &[
                    IntegerType::new(context, 128).into(),
                    IntegerType::new(context, 128).into(),
                ],
                false,
            ),
            llvm::r#type::r#struct(
                context,
                &[
                    IntegerType::new(context, 128).into(),
                    IntegerType::new(context, 128).into(),
                ],
                false,
            ),
            IntegerType::new(context, 1).into(),
        ],
        false,
    ))
}

pub fn build_sha256_state_handle<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    let location = Location::unknown(context);
    if metadata.get::<ReallocBindingsMeta>().is_none() {
        metadata.insert(ReallocBindingsMeta::new(context, module));
    }

    DupOverridesMeta::register_with(context, module, registry, metadata, info.self_ty(), |_| {
        let region = Region::new();
        let block =
            region.append_block(Block::new(&[(llvm::r#type::pointer(context, 0), location)]));

        let null_ptr =
            block.append_op_result(llvm::zero(llvm::r#type::pointer(context, 0), location))?;
        let k32 = block.const_int(context, location, 32, 64)?;
        let new_ptr = block.append_op_result(ReallocBindingsMeta::realloc(
            context, null_ptr, k32, location,
        )?)?;

        block.append_operation(
            ods::llvm::intr_memcpy_inline(
                context,
                new_ptr,
                block.arg(0)?,
                IntegerAttribute::new(IntegerType::new(context, 64).into(), 32),
                IntegerAttribute::new(IntegerType::new(context, 1).into(), 0),
                location,
            )
            .into(),
        );

        block.append_operation(func::r#return(&[block.arg(0)?, new_ptr], location));
        Ok(Some(region))
    })?;
    DropOverridesMeta::register_with(context, module, registry, metadata, info.self_ty(), |_| {
        let region = Region::new();
        let block =
            region.append_block(Block::new(&[(llvm::r#type::pointer(context, 0), location)]));

        block.append_operation(ReallocBindingsMeta::free(context, block.arg(0)?, location)?);

        block.append_operation(func::r#return(&[], location));
        Ok(Some(region))
    })?;

    // A ptr to a heap (realloc) allocated [u32; 8]
    Ok(llvm::r#type::pointer(context, 0))
}
