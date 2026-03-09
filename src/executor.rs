//! # Executors
//!
//! This module provides methods to execute the programs, either via JIT or compiled ahead
//! of time. It also provides a cache to avoid recompiling previously compiled programs.

pub use self::{aot::AotNativeExecutor, contract::AotContractExecutor, jit::JitNativeExecutor};
use crate::{
    arch::{AbiArgument, ValueWithInfoWrapper},
    error::{panic::ToNativeAssertError, Error},
    execution_result::{
        BuiltinStats, ExecutionResult, ADD_MOD_BUILTIN_SIZE, BITWISE_BUILTIN_SIZE,
        BLAKE_BUILTIN_SIZE, EC_OP_BUILTIN_SIZE, MUL_MOD_BUILTIN_SIZE, PEDERSEN_BUILTIN_SIZE,
        POSEIDON_BUILTIN_SIZE, RANGE_CHECK96_BUILTIN_SIZE, RANGE_CHECK_BUILTIN_SIZE,
        SEGMENT_ARENA_BUILTIN_SIZE,
    },
    native_panic,
    runtime::{BLAKE_CALL_COUNT, BUILTIN_COSTS},
    starknet::{handler::StarknetSyscallHandlerCallbacks, StarknetSyscallHandler},
    types::TypeBuilder,
    utils::{libc_free, BuiltinCosts, RangeExt},
    values::Value,
};
use bumpalo::Bump;
use cairo_lang_sierra::{
    extensions::{
        circuit::CircuitTypeConcrete,
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        starknet::StarknetTypeConcrete,
        ConcreteType,
    },
    ids::ConcreteTypeId,
    program::FunctionSignature,
    program_registry::ProgramRegistry,
};
use libc::c_void;
use num_bigint::BigInt;
use num_traits::One;
use std::{alloc::Layout, arch::global_asm, ptr::NonNull};

mod aot;
mod contract;
mod jit;

#[cfg(target_arch = "aarch64")]
global_asm!(include_str!("arch/aarch64.s"));
#[cfg(target_arch = "x86_64")]
global_asm!(include_str!("arch/x86_64.s"));

extern "C" {
    /// Invoke an AOT or JIT-compiled function.
    ///
    /// The `ret_ptr` argument is only used when the first argument (the actual return pointer) is
    /// unused. Used for u8, u16, u32, u64, u128 and felt252, but not for arrays, enums or structs.
    #[cfg_attr(not(target_os = "macos"), link_name = "_invoke_trampoline")]
    fn invoke_trampoline(
        fn_ptr: *const c_void,
        args_ptr: *const u64,
        args_len: usize,
        ret_ptr: *mut u64,
    );
}

