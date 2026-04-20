//! # Box libfuncs
//!
//! A heap allocated value, which is internally a pointer that can't be null.
//!
//! Allocations are made from the per-invocation arena (`cairo_native__box_alloc` runtime
//! function) and are freed all at once when the program invocation completes.  Individual
//! boxes are therefore never freed on unbox.

use std::alloc::Layout;

use super::LibfuncHelper;
use crate::{
    error::Result,
    metadata::{runtime_bindings::RuntimeBindingsMeta, MetadataStorage},
    types::TypeBuilder,
    utils::ProgramRegistryExt,
};
use cairo_lang_sierra::{
    extensions::{
        boxing::BoxConcreteLibfunc,
        core::{CoreLibfunc, CoreType},
        lib_func::SignatureAndTypeConcreteLibfunc,
    },
    ids::ConcreteTypeId,
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::llvm::{self, LoadStoreOptions},
    helpers::{ArithBlockExt, BuiltinBlockExt},
    ir::{
        attribute::IntegerAttribute, r#type::IntegerType, Block, BlockLike, Location, Module, Value,
    },
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
    selector: &BoxConcreteLibfunc,
) -> Result<()> {
    match selector {
        BoxConcreteLibfunc::Into(info) | BoxConcreteLibfunc::LocalInto(info) => {
            build_into_box(context, registry, entry, location, helper, metadata, info)
        }
        BoxConcreteLibfunc::Unbox(info) => {
            build_unbox(context, registry, entry, location, helper, metadata, info)
        }
        BoxConcreteLibfunc::ForwardSnapshot(info) => super::build_noop::<1, false>(
            context,
            registry,
            entry,
            location,
            helper,
            metadata,
            &info.signature.param_signatures,
        ),
    }
}

/// Generate MLIR operations for the `into_box` libfunc.
pub fn build_into_box<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureAndTypeConcreteLibfunc,
) -> Result<()> {
    let inner_type = registry.get_type(&info.ty)?;
    let inner_layout = inner_type.layout(registry)?;

    let ptr = into_box(
        context,
        helper.module,
        entry,
        location,
        entry.arg(0)?,
        inner_layout,
        metadata,
    )?;

    helper.br(entry, 0, &[ptr], location)
}

/// Allocate an arena slot and store `inner_val` into it, returning a pointer.
///
/// Uses `RuntimeBindingsMeta::box_alloc` which calls the Rust-side
/// `cairo_native__box_alloc` through the global function-pointer mechanism —
/// no direct symbol-table lookup required.
pub fn into_box<'ctx, 'this>(
    context: &'ctx Context,
    module: &Module<'ctx>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    inner_val: Value<'ctx, 'this>,
    inner_layout: Layout,
    metadata: &mut MetadataStorage,
) -> Result<Value<'ctx, 'this>> {
    let size = entry.const_int(context, location, inner_layout.size(), 64)?;
    let align = entry.const_int(context, location, inner_layout.align(), 64)?;

    let rtb = metadata.get_or_insert_with(RuntimeBindingsMeta::default);
    let ptr = rtb.box_alloc(context, module, entry, location, size, align)?;

    entry.append_operation(llvm::store(
        context,
        inner_val,
        ptr,
        location,
        LoadStoreOptions::new().align(Some(IntegerAttribute::new(
            IntegerType::new(context, 64).into(),
            inner_layout.align() as i64,
        ))),
    ));

    Ok(ptr)
}

/// Generate MLIR operations for the `unbox` libfunc.
pub fn build_unbox<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureAndTypeConcreteLibfunc,
) -> Result<()> {
    let value = unbox(
        context, registry, entry, location, helper, metadata, &info.ty,
    )?;

    helper.br(entry, 0, &[value], location)
}

// Gets the value that is inside a `Box`
pub fn unbox<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    inner_ty_id: &ConcreteTypeId,
) -> Result<Value<'ctx, 'this>> {
    let (inner_type, inner_layout) =
        registry.build_type_with_layout(context, helper, metadata, inner_ty_id)?;

    // Load the boxed value from memory.
    let value = entry
        .append_operation(llvm::load(
            context,
            entry.arg(0)?,
            inner_type,
            location,
            LoadStoreOptions::new().align(Some(IntegerAttribute::new(
                IntegerType::new(context, 64).into(),
                inner_layout.align() as i64,
            ))),
        ))
        .result(0)?
        .into();

    Ok(value)
}

#[cfg(test)]
mod test {
    use crate::{
        jit_enum, jit_struct,
        utils::testing::{get_compiled_program, run_program_assert_output},
        values::Value,
    };

    #[test]
    fn run_box_unbox() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/box_unbox");

        run_program_assert_output(&program, "run_test", &[], Value::Uint32(2));
    }

    #[test]
    fn run_box() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/box");

        run_program_assert_output(&program, "run_test", &[], Value::Uint32(2));
    }

    #[test]
    fn box_unbox_stack_allocated_enum_single() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/box_unbox_enum_single");

        run_program_assert_output(
            &program,
            "run_test",
            &[],
            Value::Enum {
                tag: 0,
                value: Box::new(Value::Felt252(1234.into())),
                debug_name: None,
            },
        );
    }

    #[test]
    fn box_unbox_stack_allocated_enum_c() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/box_unbox_enum_c");

        run_program_assert_output(
            &program,
            "run_test",
            &[],
            Value::Enum {
                tag: 0,
                value: Box::new(Value::Struct {
                    fields: Vec::new(),
                    debug_name: None,
                }),
                debug_name: None,
            },
        );
    }

    #[test]
    fn box_unbox_stack_allocated_enum_c2() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/box_unbox_enum_c2");

        run_program_assert_output(
            &program,
            "run_test",
            &[],
            Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: Vec::new(),
                    debug_name: None,
                }),
                debug_name: None,
            },
        );
    }

    #[test]
    fn run_local_into_box_for_option() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/box_local_into_option");

        run_program_assert_output(
            &program,
            "local_into_box_for_option",
            &[],
            jit_enum!(0, Value::Uint8(6)),
        );
    }

    #[test]
    fn run_local_into_box_for_tuple() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/box_local_into_tuple");

        run_program_assert_output(
            &program,
            "local_into_box_for_tuple",
            &[],
            jit_struct!(4u8.into(), 5u8.into(), 6u8.into()),
        );
    }

    #[test]
    fn box_unbox_stack_allocated_enum() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/box_unbox_enum");

        run_program_assert_output(
            &program,
            "run_test",
            &[],
            Value::Enum {
                tag: 0,
                value: Box::new(Value::Felt252(1234.into())),
                debug_name: None,
            },
        );
    }
}
