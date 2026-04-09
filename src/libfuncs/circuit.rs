//! # Circuit libfuncs
//!
//! Relevant casm code: https://github.com/starkware-libs/cairo/blob/v2.10.0/crates/cairo-lang-sierra-to-casm/src/invocations/circuit.rs

use super::{increment_builtin_counter_by, LibfuncHelper};
use crate::{
    error::{panic::ToNativeAssertError, Result, SierraAssertError},
    execution_result::{ADD_MOD_BUILTIN_SIZE, MUL_MOD_BUILTIN_SIZE, RANGE_CHECK96_BUILTIN_SIZE},
    libfuncs::r#struct::build_struct_value,
    metadata::{
        drop_overrides::DropOverridesMeta,
        realloc_bindings::ReallocBindingsMeta,
        runtime_bindings::{CircuitArithOperationType, RuntimeBindingsMeta},
        MetadataStorage,
    },
    native_panic,
    types::{circuit::build_u384_struct_type, TypeBuilder},
    utils::{get_integer_layout, layout_repeat, ProgramRegistryExt},
};
use cairo_lang_sierra::{
    extensions::{
        circuit::{
            self, CircuitConcreteLibfunc, CircuitTypeConcrete, ConcreteGetOutputLibFunc,
            ConcreteU96LimbsLessThanGuaranteeVerifyLibfunc, MOD_BUILTIN_INSTANCE_SIZE, VALUE_SIZE,
        },
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        lib_func::{SignatureAndTypeConcreteLibfunc, SignatureOnlyConcreteLibfunc},
        ConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{
        arith::{self, CmpiPredicate},
        cf, llvm,
    },
    helpers::{ArithBlockExt, BuiltinBlockExt, GepIndex, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, BlockLike, Location, Type, Value},
    Context,
};
use num_traits::Signed;

/// Select and call the correct libfunc builder function from the selector.
pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &CircuitConcreteLibfunc,
) -> Result<()> {
    match selector {
        CircuitConcreteLibfunc::AddInput(info) => {
            build_add_input(context, registry, entry, location, helper, metadata, info)
        }
        CircuitConcreteLibfunc::Eval(info) => {
            build_eval(context, registry, entry, location, helper, metadata, info)
        }
        CircuitConcreteLibfunc::GetDescriptor(info) => {
            build_get_descriptor(context, registry, entry, location, helper, metadata, info)
        }
        CircuitConcreteLibfunc::InitCircuitData(info) => {
            build_init_circuit_data(context, registry, entry, location, helper, metadata, info)
        }
        CircuitConcreteLibfunc::GetOutput(info) => {
            build_get_output(context, registry, entry, location, helper, metadata, info)
        }
        CircuitConcreteLibfunc::TryIntoCircuitModulus(info) => build_try_into_circuit_modulus(
            context, registry, entry, location, helper, metadata, info,
        ),
        CircuitConcreteLibfunc::FailureGuaranteeVerify(info) => build_failure_guarantee_verify(
            context, registry, entry, location, helper, metadata, info,
        ),
        CircuitConcreteLibfunc::IntoU96Guarantee(info) => {
            build_into_u96_guarantee(context, registry, entry, location, helper, metadata, info)
        }
        CircuitConcreteLibfunc::U96SingleLimbLessThanGuaranteeVerify(info) => {
            build_u96_single_limb_less_than_guarantee_verify(
                context, registry, entry, location, helper, metadata, info,
            )
        }
        CircuitConcreteLibfunc::U96GuaranteeVerify(info) => {
            build_u96_guarantee_verify(context, registry, entry, location, helper, metadata, info)
        }
        CircuitConcreteLibfunc::U96LimbsLessThanGuaranteeVerify(info) => {
            build_u96_limbs_less_than_guarantee_verify(
                context, registry, entry, location, helper, metadata, info,
            )
        }
    }
}

/// Generate MLIR operations for the `init_circuit_data` libfunc.
#[allow(clippy::too_many_arguments)]
fn build_init_circuit_data<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureAndTypeConcreteLibfunc,
) -> Result<()> {
    metadata.get_or_insert_with(|| ReallocBindingsMeta::new(context, helper));

    let circuit_info = match registry.get_type(&info.ty)? {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) => &info.circuit_info,
        _ => return Err(SierraAssertError::BadTypeInfo.into()),
    };

    let rc = increment_builtin_counter_by(
        context,
        entry,
        location,
        entry.arg(0)?,
        circuit_info.rc96_usage(),
    )?;

    // Calculate full capacity for array.
    let capacity = circuit_info.n_inputs;
    let u384_layout = get_integer_layout(384);
    let capacity_bytes = layout_repeat(&u384_layout, capacity)?
        .0
        .pad_to_align()
        .size();
    let capacity_bytes_value = entry.const_int(context, location, capacity_bytes, 64)?;

    // Alloc memory for array.
    let ptr_ty = llvm::r#type::pointer(context, 0);
    let ptr = entry.append_op_result(llvm::zero(ptr_ty, location))?;
    let ptr = entry.append_op_result(ReallocBindingsMeta::realloc(
        context,
        ptr,
        capacity_bytes_value,
        location,
    )?)?;

    // Create accumulator struct.
    let k0 = entry.const_int(context, location, 0, 64)?;
    let accumulator_ty = &info.branch_signatures()[0].vars[1].ty;
    let accumulator = build_struct_value(
        context,
        registry,
        entry,
        location,
        helper,
        metadata,
        accumulator_ty,
        &[k0, ptr],
    )?;

    helper.br(entry, 0, &[rc, accumulator], location)
}