/// Internal method.
///
/// Invokes the given function by constructing the function call depending on the arguments given.
/// Usually calling a function requires knowing it's signature at compile time, but we need to be
/// able to call any given function provided it's signatue (arguments and return type) at runtime,
/// to do so we have a "trampoline" in the given platform assembly (x86_64, aarch64) which
/// constructs the function call in place.
///
/// To pass the arguments, they are stored in a arena.
#[allow(clippy::too_many_arguments)]
fn invoke_dynamic(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    function_ptr: *const c_void,
    function_signature: &FunctionSignature,
    args: &[Value],
    gas: u64,
    mut syscall_handler: Option<impl StarknetSyscallHandler>,
    find_dict_drop_override: impl Copy + Fn(&ConcreteTypeId) -> Option<extern "C" fn(*mut c_void)>,
) -> Result<ExecutionResult, Error> {
    tracing::info!("Invoking function with signature: {function_signature:?}.");
    let arena = Bump::new();
    let mut invoke_data = Vec::<u8>::new();

    // Generate return pointer (if necessary).
    //
    // Generated when either:
    //   - There are more than one non-zst return values.
    //     - All builtins except GasBuiltin and Starknet are ZST.
    //     - The unit struct is a ZST.
    //   - The return argument is complex.
    let mut ret_types_iter = function_signature
        .ret_types
        .iter()
        .filter_map(|id| {
            let type_info = match registry.get_type(id) {
                Ok(x) => x,
                Err(e) => return Some(Err(e.into())),
            };
            let is_zst = match type_info.is_zst(registry) {
                Ok(x) => x,
                Err(e) => return Some(Err(e)),
            };

            Ok((!(type_info.is_builtin() && is_zst)).then_some(id)).transpose()
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .peekable();

    let num_return_args = ret_types_iter.clone().count();
    // If there is more than one return value, or the return value is _complex_,
    // as defined by the architecture ABI, then we pass a return pointer as
    // the first argument to the program entrypoint.
    let mut return_ptr = if num_return_args > 1
        || ret_types_iter
            .peek()
            .map(|id| registry.get_type(id)?.is_complex(registry))
            .transpose()?
            == Some(true)
    {
        // The return pointer should be able to hold all the return values.
        let layout = ret_types_iter.try_fold(Layout::new::<()>(), |layout, id| {
            let type_info = registry.get_type(id)?;
            Result::<_, Error>::Ok(layout.extend(type_info.layout(registry)?)?.0)
        })?;

        let return_ptr = arena.alloc_layout(layout).cast::<()>();
        return_ptr
            .as_ptr()
            .to_bytes(&mut invoke_data, |_| unreachable!())?;

        Some(return_ptr)
    } else {
        None
    };

    // The Cairo compiler doesn't specify that the cheatcode syscall needs the syscall handler,
    // so we must always allocate it in case it needs it, regardless of whether it's passed
    // as an argument to the entry point or not.
    let mut syscall_handler = syscall_handler
        .as_mut()
        .map(|syscall_handler| StarknetSyscallHandlerCallbacks::new(syscall_handler));
    // We only care for the previous syscall handler if we actually modify it
    #[cfg(feature = "with-cheatcode")]
    let syscall_handler_guard = syscall_handler
        .as_mut()
        .map(|syscall_handler| SyscallHandlerGuard::install(syscall_handler as *mut _));

    // We may be inside a recursive contract, save the possible saved builtin costs to restore it after our call.
    let builtin_costs = BuiltinCosts::default();
    let builtin_costs_guard = BuiltinCostsGuard::install(builtin_costs);

    // Generate argument list.
    let mut iter = args.iter();
    for item in function_signature.param_types.iter().filter_map(|type_id| {
        let type_info = match registry.get_type(type_id) {
            Ok(x) => x,
            Err(e) => return Some(Err(e.into())),
        };
        match type_info.is_zst(registry) {
            Ok(x) => (!x).then_some(Ok((type_id, type_info))),
            Err(e) => Some(Err(e)),
        }
    }) {
        let (type_id, type_info) = item?;

        // Process gas requirements and syscall handler.
        match type_info {
            CoreTypeConcrete::GasBuiltin(_) => {
                gas.to_bytes(&mut invoke_data, |_| unreachable!())?
            }
            CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_)) => {
                let syscall_handler = syscall_handler
                    .as_mut()
                    .to_native_assert_error("syscall handler should be available")?;

                (syscall_handler as *mut StarknetSyscallHandlerCallbacks<_>)
                    .to_bytes(&mut invoke_data, |_| unreachable!())?;
            }
            CoreTypeConcrete::BuiltinCosts(_) => {
                let ptr = BUILTIN_COSTS.with(|x| x.as_ptr());
                (ptr as *const ()).to_bytes(&mut invoke_data, |_| unreachable!())?;
            }
            type_info if type_info.is_builtin() => {
                0u64.to_bytes(&mut invoke_data, |_| unreachable!())?
            }
            type_info => ValueWithInfoWrapper {
                value: iter
                    .next()
                    .to_native_assert_error("entrypoint argument is missing")?,
                type_id,
                info: type_info,

                arena: &arena,
                registry,
            }
            .to_bytes(&mut invoke_data, find_dict_drop_override)?,
        }
    }

    // Pad invoke data to the 16 byte boundary avoid segfaults.
    #[cfg(target_arch = "aarch64")]
    const REGISTER_BYTES: usize = 64;
    #[cfg(target_arch = "x86_64")]
    const REGISTER_BYTES: usize = 48;
    if invoke_data.len() > REGISTER_BYTES {
        invoke_data.resize(
            REGISTER_BYTES + (invoke_data.len() - REGISTER_BYTES).next_multiple_of(16),
            0,
        );
    }

    // Invoke the trampoline.
    #[cfg(target_arch = "x86_64")]
    let mut ret_registers = [0; 2];
    #[cfg(target_arch = "aarch64")]
    let mut ret_registers = [0; 4];

    #[allow(unused_mut)]
    let mut run_trampoline = || unsafe {
        invoke_trampoline(
            function_ptr,
            invoke_data.as_ptr().cast(),
            invoke_data.len() >> 3,
            ret_registers.as_mut_ptr(),
        );
    };
    #[cfg(feature = "with-segfault-catcher")]
    crate::utils::safe_runner::run_safely(run_trampoline).map_err(Error::SafeRunner)?;
    #[cfg(not(feature = "with-segfault-catcher"))]
    run_trampoline();

    // Restore the previous syscall handler and builtin costs.
    #[cfg(feature = "with-cheatcode")]
    drop(syscall_handler_guard);
    drop(builtin_costs_guard);

    // Parse final gas.
    unsafe fn read_value<T>(ptr: &mut NonNull<()>) -> &T {
        let align_offset = ptr
            .cast::<u8>()
            .as_ptr()
            .align_offset(std::mem::align_of::<T>());
        let value_ptr = ptr.cast::<u8>().as_ptr().add(align_offset).cast::<T>();

        *ptr = NonNull::new_unchecked(value_ptr.add(1)).cast();
        &*value_ptr
    }

    let mut remaining_gas = None;
    let mut builtin_stats = BuiltinStats::default();
    for type_id in &function_signature.ret_types {
        let type_info = registry.get_type(type_id)?;
        match type_info {
            CoreTypeConcrete::GasBuiltin(_) => {
                remaining_gas = Some(match &mut return_ptr {
                    Some(return_ptr) => unsafe { *read_value::<u64>(return_ptr) },
                    None => {
                        // If there's no return ptr then the function only returned the gas. We don't
                        // need to bother with the syscall handler builtin.
                        ret_registers[0]
                    }
                });
            }
            CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_)) => {
                if let Some(return_ptr) = &mut return_ptr {
                    unsafe {
                        let ptr = return_ptr.cast::<*mut ()>();
                        *return_ptr = NonNull::new_unchecked(ptr.as_ptr().add(1)).cast();
                    }
                }
            }
            _ if type_info.is_builtin() => {
                if !type_info.is_zst(registry)? {
                    if let CoreTypeConcrete::BuiltinCosts(_) = type_info {
                        // todo: should we use this value? See: https://github.com/lambdaclass/cairo_native/issues/1219
                        let _value = match &mut return_ptr {
                            Some(return_ptr) => unsafe { *read_value::<*mut u64>(return_ptr) },
                            None => ret_registers[0] as *mut u64,
                        };
                    } else {
                        let value = match &mut return_ptr {
                            Some(return_ptr) => unsafe { *read_value::<u64>(return_ptr) },
                            None => ret_registers[0],
                        } as usize;

                        match type_info {
                            CoreTypeConcrete::RangeCheck(_) => {
                                builtin_stats.range_check = value / RANGE_CHECK_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::Pedersen(_) => {
                                builtin_stats.pedersen = value / PEDERSEN_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::Bitwise(_) => {
                                builtin_stats.bitwise = value / BITWISE_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::EcOp(_) => {
                                builtin_stats.ec_op = value / EC_OP_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::Poseidon(_) => {
                                builtin_stats.poseidon = value / POSEIDON_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::SegmentArena(_) => {
                                builtin_stats.segment_arena = value / SEGMENT_ARENA_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::RangeCheck96(_) => {
                                builtin_stats.range_check96 = value / RANGE_CHECK96_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::Circuit(CircuitTypeConcrete::AddMod(_)) => {
                                builtin_stats.add_mod = value / ADD_MOD_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::Circuit(CircuitTypeConcrete::MulMod(_)) => {
                                builtin_stats.mul_mod = value / MUL_MOD_BUILTIN_SIZE
                            }
                            CoreTypeConcrete::Blake(_) => {
                                builtin_stats.blake = value / BLAKE_BUILTIN_SIZE
                            }
                            _ => native_panic!("given type should be a builtin: {type_id:?}"),
                        }
                    }
                }
            }
            _ => break,
        }
    }

    // Parse return values.
    let return_value = function_signature
        .ret_types
        .last()
        .and_then(|ret_type| {
            let type_info = match registry.get_type(ret_type) {
                Ok(x) => x,
                Err(e) => return Some(Err(e.into())),
            };

            if type_info.is_builtin() {
                None
            } else {
                Some(parse_result(ret_type, registry, return_ptr, ret_registers))
            }
        })
        .transpose()?
        .unwrap_or_else(|| Value::Struct {
            fields: vec![],
            debug_name: None,
        });

    // Get the blake call count from the global counter (blake doesn't have a buffer-based counter
    // like other builtins, so it's tracked globally via the blake libfuncs)
    builtin_stats.blake = BLAKE_CALL_COUNT.with(|c| c.replace(0)) as usize;

    #[cfg(feature = "with-mem-tracing")]
    crate::utils::mem_tracing::report_stats();

    Ok(ExecutionResult {
        remaining_gas,
        return_value,
        builtin_stats,
    })
}

#[cfg(feature = "with-cheatcode")]
#[derive(Debug)]
struct SyscallHandlerGuard(*mut ());

#[cfg(feature = "with-cheatcode")]
impl SyscallHandlerGuard {
    // NOTE: It is the caller's responsibility to ensure that the syscall handler is alive until the
    //   guard is dropped.
    pub fn install<T>(value: *mut T) -> Self {
        let previous_value = crate::starknet::SYSCALL_HANDLER_VTABLE.get();
        let syscall_handler_ptr = value as *mut ();
        crate::starknet::SYSCALL_HANDLER_VTABLE.set(syscall_handler_ptr);

        Self(previous_value)
    }
}

#[cfg(feature = "with-cheatcode")]
impl Drop for SyscallHandlerGuard {
    fn drop(&mut self) {
        crate::starknet::SYSCALL_HANDLER_VTABLE.set(self.0);
    }
}

#[derive(Debug)]
struct BuiltinCostsGuard(BuiltinCosts);

impl BuiltinCostsGuard {
    pub fn install(value: BuiltinCosts) -> Self {
        Self(BUILTIN_COSTS.replace(value))
    }
}

impl Drop for BuiltinCostsGuard {
    fn drop(&mut self) {
        BUILTIN_COSTS.set(self.0);
    }
}

/// Parses the result by reading from the return ptr the given type.
fn parse_result(
    type_id: &ConcreteTypeId,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    mut return_ptr: Option<NonNull<()>>,
    #[cfg(target_arch = "x86_64")] mut ret_registers: [u64; 2],
    #[cfg(target_arch = "aarch64")] mut ret_registers: [u64; 4],
) -> Result<Value, Error> {
    let type_info = registry.get_type(type_id)?;

    // Align the pointer to the actual return value.
    if let Some(return_ptr) = &mut return_ptr {
        let layout = type_info.layout(registry)?;
        let align_offset = return_ptr
            .cast::<u8>()
            .as_ptr()
            .align_offset(layout.align());

        *return_ptr = unsafe {
            NonNull::new(return_ptr.cast::<u8>().as_ptr().add(align_offset))
                .to_native_assert_error("return pointer should not be null")?
                .cast()
        };
    }

    match type_info {
        CoreTypeConcrete::Array(_) => Ok(Value::from_ptr(
            return_ptr.to_native_assert_error("return pointer should be valid")?,
            type_id,
            registry,
            true,
        )?),
        CoreTypeConcrete::Box(info) => unsafe {
            let ptr =
                return_ptr.unwrap_or_else(|| NonNull::new_unchecked(ret_registers[0] as *mut ()));
            let value = Value::from_ptr(ptr, &info.ty, registry, true)?;
            libc_free(ptr.cast().as_ptr());
            Ok(value)
        },
        CoreTypeConcrete::EcPoint(_) | CoreTypeConcrete::EcState(_) => Ok(Value::from_ptr(
            return_ptr.to_native_assert_error("return pointer should be valid")?,
            type_id,
            registry,
            true,
        )?),
        CoreTypeConcrete::QM31(_) => match return_ptr {
            Some(ptr) => Ok(Value::from_ptr(ptr, type_id, registry, true)?),
            None => {
                #[cfg(target_arch = "x86_64")]
                return Err(Error::ParseAttributeError);

                #[cfg(target_arch = "aarch64")]
                Ok(Value::QM31(
                    u32::try_from(ret_registers[0])?,
                    u32::try_from(ret_registers[1])?,
                    u32::try_from(ret_registers[2])?,
                    u32::try_from(ret_registers[3])?,
                ))
            }
        },
        CoreTypeConcrete::Felt252(_)
        | CoreTypeConcrete::Starknet(
            StarknetTypeConcrete::ClassHash(_)
            | StarknetTypeConcrete::ContractAddress(_)
            | StarknetTypeConcrete::StorageAddress(_)
            | StarknetTypeConcrete::StorageBaseAddress(_),
        ) => match return_ptr {
            Some(return_ptr) => Ok(Value::from_ptr(return_ptr, type_id, registry, true)?),
            None => {
                #[cfg(target_arch = "x86_64")]
                // Since x86_64's return values hold at most two different 64bit registers,
                // everything bigger than u128 will be returned by memory, therefore making
                // this branch is unreachable on that architecture.
                return Err(Error::ParseAttributeError);

                #[cfg(target_arch = "aarch64")]
                Ok(Value::Felt252({
                    let data = unsafe {
                        std::mem::transmute::<&mut [u64; 4], &mut [u8; 32]>(&mut ret_registers)
                    };
                    data[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).
                    starknet_types_core::felt::Felt::from_bytes_le(data)
                }))
            }
        },
        CoreTypeConcrete::Bytes31(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::from_ptr(return_ptr, type_id, registry, true)?),
            None => {
                #[cfg(target_arch = "x86_64")]
                // Since x86_64's return values hold at most two different 64bit registers,
                // everything bigger than u128 will be returned by memory, therefore making
                // this branch is unreachable on that architecture.
                return Err(Error::ParseAttributeError);

                #[cfg(target_arch = "aarch64")]
                Ok(Value::Bytes31(unsafe {
                    *std::mem::transmute::<&[u64; 4], &[u8; 31]>(&ret_registers)
                }))
            }
        },
        CoreTypeConcrete::BoundedInt(info) => match return_ptr {
            Some(return_ptr) => Ok(Value::from_ptr(return_ptr, type_id, registry, true)?),
            None => {
                let mut data = if info.range.offset_bit_width() <= 64 {
                    BigInt::from(ret_registers[0])
                } else {
                    BigInt::from(((ret_registers[1] as u128) << 64) | ret_registers[0] as u128)
                };

                data &= (BigInt::one() << info.range.offset_bit_width()) - BigInt::one();
                data += &info.range.lower;

                Ok(Value::BoundedInt {
                    value: data.into(),
                    range: info.range.clone(),
                })
            }
        },
        CoreTypeConcrete::Uint8(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Uint8(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Uint8(ret_registers[0] as u8)),
        },
        CoreTypeConcrete::Uint16(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Uint16(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Uint16(ret_registers[0] as u16)),
        },
        CoreTypeConcrete::Uint32(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Uint32(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Uint32(ret_registers[0] as u32)),
        },
        CoreTypeConcrete::Uint64(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Uint64(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Uint64(ret_registers[0])),
        },
        CoreTypeConcrete::Uint128(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Uint128(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Uint128(
                ((ret_registers[1] as u128) << 64) | ret_registers[0] as u128,
            )),
        },
        CoreTypeConcrete::Sint8(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Sint8(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Sint8(ret_registers[0] as i8)),
        },
        CoreTypeConcrete::Sint16(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Sint16(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Sint16(ret_registers[0] as i16)),
        },
        CoreTypeConcrete::Sint32(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Sint32(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Sint32(ret_registers[0] as i32)),
        },
        CoreTypeConcrete::Sint64(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Uint64(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Sint64(ret_registers[0] as i64)),
        },
        CoreTypeConcrete::Sint128(_) => match return_ptr {
            Some(return_ptr) => Ok(Value::Uint128(unsafe { *return_ptr.cast().as_ref() })),
            None => Ok(Value::Sint128(
                ((ret_registers[1] as i128) << 64) | ret_registers[0] as i128,
            )),
        },
        CoreTypeConcrete::NonZero(info) => {
            parse_result(&info.ty, registry, return_ptr, ret_registers)
        }
        CoreTypeConcrete::Nullable(info) => unsafe {
            let ptr = return_ptr.map_or(ret_registers[0] as *mut (), |x| {
                *x.cast::<*mut ()>().as_ref()
            });
            if ptr.is_null() {
                Ok(Value::Null)
            } else {
                let ptr = NonNull::new_unchecked(ptr);
                let value = Value::from_ptr(ptr, &info.ty, registry, true)?;
                libc_free(ptr.as_ptr().cast());
                Ok(value)
            }
        },
        CoreTypeConcrete::Enum(info) => {
            let (_, tag_layout, variant_layouts) =
                crate::types::r#enum::get_layout_for_variants(registry, &info.variants)?;

            let (tag, ptr) = if type_info.is_memory_allocated(registry)? || return_ptr.is_some() {
                let ptr = return_ptr.to_native_assert_error("return pointer should be valid")?;

                let tag = unsafe {
                    match tag_layout.size() {
                        0 => 0,
                        1 => *ptr.cast::<u8>().as_ref() as usize,
                        2 => *ptr.cast::<u16>().as_ref() as usize,
                        4 => *ptr.cast::<u32>().as_ref() as usize,
                        8 => *ptr.cast::<u64>().as_ref() as usize,
                        _ => return Err(Error::ParseAttributeError),
                    }
                };

                // Filter out bits that are not part of the enum's tag.
                let tag = tag
                    & 1usize
                        .wrapping_shl(info.variants.len().next_power_of_two().trailing_zeros())
                        .wrapping_sub(1);

                (
                    tag,
                    Ok(unsafe {
                        NonNull::new_unchecked(
                            ptr.cast::<u8>()
                                .as_ptr()
                                .add(tag_layout.extend(variant_layouts[tag])?.1),
                        )
                        .cast()
                    }),
                )
            } else {
                match info.variants.len() {
                    0 | 1 => (0, Err(0)),
                    _ => (
                        match tag_layout.size() {
                            1 => ret_registers[0] as u8 as usize,
                            2 => ret_registers[0] as u16 as usize,
                            4 => ret_registers[0] as u32 as usize,
                            8 => ret_registers[0] as usize,
                            _ => return Err(Error::ParseAttributeError),
                        },
                        Err(1),
                    ),
                }
            };
            let value = match ptr {
                Ok(ptr) => Box::new(Value::from_ptr(ptr, &info.variants[tag], registry, true)?),
                Err(offset) => {
                    ret_registers.copy_within(offset.., 0);
                    Box::new(parse_result(
                        &info.variants[tag],
                        registry,
                        None,
                        ret_registers,
                    )?)
                }
            };

            Ok(Value::Enum {
                tag,
                value,
                debug_name: Some(type_info.info().long_id.to_string()),
            })
        }
        CoreTypeConcrete::Struct(info) => {
            if info.members.is_empty() {
                Ok(Value::Struct {
                    fields: Vec::new(),
                    debug_name: Some(type_info.info().long_id.to_string()),
                })
            } else {
                Ok(Value::from_ptr(
                    return_ptr.to_native_assert_error("return pointer should be valid")?,
                    type_id,
                    registry,
                    true,
                )?)
            }
        }
        CoreTypeConcrete::Felt252Dict(_) | CoreTypeConcrete::SquashedFelt252Dict(_) => unsafe {
            let ptr = return_ptr
                .unwrap_or_else(|| NonNull::new_unchecked((&raw mut ret_registers[0]) as *mut ()));
            Ok(Value::from_ptr(ptr, type_id, registry, true)?)
        },

        CoreTypeConcrete::Snapshot(info) => {
            parse_result(&info.ty, registry, return_ptr, ret_registers)
        }

        // Builtins are handled before the call to parse_result
        // and should not be reached here.
        CoreTypeConcrete::Bitwise(_)
        | CoreTypeConcrete::Const(_)
        | CoreTypeConcrete::EcOp(_)
        | CoreTypeConcrete::GasBuiltin(_)
        | CoreTypeConcrete::BuiltinCosts(_)
        | CoreTypeConcrete::RangeCheck(_)
        | CoreTypeConcrete::Pedersen(_)
        | CoreTypeConcrete::Poseidon(_)
        | CoreTypeConcrete::SegmentArena(_)
        | CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_)) => {
            native_panic!("builtins should have been handled before")
        }

        CoreTypeConcrete::Felt252DictEntry(_)
        | CoreTypeConcrete::Span(_)
        | CoreTypeConcrete::Uninitialized(_)
        | CoreTypeConcrete::Coupon(_)
        | CoreTypeConcrete::Starknet(_)
        | CoreTypeConcrete::Uint128MulGuarantee(_)
        | CoreTypeConcrete::Circuit(_)
        | CoreTypeConcrete::RangeCheck96(_) => {
            native_panic!("range check 96 not yet implemented as results")
        }
        // 2.9.0
        CoreTypeConcrete::IntRange(_) => native_panic!("int range not yet implemented as results"),
        // 2.11.1
        CoreTypeConcrete::Blake(_) => native_panic!("blake not yet implemented as results"),
        // 2.12.0
        CoreTypeConcrete::GasReserve(_) => {
            native_panic!("gas reserve not yet implemented as results")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        context::NativeContext, include_contract, starknet_stub::StubSyscallHandler,
        utils::testing::load_program, OptLevel,
    };
    use cairo_lang_sierra::program::Program;
    use rstest::*;
    use starknet_types_core::felt::Felt;

    #[fixture]
    fn program() -> Program {
        load_program("test_data_artifacts/programs/executor_program")
    }

    #[fixture]
    fn starknet_program() -> Program {
        include_contract!("test_data_artifacts/contracts/simple_storage_42.contract.json")
            .extract_sierra_program(true)
            .unwrap()
            .program
    }

    #[rstest]
    fn test_invoke_dynamic_aot_native_executor(program: Program) {
        let native_context = NativeContext::new();
        let module = native_context
            .compile(&program, false, Some(Default::default()), None)
            .expect("failed to compile context");
        let executor = AotNativeExecutor::from_native_module(module, OptLevel::default()).unwrap();

        // The first function in the program is `run_test`.
        let entrypoint_function_id = &program.funcs.first().expect("should have a function").id;

        let result = executor
            .invoke_dynamic(entrypoint_function_id, &[], Some(u64::MAX))
            .unwrap();

        assert_eq!(result.return_value, Value::Felt252(Felt::from(42)));
    }

    #[rstest]
    fn test_invoke_dynamic_jit_native_executor(program: Program) {
        let native_context = NativeContext::new();
        let module = native_context
            .compile(&program, false, None, None)
            .expect("failed to compile context");
        let executor = JitNativeExecutor::from_native_module(module, OptLevel::default()).unwrap();

        // The first function in the program is `run_test`.
        let entrypoint_function_id = &program.funcs.first().expect("should have a function").id;

        let result = executor
            .invoke_dynamic(entrypoint_function_id, &[], Some(u64::MAX))
            .unwrap();

        assert_eq!(result.return_value, Value::Felt252(Felt::from(42)));
    }

    #[rstest]
    fn test_invoke_contract_dynamic_aot(starknet_program: Program) {
        let native_context = NativeContext::new();
        let module = native_context
            .compile(&starknet_program, false, Some(Default::default()), None)
            .expect("failed to compile context");
        let executor = AotNativeExecutor::from_native_module(module, OptLevel::default()).unwrap();

        let entrypoint_function_id = &starknet_program
            .funcs
            .iter()
            .find(|f| {
                f.id.debug_name
                    .as_ref()
                    .map(|name| name.contains("__wrapper__ISimpleStorageImpl__get"))
                    .unwrap_or_default()
            })
            .expect("should have a function")
            .id;

        let result = executor
            .invoke_contract_dynamic(
                entrypoint_function_id,
                &[],
                Some(u64::MAX),
                &mut StubSyscallHandler::default(),
            )
            .unwrap();

        assert_eq!(result.return_values, vec![Felt::from(42)]);
    }

    #[rstest]
    fn test_invoke_contract_dynamic_jit(starknet_program: Program) {
        let native_context = NativeContext::new();
        let module = native_context
            .compile(&starknet_program, false, Some(Default::default()), None)
            .expect("failed to compile context");
        let executor = JitNativeExecutor::from_native_module(module, OptLevel::default()).unwrap();

        let entrypoint_function_id = &starknet_program
            .funcs
            .iter()
            .find(|f| {
                f.id.debug_name
                    .as_ref()
                    .map(|name| name.contains("__wrapper__ISimpleStorageImpl__get"))
                    .unwrap_or_default()
            })
            .expect("should have a function")
            .id;

        let result = executor
            .invoke_contract_dynamic(
                entrypoint_function_id,
                &[],
                Some(u64::MAX),
                &mut StubSyscallHandler::default(),
            )
            .unwrap();

        assert_eq!(result.return_values, vec![Felt::from(42)]);
    }
}
