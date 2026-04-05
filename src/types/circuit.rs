//! # `Circuit` type

use std::alloc::Layout;

use super::WithSelf;
use crate::{
    error::{Result, SierraAssertError},
    metadata::{
        drop_overrides::DropOverridesMeta, dup_overrides::DupOverridesMeta,
        realloc_bindings::ReallocBindingsMeta, MetadataStorage,
    },
    utils::{get_integer_layout, layout_repeat, ProgramRegistryExt},
};
use cairo_lang_sierra::{
    extensions::{
        circuit::{CircuitTypeConcrete, ConcreteU96LimbsLessThanGuarantee},
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        types::InfoOnlyConcreteType,
    },
    program::GenericArg,
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{func, llvm},
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, BlockLike, Location, Module, Region, Type, Value},
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
    selector: WithSelf<CircuitTypeConcrete>,
) -> Result<Type<'ctx>> {
    match &*selector {
        CircuitTypeConcrete::CircuitModulus(_) => Ok(IntegerType::new(context, 384).into()),
        CircuitTypeConcrete::U96Guarantee(_) => Ok(IntegerType::new(context, 96).into()),
        CircuitTypeConcrete::CircuitInputAccumulator(info) => build_circuit_accumulator(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        CircuitTypeConcrete::CircuitData(info) => build_circuit_data(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        CircuitTypeConcrete::CircuitOutputs(info) => build_circuit_outputs(
            context,
            module,
            registry,
            metadata,
            WithSelf::new(selector.self_ty(), info),
        ),
        CircuitTypeConcrete::U96LimbsLessThanGuarantee(info) => {
            build_u96_limbs_less_than_guarantee(
                context,
                module,
                registry,
                metadata,
                WithSelf::new(selector.self_ty(), info),
            )
        }
        // builtins
        CircuitTypeConcrete::AddMod(_) | CircuitTypeConcrete::MulMod(_) => {
            Ok(IntegerType::new(context, 64).into())
        }
        // noops
        CircuitTypeConcrete::CircuitDescriptor(_)
        | CircuitTypeConcrete::CircuitFailureGuarantee(_)
        | CircuitTypeConcrete::CircuitPartialOutputs(_) => {
            Ok(llvm::r#type::array(IntegerType::new(context, 8).into(), 0))
        }
        // phantoms
        CircuitTypeConcrete::Circuit(_)
        | CircuitTypeConcrete::AddModGate(_)
        | CircuitTypeConcrete::SubModGate(_)
        | CircuitTypeConcrete::MulModGate(_)
        | CircuitTypeConcrete::InverseGate(_)
        | CircuitTypeConcrete::CircuitInput(_) => {
            Err(SierraAssertError::BadTypeInit(selector.self_ty.clone()))?
        }
    }
}

/// Builds the circuit accumulator type.
///
/// ## Layout:
///
/// Holds up to N_INPUTS elements. Where each element is an u384 integer.
///
/// ```txt
/// type = struct {
///     size: u64,
///     data: *u384,
/// }
/// ```
pub fn build_circuit_accumulator<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    if metadata.get::<ReallocBindingsMeta>().is_none() {
        metadata.insert(ReallocBindingsMeta::new(context, module));
    }

    let Some(GenericArg::Type(circuit_type_id)) = info.info.long_id.generic_args.first() else {
        return Err(SierraAssertError::BadTypeInfo.into());
    };
    let CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(circuit)) =
        registry.get_type(circuit_type_id)?
    else {
        return Err(SierraAssertError::BadTypeInfo.into());
    };

    DupOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            let location = Location::unknown(context);
            let region = Region::new();
            let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;
            let entry = region.append_block(Block::new(&[(value_ty, location)]));

            let accumulator = entry.arg(0)?;
            let inputs_ptr = entry.extract_value(
                context,
                location,
                accumulator,
                llvm::r#type::pointer(context, 0),
                1,
            )?;

            let u384_layout = get_integer_layout(384);

            let new_inputs_ptr = build_array_dup(
                context,
                &entry,
                location,
                inputs_ptr,
                circuit.circuit_info.n_inputs,
                u384_layout,
            )?;

            let new_accumulator =
                entry.insert_value(context, location, accumulator, new_inputs_ptr, 1)?;
            entry.append_operation(func::r#return(&[accumulator, new_accumulator], location));

            Ok(Some(region))
        },
    )?;
    DropOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            let location = Location::unknown(context);
            let region = Region::new();
            let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;
            let entry = region.append_block(Block::new(&[(value_ty, location)]));

            let accumulator = entry.arg(0)?;
            let inputs_ptr = entry.extract_value(
                context,
                location,
                accumulator,
                llvm::r#type::pointer(context, 0),
                1,
            )?;

            entry.append_operation(ReallocBindingsMeta::free(context, inputs_ptr, location)?);
            entry.append_operation(func::r#return(&[], location));

            Ok(Some(region))
        },
    )?;

    let fields = vec![
        IntegerType::new(context, 64).into(),
        llvm::r#type::pointer(context, 0),
    ];

    Ok(llvm::r#type::r#struct(context, &fields, false))
}