/// Generate MLIR operations for the `add_circuit_input` libfunc.
#[allow(clippy::too_many_arguments)]
fn build_add_input<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &SignatureAndTypeConcreteLibfunc,
) -> Result<()> {
    let n_inputs = match registry.get_type(&info.ty)? {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) => info.circuit_info.n_inputs,
        _ => return Err(SierraAssertError::BadTypeInfo.into()),
    };

    let accumulator: Value = entry.arg(0)?;

    // Get accumulator current length
    let current_length = entry.extract_value(
        context,
        location,
        accumulator,
        IntegerType::new(context, 64).into(),
        0,
    )?;
    // Calculate next length: next_length = current_length + 1
    let k1 = entry.const_int(context, location, 1, 64)?;
    let next_length = entry.addi(current_length, k1, location)?;
    // Insert next_length into accumulator
    let accumulator = entry.insert_value(context, location, accumulator, next_length, 0)?;

    // Get pointer to inputs array
    let inputs_ptr = entry.extract_value(
        context,
        location,
        accumulator,
        llvm::r#type::pointer(context, 0),
        1,
    )?;
    // Get pointer to next input to insert
    let next_input_ptr = entry.gep(
        context,
        location,
        inputs_ptr,
        &[GepIndex::Value(current_length)],
        IntegerType::new(context, 384).into(),
    )?;

    // Interpret u384 struct (input) as u384 integer
    let u384_struct = entry.arg(1)?;
    let new_input = u384_struct_to_integer(context, entry, location, u384_struct)?;
    // Store the u384 into next input pointer
    entry.store(context, location, next_input_ptr, new_input)?;

    // Check if last_insert: next_length == number_of_inputs
    let n_inputs = entry.const_int(context, location, n_inputs, 64)?;
    let last_insert = entry.cmpi(
        context,
        arith::CmpiPredicate::Eq,
        next_length,
        n_inputs,
        location,
    )?;

    let middle_insert_block = helper.append_block(Block::new(&[]));
    let last_insert_block = helper.append_block(Block::new(&[]));
    entry.append_operation(cf::cond_br(
        context,
        last_insert,
        last_insert_block,
        middle_insert_block,
        &[],
        &[],
        location,
    ));

    // If not last insert, then return accumulator
    {
        helper.br(middle_insert_block, 1, &[accumulator], location)?;
    }

    // If is last insert, then return accumulator.pointer
    {
        // Get pointer to inputs array
        let inputs_ptr = last_insert_block.extract_value(
            context,
            location,
            accumulator,
            llvm::r#type::pointer(context, 0),
            1,
        )?;

        helper.br(last_insert_block, 0, &[inputs_ptr], location)?;
    }

    Ok(())
}

/// Generate MLIR operations for the `try_into_circuit_modulus` libfunc.
#[allow(clippy::too_many_arguments)]
fn build_try_into_circuit_modulus<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let modulus = u384_struct_to_integer(context, entry, location, entry.arg(0)?)?;
    let k1 = entry.const_int(context, location, 1, 384)?;

    let is_valid = entry.cmpi(context, arith::CmpiPredicate::Ugt, modulus, k1, location)?;

    helper.cond_br(
        context,
        entry,
        is_valid,
        [0, 1],
        [&[modulus], &[]],
        location,
    )
}

/// Generate MLIR operations for the `get_circuit_descriptor` libfunc.
/// NOOP
#[allow(clippy::too_many_arguments)]
fn build_get_descriptor<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureAndTypeConcreteLibfunc,
) -> Result<()> {
    let descriptor_type_id = &info.branch_signatures()[0].vars[0].ty;
    let descriptor_type = registry.build_type(context, helper, metadata, descriptor_type_id)?;

    let unit = entry.append_op_result(llvm::undef(descriptor_type, location))?;

    helper.br(entry, 0, &[unit], location)
}

