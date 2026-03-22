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
    metadata::MetadataStorage,
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
    dialect::llvm,
    ir::{r#type::IntegerType, Module, Type},
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
    _module: &Module<'ctx>,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _metadata: &mut MetadataStorage,
    _info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    // Arena-allocated [u32; 8].  No dup/drop overrides needed — bitwise
    // copy of the pointer is safe and drop is a noop (arena owns memory).
    Ok(llvm::r#type::pointer(context, 0))
}
