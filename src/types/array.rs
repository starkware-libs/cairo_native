//! # Array type
//!
//! An array type is a dynamically allocated list of items.
//!
//! ## Layout
//!
//! | Index | Type        | Description                              |
//! | ----- | ----------- | ---------------------------------------- |
//! |   0   | `!llvm.ptr` | Pointer to the element data buffer[^1].  |
//! |   1   | `i32`       | Array start offset[^2].                  |
//! |   2   | `i32`       | Array end offset[^2].                    |
//! |   3   | `i32`       | Allocated capacity[^2].                  |
//!
//! [^1]: This pointer is null when no buffer has been allocated yet (i.e. for a fresh empty array).
//! [^2]: Those numbers are number of items, **not bytes**.

use super::WithSelf;
use crate::{error::Result, metadata::MetadataStorage, utils::ProgramRegistryExt};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        types::InfoAndTypeConcreteType,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::llvm,
    ir::{r#type::IntegerType, Module, Type},
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
    info: WithSelf<InfoAndTypeConcreteType>,
) -> Result<Type<'ctx>> {
    // Ensure the element type is built.
    registry.build_type(context, module, metadata, &info.ty)?;

    let ptr_ty = llvm::r#type::pointer(context, 0);
    let len_ty = IntegerType::new(context, 32).into();

    Ok(llvm::r#type::r#struct(
        context,
        &[ptr_ty, len_ty, len_ty, len_ty],
        false,
    ))
}

#[cfg(test)]
mod test {
    use crate::{
        utils::testing::{get_compiled_program, run_program},
        values::Value,
    };
    use pretty_assertions_sorted::assert_eq;

    #[test]
    fn test_array_snapshot_deep_clone() {
        let program = get_compiled_program("test_data_artifacts/programs/types/nested_arrays");
        let result = run_program(&program, "run_test", &[]).return_value;

        assert_eq!(
            result,
            Value::Array(vec![
                Value::Array(vec![
                    Value::Felt252(1.into()),
                    Value::Felt252(2.into()),
                    Value::Felt252(3.into()),
                ]),
                Value::Array(vec![
                    Value::Felt252(4.into()),
                    Value::Felt252(5.into()),
                    Value::Felt252(6.into()),
                ]),
            ]),
        );
    }

    #[test]
    fn test_dup_snapshots_of_dictionary_array() {
        let program = get_compiled_program(
            "test_data_artifacts/programs/types/dup_snapshots_of_dictionary_array",
        );
        let result = run_program(&program, "run_test", &[]).return_value;
        assert_eq!(result, Value::Felt252(6.into()));
    }
}