/// Generate MLIR operations for the `eval_circuit` libfunc.
#[allow(clippy::too_many_arguments)]
fn build_eval<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureAndTypeConcreteLibfunc,
) -> Result<()> {
    metadata.get_or_insert_with(|| ReallocBindingsMeta::new(context, helper));

    let circuit_info = match registry.get_type(&info.ty)? {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) => &info.circuit_info,
        _ => return Err(SierraAssertError::BadTypeInfo.into()),
    };
    let add_mod = entry.arg(0)?;
    let mul_mod = entry.arg(1)?;
    let circuit_data = entry.arg(3)?;
    let circuit_modulus = entry.arg(4)?;

    // Arguments 5 and 6 are used to build the gate 0 (with constant value 1).
    // let zero = entry.argument(5)?;
    // let one = entry.argument(6)?;

    // Always increase the add mod builtin pointer, regardless of the evaluation result.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/circuit.rs?plain=1#L257
    let add_mod = increment_builtin_counter_by(
        context,
        entry,
        location,
        add_mod,
        circuit_info.add_offsets.len() * ADD_MOD_BUILTIN_SIZE,
    )?;

    let ([ok_block, err_block], gates) = build_gate_evaluation(
        context,
        entry,
        location,
        helper,
        metadata,
        circuit_info,
        circuit_data,
        circuit_modulus,
    )?;

    // Ok case
    {
        // We drop circuit_data, as its consumed by this libfunc.
        DropOverridesMeta::invoke_override(
            context,
            registry,
            helper,
            helper.init_block(),
            ok_block,
            location,
            metadata,
            &info.signature.param_signatures[3].ty,
            circuit_data,
        )?;

        // Increase the mul mod builtin pointer by the number of evaluated gates.
        // If the evaluation succedes, then we assume that every gate was evaluated.
        // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/circuit.rs?plain=1#L261
        let mul_mod = increment_builtin_counter_by(
            context,
            ok_block,
            location,
            mul_mod,
            circuit_info.mul_offsets.len() * MUL_MOD_BUILTIN_SIZE,
        )?;

        // Calculate capacity for array.
        let outputs_capacity = circuit_info.values.len();
        let u384_integer_layout = get_integer_layout(384);
        let outputs_layout = layout_repeat(&u384_integer_layout, outputs_capacity)?.0;
        let outputs_capacity_bytes = outputs_layout.pad_to_align().size();
        let outputs_capacity_bytes_value =
            ok_block.const_int(context, location, outputs_capacity_bytes, 64)?;

        // Alloc memory for array.
        let ptr_ty = llvm::r#type::pointer(context, 0);
        let outputs_ptr = ok_block.append_op_result(llvm::zero(ptr_ty, location))?;
        let outputs_ptr = ok_block.append_op_result(ReallocBindingsMeta::realloc(
            context,
            outputs_ptr,
            outputs_capacity_bytes_value,
            location,
        )?)?;

        // Insert evaluated gates into the array.
        for (i, gate) in gates.into_iter().enumerate() {
            let value_ptr = ok_block.gep(
                context,
                location,
                outputs_ptr,
                &[GepIndex::Const(i as i32)],
                IntegerType::new(context, 384).into(),
            )?;
            ok_block.store(context, location, value_ptr, gate)?;
        }

        let modulus_struct = u384_integer_to_struct(context, ok_block, location, circuit_modulus)?;

        // Build output struct
        let outputs_type_id = &info.branch_signatures()[0].vars[2].ty;
        let outputs = build_struct_value(
            context,
            registry,
            ok_block,
            location,
            helper,
            metadata,
            outputs_type_id,
            &[outputs_ptr, modulus_struct],
        )?;

        helper.br(ok_block, 0, &[add_mod, mul_mod, outputs], location)?;
    }

    // Error case
    {
        // We drop circuit_data, as its consumed by this libfunc.
        DropOverridesMeta::invoke_override(
            context,
            registry,
            helper,
            helper.init_block(),
            err_block,
            location,
            metadata,
            &info.signature.param_signatures[3].ty,
            circuit_data,
        )?;

        // We only consider mul gates evaluated before failure
        // Increase the mul mod builtin pointer by the number of evaluated gates.
        // As the evaluation failed, we read the number of evaluated gates from
        // the first argument of the error block.
        // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/circuit.rs?plain=1#L261
        let mul_mod = {
            let mul_mod_usage = err_block.muli(
                err_block.arg(0)?,
                err_block.const_int(context, location, MUL_MOD_BUILTIN_SIZE, 64)?,
                location,
            )?;
            err_block.addi(mul_mod, mul_mod_usage, location)
        }?;

        let partial_type_id = &info.branch_signatures()[1].vars[2].ty;
        let partial = err_block.append_op_result(llvm::undef(
            registry.build_type(context, helper, metadata, partial_type_id)?,
            location,
        ))?;
        let failure_type_id = &info.branch_signatures()[1].vars[3].ty;
        let failure = err_block.append_op_result(llvm::undef(
            registry.build_type(context, helper, metadata, failure_type_id)?,
            location,
        ))?;
        helper.br(
            err_block,
            1,
            &[add_mod, mul_mod, partial, failure],
            location,
        )?;
    }

    Ok(())
}

