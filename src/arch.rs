use crate::{
    error::Result,
    native_panic,
    starknet::{ArrayAbi, Secp256k1Point, Secp256r1Point},
    types::TypeBuilder,
    utils::libc_malloc,
    values::Value,
};
use bumpalo::Bump;
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        starknet::{secp256::Secp256PointTypeConcrete, StarknetTypeConcrete},
    },
    ids::ConcreteTypeId,
    program_registry::ProgramRegistry,
};
use std::{
    ffi::c_void,
    ptr::{null, NonNull},
};

mod aarch64;
mod x86_64;

/// Implemented by all supported argument types.
pub trait AbiArgument {
    /// Serialize the argument into the buffer. This method should keep track of arch-dependent
    /// stuff like register vs stack allocation.
    fn to_bytes(
        &self,
        buffer: &mut Vec<u8>,
        find_dict_drop_override: impl Copy + Fn(&ConcreteTypeId) -> Option<extern "C" fn(*mut c_void)>,
    ) -> Result<()>;
}

/// A wrapper that implements `AbiArgument` for `Value`s. It contains all the required stuff to
/// serialize all possible `Value`s.
pub struct ValueWithInfoWrapper<'a> {
    pub value: &'a Value,
    pub type_id: &'a ConcreteTypeId,
    pub info: &'a CoreTypeConcrete,

    pub arena: &'a Bump,
    pub registry: &'a ProgramRegistry<CoreType, CoreLibfunc>,
}

impl<'a> ValueWithInfoWrapper<'a> {
    fn map<'b>(
        &'b self,
        value: &'b Value,
        type_id: &'b ConcreteTypeId,
    ) -> Result<ValueWithInfoWrapper<'b>>
    where
        'b: 'a,
    {
        Ok(Self {
            value,
            type_id,
            info: self.registry.get_type(type_id)?,
            arena: self.arena,
            registry: self.registry,
        })
    }
}