/// Builds the circuit data type.
///
/// ## Layout:
///
/// Holds N_INPUTS elements. Where each element is an u384.
///
/// ```txt
/// type = *u384
/// ```
pub fn build_circuit_data<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    if metadata.get::<ReallocBindingsMeta>().is_none() {
        metadata.insert(ReallocBindingsMeta::new(context, module));
    }

    let Some(GenericArg::Type(circuit_type_id)) = info.info.long_id.generic_args.first() else {
        return Err(SierraAssertError::BadTypeInfo.into());
    };
    let CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(circuit)) =
        registry.get_type(circuit_type_id)?
    else {
        return Err(SierraAssertError::BadTypeInfo.into());
    };

    DupOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            let location = Location::unknown(context);
            let region = Region::new();
            let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;
            let entry = region.append_block(Block::new(&[(value_ty, location)]));

            let data_ptr = entry.arg(0)?;

            let u384_layout = get_integer_layout(384);

            let new_data_ptr = build_array_dup(
                context,
                &entry,
                location,
                data_ptr,
                circuit.circuit_info.n_inputs,
                u384_layout,
            )?;

            entry.append_operation(func::r#return(&[data_ptr, new_data_ptr], location));

            Ok(Some(region))
        },
    )?;
    DropOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            let location = Location::unknown(context);
            let region = Region::new();
            let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;
            let entry = region.append_block(Block::new(&[(value_ty, location)]));

            let data_ptr = entry.arg(0)?;

            entry.append_operation(ReallocBindingsMeta::free(context, data_ptr, location)?);
            entry.append_operation(func::r#return(&[], location));

            Ok(Some(region))
        },
    )?;

    Ok(llvm::r#type::pointer(context, 0))
}