/// Receives the circuit inputs, and builds the evaluation of the full circuit.
///
/// Returns two branches. The success block and the error block respectively.
/// - The success block receives nothing.
/// - The error block receives:
///   - The index of the first gate that could not be computed.
///
/// The evaluated gates are returned separately, as a vector of `MLIR` values.
/// Note that in the case of error, not all MLIR values are guaranteed to have been computed,
/// and should not be used carelessly.
///
/// TODO: Consider returning the evaluated gates through the block directly:
/// - As a pointer to a heap allocated array of gates.
/// - As a llvm struct/array of evaluted gates (its size could get really big).
/// - As arguments to the block (one argument per block).
///
/// The original Cairo hint evaluates all gates, even in case of failure.
/// This implementation exits on first error, as there is no need for the partial outputs yet.
#[allow(clippy::too_many_arguments)]
fn build_gate_evaluation<'ctx, 'this>(
    context: &'this Context,
    mut block: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    circuit_info: &circuit::CircuitInfo,
    circuit_data: Value<'ctx, 'ctx>,
    circuit_modulus: Value<'ctx, 'ctx>,
) -> Result<([&'this Block<'ctx>; 2], Vec<Value<'ctx, 'ctx>>)> {
    let runtime_bindings_meta = metadata
        .get_mut::<RuntimeBindingsMeta>()
        .to_native_assert_error("Unable to get the RuntimeBindingsMeta from MetadataStorage")?;

    // Each gate is represented as a MLIR value, and identified by an offset in the gate vector.
    // - `None` implies that the gate value *has not* been compiled yet.
    // - `Some` implies that the gate values *has* already been compiled, and therefore can be safely used.
    // Initially, some gate values are already known.
    let mut gates = vec![None; 1 + circuit_info.n_inputs + circuit_info.values.len()];

    // The first gate always has a value of 1. It is implicity referred by some gate offsets.
    gates[0] = Some(block.const_int(context, location, 1, 384)?);

    let u384_type = IntegerType::new(context, 384).into();

    // The input gates are also known at the start. We take them from the `circuit_data` array.
    for i in 0..circuit_info.n_inputs {
        let value_ptr = block.gep(
            context,
            location,
            circuit_data,
            &[GepIndex::Const(i as i32)],
            u384_type,
        )?;
        gates[i + 1] = Some(block.load(context, location, value_ptr, u384_type)?);
    }

    let err_block = helper.append_block(Block::new(&[(
        IntegerType::new(context, 64).into(),
        location,
    )]));
    let ok_block = helper.append_block(Block::new(&[]));

    let mut add_offsets = circuit_info.add_offsets.iter().peekable();
    let mut mul_offsets = circuit_info.mul_offsets.iter().enumerate();

    // We loop until all gates have been solved
    loop {
        // We iterate the add gate offsets as long as we can
        while let Some(&gate_offset) = add_offsets.peek() {
            let lhs_value = gates[gate_offset.lhs].to_owned();
            let rhs_value = gates[gate_offset.rhs].to_owned();
            let output_value = gates[gate_offset.output].to_owned();

            // Depending on the values known at the time, we can deduce if we are dealing with an ADD gate or a SUB gate.
            match (lhs_value, rhs_value, output_value) {
                // ADD: lhs + rhs = out
                (Some(lhs_value), Some(rhs_value), None) => {
                    let value = runtime_bindings_meta.circuit_arith_operation(
                        context,
                        helper.module,
                        block,
                        location,
                        CircuitArithOperationType::Add,
                        lhs_value,
                        rhs_value,
                        circuit_modulus,
                    )?;
                    gates[gate_offset.output] = Some(value);
                }
                // SUB: lhs = out - rhs
                (None, Some(rhs_value), Some(output_value)) => {
                    let value = runtime_bindings_meta.circuit_arith_operation(
                        context,
                        helper.module,
                        block,
                        location,
                        CircuitArithOperationType::Sub,
                        output_value,
                        rhs_value,
                        circuit_modulus,
                    )?;
                    gates[gate_offset.lhs] = Some(value);
                }
                // We can't solve this add gate yet, so we break from the loop
                _ => break,
            }

            add_offsets.next();
        }

        // If we can't advance any more with add gate offsets, then we solve the next mul gate offset and go back to the start of the loop (solving add gate offsets).
        if let Some((gate_offset_idx, gate_offset)) = mul_offsets.next() {
            let lhs_value = gates[gate_offset.lhs].to_owned();
            let rhs_value = gates[gate_offset.rhs].to_owned();
            let output_value = gates[gate_offset.output].to_owned();

            // Depending on the values known at the time, we can deduce if we are dealing with an MUL gate or a INV gate.
            match (lhs_value, rhs_value, output_value) {
                // MUL: lhs * rhs = out
                (Some(lhs_value), Some(rhs_value), None) => {
                    let value = runtime_bindings_meta.circuit_arith_operation(
                        context,
                        helper.module,
                        block,
                        location,
                        CircuitArithOperationType::Mul,
                        lhs_value,
                        rhs_value,
                        circuit_modulus,
                    )?;
                    gates[gate_offset.output] = Some(value)
                }
                // INV: lhs = 1 / rhs
                (None, Some(rhs_value), Some(_)) => {
                    // Apply egcd to find gcd and inverse
                    let euclidean_result = runtime_bindings_meta
                        .u384_extended_euclidean_algorithm(
                            context,
                            helper.module,
                            block,
                            location,
                            rhs_value,
                            circuit_modulus,
                        )?;
                    // Extract the values from the result struct
                    let gcd =
                        block.extract_value(context, location, euclidean_result, u384_type, 0)?;
                    let inverse =
                        block.extract_value(context, location, euclidean_result, u384_type, 1)?;

                    // if the gcd is not 1, then fail (a and b are not coprimes)
                    let one = block.const_int_from_type(context, location, 1, u384_type)?;
                    let gate_offset_idx_value = block.const_int_from_type(
                        context,
                        location,
                        gate_offset_idx,
                        IntegerType::new(context, 64).into(),
                    )?;
                    let has_inverse = block.cmpi(context, CmpiPredicate::Eq, gcd, one, location)?;
                    let has_inverse_block = helper.append_block(Block::new(&[]));
                    block.append_operation(cf::cond_br(
                        context,
                        has_inverse,
                        has_inverse_block,
                        err_block,
                        &[],
                        &[gate_offset_idx_value],
                        location,
                    ));
                    block = has_inverse_block;

                    gates[gate_offset.lhs] = Some(inverse);
                }
                // The imposibility to solve this mul gate offset would render the circuit unsolvable
                _ => return Err(SierraAssertError::ImpossibleCircuit.into()),
            }
        } else {
            // If there are no mul gate offsets left, then we have the finished evaluation.
            break;
        }
    }

    block.append_operation(cf::br(ok_block, &[], location));

    // Validate all values have been calculated
    // Should only fail if the circuit is not solvable (bad form)
    let evaluated_gates = gates
        .into_iter()
        .skip(1 + circuit_info.n_inputs)
        .collect::<Option<Vec<Value>>>()
        .ok_or(SierraAssertError::ImpossibleCircuit)?;

    Ok(([ok_block, err_block], evaluated_gates))
}

