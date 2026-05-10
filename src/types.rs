//! # Compiler type infrastructure
//!
//! Contains type generation stuff (aka. conversion from Sierra to MLIR types).

use crate::{
    error::Error as CoreTypeBuilderError,
    libfuncs::LibfuncHelper,
    metadata::MetadataStorage,
    native_panic,
    utils::{get_integer_layout, layout_repeat, RangeExt, PRIME},
};
use cairo_lang_sierra::{
    extensions::{
        circuit::CircuitTypeConcrete,
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        starknet::StarknetTypeConcrete,
        utils::Range,
    },
    ids::{ConcreteTypeId, UserTypeId},
    program::GenericArg,
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::llvm,
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, Location, Module, Type, Value},
    Context,
};
use num_bigint::{BigInt, Sign};
use num_traits::{Bounded, One};
use std::{alloc::Layout, error::Error, ops::Deref, sync::OnceLock};

pub mod array;
mod bitwise;
mod bounded_int;
mod r#box;
mod builtin_costs;
mod bytes31;
pub mod circuit;
mod coupon;
mod ec_op;
mod ec_point;
mod ec_state;
pub mod r#enum;
mod felt252;
mod felt252_dict;
mod felt252_dict_entry;
mod gas_builtin;
mod gas_reserve;
mod int_range;
mod non_zero;
mod nullable;
mod pedersen;
mod poseidon;
mod qm31;
mod range_check;
mod segment_arena;
mod snapshot;
mod squashed_felt252_dict;
mod starknet;
mod r#struct;
mod uint128;
mod uint128_mul_guarantee;
mod uint16;
mod uint32;
mod uint64;
mod uint8;
mod uninitialized;

/// Generation of MLIR types from their Sierra counterparts.
///
/// All possible Sierra types must implement it. It is already implemented for all the core Sierra
/// types, contained in [CoreTypeConcrete].
pub trait TypeBuilder {
    /// Error type returned by this trait's methods.
    type Error: Error;