/// Builds the circuit outputs type.
///
/// ## Layout:
///
/// Holds the evaluated circuit output gates and the circuit modulus.
/// - The data is stored as a dynamic array of u384 integers.
/// - The modulus is stored as a u384 in struct form (multi-limb).
///
/// ```txt
/// type = struct {
///     data: *u384,
///     modulus: u384struct,
/// };
///
/// u384struct = struct {
///     limb1: u96,
///     limb2: u96,
///     limb3: u96,
///     limb4: u96,
/// }
/// ```
pub fn build_circuit_outputs<'ctx>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    metadata: &mut MetadataStorage,
    info: WithSelf<InfoOnlyConcreteType>,
) -> Result<Type<'ctx>> {
    if metadata.get::<ReallocBindingsMeta>().is_none() {
        metadata.insert(ReallocBindingsMeta::new(context, module));
    }

    let Some(GenericArg::Type(circuit_type_id)) = info.info.long_id.generic_args.first() else {
        return Err(SierraAssertError::BadTypeInfo.into());
    };
    let CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(circuit)) =
        registry.get_type(circuit_type_id)?
    else {
        return Err(SierraAssertError::BadTypeInfo.into());
    };

    DupOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            let location = Location::unknown(context);
            let region = Region::new();
            let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;
            let entry = region.append_block(Block::new(&[(value_ty, location)]));

            let outputs = entry.arg(0)?;
            let gates_ptr = entry.extract_value(
                context,
                location,
                outputs,
                llvm::r#type::pointer(context, 0),
                0,
            )?;

            let u384_integer_layout = get_integer_layout(384);

            let new_gates_ptr = build_array_dup(
                context,
                &entry,
                location,
                gates_ptr,
                circuit.circuit_info.values.len(),
                u384_integer_layout,
            )?;

            let new_outputs = entry.insert_value(context, location, outputs, new_gates_ptr, 0)?;
            entry.append_operation(func::r#return(&[outputs, new_outputs], location));

            Ok(Some(region))
        },
    )?;
    DropOverridesMeta::register_with(
        context,
        module,
        registry,
        metadata,
        info.self_ty(),
        |metadata| {
            let location = Location::unknown(context);
            let region = Region::new();
            let value_ty = registry.build_type(context, module, metadata, info.self_ty())?;
            let entry = region.append_block(Block::new(&[(value_ty, location)]));

            let outputs = entry.arg(0)?;
            let gates_ptr = entry.extract_value(
                context,
                location,
                outputs,
                llvm::r#type::pointer(context, 0),
                0,
            )?;

            entry.append_operation(ReallocBindingsMeta::free(context, gates_ptr, location)?);
            entry.append_operation(func::r#return(&[], location));

            Ok(Some(region))
        },
    )?;

    Ok(llvm::r#type::r#struct(
        context,
        &[
            llvm::r#type::pointer(context, 0),
            build_u384_struct_type(context),
        ],
        false,
    ))
}

pub fn build_u96_limbs_less_than_guarantee<'ctx>(
    context: &'ctx Context,
    _module: &Module<'ctx>,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _metadata: &mut MetadataStorage,
    info: WithSelf<ConcreteU96LimbsLessThanGuarantee>,
) -> Result<Type<'ctx>> {
    let limbs = info.inner.limb_count;

    let u96_type = IntegerType::new(context, 96).into();
    let limb_struct_type = llvm::r#type::r#struct(context, &vec![u96_type; limbs], false);

    Ok(llvm::r#type::r#struct(
        context,
        &[limb_struct_type, limb_struct_type],
        false,
    ))
}

pub fn build_array_dup<'ctx, 'this>(
    context: &'ctx Context,
    block: &'this Block<'ctx>,
    location: Location<'ctx>,
    ptr: Value<'ctx, 'this>,
    capacity: usize,
    layout: Layout,
) -> Result<Value<'ctx, 'this>> {
    let capacity_bytes = layout_repeat(&layout, capacity)?.0.pad_to_align().size();
    let capacity_bytes_value = block.const_int(context, location, capacity_bytes, 64)?;

    let new_inputs_ptr = {
        let ptr_ty = llvm::r#type::pointer(context, 0);
        let new_inputs_ptr = block.append_op_result(llvm::zero(ptr_ty, location))?;
        block.append_op_result(ReallocBindingsMeta::realloc(
            context,
            new_inputs_ptr,
            capacity_bytes_value,
            location,
        )?)?
    };

    block.memcpy(context, location, ptr, new_inputs_ptr, capacity_bytes_value);

    Ok(new_inputs_ptr)
}