/// Generate MLIR operations for the `circuit_failure_guarantee_verify` libfunc.
/// NOOP
#[allow(clippy::too_many_arguments)]
fn build_failure_guarantee_verify<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let rc = entry.arg(0)?;
    let mul_mod = entry.arg(1)?;
    let rc = increment_builtin_counter_by(context, entry, location, rc, 2 + VALUE_SIZE)?;

    let mul_mod =
        increment_builtin_counter_by(context, entry, location, mul_mod, MOD_BUILTIN_INSTANCE_SIZE)?;

    let guarantee_type_id = &info.branch_signatures()[0].vars[2].ty;
    let guarantee_type = registry.build_type(context, helper, metadata, guarantee_type_id)?;

    let guarantee = entry.append_op_result(llvm::undef(guarantee_type, location))?;

    helper.br(entry, 0, &[rc, mul_mod, guarantee], location)
}

/// Generate MLIR operations for the `u96_limbs_less_than_guarantee_verify` libfunc.
#[allow(clippy::too_many_arguments)]
fn build_u96_limbs_less_than_guarantee_verify<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &ConcreteU96LimbsLessThanGuaranteeVerifyLibfunc,
) -> Result<()> {
    let guarantee = entry.arg(0)?;
    let limb_count = info.limb_count;

    let u96_type = IntegerType::new(context, 96).into();
    let limb_struct_type = llvm::r#type::r#struct(context, &vec![u96_type; limb_count], false);

    // extract gate and modulus from input value
    let gate = entry.extract_value(context, location, guarantee, limb_struct_type, 0)?;
    let modulus = entry.extract_value(context, location, guarantee, limb_struct_type, 1)?;

    // extract last limb from gate and modulus
    let gate_last_limb = entry.extract_value(context, location, gate, u96_type, limb_count - 1)?;
    let modulus_last_limb =
        entry.extract_value(context, location, modulus, u96_type, limb_count - 1)?;

    // calcualte diff between limbs
    let diff = entry.append_op_result(arith::subi(modulus_last_limb, gate_last_limb, location))?;
    let k0 = entry.const_int_from_type(context, location, 0, u96_type)?;
    let has_diff = entry.cmpi(context, CmpiPredicate::Ne, diff, k0, location)?;

    let diff_block = helper.append_block(Block::new(&[]));
    let next_block = helper.append_block(Block::new(&[]));
    entry.append_operation(cf::cond_br(
        context,
        has_diff,
        diff_block,
        next_block,
        &[],
        &[],
        location,
    ));

    {
        // if there is diff, return it
        helper.br(diff_block, 1, &[diff], location)?;
    }
    {
        // if there is no diff, build a new guarantee, skipping last limb
        let new_limb_struct_type =
            llvm::r#type::r#struct(context, &vec![u96_type; limb_count - 1], false);
        let new_gate = build_array_slice(
            context,
            next_block,
            location,
            gate,
            u96_type,
            new_limb_struct_type,
            0,
            limb_count - 1,
        )?;
        let new_modulus = build_array_slice(
            context,
            next_block,
            location,
            modulus,
            u96_type,
            new_limb_struct_type,
            0,
            limb_count - 1,
        )?;

        let guarantee_type_id = &info.branch_signatures()[0].vars[0].ty;
        let new_guarantee = build_struct_value(
            context,
            registry,
            next_block,
            location,
            helper,
            metadata,
            guarantee_type_id,
            &[new_gate, new_modulus],
        )?;

        helper.br(next_block, 0, &[new_guarantee], location)?;
    }

    Ok(())
}