    /// Build the MLIR type.
    fn build<'ctx>(
        &self,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
        metadata: &mut MetadataStorage,
        self_ty: &ConcreteTypeId,
    ) -> Result<Type<'ctx>, Self::Error>;

    /// Return whether the type is a builtin.
    fn is_builtin(&self) -> bool;
    /// Return whether the type requires a return pointer when returning,
    /// instead of using the CPU registers.
    ///
    /// This attribute does not modify the compilation, and it only reflects
    /// what the ABI of the target architecture already specifies.
    /// - For x86-64: https://gitlab.com/x86-psABIs/x86-64-ABI.
    /// - For AArch64: https://github.com/ARM-software/abi-aa.
    ///
    /// We can validate this empirically, by building a Cairo program that
    /// returns a particular type, and seeing how it is lowered to machine code.
    ///
    /// ```bash
    /// llc a.llvmir -o - --mtriple "aarch64"
    /// llc a.llvmir -o - --mtriple "x86_64"
    /// ```
    fn is_complex(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error>;

    /// Return whether the Sierra type resolves to a zero-sized type.
    fn is_zst(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error>;

    /// Generate the layout of the MLIR type.
    ///
    /// Used in both the compiler and the interface when calling the compiled code.
    fn layout(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<Layout, Self::Error>;

    /// Whether the layout should be allocated in memory (either the stack or
    /// the heap) when used as a function invocation argument or return value.
    ///
    /// Unlike `is_complex`, this attribute alters the compilation:
    ///
    /// - When passing a memory allocated value to a function, we allocate that
    ///   value on the stack, and pass a pointer to it.
    ///
    /// - If a function returns a memory allocated value, we receive a return
    ///   pointer as its first argument, and write the return value there
    ///   instead.
    ///
    /// The rationale behind allocating a value in memory, rather than
    /// registers, is to avoid putting too much pressure on the register
    /// allocation pass for really complex types, like enums.
    fn is_memory_allocated(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error>;

    /// If the type is an integer, return its value range.
    fn integer_range(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<Range, Self::Error>;

    /// Return whether the type is a `BoundedInt<>`, either directly or indirectly (ex. through
    /// `NonZero<BoundedInt<>>`).
    fn is_bounded_int(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error>;

    /// Return whether the type is a `felt252`, either directly or indirectly (ex. through
    /// `NonZero<BoundedInt<>>`).
    fn is_felt252(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error>;

    /// If the type is a enum type, return all possible variants.
    ///
    /// TODO: How is it used?
    fn variants(&self) -> Option<&[ConcreteTypeId]>;

    #[allow(clippy::too_many_arguments)]
    fn build_default<'ctx, 'this>(
        &self,
        context: &'ctx Context,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
        entry: &'this Block<'ctx>,
        location: Location<'ctx>,
        helper: &LibfuncHelper<'ctx, 'this>,
        metadata: &mut MetadataStorage,
        self_ty: &ConcreteTypeId,
    ) -> Result<Value<'ctx, 'this>, Self::Error>;
}

impl TypeBuilder for CoreTypeConcrete {
    type Error = CoreTypeBuilderError;

    fn build<'ctx>(
        &self,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
        metadata: &mut MetadataStorage,
        self_ty: &ConcreteTypeId,
    ) -> Result<Type<'ctx>, Self::Error> {
        match self {
            Self::Array(info) => self::array::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Bitwise(info) => self::bitwise::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::BoundedInt(info) => self::bounded_int::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Box(info) => self::r#box::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Bytes31(info) => self::bytes31::build(context, module, registry, metadata, info),
            Self::BuiltinCosts(info) => self::builtin_costs::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Const(_) => native_panic!("todo: Const type to MLIR type"),
            Self::EcOp(info) => self::ec_op::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::EcPoint(info) => self::ec_point::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::EcState(info) => self::ec_state::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Enum(info) => self::r#enum::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Felt252(info) => self::felt252::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Felt252Dict(info) => self::felt252_dict::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Felt252DictEntry(info) => self::felt252_dict_entry::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::GasBuiltin(info) => self::gas_builtin::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::NonZero(info) => self::non_zero::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Nullable(info) => self::nullable::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Pedersen(info) => self::pedersen::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Poseidon(info) => self::poseidon::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::RangeCheck(info) => self::range_check::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::RangeCheck96(info) => self::range_check::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::SegmentArena(info) => self::segment_arena::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Sint8(info) => self::uint8::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Sint16(info) => self::uint16::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Sint32(info) => self::uint32::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Sint64(info) => self::uint64::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Sint128(info) => self::uint128::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Snapshot(info) => self::snapshot::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Span(_) => native_panic!("todo: Span type to MLIR type"),
            Self::SquashedFelt252Dict(info) => self::squashed_felt252_dict::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Starknet(selector) => self::starknet::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, selector),
            ),
            Self::Struct(info) => self::r#struct::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Uint128(info) => self::uint128::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Uint128MulGuarantee(info) => self::uint128_mul_guarantee::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Uint16(info) => self::uint16::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Uint32(info) => self::uint32::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Uint64(info) => self::uint64::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Uint8(info) => self::uint8::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Uninitialized(info) => self::uninitialized::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            CoreTypeConcrete::Coupon(info) => self::coupon::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            CoreTypeConcrete::Circuit(info) => self::circuit::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::IntRange(info) => self::int_range::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            CoreTypeConcrete::GasReserve(info) => self::gas_reserve::build(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(self_ty, info),
            ),
            Self::Blake(_) => native_panic!("Build Blake type"),
            CoreTypeConcrete::QM31(info) => {
                self::qm31::build(context, module, registry, metadata, info)
            }
        }
    }

    fn is_builtin(&self) -> bool {
        matches!(
            self,
            CoreTypeConcrete::Bitwise(_)
                | CoreTypeConcrete::EcOp(_)
                | CoreTypeConcrete::GasBuiltin(_)
                | CoreTypeConcrete::BuiltinCosts(_)
                | CoreTypeConcrete::RangeCheck(_)
                | CoreTypeConcrete::RangeCheck96(_)
                | CoreTypeConcrete::Pedersen(_)
                | CoreTypeConcrete::Poseidon(_)
                | CoreTypeConcrete::Coupon(_)
                | CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_))
                | CoreTypeConcrete::SegmentArena(_)
                | CoreTypeConcrete::Circuit(CircuitTypeConcrete::AddMod(_))
                | CoreTypeConcrete::Circuit(CircuitTypeConcrete::MulMod(_))
                | CoreTypeConcrete::Blake(_)
        )
    }

    fn is_complex(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error> {
        Ok(match self {
            // Builtins.
            CoreTypeConcrete::Bitwise(_)
            | CoreTypeConcrete::EcOp(_)
            | CoreTypeConcrete::GasBuiltin(_)
            | CoreTypeConcrete::BuiltinCosts(_)
            | CoreTypeConcrete::RangeCheck(_)
            | CoreTypeConcrete::Pedersen(_)
            | CoreTypeConcrete::Poseidon(_)
            | CoreTypeConcrete::RangeCheck96(_)
            | CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_)) // u64 is not complex
            | CoreTypeConcrete::SegmentArena(_) => false,

            CoreTypeConcrete::Box(_)
            | CoreTypeConcrete::Uint8(_)
            | CoreTypeConcrete::Uint16(_)
            | CoreTypeConcrete::Uint32(_)
            | CoreTypeConcrete::Uint64(_)
            | CoreTypeConcrete::Uint128(_)
            | CoreTypeConcrete::Uint128MulGuarantee(_)
            | CoreTypeConcrete::Sint8(_)
            | CoreTypeConcrete::Sint16(_)
            | CoreTypeConcrete::Sint32(_)
            | CoreTypeConcrete::Sint64(_)
            | CoreTypeConcrete::Sint128(_)
            | CoreTypeConcrete::Nullable(_)
            | CoreTypeConcrete::Felt252Dict(_)
            | CoreTypeConcrete::SquashedFelt252Dict(_) => false,
            CoreTypeConcrete::Array(_) => true,
            CoreTypeConcrete::EcPoint(_) => true,
            CoreTypeConcrete::EcState(_) => true,
            CoreTypeConcrete::Felt252DictEntry(_) => true,

            CoreTypeConcrete::Felt252(_)
            | CoreTypeConcrete::QM31(_)
            | CoreTypeConcrete::Bytes31(_)
            | CoreTypeConcrete::Starknet(
                StarknetTypeConcrete::ClassHash(_)
                | StarknetTypeConcrete::ContractAddress(_)
                | StarknetTypeConcrete::StorageAddress(_)
                | StarknetTypeConcrete::StorageBaseAddress(_)
            ) => {
                #[cfg(target_arch = "x86_64")]
                let value = true;

                #[cfg(target_arch = "aarch64")]
                let value = false;

                value
            },

            CoreTypeConcrete::NonZero(info)
            | CoreTypeConcrete::Uninitialized(info)
            | CoreTypeConcrete::Snapshot(info) => registry.get_type(&info.ty)?.is_complex(registry)?,

            CoreTypeConcrete::Enum(info) => match info.variants.len() {
                0 => false,
                1 => registry.get_type(&info.variants[0])?.is_complex(registry)?,
                _ => !self.is_zst(registry)?,
            },
            CoreTypeConcrete::Struct(_) => true,

            CoreTypeConcrete::BoundedInt(_info) => {
                #[cfg(target_arch = "x86_64")]
                let value = _info.range.repr_bit_width() > 128;

                #[cfg(target_arch = "aarch64")]
                let value = false;

                value
            },
            CoreTypeConcrete::Const(_) => native_panic!("todo: check Const is complex"),
            CoreTypeConcrete::Span(_) => native_panic!("todo: check Span is complex"),
            CoreTypeConcrete::Starknet(StarknetTypeConcrete::Secp256Point(_))
            | CoreTypeConcrete::Starknet(StarknetTypeConcrete::Sha256StateHandle(_)) => native_panic!("todo: check Sha256StateHandle is complex"),
            CoreTypeConcrete::Coupon(_) => false,

            CoreTypeConcrete::Circuit(info) => circuit::is_complex(info),

            CoreTypeConcrete::IntRange(_info) => false,
            CoreTypeConcrete::GasReserve(_info) => false,
            CoreTypeConcrete::Blake(_) => false,
        })
    }

    fn is_zst(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error> {
        Ok(match self {
            // Builtin counters:
            CoreTypeConcrete::Bitwise(_)
            | CoreTypeConcrete::EcOp(_)
            | CoreTypeConcrete::RangeCheck(_)
            | CoreTypeConcrete::Pedersen(_)
            | CoreTypeConcrete::Poseidon(_)
            | CoreTypeConcrete::RangeCheck96(_)
            | CoreTypeConcrete::SegmentArena(_) => false,

            // A ptr to a list of costs.
            CoreTypeConcrete::BuiltinCosts(_) => false,

            // Other builtins:
            CoreTypeConcrete::Uint128MulGuarantee(_) | CoreTypeConcrete::Coupon(_) => true,

            // Normal types:
            CoreTypeConcrete::Array(_)
            | CoreTypeConcrete::Box(_)
            | CoreTypeConcrete::Bytes31(_)
            | CoreTypeConcrete::EcPoint(_)
            | CoreTypeConcrete::EcState(_)
            | CoreTypeConcrete::Felt252(_)
            | CoreTypeConcrete::GasBuiltin(_)
            | CoreTypeConcrete::Uint8(_)
            | CoreTypeConcrete::Uint16(_)
            | CoreTypeConcrete::Uint32(_)
            | CoreTypeConcrete::Uint64(_)
            | CoreTypeConcrete::Uint128(_)
            | CoreTypeConcrete::Sint8(_)
            | CoreTypeConcrete::Sint16(_)
            | CoreTypeConcrete::Sint32(_)
            | CoreTypeConcrete::Sint64(_)
            | CoreTypeConcrete::Sint128(_)
            | CoreTypeConcrete::Felt252Dict(_)
            | CoreTypeConcrete::Felt252DictEntry(_)
            | CoreTypeConcrete::SquashedFelt252Dict(_)
            | CoreTypeConcrete::Starknet(_)
            | CoreTypeConcrete::Nullable(_) => false,
            CoreTypeConcrete::QM31(_) => false,

            // Containers:
            CoreTypeConcrete::NonZero(info)
            | CoreTypeConcrete::Uninitialized(info)
            | CoreTypeConcrete::Snapshot(info) => {
                let type_info = registry.get_type(&info.ty)?;
                type_info.is_zst(registry)?
            }

            // Enums and structs:
            CoreTypeConcrete::Enum(info) => {
                info.variants.is_empty()
                    || (info.variants.len() == 1
                        && registry.get_type(&info.variants[0])?.is_zst(registry)?)
            }
            CoreTypeConcrete::Struct(info) => {
                let mut is_zst = true;
                for member in &info.members {
                    if !registry.get_type(member)?.is_zst(registry)? {
                        is_zst = false;
                        break;
                    }
                }
                is_zst
            }

            CoreTypeConcrete::BoundedInt(_) => false,
            CoreTypeConcrete::GasReserve(_info) => false,
            CoreTypeConcrete::Const(info) => {
                let type_info = registry.get_type(&info.inner_ty)?;
                type_info.is_zst(registry)?
            }
            CoreTypeConcrete::Span(_) => native_panic!("todo: check Span is zero sized"),
            CoreTypeConcrete::Circuit(info) => circuit::is_zst(info),

            CoreTypeConcrete::IntRange(info) => {
                let type_info = registry.get_type(&info.ty)?;
                type_info.is_zst(registry)?
            }
            CoreTypeConcrete::Blake(_) => false,
        })
    }

    fn layout(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<Layout, Self::Error> {
        Ok(match self {
            CoreTypeConcrete::Array(_) => {
                Layout::new::<*mut ()>()
                    .extend(get_integer_layout(32))?
                    .0
                    .extend(get_integer_layout(32))?
                    .0
                    .extend(get_integer_layout(32))?
                    .0
            }
            CoreTypeConcrete::Bitwise(_) => Layout::new::<u64>(),
            CoreTypeConcrete::Box(_) => Layout::new::<*mut ()>(),
            CoreTypeConcrete::EcOp(_) => Layout::new::<u64>(),
            CoreTypeConcrete::EcPoint(_) => layout_repeat(&get_integer_layout(252), 2)?.0,
            CoreTypeConcrete::EcState(_) => layout_repeat(&get_integer_layout(252), 4)?.0,
            CoreTypeConcrete::Felt252(_) => get_integer_layout(252),
            CoreTypeConcrete::GasBuiltin(_) => get_integer_layout(64),
            CoreTypeConcrete::BuiltinCosts(_) => Layout::new::<*const ()>(),
            CoreTypeConcrete::Uint8(_) => get_integer_layout(8),
            CoreTypeConcrete::Uint16(_) => get_integer_layout(16),
            CoreTypeConcrete::Uint32(_) => get_integer_layout(32),
            CoreTypeConcrete::Uint64(_) => get_integer_layout(64),
            CoreTypeConcrete::Uint128(_) => get_integer_layout(128),
            CoreTypeConcrete::Uint128MulGuarantee(_) => Layout::new::<()>(),
            CoreTypeConcrete::NonZero(info) => registry.get_type(&info.ty)?.layout(registry)?,
            CoreTypeConcrete::Nullable(_) => Layout::new::<*mut ()>(),
            CoreTypeConcrete::RangeCheck(_) => Layout::new::<u64>(),
            CoreTypeConcrete::Uninitialized(info) => {
                registry.get_type(&info.ty)?.layout(registry)?
            }
            CoreTypeConcrete::Enum(info) => {
                let tag_layout =
                    get_integer_layout(info.variants.len().next_power_of_two().trailing_zeros());

                info.variants.iter().try_fold(tag_layout, |acc, id| {
                    let layout = tag_layout
                        .extend(registry.get_type(id)?.layout(registry)?)?
                        .0;

                    Result::<_, Self::Error>::Ok(Layout::from_size_align(
                        acc.size().max(layout.size()),
                        acc.align().max(layout.align()),
                    )?)
                })?
            }
            CoreTypeConcrete::Struct(info) => info
                .members
                .iter()
                .try_fold(Option::<Layout>::None, |acc, id| {
                    Result::<_, Self::Error>::Ok(Some(match acc {
                        Some(layout) => layout.extend(registry.get_type(id)?.layout(registry)?)?.0,
                        None => registry.get_type(id)?.layout(registry)?,
                    }))
                })?
                .unwrap_or(Layout::from_size_align(0, 1)?),
            CoreTypeConcrete::Felt252Dict(_) => Layout::new::<*mut std::ffi::c_void>(), // ptr
            CoreTypeConcrete::Felt252DictEntry(_) => {
                get_integer_layout(252)
                    .extend(Layout::new::<*mut std::ffi::c_void>())?
                    .0
                    .extend(Layout::new::<*mut std::ffi::c_void>())?
                    .0
            }
            CoreTypeConcrete::SquashedFelt252Dict(_) => Layout::new::<*mut std::ffi::c_void>(), // ptr
            CoreTypeConcrete::Pedersen(_) => Layout::new::<u64>(),
            CoreTypeConcrete::Poseidon(_) => Layout::new::<u64>(),
            CoreTypeConcrete::Span(_) => native_panic!("todo: create layout for Span"),
            CoreTypeConcrete::Starknet(info) => match info {
                StarknetTypeConcrete::ClassHash(_) => get_integer_layout(252),
                StarknetTypeConcrete::ContractAddress(_) => get_integer_layout(252),
                StarknetTypeConcrete::StorageBaseAddress(_) => get_integer_layout(252),
                StarknetTypeConcrete::StorageAddress(_) => get_integer_layout(252),
                StarknetTypeConcrete::System(_) => Layout::new::<*mut ()>(),
                StarknetTypeConcrete::Secp256Point(_) => {
                    get_integer_layout(256)
                        .extend(get_integer_layout(256))?
                        .0
                        .extend(get_integer_layout(1))?
                        .0
                }
                StarknetTypeConcrete::Sha256StateHandle(_) => Layout::new::<*mut ()>(),
            },
            CoreTypeConcrete::SegmentArena(_) => Layout::new::<u64>(),
            CoreTypeConcrete::Snapshot(info) => registry.get_type(&info.ty)?.layout(registry)?,
            CoreTypeConcrete::Sint8(_) => get_integer_layout(8),
            CoreTypeConcrete::Sint16(_) => get_integer_layout(16),
            CoreTypeConcrete::Sint32(_) => get_integer_layout(32),
            CoreTypeConcrete::Sint64(_) => get_integer_layout(64),
            CoreTypeConcrete::Sint128(_) => get_integer_layout(128),
            CoreTypeConcrete::Bytes31(_) => get_integer_layout(248),
            CoreTypeConcrete::BoundedInt(info) => get_integer_layout(info.range.repr_bit_width()),
            CoreTypeConcrete::QM31(_info) => layout_repeat(&get_integer_layout(31), 4)?.0,
            CoreTypeConcrete::GasReserve(_info) => get_integer_layout(128),

            CoreTypeConcrete::Const(const_type) => {
                registry.get_type(&const_type.inner_ty)?.layout(registry)?
            }
            CoreTypeConcrete::Coupon(_) => Layout::new::<()>(),
            CoreTypeConcrete::RangeCheck96(_) => get_integer_layout(64),
            CoreTypeConcrete::Circuit(info) => circuit::layout(registry, info)?,

            CoreTypeConcrete::IntRange(info) => {
                let inner = registry.get_type(&info.ty)?.layout(registry)?;
                inner.extend(inner)?.0
            }
            CoreTypeConcrete::Blake(_) => get_integer_layout(64),
        }
        .pad_to_align())
    }

    fn is_memory_allocated(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error> {
        // Right now, only enums and other structures which may end up passing a flattened enum as
        // arguments.
        Ok(match self {
            CoreTypeConcrete::IntRange(_) => false,
            CoreTypeConcrete::Array(_) => false,
            CoreTypeConcrete::Bitwise(_) => false,
            CoreTypeConcrete::Box(_) => false,
            CoreTypeConcrete::EcOp(_) => false,
            CoreTypeConcrete::EcPoint(_) => false,
            CoreTypeConcrete::EcState(_) => false,
            CoreTypeConcrete::Felt252(_) => false,
            CoreTypeConcrete::GasBuiltin(_) => false,
            CoreTypeConcrete::BuiltinCosts(_) => false,
            CoreTypeConcrete::Uint8(_) => false,
            CoreTypeConcrete::Uint16(_) => false,
            CoreTypeConcrete::Uint32(_) => false,
            CoreTypeConcrete::Uint64(_) => false,
            CoreTypeConcrete::Uint128(_) => false,
            CoreTypeConcrete::Uint128MulGuarantee(_) => false,
            CoreTypeConcrete::Sint8(_) => false,
            CoreTypeConcrete::Sint16(_) => false,
            CoreTypeConcrete::Sint32(_) => false,
            CoreTypeConcrete::Sint64(_) => false,
            CoreTypeConcrete::Sint128(_) => false,
            CoreTypeConcrete::NonZero(_) => false,
            CoreTypeConcrete::Nullable(_) => false,
            CoreTypeConcrete::RangeCheck(_) => false,
            CoreTypeConcrete::RangeCheck96(_) => false,
            CoreTypeConcrete::Uninitialized(_) => false,
            CoreTypeConcrete::Enum(info) => match info.variants.len() {
                0 => false,
                1 => registry
                    .get_type(&info.variants[0])?
                    .is_memory_allocated(registry)?,
                _ => true,
            },
            CoreTypeConcrete::Struct(info) => {
                let mut is_memory_allocated = false;
                for member in &info.members {
                    if registry.get_type(member)?.is_memory_allocated(registry)? {
                        is_memory_allocated = true;
                        break;
                    }
                }
                is_memory_allocated
            }
            CoreTypeConcrete::Felt252Dict(_) => false,
            CoreTypeConcrete::Felt252DictEntry(_) => false,
            CoreTypeConcrete::SquashedFelt252Dict(_) => false,
            CoreTypeConcrete::Pedersen(_) => false,
            CoreTypeConcrete::Poseidon(_) => false,
            CoreTypeConcrete::Span(_) => false,
            CoreTypeConcrete::Starknet(_) => false,
            CoreTypeConcrete::SegmentArena(_) => false,
            CoreTypeConcrete::Snapshot(info) => {
                registry.get_type(&info.ty)?.is_memory_allocated(registry)?
            }
            CoreTypeConcrete::Bytes31(_) => false,

            CoreTypeConcrete::BoundedInt(_) => false,
            CoreTypeConcrete::Const(info) => registry
                .get_type(&info.inner_ty)?
                .is_memory_allocated(registry)?,
            CoreTypeConcrete::Coupon(_) => false,
            CoreTypeConcrete::Circuit(_) => false,
            CoreTypeConcrete::QM31(_) => false,
            CoreTypeConcrete::GasReserve(_) => false,
            CoreTypeConcrete::Blake(_) => false,
        })
    }

    fn integer_range(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<Range, Self::Error> {
        fn range_of<T>() -> Range
        where
            T: Bounded + Into<BigInt>,
        {
            Range {
                lower: T::min_value().into(),
                upper: T::max_value().into() + BigInt::one(),
            }
        }

        Ok(match self {
            Self::Uint8(_) => range_of::<u8>(),
            Self::Uint16(_) => range_of::<u16>(),
            Self::Uint32(_) => range_of::<u32>(),
            Self::Uint64(_) => range_of::<u64>(),
            Self::Uint128(_) => range_of::<u128>(),
            Self::Felt252(_) => Range {
                lower: BigInt::ZERO,
                upper: BigInt::from_biguint(Sign::Plus, PRIME.clone()),
            },
            Self::Sint8(_) => range_of::<i8>(),
            Self::Sint16(_) => range_of::<i16>(),
            Self::Sint32(_) => range_of::<i32>(),
            Self::Sint64(_) => range_of::<i64>(),
            Self::Sint128(_) => range_of::<i128>(),

            Self::BoundedInt(info) => info.range.clone(),
            Self::Bytes31(_) => Range {
                lower: BigInt::ZERO,
                upper: BigInt::one() << 248,
            },
            Self::Const(info) => registry.get_type(&info.inner_ty)?.integer_range(registry)?,
            Self::NonZero(info) => registry.get_type(&info.ty)?.integer_range(registry)?,

            _ => return Err(crate::error::Error::IntegerLikeTypeExpected),
        })
    }

    fn is_bounded_int(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error> {
        Ok(match self {
            CoreTypeConcrete::BoundedInt(_) => true,
            CoreTypeConcrete::NonZero(info) => {
                registry.get_type(&info.ty)?.is_bounded_int(registry)?
            }

            _ => false,
        })
    }

    fn is_felt252(
        &self,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    ) -> Result<bool, Self::Error> {
        Ok(match self {
            CoreTypeConcrete::Felt252(_) => true,
            CoreTypeConcrete::NonZero(info) => registry.get_type(&info.ty)?.is_felt252(registry)?,

            _ => false,
        })
    }

    fn variants(&self) -> Option<&[ConcreteTypeId]> {
        match self {
            Self::Enum(info) => Some(&info.variants),
            _ => None,
        }
    }

    fn build_default<'ctx, 'this>(
        &self,
        context: &'ctx Context,
        _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
        entry: &'this Block<'ctx>,
        location: Location<'ctx>,
        _helper: &LibfuncHelper<'ctx, 'this>,
        _metadata: &mut MetadataStorage,
        _self_ty: &ConcreteTypeId,
    ) -> Result<Value<'ctx, 'this>, Self::Error> {
        static BOOL_USER_TYPE_ID: OnceLock<UserTypeId> = OnceLock::new();
        let bool_user_type_id =
            BOOL_USER_TYPE_ID.get_or_init(|| UserTypeId::from_string("core::bool"));

        Ok(match self {
            Self::Enum(info) => match &info.info.long_id.generic_args[0] {
                GenericArg::UserType(id) if id == bool_user_type_id => {
                    let tag = entry.const_int(context, location, 0, 1)?;

                    let value = entry.append_op_result(llvm::undef(
                        llvm::r#type::r#struct(
                            context,
                            &[
                                IntegerType::new(context, 1).into(),
                                llvm::r#type::array(IntegerType::new(context, 8).into(), 0),
                            ],
                            false,
                        ),
                        location,
                    ))?;

                    entry.insert_value(context, location, value, tag, 0)?
                }
                _ => native_panic!("unsupported dict value type"),
            },
            Self::Felt252(_) => entry.const_int(context, location, 0, 252)?,
            Self::Nullable(_) => {
                entry.append_op_result(llvm::zero(llvm::r#type::pointer(context, 0), location))?
            }
            Self::Uint8(_) => entry.const_int(context, location, 0, 8)?,
            Self::Uint16(_) => entry.const_int(context, location, 0, 16)?,
            Self::Uint32(_) => entry.const_int(context, location, 0, 32)?,
            Self::Uint64(_) => entry.const_int(context, location, 0, 64)?,
            Self::Uint128(_) => entry.const_int(context, location, 0, 128)?,
            _ => native_panic!("unsupported dict value type"),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WithSelf<'a, T> {
    self_ty: &'a ConcreteTypeId,
    inner: &'a T,
}

impl<'a, T> WithSelf<'a, T> {
    pub const fn new(self_ty: &'a ConcreteTypeId, inner: &'a T) -> Self {
        Self { self_ty, inner }
    }

    pub const fn self_ty(&self) -> &ConcreteTypeId {
        self.self_ty
    }
}

impl<T> AsRef<T> for WithSelf<'_, T> {
    fn as_ref(&self) -> &T {
        self.inner
    }
}

impl<T> Deref for WithSelf<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner
    }
}

#[cfg(test)]
mod test {
    use super::TypeBuilder;
    use crate::utils::testing::load_program;
    use cairo_lang_sierra::{
        extensions::core::{CoreLibfunc, CoreType},
        program_registry::ProgramRegistry,
    };

    #[test]
    fn ensure_padded_layouts() {
        let program = load_program("test_data_artifacts/programs/types/ensure_padded_layouts");

        let registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(&program).unwrap();
        for ty in &program.type_declarations {
            let ty = registry.get_type(&ty.id).unwrap();
            let layout = ty.layout(&registry).unwrap();
            assert_eq!(layout, layout.pad_to_align());
        }
    }
}