pub const fn is_complex(info: &CircuitTypeConcrete) -> bool {
    match *info {
        CircuitTypeConcrete::AddMod(_)
        | CircuitTypeConcrete::MulMod(_)
        | CircuitTypeConcrete::AddModGate(_)
        | CircuitTypeConcrete::SubModGate(_)
        | CircuitTypeConcrete::MulModGate(_)
        | CircuitTypeConcrete::U96Guarantee(_)
        | CircuitTypeConcrete::InverseGate(_)
        | CircuitTypeConcrete::U96LimbsLessThanGuarantee(_)
        | CircuitTypeConcrete::CircuitModulus(_)
        | CircuitTypeConcrete::CircuitInput(_)
        | CircuitTypeConcrete::Circuit(_)
        | CircuitTypeConcrete::CircuitDescriptor(_)
        | CircuitTypeConcrete::CircuitFailureGuarantee(_) => false,

        CircuitTypeConcrete::CircuitInputAccumulator(_)
        | CircuitTypeConcrete::CircuitPartialOutputs(_)
        | CircuitTypeConcrete::CircuitData(_)
        | CircuitTypeConcrete::CircuitOutputs(_) => true,
    }
}

pub const fn is_zst(info: &CircuitTypeConcrete) -> bool {
    match *info {
        CircuitTypeConcrete::AddModGate(_)
        | CircuitTypeConcrete::SubModGate(_)
        | CircuitTypeConcrete::MulModGate(_)
        | CircuitTypeConcrete::CircuitInput(_)
        | CircuitTypeConcrete::InverseGate(_)
        | CircuitTypeConcrete::U96LimbsLessThanGuarantee(_)
        | CircuitTypeConcrete::Circuit(_)
        | CircuitTypeConcrete::CircuitDescriptor(_)
        | CircuitTypeConcrete::CircuitFailureGuarantee(_) => true,

        CircuitTypeConcrete::AddMod(_)
        | CircuitTypeConcrete::CircuitModulus(_)
        | CircuitTypeConcrete::U96Guarantee(_)
        | CircuitTypeConcrete::MulMod(_)
        | CircuitTypeConcrete::CircuitInputAccumulator(_)
        | CircuitTypeConcrete::CircuitPartialOutputs(_)
        | CircuitTypeConcrete::CircuitData(_)
        | CircuitTypeConcrete::CircuitOutputs(_) => false,
    }
}

pub fn layout(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &CircuitTypeConcrete,
) -> Result<Layout> {
    match info {
        CircuitTypeConcrete::AddMod(_) | CircuitTypeConcrete::MulMod(_) => {
            Ok(get_integer_layout(64))
        }
        CircuitTypeConcrete::CircuitModulus(_) => Ok(get_integer_layout(384)),
        CircuitTypeConcrete::U96Guarantee(_) => Ok(get_integer_layout(96)),

        CircuitTypeConcrete::AddModGate(_)
        | CircuitTypeConcrete::SubModGate(_)
        | CircuitTypeConcrete::MulModGate(_)
        | CircuitTypeConcrete::CircuitInput(_)
        | CircuitTypeConcrete::InverseGate(_)
        | CircuitTypeConcrete::U96LimbsLessThanGuarantee(_)
        | CircuitTypeConcrete::Circuit(_)
        | CircuitTypeConcrete::CircuitDescriptor(_)
        | CircuitTypeConcrete::CircuitFailureGuarantee(_) => Ok(Layout::new::<()>()),

        CircuitTypeConcrete::CircuitData(_) => Ok(Layout::new::<*mut ()>()),
        CircuitTypeConcrete::CircuitOutputs(_) => {
            let u384_struct_layout = layout_repeat(&get_integer_layout(96), 4)?.0;
            let pointer_layout = Layout::new::<*mut ()>();

            Ok(pointer_layout.extend(u384_struct_layout)?.0)
        }
        CircuitTypeConcrete::CircuitInputAccumulator(_) => {
            let integer_layout = get_integer_layout(64);
            let pointer_layout = Layout::new::<*mut ()>();

            Ok(integer_layout.extend(pointer_layout)?.0)
        }
        CircuitTypeConcrete::CircuitPartialOutputs(_) => Ok(Layout::new::<()>()),
    }
}

pub fn build_u384_struct_type(context: &Context) -> Type<'_> {
    llvm::r#type::r#struct(
        context,
        &[
            IntegerType::new(context, 96).into(),
            IntegerType::new(context, 96).into(),
            IntegerType::new(context, 96).into(),
            IntegerType::new(context, 96).into(),
        ],
        false,
    )
}