fn build_u96_single_limb_less_than_guarantee_verify<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let guarantee = entry.arg(0)?;

    let u96_type = IntegerType::new(context, 96).into();
    // this libfunc will always receive gate and modulus with single limb
    let limb_struct_type = llvm::r#type::r#struct(context, &[u96_type; 1], false);

    // extract gate and modulus from input value
    let gate = entry.extract_value(context, location, guarantee, limb_struct_type, 0)?;
    let modulus = entry.extract_value(context, location, guarantee, limb_struct_type, 1)?;

    // extract the only limb from gate and modulus
    let gate_limb = entry.extract_value(context, location, gate, u96_type, 0)?;
    let modulus_limb = entry.extract_value(context, location, modulus, u96_type, 0)?;

    // calcualte diff between limbs
    let diff = entry.append_op_result(arith::subi(modulus_limb, gate_limb, location))?;

    helper.br(entry, 0, &[diff], location)
}

/// Generate MLIR operations for the `get_circuit_output` libfunc.
#[allow(clippy::too_many_arguments)]
fn build_get_output<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &ConcreteGetOutputLibFunc,
) -> Result<()> {
    let circuit_info = match registry.get_type(&info.circuit_ty)? {
        CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) => &info.circuit_info,
        _ => return Err(SierraAssertError::BadTypeInfo.into()),
    };
    let output_type_id = &info.output_ty;

    let u384_type = IntegerType::new(context, 384).into();

    let output_offset_idx = *circuit_info
        .values
        .get(output_type_id)
        .ok_or(SierraAssertError::BadTypeInfo)?;

    let output_idx = output_offset_idx - circuit_info.n_inputs - 1;

    let outputs = entry.arg(0)?;

    let circuit_ptr = entry.extract_value(
        context,
        location,
        outputs,
        llvm::r#type::pointer(context, 0),
        0,
    )?;
    let modulus_struct = entry.extract_value(
        context,
        location,
        outputs,
        build_u384_struct_type(context),
        1,
    )?;

    let output_integer_ptr = entry.gep(
        context,
        location,
        circuit_ptr,
        &[GepIndex::Const(output_idx as i32)],
        u384_type,
    )?;
    let output_integer = entry.load(context, location, output_integer_ptr, u384_type)?;
    let output_struct = u384_integer_to_struct(context, entry, location, output_integer)?;

    let guarantee_type_id = &info.branch_signatures()[0].vars[1].ty;
    let guarantee = build_struct_value(
        context,
        registry,
        entry,
        location,
        helper,
        metadata,
        guarantee_type_id,
        &[output_struct, modulus_struct],
    )?;

    // We drop the circuit outputs value, as its consumed by this libfunc.
    // NOTE: As this libfunc consumes circuit_outputs, this implies that
    // calling it multiple times involves duplicating the circuit outputs
    // each time. This could be fixed by implementing a reference counter,
    // like we do with regular arrays.
    DropOverridesMeta::invoke_override(
        context,
        registry,
        helper,
        helper.init_block(),
        entry,
        location,
        metadata,
        &info.signature.param_signatures[0].ty,
        outputs,
    )?;

    helper.br(entry, 0, &[output_struct, guarantee], location)?;

    Ok(())
}

