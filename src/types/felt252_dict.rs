//! # `Felt` dictionary type
//!
//! A key value storage for values whose type implement Copy. The key is always a felt.
//!
//! This type is represented as a pointer to a tuple of a heap allocated Rust hashmap along with a u64
//! used to count accesses to the dictionary. The type is interacted through the runtime functions to
//! insert, get elements and increment the access counter.

use super::WithSelf;
use crate::{error::Result, metadata::MetadataStorage};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        types::InfoAndTypeConcreteType,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::llvm,
    ir::{Module, Type},
    Context,
};

/// Build the MLIR type.
///
/// Dict snapshots are semantically useless in Cairo, so the default dup
/// (returns the same pointer twice) is correct. The arena owns dict memory and
/// HashMaps are reclaimed via DICT_REGISTRY during arena reset, so the default
/// drop (noop) is also correct. No dup/drop overrides are registered.
///
/// Check out [the module](self) for more info.
pub fn build<'ctx>(
    context: &'ctx Context,
    _module: &Module<'ctx>,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _metadata: &mut MetadataStorage,
    _info: WithSelf<InfoAndTypeConcreteType>,
) -> Result<Type<'ctx>> {
    Ok(llvm::r#type::pointer(context, 0))
}

#[cfg(test)]
mod test {
    use crate::{
        jit_dict,
        utils::testing::{get_compiled_program, run_program},
        values::Value,
    };
    use pretty_assertions_sorted::assert_eq;
    use starknet_types_core::felt::Felt;
    use std::collections::HashMap;

    #[test]
    fn dict_snapshot_take() {
        let program = get_compiled_program("programs/types/dict_snapshot_take");
        let result = run_program(&program, "run_test", &[]).return_value;

        assert_eq!(
            result,
            jit_dict!(
                2 => 1u32
            ),
        );
    }

    #[test]
    fn dict_snapshot_take_complex() {
        let program = get_compiled_program("programs/types/dict_snapshot_complex");
        let result = run_program(&program, "run_test", &[]).return_value;

        assert_eq!(
            result,
            jit_dict!(
                2 => Value::Array(vec![3u32.into(), 4u32.into()])
            ),
        );
    }

    #[test]
    fn dict_snapshot_take_compare() {
        let program = get_compiled_program("programs/types/dict_snapshot_compare_snapshot");
        let program2 = get_compiled_program("programs/types/dict_snapshot_compare_owned");

        let result1 = run_program(&program, "run_test", &[]).return_value;
        let result2 = run_program(&program2, "run_test", &[]).return_value;

        assert_eq!(result1, result2);
    }

    /// Ensure that a dictionary of booleans compiles.
    #[test]
    fn dict_type_bool() {
        let program = get_compiled_program("programs/types/dict_bool");

        let result = run_program(&program, "run_program", &[]);
        assert_eq!(
            result.return_value,
            Value::Felt252Dict {
                value: HashMap::from([
                    (
                        Felt::ZERO,
                        Value::Enum {
                            tag: 0,
                            value: Box::new(Value::Struct {
                                fields: Vec::new(),
                                debug_name: None
                            }),
                            debug_name: None,
                        },
                    ),
                    (
                        Felt::ONE,
                        Value::Enum {
                            tag: 1,
                            value: Box::new(Value::Struct {
                                fields: Vec::new(),
                                debug_name: None
                            }),
                            debug_name: None,
                        },
                    ),
                ]),
                debug_name: None,
            },
        );
    }

    /// Ensure that a dictionary of felts compiles.
    #[test]
    fn dict_type_felt252() {
        let program = get_compiled_program("programs/types/dict_felt252");

        let result = run_program(&program, "run_program", &[]);
        assert_eq!(
            result.return_value,
            Value::Felt252Dict {
                value: HashMap::from([
                    (Felt::ZERO, Value::Felt252(Felt::ZERO)),
                    (Felt::ONE, Value::Felt252(Felt::ONE)),
                    (Felt::TWO, Value::Felt252(Felt::TWO)),
                    (Felt::THREE, Value::Felt252(Felt::THREE)),
                ]),
                debug_name: None,
            },
        );
    }

    /// Ensure that a dictionary of nullables compiles.
    #[test]
    fn dict_type_nullable() {
        let program = get_compiled_program("programs/types/dict_nullable");

        let result = run_program(&program, "run_program", &[]);
        pretty_assertions_sorted::assert_eq_sorted!(
            result.return_value,
            Value::Felt252Dict {
                value: HashMap::from([
                    (Felt::ZERO, Value::Null),
                    (
                        Felt::ONE,
                        Value::Struct {
                            fields: Vec::from([
                                Value::Uint8(0),
                                Value::Sint16(1),
                                Value::Felt252(2.into()),
                            ]),
                            debug_name: None,
                        },
                    ),
                    (
                        Felt::TWO,
                        Value::Struct {
                            fields: Vec::from([
                                Value::Uint8(1),
                                Value::Sint16(-2),
                                Value::Felt252(3.into()),
                            ]),
                            debug_name: None,
                        },
                    ),
                    (
                        Felt::THREE,
                        Value::Struct {
                            fields: Vec::from([
                                Value::Uint8(2),
                                Value::Sint16(3),
                                Value::Felt252(4.into()),
                            ]),
                            debug_name: None,
                        },
                    ),
                ]),
                debug_name: None,
            },
        );
    }

    /// Ensure that a dictionary of unsigned integers compiles.
    #[test]
    fn dict_type_unsigned() {
        let program = get_compiled_program("programs/types/dict_u128");

        let result = run_program(&program, "run_program", &[]);
        assert_eq!(
            result.return_value,
            Value::Felt252Dict {
                value: HashMap::from([
                    (Felt::ZERO, Value::Uint128(0)),
                    (Felt::ONE, Value::Uint128(1)),
                    (Felt::TWO, Value::Uint128(2)),
                    (Felt::THREE, Value::Uint128(3)),
                ]),
                debug_name: None,
            },
        );
    }
}
