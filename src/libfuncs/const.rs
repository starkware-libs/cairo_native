//! # Const libfuncs

use super::LibfuncHelper;
use crate::{
    error::{CompilerError, Error, Result},
    metadata::{const_globals::ConstGlobalsMeta, MetadataStorage},
    native_panic,
    types::{r#enum::get_layout_for_variants, TypeBuilder},
    utils::{felt_to_unsigned, get_integer_layout, ProgramRegistryExt, RangeExt},
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
    ids::ConcreteTypeId,
    program::GenericArg,
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{
        llvm::{self, LoadStoreOptions},
        ods,
    },
    helpers::{ArithBlockExt, BuiltinBlockExt},
    ir::{
        attribute::{
            DenseElementsAttribute, FlatSymbolRefAttribute, IntegerAttribute, StringAttribute,
            TypeAttribute,
        },
        r#type::{IntegerType, RankedTensorType},
        Attribute, Block, BlockLike, Location, Region, TypeLike, Value,
    },
    Context,
};
use num_bigint::{BigInt, BigUint};
use num_traits::{Euclid, One};

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
///
/// The box is a pointer to the constant's global. This is sound because boxes are arena
/// allocated and never written through or freed.
pub fn build_const_as_box<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &ConstAsBoxConcreteLibfunc,
) -> Result<()> {
    let ptr = build_const_global_ptr(
        context,
        registry,
        entry,
        location,
        helper,
        metadata,
        &info.const_type,
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
    let const_type = as_const_type(registry, &info.const_type)?;
    let (inner_type, inner_layout) =
        registry.build_type_with_layout(context, helper, metadata, &const_type.inner_ty)?;

    // Integers (including felts) are emitted as immediates: a load from the global would only
    // be folded by LLVM at `OptLevel::Less` or above.
    if inner_type.is_integer() {
        let bytes = build_const_type_bytes(registry, const_type)?;
        let value = entry.const_int_from_type(
            context,
            location,
            BigUint::from_bytes_le(&bytes),
            inner_type,
        )?;

        return helper.br(entry, 0, &[value], location);
    }

    let ptr = build_const_global_ptr(
        context,
        registry,
        entry,
        location,
        helper,
        metadata,
        &info.const_type,
    )?;
    let value = entry.append_op_result(llvm::load(
        context,
        ptr,
        inner_type,
        location,
        LoadStoreOptions::new().align(Some(IntegerAttribute::new(
            IntegerType::new(context, 64).into(),
            inner_layout.align() as i64,
        ))),
    ))?;

    helper.br(entry, 0, &[value], location)
}

/// Returns a pointer to the read-only global holding the constant's native memory
/// representation, emitting the global the first time the const type is seen in the module.
fn build_const_global_ptr<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    const_type_id: &ConcreteTypeId,
) -> Result<Value<'ctx, 'this>> {
    let symbol = format!("cairo_native__const_{}", const_type_id.id);

    if metadata
        .get_or_insert_with(ConstGlobalsMeta::default)
        .mark_emitted(const_type_id)
    {
        let const_type = as_const_type(registry, const_type_id)?;
        let align = registry
            .get_type(&const_type.inner_ty)?
            .layout(registry)?
            .align();
        let bytes = build_const_type_bytes(registry, const_type)?;

        let i8_ty = IntegerType::new(context, 8).into();
        let value = DenseElementsAttribute::new(
            RankedTensorType::new(&[bytes.len() as u64], i8_ty, None).into(),
            &bytes
                .iter()
                .map(|byte| IntegerAttribute::new(i8_ty, *byte as i8 as i64).into())
                .collect::<Vec<_>>(),
        )?;

        helper.module.body().append_operation(
            ods::llvm::GlobalOperationBuilder::new(context, location)
                .initializer(Region::new())
                .global_type(TypeAttribute::new(llvm::r#type::array(
                    i8_ty,
                    bytes.len() as u32,
                )))
                .sym_name(StringAttribute::new(context, &symbol))
                .linkage(
                    Attribute::parse(context, "#llvm.linkage<private>")
                        .ok_or(Error::ParseAttributeError)?,
                )
                .value(value.into())
                .constant(Attribute::unit(context))
                .alignment(IntegerAttribute::new(
                    IntegerType::new(context, 64).into(),
                    align as i64,
                ))
                .build()
                .into(),
        );
    }

    Ok(entry.append_op_result(
        ods::llvm::mlir_addressof(
            context,
            llvm::r#type::pointer(context, 0),
            FlatSymbolRefAttribute::new(context, &symbol),
            location,
        )
        .into(),
    )?)
}

/// Serialize a constant into the native memory representation of its inner type.
///
/// This must match both the layouts in `TypeBuilder::layout` and the encoding in `Value::to_ptr`.
///
/// `info.inner_data` should be one of the following:
/// - A single value, if the inner type is a simple numeric type (e.g., `felt252`, `u32`, etc.).
/// - A list of const types, if the inner type is a struct. The type of each const type must be
///   the same as the corresponding struct member type.
/// - A selector (a single value) followed by a const type, if the inner type is an enum. The
///   type of the const type must be the same as the corresponding enum variant type.
fn build_const_type_bytes(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &ConstConcreteType,
) -> Result<Vec<u8>> {
    let layout = registry.get_type(&info.inner_ty)?.layout(registry)?;

    let mut bytes = Vec::with_capacity(layout.size());
    build_const_type_bytes_into(registry, info, &mut bytes)?;
    Ok(bytes)
}