fn u384_struct_to_integer<'a>(
    context: &'a Context,
    block: &'a Block<'a>,
    location: Location<'a>,
    u384_struct: Value<'a, 'a>,
) -> Result<Value<'a, 'a>> {
    let u96_type = IntegerType::new(context, 96).into();

    let limb1 = block.extui(
        block.extract_value(context, location, u384_struct, u96_type, 0)?,
        IntegerType::new(context, 384).into(),
        location,
    )?;

    let limb2 = {
        let limb = block.extui(
            block.extract_value(context, location, u384_struct, u96_type, 1)?,
            IntegerType::new(context, 384).into(),
            location,
        )?;
        let k96 = block.const_int(context, location, 96, 384)?;
        block.shli(limb, k96, location)?
    };

    let limb3 = {
        let limb = block.extui(
            block.extract_value(context, location, u384_struct, u96_type, 2)?,
            IntegerType::new(context, 384).into(),
            location,
        )?;
        let k192 = block.const_int(context, location, 96 * 2, 384)?;
        block.shli(limb, k192, location)?
    };

    let limb4 = {
        let limb = block.extui(
            block.extract_value(context, location, u384_struct, u96_type, 3)?,
            IntegerType::new(context, 384).into(),
            location,
        )?;
        let k288 = block.const_int(context, location, 96 * 3, 384)?;
        block.shli(limb, k288, location)?
    };

    let value = block.append_op_result(arith::ori(limb1, limb2, location))?;
    let value = block.append_op_result(arith::ori(value, limb3, location))?;
    let value = block.append_op_result(arith::ori(value, limb4, location))?;

    Ok(value)
}

fn u384_integer_to_struct<'a>(
    context: &'a Context,
    block: &'a Block<'a>,
    location: Location<'a>,
    integer: Value<'a, 'a>,
) -> Result<Value<'a, 'a>> {
    let u96_type = IntegerType::new(context, 96).into();

    let limb1 = block.trunci(integer, IntegerType::new(context, 96).into(), location)?;
    let limb2 = {
        let k96 = block.const_int(context, location, 96, 384)?;
        let limb = block.shrui(integer, k96, location)?;
        block.trunci(limb, u96_type, location)?
    };
    let limb3 = {
        let k192 = block.const_int(context, location, 96 * 2, 384)?;
        let limb = block.shrui(integer, k192, location)?;
        block.trunci(limb, u96_type, location)?
    };
    let limb4 = {
        let k288 = block.const_int(context, location, 96 * 3, 384)?;
        let limb = block.shrui(integer, k288, location)?;
        block.trunci(limb, u96_type, location)?
    };

    let struct_type = build_u384_struct_type(context);
    let struct_value = block.append_op_result(llvm::undef(struct_type, location))?;

    Ok(block.insert_values(
        context,
        location,
        struct_value,
        &[limb1, limb2, limb3, limb4],
    )?)
}

/// Extracts values from indexes `from` - `to` (exclusive) and builds a new value of type `result_type`
///
/// Can be used with arrays, or structs with multiple elements of a single type.
#[allow(clippy::too_many_arguments)]
fn build_array_slice<'ctx>(
    context: &'ctx Context,
    block: &'ctx Block<'ctx>,
    location: Location<'ctx>,
    aggregate: Value<'ctx, 'ctx>,
    element_type: Type<'ctx>,
    result_type: Type<'ctx>,
    from: usize,
    to: usize,
) -> Result<Value<'ctx, 'ctx>> {
    let mut values = Vec::with_capacity(to - from);

    for i in from..to {
        let value = block.extract_value(context, location, aggregate, element_type, i)?;
        values.push(value);
    }

    Ok(block.insert_values(
        context,
        location,
        block.append_op_result(llvm::undef(result_type, location))?,
        &values,
    )?)
}

/// Converts input to an U96Guarantee.
/// Input type must fit inside of an u96.
///
/// # Signature
/// ```cairo
/// extern fn into_u96_guarantee<T>(val: T) -> U96Guarantee nopanic;
/// ```
fn build_into_u96_guarantee<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    info: &SignatureAndTypeConcreteLibfunc,
) -> Result<()> {
    let src = entry.argument(0)?.into();

    let src_ty = registry.get_type(&info.param_signatures()[0].ty)?;

    let src_range = src_ty.integer_range(registry)?;

    // We expect the input value to be unsigned, but we check it just in case.
    if src_range.lower.is_negative() {
        native_panic!("into_u96_guarantee expects an unsigned integer")
    }

    // Extend the input value to an u96
    let mut dst = entry.extui(src, IntegerType::new(context, 96).into(), location)?;

    // If the lower bound is positive, we offset the value by the lower bound
    // to obtain the actual value.
    if src_range.lower.is_positive() {
        let klower = entry.const_int_from_type(
            context,
            location,
            src_range.lower,
            IntegerType::new(context, 96).into(),
        )?;
        dst = entry.addi(dst, klower, location)?
    }

    helper.br(entry, 0, &[dst], location)
}

