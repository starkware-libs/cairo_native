//! # Const libfuncs

use super::LibfuncHelper;
use crate::{
    error::{Error, Result},
    libfuncs::{r#box::into_box, r#enum::build_enum_value, r#struct::build_struct_value},
    metadata::MetadataStorage,
    native_panic,
    types::TypeBuilder,
    utils::{felt_to_unsigned, ProgramRegistryExt, RangeExt},
};
use cairo_lang_sierra::{
    extensions::{
        bounded_int::BoundedIntConcreteType,
        const_type::{
            ConstAsBoxConcreteLibfunc, ConstAsImmediateConcreteLibfunc, ConstConcreteLibfunc,
            ConstConcreteType,
        },
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        starknet::StarknetTypeConcrete,
    },
    program::GenericArg,
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::llvm::{self},
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, Location, Value},
    Context,
};

/// Select and call the correct libfunc builder function from the selector.
pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &ConstConcreteLibfunc,
) -> Result<()> {
    match selector {
        ConstConcreteLibfunc::AsBox(info) => {
            build_const_as_box(context, registry, entry, location, helper, metadata, info)
        }
        ConstConcreteLibfunc::AsImmediate(info) => {
            build_const_as_immediate(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `const_as_box` libfunc.
pub fn build_const_as_box<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &ConstAsBoxConcreteLibfunc,
) -> Result<()> {
    let const_type_outer = registry.get_type(&info.const_type)?;

    // Create constant
    let const_type = match &const_type_outer {
        CoreTypeConcrete::Const(inner) => inner,
        _ => native_panic!("matched an unexpected CoreTypeConcrete that is not a Const"),
    };

    let value = build_const_type_value(
        context, registry, entry, location, helper, metadata, const_type,
    )?;

    let const_ty = registry.get_type(&const_type.inner_ty)?;
    let inner_layout = const_ty.layout(registry)?;

    let ptr = into_box(
        context,
        helper.module,
        entry,
        location,
        value,
        inner_layout,
        metadata,
    )?;

    helper.br(entry, 0, &[ptr], location)
}

/// Generate MLIR operations for the `const_as_immediate` libfunc.
pub fn build_const_as_immediate<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &ConstAsImmediateConcreteLibfunc,
) -> Result<()> {
    let const_ty = registry.get_type(&info.const_type)?;

    let const_type = match &const_ty {
        CoreTypeConcrete::Const(inner) => inner,
        _ => native_panic!("matched an unexpected CoreTypeConcrete that is not a Const"),
    };

    let value = build_const_type_value(
        context, registry, entry, location, helper, metadata, const_type,
    )?;

    helper.br(entry, 0, &[value], location)
}

pub fn build_const_type_value<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &ConstConcreteType,
) -> Result<Value<'ctx, 'this>> {
    // const_type.inner_data Should be one of the following:
    // - A single value, if the inner type is a simple numeric type (e.g., `felt252`, `u32`,
    //   etc.).
    // - A list of const types, if the inner type is a struct. The type of each const type must be
    //   the same as the corresponding struct member type.
    // - A selector (a single value) followed by a const type, if the inner type is an enum. The
    //   type of the const type must be the same as the corresponding enum variant type.

    let inner_type = registry.get_type(&info.inner_ty)?;
    let inner_ty = registry.build_type(context, helper, metadata, &info.inner_ty)?;

    match inner_type {
        CoreTypeConcrete::Struct(_) => {
            let mut fields = Vec::new();

            for field in &info.inner_data {
                match field {
                    GenericArg::Type(const_field_ty) => {
                        let field_type = registry.get_type(const_field_ty)?;

                        let const_field_type = match &field_type {
                            CoreTypeConcrete::Const(inner) => inner,
                            _ => native_panic!(
                                "matched an unexpected CoreTypeConcrete that is not a Const"
                            ),
                        };

                        let field_value = build_const_type_value(
                            context,
                            registry,
                            entry,
                            location,
                            helper,
                            metadata,
                            const_field_type,
                        )?;
                        fields.push(field_value);
                    }
                    _ => return Err(Error::ConstDataMismatch),
                }
            }

            build_struct_value(
                context,
                registry,
                entry,
                location,
                helper,
                metadata,
                &info.inner_ty,
                &fields,
            )
        }
        CoreTypeConcrete::Enum(_enum_info) => match &info.inner_data[..] {
            [GenericArg::Value(variant_index), GenericArg::Type(payload_ty)] => {
                let payload_type = registry.get_type(payload_ty)?;
                let const_payload_type = match payload_type {
                    CoreTypeConcrete::Const(inner) => inner,
                    _ => {
                        native_panic!("matched an unexpected CoreTypeConcrete that is not a Const")
                    }
                };

                let payload_value = build_const_type_value(
                    context,
                    registry,
                    entry,
                    location,
                    helper,
                    metadata,
                    const_payload_type,
                )?;

                build_enum_value(
                    context,
                    registry,
                    entry,
                    location,
                    helper,
                    metadata,
                    payload_value,
                    &info.inner_ty,
                    payload_ty,
                    variant_index
                        .try_into()
                        .map_err(|_| Error::IntegerConversion)?,
                )
            }
            _ => Err(Error::ConstDataMismatch),
        },
        CoreTypeConcrete::NonZero(_) => match &info.inner_data[..] {
            // Copied from the sierra to casm lowering
            // NonZero is the same type as the inner type in native.
            [GenericArg::Type(inner)] => {
                let inner_type = registry.get_type(inner)?;
                let const_inner_type = match inner_type {
                    CoreTypeConcrete::Const(inner) => inner,
                    _ => native_panic!("unreachable: unexpected CoreTypeConcrete found"),
                };

                build_const_type_value(
                    context,
                    registry,
                    entry,
                    location,
                    helper,
                    metadata,
                    const_inner_type,
                )
            }
            _ => Err(Error::ConstDataMismatch),
        },
        CoreTypeConcrete::BoundedInt(BoundedIntConcreteType { range, .. }) => {
            let value = match &info.inner_data.as_slice() {
                [GenericArg::Value(value)] => value.clone(),
                _ => return Err(Error::ConstDataMismatch),
            };

            // Offset the value so that 0 matches with lower.
            let value = &value - &range.lower;

            Ok(entry.const_int(
                context,
                location,
                value,
                inner_type.integer_range(registry)?.repr_bit_width(),
            )?)
        }
        CoreTypeConcrete::Felt252(_)
        | CoreTypeConcrete::Starknet(
            StarknetTypeConcrete::ClassHash(_) | StarknetTypeConcrete::ContractAddress(_),
        ) => match &info.inner_data[..] {
            [GenericArg::Value(value)] => Ok(entry.const_int_from_type(
                context,
                location,
                felt_to_unsigned(value),
                inner_ty,
            )?),
            _ => Err(Error::ConstDataMismatch),
        },
        CoreTypeConcrete::EcPoint(_) => match &info.inner_data[..] {
            [GenericArg::Value(x), GenericArg::Value(y)] => {
                let felt252_ty = IntegerType::new(context, 252).into();

                let x = entry.const_int(context, location, felt_to_unsigned(x), 252)?;
                let y = entry.const_int(context, location, felt_to_unsigned(y), 252)?;

                let ec_point_ty = llvm::r#type::r#struct(context, &[felt252_ty, felt252_ty], false);
                let value = entry.append_op_result(llvm::undef(ec_point_ty, location))?;
                let value = entry.insert_value(context, location, value, x, 0)?;
                let value = entry.insert_value(context, location, value, y, 1)?;
                Ok(value)
            }
            _ => Err(Error::ConstDataMismatch),
        },
        CoreTypeConcrete::Uint8(_)
        | CoreTypeConcrete::Uint16(_)
        | CoreTypeConcrete::Uint32(_)
        | CoreTypeConcrete::Uint64(_)
        | CoreTypeConcrete::Uint128(_)
        | CoreTypeConcrete::Sint8(_)
        | CoreTypeConcrete::Sint16(_)
        | CoreTypeConcrete::Sint32(_)
        | CoreTypeConcrete::Sint64(_)
        | CoreTypeConcrete::Sint128(_)
        | CoreTypeConcrete::Bytes31(_) => match &info.inner_data.as_slice() {
            [GenericArg::Value(value)] => {
                Ok(entry.const_int_from_type(context, location, value.clone(), inner_ty)?)
            }
            _ => Err(Error::ConstDataMismatch),
        },
        _ => native_panic!("const for type {} not implemented", info.inner_ty),
    }
}

#[cfg(test)]
pub mod test {
    use crate::{
        jit_struct,
        utils::testing::{get_compiled_program, run_program},
        values::Value,
    };

    #[test]
    fn run_const_as_box() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/const_as_box");

        let result = run_program(&program, "run_test", &[]).return_value;
        assert_eq!(result, jit_struct!(Value::Sint32(-2)));
    }

    #[test]
    fn run_ec_point_const() {
        // Tests const_as_box on EcPoint type (Const<EcPoint, x, y>).
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/ec_point_const");

        // Just verify it compiles and runs without panicking.
        let _ = run_program(&program, "run_test", &[]);
    }
}