/// Appends the constant's bytes to `bytes`. Offsets are relative to the buffer length on entry.
fn build_const_type_bytes_into(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    info: &ConstConcreteType,
    bytes: &mut Vec<u8>,
) -> Result<()> {
    let inner_type = registry.get_type(&info.inner_ty)?;
    let layout = inner_type.layout(registry)?;

    let start = bytes.len();
    match inner_type {
        CoreTypeConcrete::Struct(_) => {
            for field in &info.inner_data {
                let GenericArg::Type(const_field_ty) = field else {
                    return Err(Error::ConstDataMismatch);
                };
                let const_field_type = as_const_type(registry, const_field_ty)?;
                let field_align = registry
                    .get_type(&const_field_type.inner_ty)?
                    .layout(registry)?
                    .align();

                bytes.resize(
                    start + (bytes.len() - start).next_multiple_of(field_align),
                    0,
                );
                build_const_type_bytes_into(registry, const_field_type, bytes)?;
            }
        }
        CoreTypeConcrete::Enum(enum_info) => match &info.inner_data[..] {
            [GenericArg::Value(variant_index), GenericArg::Type(payload_ty)] => {
                let variant_index: usize = variant_index
                    .try_into()
                    .map_err(|_| Error::IntegerConversion)?;
                let (_, tag_layout, variant_layouts) =
                    get_layout_for_variants(registry, &enum_info.variants)?;
                let payload_offset = tag_layout.extend(variant_layouts[variant_index])?.1;

                bytes.extend(&variant_index.to_le_bytes()[..tag_layout.size()]);
                bytes.resize(start + payload_offset, 0);
                build_const_type_bytes_into(registry, as_const_type(registry, payload_ty)?, bytes)?;
            }
            _ => return Err(Error::ConstDataMismatch),
        },
        // NonZero is the same type as the inner type in native.
        CoreTypeConcrete::NonZero(_) => match &info.inner_data[..] {
            [GenericArg::Type(inner)] => {
                build_const_type_bytes_into(registry, as_const_type(registry, inner)?, bytes)?;
            }
            _ => return Err(Error::ConstDataMismatch),
        },
        CoreTypeConcrete::BoundedInt(BoundedIntConcreteType { range, .. }) => {
            match &info.inner_data[..] {
                [GenericArg::Value(value)] => {
                    bytes.extend(range.repr_encode(value).ok_or_else(|| {
                        CompilerError::BoundedIntOutOfRange {
                            value: Box::new(value.clone()),
                            range: Box::new((range.lower.clone(), range.upper.clone())),
                        }
                    })?);
                }
                _ => return Err(Error::ConstDataMismatch),
            }
        }
        CoreTypeConcrete::Felt252(_)
        | CoreTypeConcrete::Starknet(
            StarknetTypeConcrete::ClassHash(_) | StarknetTypeConcrete::ContractAddress(_),
        ) => match &info.inner_data[..] {
            [GenericArg::Value(value)] => bytes.extend(felt_to_unsigned(value).to_bytes_le()),
            _ => return Err(Error::ConstDataMismatch),
        },
        CoreTypeConcrete::EcPoint(_) => match &info.inner_data[..] {
            [GenericArg::Value(x), GenericArg::Value(y)] => {
                bytes.extend(felt_to_unsigned(x).to_bytes_le());
                bytes.resize(start + get_integer_layout(252).size(), 0);
                bytes.extend(felt_to_unsigned(y).to_bytes_le());
            }
            _ => return Err(Error::ConstDataMismatch),
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
        | CoreTypeConcrete::Bytes31(_) => match &info.inner_data[..] {
            // Two's complement over the full layout size.
            [GenericArg::Value(value)] => {
                let modulus = BigInt::one() << (8 * layout.size());
                bytes.extend(value.rem_euclid(&modulus).to_bytes_le().1);
            }
            _ => return Err(Error::ConstDataMismatch),
        },
        _ => native_panic!("const for type {} not implemented", info.inner_ty),
    };

    bytes.resize(start + layout.size(), 0);
    Ok(())
}

fn as_const_type<'a>(
    registry: &'a ProgramRegistry<CoreType, CoreLibfunc>,
    type_id: &ConcreteTypeId,
) -> Result<&'a ConstConcreteType> {
    match registry.get_type(type_id)? {
        CoreTypeConcrete::Const(inner) => Ok(inner),
        _ => native_panic!("matched an unexpected CoreTypeConcrete that is not a Const"),
    }
}

#[cfg(test)]
pub mod test {
    use crate::{
        context::NativeContext,
        executor::AotNativeExecutor,
        jit_struct, load_cairo,
        utils::testing::{get_compiled_program, run_program},
        values::Value,
        OptLevel,
    };

    #[test]
    fn run_const_as_box_aot() {
        // The constant's global must survive AOT object emission.
        let program = load_cairo! {
            use core::box::BoxTrait;

            fn run_test() -> Box<felt252> {
                BoxTrait::new(1234)
            }
        };

        let native_context = NativeContext::new();
        let module = native_context
            .compile(&program.1, false, Some(Default::default()), None)
            .unwrap();
        let executor = AotNativeExecutor::from_native_module(module, OptLevel::Aggressive).unwrap();
        let result = executor
            .invoke_dynamic(&program.1.funcs[0].id, &[], Some(u64::MAX))
            .unwrap();
        assert_eq!(result.return_value, Value::Felt252(1234.into()));
    }

    #[test]
    fn run_const_as_box() {
        let program = get_compiled_program("programs/libfuncs/const_as_box");

        let result = run_program(&program, "run_test", &[]).return_value;
        assert_eq!(result, jit_struct!(Value::Sint32(-2)));
    }

    #[test]
    fn run_ec_point_const() {
        // Tests const_as_box on EcPoint type (Const<EcPoint, x, y>).
        let program = get_compiled_program("programs/libfuncs/ec_point_const");

        // Just verify it compiles and runs without panicking.
        let _ = run_program(&program, "run_test", &[]);
    }
}