/// Verifies an U96Guarantee
///
/// # Signature
/// ```cairo
/// extern fn u96_guarantee_verify(guarantee: U96Guarantee) implicits(RangeCheck96) nopanic;
/// ```
///
/// This is actually a noop in Cairo Native.
/// We should only increase the builtin counter.
///
/// The implementation is adapted from the [sierra-to-casm compiler](https://github.com/starkware-libs/cairo/blob/dc8b4f0b2e189a3b107b15062895597588b78a46/crates/cairo-lang-sierra-to-casm/src/invocations/circuit.rs?plain=1#L523).
fn build_u96_guarantee_verify<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    // We increase the range_check96 builtin by 1 usage
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/circuit.rs?plain=1#L534
    let range_check96 = increment_builtin_counter_by(
        context,
        entry,
        location,
        entry.arg(0)?,
        RANGE_CHECK96_BUILTIN_SIZE,
    )?;

    helper.br(entry, 0, &[range_check96], location)
}

#[cfg(test)]
mod test {
    use crate::{
        jit_enum, jit_panic, jit_struct,
        utils::{
            felt252_str,
            testing::{get_compiled_program, run_program_assert_output},
        },
        values::Value,
    };
    use cairo_lang_sierra::extensions::utils::Range;
    use num_bigint::{BigInt, BigUint};
    use num_traits::{Num, One};
    use starknet_types_core::felt::Felt;

    fn u384(limbs: [&'static str; 4]) -> Value {
        fn u96_range() -> Range {
            Range {
                lower: BigUint::from_str_radix("0", 16).unwrap().into(),
                upper: BigUint::from_str_radix("79228162514264337593543950336", 10)
                    .unwrap()
                    .into(),
            }
        }

        Value::Struct {
            fields: vec![
                Value::BoundedInt {
                    value: Felt::from_hex_unwrap(limbs[0]),
                    range: u96_range(),
                },
                Value::BoundedInt {
                    value: Felt::from_hex_unwrap(limbs[1]),
                    range: u96_range(),
                },
                Value::BoundedInt {
                    value: Felt::from_hex_unwrap(limbs[2]),
                    range: u96_range(),
                },
                Value::BoundedInt {
                    value: Felt::from_hex_unwrap(limbs[3]),
                    range: u96_range(),
                },
            ],
            debug_name: None,
        }
    }

    #[test]
    fn run_add_circuit() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/circuit_add");

        run_program_assert_output(
            &program,
            "main",
            &[],
            jit_enum!(0, jit_struct!(u384(["0x9", "0x9", "0x9", "0x9"]))),
        );
    }

    #[test]
    fn run_sub_circuit() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/circuit_sub");

        run_program_assert_output(
            &program,
            "main",
            &[],
            jit_enum!(0, jit_struct!(u384(["0x3", "0x3", "0x3", "0x3"]))),
        );
    }

    #[test]
    fn run_mul_circuit() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/circuit_mul");

        run_program_assert_output(
            &program,
            "main",
            &[],
            jit_enum!(0, jit_struct!(u384(["0x9", "0x9", "0x9", "0x9"]))),
        );
    }

    #[test]
    fn run_inverse_circuit() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/circuit_inverse");

        run_program_assert_output(
            &program,
            "main",
            &[],
            jit_enum!(0, jit_struct!(u384(["0x6", "0x0", "0x0", "0x0"]))),
        );
    }

    #[test]
    fn run_no_coprime_circuit() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/circuit_no_coprime");

        run_program_assert_output(
            &program,
            "main",
            &[],
            jit_panic!(felt252_str(
                "30828113188794245257250221355944970489240709081949230"
            )),
        );
    }

    #[test]
    fn run_mul_overflow_circuit() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/circuit_mul_overflow");

        run_program_assert_output(
            &program,
            "main",
            &[],
            jit_enum!(
                0,
                jit_struct!(u384(["0xf", "0x0", "0x0", "0xfffffffffffffffffffffff0"]))
            ),
        );
    }

    #[test]
    fn run_full_circuit() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/circuit_full");

        run_program_assert_output(
            &program,
            "main",
            &[],
            jit_enum!(
                0,
                jit_struct!(u384([
                    "0x76956587ccb74125e760fdf3",
                    "0xe8c82ede90011c6adc4b5cfa",
                    "0xaf4bed7eef975ff1941fdf3d",
                    "0x7"
                ]))
            ),
        );
    }

    #[test]
    fn run_into_u96_guarantee() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/circuit_u96_guarantee");

        let range = Range {
            lower: BigInt::ZERO,
            upper: BigInt::one() << 96,
        };

        run_program_assert_output(
            &program,
            "main",
            &[],
            jit_struct!(
                Value::BoundedInt {
                    value: 123.into(),
                    range: range.clone()
                },
                Value::BoundedInt {
                    value: 123.into(),
                    range: range.clone()
                },
                Value::BoundedInt {
                    value: 123.into(),
                    range: range.clone()
                }
            ),
        );
    }
}
