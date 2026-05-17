#![cfg(feature = "with-trace-dump")]

use crate::error::{Error, Result};
use cairo_lang_sierra::{
    ids::{ConcreteTypeId, VarId},
    program::StatementIdx,
};
use melior::{
    dialect::{llvm, memref, ods},
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{
        attribute::{FlatSymbolRefAttribute, StringAttribute, TypeAttribute},
        operation::OperationBuilder,
        r#type::{IntegerType, MemRefType},
        Attribute, Block, BlockLike, Location, Module, Region, Value,
    },
    Context,
};
use std::{collections::HashSet, ffi::c_void, ptr};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TraceBinding {
    State,
    Push,
    TraceId,
}

impl TraceBinding {
    pub const fn symbol(self) -> &'static str {
        match self {
            TraceBinding::State => "cairo_native__trace_dump__add_variable_to_state",
            TraceBinding::Push => "cairo_native__trace_dump__push_state_to_trace_dump",
            TraceBinding::TraceId => "cairo_native__trace_dump__trace_id",
        }
    }

    const fn function_ptr(self) -> *const () {
        match self {
            TraceBinding::State => trace_dump_runtime::add_variable_to_state as *const (),
            TraceBinding::Push => trace_dump_runtime::push_state_to_trace_dump as *const (),
            // it has no function pointer, as its a global constant
            TraceBinding::TraceId => ptr::null(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TraceDumpMeta {
    active_map: HashSet<TraceBinding>,
}

impl TraceDumpMeta {
    /// Register the global for the given binding, if not yet registered, and return
    /// a pointer to the stored value.
    ///
    /// For the function to be available, `setup_runtime` must be called before running the module
    fn build_function<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        binding: TraceBinding,
    ) -> Result<Value<'c, 'a>> {
        if self.active_map.insert(binding) {
            module.body().append_operation(
                ods::llvm::mlir_global(
                    context,
                    Region::new(),
                    TypeAttribute::new(llvm::r#type::pointer(context, 0)),
                    StringAttribute::new(context, binding.symbol()),
                    Attribute::parse(context, "#llvm.linkage<weak>")
                        .ok_or(Error::ParseAttributeError)?,
                    location,
                )
                .into(),
            );
        }

        let global_address = block.append_op_result(
            ods::llvm::mlir_addressof(
                context,
                llvm::r#type::pointer(context, 0),
                FlatSymbolRefAttribute::new(context, binding.symbol()),
                location,
            )
            .into(),
        )?;

        Ok(block.load(
            context,
            location,
            global_address,
            llvm::r#type::pointer(context, 0),
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_state(
        &mut self,
        context: &Context,
        module: &Module,
        block: &Block,
        var_id: &VarId,
        value_ty: &ConcreteTypeId,
        value_ptr: Value,
        location: Location,
    ) -> Result<()> {
        let trace_id = self.build_trace_id(context, module, block, location)?;

        let var_id = block.const_int(context, location, var_id.id, 64)?;
        let value_ty = block.const_int(context, location, value_ty.id, 64)?;

        let function =
            self.build_function(context, module, block, location, TraceBinding::State)?;
        block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[trace_id, var_id, value_ty, value_ptr])
                .build()?,
        );

        Ok(())
    }

    pub fn build_push(
        &mut self,
        context: &Context,
        module: &Module,
        block: &Block,
        statement_idx: StatementIdx,
        location: Location,
    ) -> Result<()> {
        let trace_id = self.build_trace_id(context, module, block, location)?;
        let statement_idx = block.const_int(context, location, statement_idx.0, 64)?;

        let function = self.build_function(context, module, block, location, TraceBinding::Push)?;

        block.append_operation(
            OperationBuilder::new("llvm.call", location)
                .add_operands(&[function])
                .add_operands(&[trace_id, statement_idx])
                .build()?,
        );

        Ok(())
    }

    pub fn build_trace_id<'c, 'a>(
        &mut self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
    ) -> Result<Value<'c, 'a>> {
        if self.active_map.insert(TraceBinding::TraceId) {
            module.body().append_operation(memref::global(
                context,
                TraceBinding::TraceId.symbol(),
                None,
                MemRefType::new(IntegerType::new(context, 64).into(), &[], None, None),
                None,
                false,
                None,
                location,
            ));
        }

        let trace_id_ptr = block
            .append_op_result(memref::get_global(
                context,
                TraceBinding::TraceId.symbol(),
                MemRefType::new(IntegerType::new(context, 64).into(), &[], None, None),
                location,
            ))
            .unwrap();

        Ok(block.append_op_result(memref::load(trace_id_ptr, &[], location))?)
    }
}

pub fn setup_runtime(find_symbol_ptr: impl Fn(&str) -> Option<*mut c_void>) {
    let bindings = &[TraceBinding::State, TraceBinding::Push];

    for binding in bindings {
        if let Some(global) = find_symbol_ptr(binding.symbol()) {
            let global = global.cast::<*const ()>();
            unsafe { *global = binding.function_ptr() };
        }
    }
}

pub mod trace_dump_runtime {
    #![allow(non_snake_case)]

    use cairo_lang_sierra::{
        extensions::{
            bounded_int::BoundedIntConcreteType,
            circuit::CircuitTypeConcrete,
            core::{CoreLibfunc, CoreType, CoreTypeConcrete},
            starknet::{secp256::Secp256PointTypeConcrete, StarknetTypeConcrete},
        },
        ids::{ConcreteTypeId, VarId},
        program::{GenericArg, StatementIdx},
        program_registry::ProgramRegistry,
    };
    use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
    use itertools::Itertools;
    use num_bigint::{BigInt, BigUint, Sign};
    use num_traits::One;
    use sierra_emu::{
        starknet::{
            value_conv::{secp256k1_point_into_value, secp256r1_point_into_value},
            Secp256k1Point as EmuSecp256k1Point, Secp256r1Point as EmuSecp256r1Point,
            U256 as EmuU256,
        },
        ProgramTrace, StateDump, Value,
    };
    use starknet_types_core::felt::Felt;
    use std::{
        alloc::Layout,
        collections::HashMap,
        mem::swap,
        ops::Range,
        ptr::NonNull,
        sync::{LazyLock, Mutex},
    };

    use crate::{
        starknet::ArrayAbi,
        types::TypeBuilder,
        utils::{get_integer_layout, layout_repeat},
    };

    use crate::runtime::FeltDict;

    pub static TRACE_DUMP: LazyLock<Mutex<HashMap<u64, TraceDump>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    /// An in-progress trace dump for a particular execution
    pub struct TraceDump {
        pub trace: ProgramTrace,
        /// Represents the latest state. All values are added to
        /// this state until pushed to the trace.
        state: OrderedHashMap<VarId, Value>,
        registry: ProgramRegistry<CoreType, CoreLibfunc>,
    }

    impl TraceDump {
        pub fn new(registry: ProgramRegistry<CoreType, CoreLibfunc>) -> Self {
            Self {
                trace: ProgramTrace::default(),
                state: OrderedHashMap::default(),
                registry,
            }
        }
    }

    /// Adds a new variable to the current state of the trace dump with the
    /// given identifier.
    ///
    /// Receives a pointer to the value, even if the value is a pointer itself.
    pub unsafe extern "C" fn add_variable_to_state(
        trace_id: u64,
        var_id: u64,
        type_id: u64,
        value_ptr: NonNull<()>,
    ) {
        let mut trace_dump = TRACE_DUMP.lock().unwrap();
        let Some(trace_dump) = trace_dump.get_mut(&trace_id) else {
            eprintln!("Could not find trace dump!");
            return;
        };

        let type_id = ConcreteTypeId::new(type_id);
        let value = value_from_ptr(&trace_dump.registry, &type_id, value_ptr);

        trace_dump.state.insert(VarId::new(var_id), value);
    }

    /// Pushes the latest state to the trace dump with the given identifier.
    ///
    /// It is called after all variables have been added with `add_variable_to_state`.
    pub unsafe extern "C" fn push_state_to_trace_dump(trace_id: u64, statement_idx: u64) {
        let mut trace_dump = TRACE_DUMP.lock().unwrap();
        let Some(trace_dump) = trace_dump.get_mut(&trace_id) else {
            eprintln!("Could not find trace dump!");
            return;
        };

        let mut items = OrderedHashMap::default();
        swap(&mut items, &mut trace_dump.state);

        trace_dump
            .trace
            .push(StateDump::new(StatementIdx(statement_idx as usize), items));
    }

    /// TODO: Can we reuse `cairo_native::Value::from_ptr`?
    unsafe fn value_from_ptr(
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
        type_id: &ConcreteTypeId,
        value_ptr: NonNull<()>,
    ) -> Value {
        let type_info = registry.get_type(type_id).unwrap();
        match type_info {
            CoreTypeConcrete::Felt252(_)
            | CoreTypeConcrete::Starknet(StarknetTypeConcrete::ContractAddress(_))
            | CoreTypeConcrete::Starknet(StarknetTypeConcrete::ClassHash(_))
            | CoreTypeConcrete::Starknet(StarknetTypeConcrete::StorageAddress(_))
            | CoreTypeConcrete::Starknet(StarknetTypeConcrete::StorageBaseAddress(_)) => {
                Value::Felt(Felt::from_bytes_le(value_ptr.cast().as_ref()))
            }
            CoreTypeConcrete::Uint8(_) => Value::U8(value_ptr.cast().read()),
            CoreTypeConcrete::Uint16(_) => Value::U16(value_ptr.cast().read()),
            CoreTypeConcrete::Uint32(_) => Value::U32(value_ptr.cast().read()),
            CoreTypeConcrete::Uint64(_) | CoreTypeConcrete::GasBuiltin(_) => {
                Value::U64(value_ptr.cast().read())
            }
            CoreTypeConcrete::Uint128(_) => Value::U128(value_ptr.cast().read()),

            CoreTypeConcrete::BoundedInt(BoundedIntConcreteType { range, .. }) => {
                let n_bits = ((range.size() - BigInt::one()).bits() as u32).max(1);
                let n_bytes = n_bits.next_multiple_of(8) >> 3;

                let data = NonNull::slice_from_raw_parts(value_ptr.cast::<u8>(), n_bytes as usize);

                let value = BigInt::from_bytes_le(num_bigint::Sign::Plus, data.as_ref());

                Value::BoundedInt {
                    range: Range {
                        start: range.lower.clone(),
                        end: range.upper.clone(),
                    },
                    value: value + &range.lower,
                }
            }

            CoreTypeConcrete::EcPoint(_) => {
                let layout = Layout::new::<()>();
                let (x, layout) = {
                    let (layout, offset) = layout.extend(Layout::new::<[u128; 2]>()).unwrap();
                    (
                        Felt::from_bytes_le(value_ptr.byte_add(offset).cast().as_ref()),
                        layout,
                    )
                };
                let (y, _) = {
                    let (layout, offset) = layout.extend(Layout::new::<[u128; 2]>()).unwrap();
                    (
                        Felt::from_bytes_le(value_ptr.byte_add(offset).cast().as_ref()),
                        layout,
                    )
                };

                Value::EcPoint { x, y }
            }
            CoreTypeConcrete::EcState(_) => {
                let layout = Layout::new::<()>();
                let (x0, layout) = {
                    let (layout, offset) = layout.extend(Layout::new::<[u128; 2]>()).unwrap();
                    (
                        Felt::from_bytes_le(value_ptr.byte_add(offset).cast().as_ref()),
                        layout,
                    )
                };
                let (y0, layout) = {
                    let (layout, offset) = layout.extend(Layout::new::<[u128; 2]>()).unwrap();
                    (
                        Felt::from_bytes_le(value_ptr.byte_add(offset).cast().as_ref()),
                        layout,
                    )
                };
                let (x1, layout) = {
                    let (layout, offset) = layout.extend(Layout::new::<[u128; 2]>()).unwrap();
                    (
                        Felt::from_bytes_le(value_ptr.byte_add(offset).cast().as_ref()),
                        layout,
                    )
                };
                let (y1, _) = {
                    let (layout, offset) = layout.extend(Layout::new::<[u128; 2]>()).unwrap();
                    (
                        Felt::from_bytes_le(value_ptr.byte_add(offset).cast().as_ref()),
                        layout,
                    )
                };

                Value::EcState { x0, y0, x1, y1 }
            }

            CoreTypeConcrete::Uninitialized(info) => Value::Uninitialized {
                ty: info.ty.clone(),
            },
            CoreTypeConcrete::Box(info) => {
                value_from_ptr(registry, &info.ty, value_ptr.cast::<NonNull<()>>().read())
            }
            CoreTypeConcrete::Array(info) => {
                let array = value_ptr.cast::<ArrayAbi<()>>().read();

                let layout = registry
                    .get_type(&info.ty)
                    .unwrap()
                    .layout(registry)
                    .unwrap()
                    .pad_to_align();

                let mut data = Vec::with_capacity((array.until - array.since) as usize);

                if !array.ptr.is_null() {
                    let data_ptr = array.ptr.read();
                    for index in (array.since)..array.until {
                        let index = index as usize;

                        data.push(value_from_ptr(
                            registry,
                            &info.ty,
                            NonNull::new(data_ptr.byte_add(layout.size() * index)).unwrap(),
                        ));
                    }
                }

                Value::Array {
                    ty: info.ty.clone(),
                    data,
                }
            }

            CoreTypeConcrete::Struct(info) => {
                let mut layout = Layout::new::<()>();
                let mut members = Vec::with_capacity(info.members.len());
                for member_ty in &info.members {
                    let type_info = registry.get_type(member_ty).unwrap();
                    let member_layout = type_info.layout(registry).unwrap();

                    let offset;
                    (layout, offset) = layout.extend(member_layout).unwrap();

                    let current_ptr = value_ptr.byte_add(offset);
                    members.push(value_from_ptr(registry, member_ty, current_ptr));
                }

                Value::Struct(members)
            }
            CoreTypeConcrete::Enum(info) => {
                let tag_bits = info.variants.len().next_power_of_two().trailing_zeros();
                let (tag_value, layout) = match tag_bits {
                    0 => (0, Layout::new::<()>()),
                    width if width <= 8 => {
                        (value_ptr.cast::<u8>().read() as usize, Layout::new::<u8>())
                    }
                    width if width <= 16 => (
                        value_ptr.cast::<u16>().read() as usize,
                        Layout::new::<u16>(),
                    ),
                    width if width <= 32 => (
                        value_ptr.cast::<u32>().read() as usize,
                        Layout::new::<u32>(),
                    ),
                    width if width <= 64 => (
                        value_ptr.cast::<u64>().read() as usize,
                        Layout::new::<u64>(),
                    ),
                    width if width <= 128 => (
                        value_ptr.cast::<u128>().read() as usize,
                        Layout::new::<u128>(),
                    ),
                    _ => todo!(),
                };

                let payload = {
                    let (_, offset) = layout
                        .extend(
                            registry
                                .get_type(&info.variants[tag_value])
                                .unwrap()
                                .layout(registry)
                                .unwrap(),
                        )
                        .unwrap();

                    value_from_ptr(
                        registry,
                        &info.variants[tag_value],
                        value_ptr.byte_add(offset),
                    )
                };

                Value::Enum {
                    self_ty: type_id.clone(),
                    index: tag_value,
                    payload: Box::new(payload),
                }
            }

            CoreTypeConcrete::NonZero(info) | CoreTypeConcrete::Snapshot(info) => {
                value_from_ptr(registry, &info.ty, value_ptr)
            }

            // Builtins and other unit types:
            CoreTypeConcrete::Bitwise(_)
            | CoreTypeConcrete::EcOp(_)
            | CoreTypeConcrete::Pedersen(_)
            | CoreTypeConcrete::Poseidon(_)
            | CoreTypeConcrete::RangeCheck96(_)
            | CoreTypeConcrete::RangeCheck(_)
            | CoreTypeConcrete::SegmentArena(_)
            | CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_))
            | CoreTypeConcrete::Uint128MulGuarantee(_) => Value::Unit,

            CoreTypeConcrete::BuiltinCosts(_) => {
                let builtin_costs = value_ptr.cast::<&[u64; 8]>().read();
                Value::BuiltinCosts(sierra_emu::BuiltinCosts {
                    r#const: builtin_costs[0],
                    pedersen: builtin_costs[1],
                    bitwise: builtin_costs[2],
                    ecop: builtin_costs[3],
                    poseidon: builtin_costs[4],
                    add_mod: builtin_costs[5],
                    mul_mod: builtin_costs[6],
                    blake: builtin_costs[7],
                })
            }

            // TODO:
            CoreTypeConcrete::Coupon(_) => todo!("CoreTypeConcrete::Coupon"),
            CoreTypeConcrete::Circuit(circuit) => match circuit {
                CircuitTypeConcrete::AddMod(_) => Value::Unit,
                CircuitTypeConcrete::MulMod(_) => Value::Unit,
                CircuitTypeConcrete::AddModGate(_) => Value::Unit,
                CircuitTypeConcrete::Circuit(_) => Value::Unit,
                CircuitTypeConcrete::CircuitData(info) => {
                    let Some(GenericArg::Type(circuit_type_id)) =
                        info.info.long_id.generic_args.first()
                    else {
                        panic!("generic arg should be a type");
                    };
                    let CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(circuit)) =
                        registry.get_type(circuit_type_id).unwrap()
                    else {
                        panic!("generic arg should be a Circuit");
                    };

                    let u384_layout = Layout::from_size_align(48, 16).unwrap();

                    let n_inputs = circuit.circuit_info.n_inputs;
                    let mut values = Vec::with_capacity(n_inputs);

                    let value_ptr = value_ptr.cast::<[u8; 48]>();

                    for i in 0..n_inputs {
                        let size = u384_layout.pad_to_align().size();
                        let current_ptr = value_ptr.byte_add(size * i);
                        let current_value = current_ptr.as_ref();
                        values.push(BigUint::from_bytes_le(current_value));
                    }

                    Value::Circuit(values)
                }
                CircuitTypeConcrete::CircuitOutputs(info) => {
                    let Some(GenericArg::Type(circuit_type_id)) =
                        info.info.long_id.generic_args.first()
                    else {
                        panic!("generic arg should be a type");
                    };
                    let CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(circuit)) =
                        registry.get_type(circuit_type_id).unwrap()
                    else {
                        panic!("generic arg should be a Circuit");
                    };

                    let u96_layout = get_integer_layout(96);

                    let n_outputs = circuit.circuit_info.values.len();
                    let mut values = Vec::with_capacity(n_outputs);

                    let (u384_struct_layout, _) = layout_repeat(&u96_layout, 4).unwrap();
                    let (gates_array_layout, gate_stride) =
                        layout_repeat(&u384_struct_layout, n_outputs).unwrap();
                    let (_, modulus_offset) =
                        gates_array_layout.extend(u384_struct_layout).unwrap();

                    let value_ptr = value_ptr.cast::<[u8; 12]>();

                    // get gate values
                    for i in 0..n_outputs {
                        let gate_ptr = value_ptr.byte_add(gate_stride * i);
                        values.push(u384_struct_to_bigint(gate_ptr, 4));
                    }

                    // get modulus value
                    let modulus_ptr = value_ptr.byte_add(modulus_offset);
                    let modulus = u384_struct_to_bigint(modulus_ptr, 4);

                    Value::CircuitOutputs {
                        circuits: values,
                        modulus,
                    }
                }
                CircuitTypeConcrete::CircuitPartialOutputs(_) => {
                    todo!("CircuitTypeConcrete::CircuitPartialOutputs")
                }
                CircuitTypeConcrete::CircuitDescriptor(_) => Value::Unit,
                CircuitTypeConcrete::CircuitFailureGuarantee(_) => {
                    todo!("CircuitTypeConcrete::CircuitFailureGuarantee")
                }
                CircuitTypeConcrete::CircuitInput(_) => {
                    todo!("CircuitTypeConcrete::CircuitInput")
                }
                CircuitTypeConcrete::CircuitInputAccumulator(info) => {
                    let Some(GenericArg::Type(circuit_type_id)) =
                        info.info.long_id.generic_args.first()
                    else {
                        panic!("generic arg should be a type");
                    };
                    let CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(_)) =
                        registry.get_type(circuit_type_id).unwrap()
                    else {
                        panic!("generic arg should be a Circuit");
                    };

                    let u64_layout = Layout::new::<u64>();
                    let u384_layout = Layout::from_size_align(48, 16).unwrap();

                    let length = unsafe { *value_ptr.cast::<u64>().as_ptr() };

                    let (_, input_start_offset) = u64_layout.extend(u384_layout).unwrap();
                    let start_ptr = value_ptr.byte_add(input_start_offset).cast::<[u8; 48]>();

                    let mut values = Vec::with_capacity(length as usize);

                    for i in 0..length {
                        let size = u384_layout.pad_to_align().size();
                        let current_ptr = start_ptr.byte_add(size * i as usize);
                        let current_value = current_ptr.as_ref();
                        values.push(BigUint::from_bytes_le(current_value));
                    }

                    Value::Circuit(values)
                }
                CircuitTypeConcrete::CircuitModulus(_) => {
                    let value_ptr = value_ptr.cast::<[u8; 48]>();
                    let value = unsafe { value_ptr.as_ref() };
                    Value::CircuitModulus(BigUint::from_bytes_le(value))
                }
                CircuitTypeConcrete::InverseGate(_) => Value::Unit,
                CircuitTypeConcrete::MulModGate(_) => Value::Unit,
                CircuitTypeConcrete::SubModGate(_) => Value::Unit,
                CircuitTypeConcrete::U96Guarantee(_) => {
                    let value_ptr = value_ptr.cast::<[u8; 12]>();
                    let value = unsafe { value_ptr.as_ref() };

                    let mut array_value = [0u8; 16];
                    array_value[..12].clone_from_slice(value);

                    Value::U128(u128::from_le_bytes(array_value))
                }
                CircuitTypeConcrete::U96LimbsLessThanGuarantee(info) => {
                    let value_ptr = value_ptr.cast::<[u8; 12]>();

                    let u96_layout = get_integer_layout(96);
                    let (u384_struct_layout, struct_stride) =
                        layout_repeat(&u96_layout, info.limb_count).unwrap();

                    let output_limbs = (0..info.limb_count)
                        .map(|i| {
                            let current_ptr = value_ptr.byte_add(struct_stride * i);
                            Value::BoundedInt {
                                range: 0.into()..BigInt::one() << 96,
                                value: BigInt::from_bytes_le(Sign::Plus, current_ptr.as_ref()),
                            }
                        })
                        .collect::<Vec<_>>();

                    let modulus_ptr = value_ptr.byte_add(u384_struct_layout.size());

                    let modulus_limbs = (0..info.limb_count)
                        .map(|i| {
                            let current_ptr = modulus_ptr.byte_add(struct_stride * i);
                            Value::BoundedInt {
                                range: 0.into()..BigInt::one() << 96,
                                value: BigInt::from_bytes_le(Sign::Plus, current_ptr.as_ref()),
                            }
                        })
                        .collect::<Vec<_>>();

                    Value::Struct(vec![
                        Value::Struct(output_limbs),
                        Value::Struct(modulus_limbs),
                    ])
                }
            },
            CoreTypeConcrete::Const(_) => todo!("CoreTypeConcrete::Const"),
            CoreTypeConcrete::Sint8(_) => Value::I8(value_ptr.cast().read()),
            CoreTypeConcrete::Sint16(_) => todo!("CoreTypeConcrete::Sint16"),
            CoreTypeConcrete::Sint32(_) => Value::I32(value_ptr.cast().read()),
            CoreTypeConcrete::Sint64(_) => todo!("CoreTypeConcrete::Sint64"),
            CoreTypeConcrete::Sint128(_) => Value::I128(value_ptr.cast().read()),
            CoreTypeConcrete::Nullable(info) => {
                let inner_ptr = value_ptr.cast::<*mut ()>().read();
                match NonNull::new(inner_ptr) {
                    Some(inner_ptr) => value_from_ptr(registry, &info.ty, inner_ptr),
                    None => Value::Uninitialized {
                        ty: info.ty.clone(),
                    },
                }
            }

            CoreTypeConcrete::SquashedFelt252Dict(info) | CoreTypeConcrete::Felt252Dict(info) => {
                let value = value_ptr.cast::<&FeltDict>().read();

                let data = value
                    .mappings
                    .iter()
                    .map(|(&k, &i)| {
                        let p = value
                            .elements
                            .byte_offset((value.layout.size() * i) as isize);
                        let v = match NonNull::new(p) {
                            Some(value_ptr) => value_from_ptr(registry, &info.ty, value_ptr.cast()),
                            None => Value::Uninitialized {
                                ty: info.ty.clone(),
                            },
                        };
                        (k, v)
                    })
                    .collect::<HashMap<Felt, Value>>();

                Value::FeltDict {
                    ty: info.ty.clone(),
                    count: value.count,
                    data,
                }
            }
            CoreTypeConcrete::Felt252DictEntry(info) => {
                let value = value_ptr.cast::<FeltDictEntry>().read();

                let data = value
                    .dict
                    .mappings
                    .iter()
                    .map(|(&k, &i)| {
                        let p = value
                            .dict
                            .elements
                            .byte_offset((value.dict.layout.size() * i) as isize);
                        let v = match NonNull::new(p) {
                            Some(value_ptr) => value_from_ptr(registry, &info.ty, value_ptr.cast()),
                            None => Value::Uninitialized {
                                ty: info.ty.clone(),
                            },
                        };
                        (k, v)
                    })
                    .collect::<HashMap<Felt, Value>>();

                Value::FeltDictEntry {
                    ty: info.ty.clone(),
                    data,
                    count: value.dict.count,
                    key: *value.key,
                }
            }
            CoreTypeConcrete::Span(_) => todo!("CoreTypeConcrete::Span"),
            CoreTypeConcrete::Starknet(selector) => match selector {
                StarknetTypeConcrete::Secp256Point(selector) => match selector {
                    Secp256PointTypeConcrete::K1(_) => {
                        let point: Secp256Point = value_ptr.cast().read();
                        let emu_point = EmuSecp256k1Point {
                            x: EmuU256 {
                                lo: point.x.lo,
                                hi: point.x.hi,
                            },
                            y: EmuU256 {
                                lo: point.y.lo,
                                hi: point.y.hi,
                            },
                            is_infinity: point.is_infinity,
                        };
                        secp256k1_point_into_value(emu_point)
                    }
                    Secp256PointTypeConcrete::R1(_) => {
                        let point: Secp256Point = value_ptr.cast().read();
                        let emu_point = EmuSecp256r1Point {
                            x: EmuU256 {
                                lo: point.x.lo,
                                hi: point.x.hi,
                            },
                            y: EmuU256 {
                                lo: point.y.lo,
                                hi: point.y.hi,
                            },
                            is_infinity: point.is_infinity,
                        };
                        secp256r1_point_into_value(emu_point)
                    }
                },
                StarknetTypeConcrete::Sha256StateHandle(_) => {
                    let raw_data = value_ptr.cast::<NonNull<[u32; 8]>>().read().read();
                    let data = raw_data.into_iter().map(Value::U32).collect_vec();
                    Value::Struct(data)
                }
                _ => unreachable!(),
            },
            CoreTypeConcrete::Bytes31(_) => {
                let original_data: [u8; 31] = value_ptr.cast().read();
                let mut data = [0u8; 32];
                for (i, v) in original_data.into_iter().enumerate() {
                    data[i] = v
                }

                Value::Bytes31(Felt::from_bytes_le(&data))
            }
            CoreTypeConcrete::IntRange(info) => {
                let type_info = registry.get_type(&info.ty).unwrap();

                match type_info {
                    CoreTypeConcrete::Sint8(_) => {
                        let value = value_ptr.cast::<IntRange<i8>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Sint16(_) => {
                        let value = value_ptr.cast::<IntRange<i16>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Sint32(_) => {
                        let value = value_ptr.cast::<IntRange<i32>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Sint64(_) => {
                        let value = value_ptr.cast::<IntRange<i64>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Sint128(_) => {
                        let value = value_ptr.cast::<IntRange<i128>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Uint8(_) => {
                        let value = value_ptr.cast::<IntRange<u8>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Uint16(_) => {
                        let value = value_ptr.cast::<IntRange<u16>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Uint32(_) => {
                        let value = value_ptr.cast::<IntRange<u32>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Uint64(_) => {
                        let value = value_ptr.cast::<IntRange<u64>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    CoreTypeConcrete::Uint128(_) => {
                        let value = value_ptr.cast::<IntRange<u128>>().read();
                        Value::IntRange {
                            x: Box::new(value.x.into()),
                            y: Box::new(value.y.into()),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            CoreTypeConcrete::Blake(_)
            | CoreTypeConcrete::GasReserve(_)
            | CoreTypeConcrete::QM31(_) => {
                todo!()
            }
        }
    }

    unsafe fn u384_struct_to_bigint(value_ptr: NonNull<[u8; 12]>, limbs_count: usize) -> BigUint {
        let u96_layout = get_integer_layout(96);
        let (_, elem_stride) = layout_repeat(&u96_layout, 4).unwrap();

        let output_limbs = (0..limbs_count)
            .flat_map(|i| {
                let offset = elem_stride * i;
                *value_ptr.byte_add(offset).as_ref()
            })
            .collect::<Vec<u8>>();

        BigUint::from_bytes_le(&output_limbs)
    }

    #[derive(Debug)]
    #[repr(C)]
    struct IntRange<T> {
        x: T,
        y: T,
    }

    #[derive(Debug)]
    struct FeltDictEntry<'a> {
        dict: &'a FeltDict,
        key: &'a Felt,
    }

    #[repr(C, align(16))]
    pub struct Secp256Point {
        pub x: U256,
        pub y: U256,
        pub is_infinity: bool,
    }

    #[repr(C, align(16))]
    pub struct U256 {
        pub lo: u128,
        pub hi: u128,
    }
}
