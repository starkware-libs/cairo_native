//! # `Felt` dictionary libfuncs

use super::LibfuncHelper;
use crate::{
    error::{panic::ToNativeAssertError, Result},
    execution_result::SEGMENT_ARENA_BUILTIN_SIZE,
    metadata::{runtime_bindings::RuntimeBindingsMeta, MetadataStorage},
    native_panic,
    types::TypeBuilder,
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        felt252_dict::Felt252DictConcreteLibfunc,
        lib_func::SignatureOnlyConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    helpers::{BuiltinBlockExt, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, Location},
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
    selector: &Felt252DictConcreteLibfunc,
) -> Result<()> {
    match selector {
        Felt252DictConcreteLibfunc::New(info) => {
            build_new(context, registry, entry, location, helper, metadata, info)
        }
        Felt252DictConcreteLibfunc::Squash(info) => {
            build_squash(context, registry, entry, location, helper, metadata, info)
        }
    }
}

pub fn build_new<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    // We increase the segment arena builtin by 1 usage.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L45-L49
    let segment_arena = super::increment_builtin_counter_by(
        context,
        entry,
        location,
        entry.arg(0)?,
        SEGMENT_ARENA_BUILTIN_SIZE,
    )?;

    let value_type_id = match registry.get_type(&info.signature.branch_signatures[0].vars[1].ty)? {
        CoreTypeConcrete::Felt252Dict(info) => &info.ty,
        _ => native_panic!("entered unreachable code"),
    };

    let runtime_bindings = metadata
        .get_mut::<RuntimeBindingsMeta>()
        .to_native_assert_error("runtime library should be available")?;
    let dict_ptr = runtime_bindings.dict_new(
        context,
        helper,
        entry,
        location,
        registry.get_type(value_type_id)?.layout(registry)?,
    )?;

    helper.br(entry, 0, &[segment_arena, dict_ptr], location)
}

pub fn build_squash<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let range_check = entry.arg(0)?;
    let gas = entry.arg(1)?;
    let segment_arena = entry.arg(2)?;
    let dict_ptr = entry.arg(3)?;

    // Increase the segment arena builtin by 1 usage.
    // https://github.com/starkware-libs/cairo/blob/v2.12.0-dev.1/crates/cairo-lang-sierra-to-casm/src/invocations/felt252_dict.rs?plain=1#L148-L151
    let segment_arena = super::increment_builtin_counter_by(
        context,
        entry,
        location,
        segment_arena,
        SEGMENT_ARENA_BUILTIN_SIZE,
    )?;

    let runtime_bindings = metadata
        .get_mut::<RuntimeBindingsMeta>()
        .to_native_assert_error("runtime library should be available")?;

    let range_check_ptr =
        entry.alloca1(context, location, IntegerType::new(context, 64).into(), 0)?;
    entry.store(context, location, range_check_ptr, range_check)?;
    let gas_ptr = entry.alloca1(context, location, IntegerType::new(context, 64).into(), 0)?;
    entry.store(context, location, gas_ptr, gas)?;

    runtime_bindings.dict_squash(
        context,
        helper,
        entry,
        dict_ptr,
        range_check_ptr,
        gas_ptr,
        location,
    )?;

    let range_check = entry.load(
        context,
        location,
        range_check_ptr,
        IntegerType::new(context, 64).into(),
    )?;
    let gas_builtin = entry.load(
        context,
        location,
        gas_ptr,
        IntegerType::new(context, 64).into(),
    )?;

    helper.br(
        entry,
        0,
        &[range_check, gas_builtin, segment_arena, entry.arg(3)?],
        location,
    )
}

#[cfg(test)]
mod test {
    use crate::{
        jit_dict, jit_enum, jit_struct,
        utils::testing::{get_compiled_program, run_program, run_program_assert_output},
        values::Value,
    };

    #[test]
    fn run_dict_new() {
        let program = get_compiled_program("programs/libfuncs/felt252_dict_new");

        run_program_assert_output(&program, "run_test", &[], jit_struct!());
    }

    #[test]
    fn run_dict_insert() {
        let program = get_compiled_program("programs/libfuncs/felt252_dict_insert");

        run_program_assert_output(&program, "run_test", &[], 1u32.into());
    }

    #[test]
    fn run_dict_insert_ret_dict() {
        let program = get_compiled_program("programs/libfuncs/felt252_dict_insert_ret_dict");

        run_program_assert_output(
            &program,
            "run_test",
            &[],
            jit_dict!(
                1 => 2u32,
                2 => 3u32,
                3 => 4u32,
                4 => 5u32,
                5 => 6u32,
            ),
        );
    }

    #[test]
    fn run_dict_deserialize() {
        let program = get_compiled_program("programs/libfuncs/felt252_dict_deserialize");

        run_program_assert_output(
            &program,
            "run_test",
            &[jit_dict!(
                1 => 2u32,
                2 => 3u32,
                3 => 4u32,
                4 => 5u32,
                5 => 6u32,
            )],
            jit_dict!(
                    1 => 2u32,
                    2 => 3u32,
                    3 => 4u32,
                    4 => 5u32,
                    5 => 6u32,
            ),
        );
    }

    #[test]
    fn run_dict_deserialize2() {
        let program = get_compiled_program("programs/libfuncs/felt252_dict_deserialize2");

        run_program_assert_output(
            &program,
            "run_test",
            &[jit_dict!(
                1 => 2u32,
                2 => 3u32,
                3 => 4u32,
                4 => 5u32,
                5 => 6u32,
            )],
            jit_struct!(
                Value::Felt252(0.into()),
                jit_dict!(
                    1 => 2u32,
                    2 => 3u32,
                    3 => 4u32,
                    4 => 5u32,
                    5 => 6u32,
                )
            ),
        );
    }

    #[test]
    fn run_dict_deserialize_struct() {
        let program = get_compiled_program("programs/libfuncs/felt252_dict_deserialize_struct");

        run_program_assert_output(
            &program,
            "run_test",
            &[],
            jit_dict!(

                0 => jit_struct!(1u32.into(), 2u64.into(), 3u128.into()),
                1 => jit_struct!(2u32.into(), 3u64.into(), 4u128.into()),
                2 => jit_struct!(3u32.into(), 4u64.into(), 5u128.into()),
            ),
        );

        run_program_assert_output(
            &program,
            "run_test",
            &[],
            jit_dict!(
                0 => jit_struct!(1u32.into(), 2u64.into(), 3u128.into()),
                1 => jit_struct!(2u32.into(), 3u64.into(), 4u128.into()),
                2 => jit_struct!(3u32.into(), 4u64.into(), 5u128.into()),
            ),
        );
    }

    #[test]
    fn run_dict_deserialize_enum() {
        let program = get_compiled_program("programs/libfuncs/felt252_dict_deserialize_enum");

        run_program_assert_output(
            &program,
            "run_test",
            &[],
            jit_dict!(
                0 => jit_enum!(0, 1u32.into()),
                1 => jit_enum!(1, 2u64.into()),
                2 => jit_enum!(2, 3u128.into()),
            ),
        );
    }

    #[test]
    fn run_dict_squash() {
        let program = get_compiled_program("programs/libfuncs/felt252_dict_squash");

        let result = run_program(&program, "main", &[]);
        assert_eq!(result.builtin_stats.range_check, 100);
    }
}