impl AbiArgument for ValueWithInfoWrapper<'_> {
    fn to_bytes(
        &self,
        buffer: &mut Vec<u8>,
        find_dict_drop_override: impl Copy + Fn(&ConcreteTypeId) -> Option<extern "C" fn(*mut c_void)>,
    ) -> Result<()> {
        match (self.value, self.info) {
            (value, CoreTypeConcrete::Box(info)) => {
                let ptr =
                    value.to_ptr(self.arena, self.registry, &info.ty, find_dict_drop_override)?;

                let layout = self.registry.get_type(&info.ty)?.layout(self.registry)?;
                let heap_ptr = unsafe {
                    let heap_ptr = libc_malloc(layout.size());
                    libc::memcpy(heap_ptr, ptr.as_ptr().cast(), layout.size());
                    heap_ptr
                };

                heap_ptr.to_bytes(buffer, find_dict_drop_override)?;
            }
            (value, CoreTypeConcrete::Nullable(info)) => {
                if matches!(value, Value::Null) {
                    null::<()>().to_bytes(buffer, find_dict_drop_override)?;
                } else {
                    let ptr = value.to_ptr(
                        self.arena,
                        self.registry,
                        self.type_id,
                        find_dict_drop_override,
                    )?;

                    let layout = self.registry.get_type(&info.ty)?.layout(self.registry)?;
                    let heap_ptr = unsafe {
                        let heap_ptr = libc_malloc(layout.size());
                        libc::memcpy(heap_ptr, ptr.as_ptr().cast(), layout.size());
                        heap_ptr
                    };

                    heap_ptr.to_bytes(buffer, find_dict_drop_override)?;
                }
            }
            (value, CoreTypeConcrete::NonZero(info) | CoreTypeConcrete::Snapshot(info)) => self
                .map(value, &info.ty)?
                .to_bytes(buffer, find_dict_drop_override)?,

            (Value::Array(_), CoreTypeConcrete::Array(_)) => {
                // TODO: Assert that `info.ty` matches all the values' types. See: https://github.com/lambdaclass/cairo_native/issues/1216

                let abi_ptr = self.value.to_ptr(
                    self.arena,
                    self.registry,
                    self.type_id,
                    find_dict_drop_override,
                )?;
                let abi = unsafe { abi_ptr.cast::<ArrayAbi<()>>().as_ref() };

                abi.ptr.to_bytes(buffer, find_dict_drop_override)?;
                abi.since.to_bytes(buffer, find_dict_drop_override)?;
                abi.until.to_bytes(buffer, find_dict_drop_override)?;
                abi.capacity.to_bytes(buffer, find_dict_drop_override)?;
            }
            (Value::BoundedInt { .. }, CoreTypeConcrete::BoundedInt(_)) => {
                // See: https://github.com/lambdaclass/cairo_native/issues/1217
                native_panic!("todo: implement AbiArgument for Value::BoundedInt case")
            }
            (Value::Bytes31(value), CoreTypeConcrete::Bytes31(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::EcPoint(x, y), CoreTypeConcrete::EcPoint(_)) => {
                x.to_bytes(buffer, find_dict_drop_override)?;
                y.to_bytes(buffer, find_dict_drop_override)?;
            }
            (Value::EcState(x, y, x0, y0), CoreTypeConcrete::EcState(_)) => {
                x.to_bytes(buffer, find_dict_drop_override)?;
                y.to_bytes(buffer, find_dict_drop_override)?;
                x0.to_bytes(buffer, find_dict_drop_override)?;
                y0.to_bytes(buffer, find_dict_drop_override)?;
            }
            (Value::QM31(a, b, c, d), CoreTypeConcrete::QM31(_)) => {
                a.to_bytes(buffer, find_dict_drop_override)?;
                b.to_bytes(buffer, find_dict_drop_override)?;
                c.to_bytes(buffer, find_dict_drop_override)?;
                d.to_bytes(buffer, find_dict_drop_override)?;
            }
            (Value::Enum { tag, value, .. }, CoreTypeConcrete::Enum(info)) => {
                if self.info.is_memory_allocated(self.registry)? {
                    let abi_ptr = self.value.to_ptr(
                        self.arena,
                        self.registry,
                        self.type_id,
                        find_dict_drop_override,
                    )?;

                    let abi_ptr = unsafe { *abi_ptr.cast::<NonNull<()>>().as_ref() };
                    abi_ptr.as_ptr().to_bytes(buffer, find_dict_drop_override)?;
                } else {
                    match info
                        .variants
                        .len()
                        .next_power_of_two()
                        .trailing_zeros()
                        .div_ceil(8)
                    {
                        0 => {}
                        _ => (*tag as u64).to_bytes(buffer, find_dict_drop_override)?,
                    }

                    self.map(value, &info.variants[*tag])?
                        .to_bytes(buffer, find_dict_drop_override)?;
                }
            }
            (
                Value::Felt252(value),
                CoreTypeConcrete::Felt252(_)
                | CoreTypeConcrete::Starknet(
                    StarknetTypeConcrete::ClassHash(_)
                    | StarknetTypeConcrete::ContractAddress(_)
                    | StarknetTypeConcrete::StorageAddress(_)
                    | StarknetTypeConcrete::StorageBaseAddress(_),
                ),
            ) => value.to_bytes(buffer, find_dict_drop_override)?,
            (Value::Felt252Dict { .. }, CoreTypeConcrete::Felt252Dict(_)) => {
                // TODO: Assert that `info.ty` matches all the values' types.

                self.value
                    .to_ptr(
                        self.arena,
                        self.registry,
                        self.type_id,
                        find_dict_drop_override,
                    )?
                    .as_ptr()
                    .to_bytes(buffer, find_dict_drop_override)?
            }
            (
                Value::Secp256K1Point(Secp256k1Point { x, y, is_infinity }),
                CoreTypeConcrete::Starknet(StarknetTypeConcrete::Secp256Point(
                    Secp256PointTypeConcrete::K1(_),
                )),
            )
            | (
                Value::Secp256R1Point(Secp256r1Point { x, y, is_infinity }),
                CoreTypeConcrete::Starknet(StarknetTypeConcrete::Secp256Point(
                    Secp256PointTypeConcrete::R1(_),
                )),
            ) => {
                x.to_bytes(buffer, find_dict_drop_override)?;
                y.to_bytes(buffer, find_dict_drop_override)?;
                is_infinity.to_bytes(buffer, find_dict_drop_override)?;
            }
            (Value::Sint128(value), CoreTypeConcrete::Sint128(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Sint16(value), CoreTypeConcrete::Sint16(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Sint32(value), CoreTypeConcrete::Sint32(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Sint64(value), CoreTypeConcrete::Sint64(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Sint8(value), CoreTypeConcrete::Sint8(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Struct { fields, .. }, CoreTypeConcrete::Struct(info)) => {
                if self.info.is_memory_allocated(self.registry)? {
                    let abi_ptr = self.value.to_ptr(
                        self.arena,
                        self.registry,
                        self.type_id,
                        find_dict_drop_override,
                    )?;

                    let abi_ptr = unsafe { *abi_ptr.cast::<NonNull<()>>().as_ref() };
                    abi_ptr.as_ptr().to_bytes(buffer, find_dict_drop_override)?;
                } else {
                    fields
                        .iter()
                        .zip(&info.members)
                        .map(|(value, type_id)| self.map(value, type_id))
                        .try_for_each(|wrapper| {
                            wrapper?.to_bytes(buffer, find_dict_drop_override)
                        })?;
                }
            }
            (Value::Uint128(value), CoreTypeConcrete::Uint128(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Uint16(value), CoreTypeConcrete::Uint16(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Uint32(value), CoreTypeConcrete::Uint32(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Uint64(value), CoreTypeConcrete::Uint64(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            (Value::Uint8(value), CoreTypeConcrete::Uint8(_)) => {
                value.to_bytes(buffer, find_dict_drop_override)?
            }
            // The catchall includes all unreachable combinations, as well
            // as some combination that may be reachable, and haven't been
            // encountered yet. Adding support for additional input arguments
            // may require implementing this function for new combinations.
            _ => native_panic!(
                "abi argument unimplemented for ({:?}, {:?})",
                self.value,
                self.type_id
            ),
        }

        Ok(())
    }
}
