use super::LibfuncHelper;
use crate::{
    error::{Error, Result},
    metadata::{
        realloc_bindings::ReallocBindingsMeta, runtime_bindings::RuntimeBindingsMeta,
        MetadataStorage,
    },
    utils::ProgramRegistryExt,
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        lib_func::SignatureAndTypeConcreteLibfunc,
        squashed_felt252_dict::SquashedFelt252DictConcreteLibfunc,
        ConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    helpers::{BuiltinBlockExt, LlvmBlockExt},
    ir::{Block, Location},
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
    selector: &SquashedFelt252DictConcreteLibfunc,
) -> Result<()> {
    match selector {
        SquashedFelt252DictConcreteLibfunc::IntoEntries(info) => {
            build_into_entries(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `squashed_felt252_dict_entries` libfunc.
///
/// Receives a `SquashedFelt252Dict<T>` and returns an `Array<(felt252, T, T)>`. This
/// array will have a tuple for each element on the dictionary. The first item represents
/// the key of the element in the dictionary, it is followed by the first value and last
/// value of that same element. Then (felt252, T, T) = (key, first_value, last_value).
///
/// # Caveats
///
/// In the tuple, the value that represents the first value will hold the value 0.
///
/// # Signature
///
/// ```cairo
/// extern fn squashed_felt252_dict_entries<T>(
///    dict: SquashedFelt252Dict<T>,
/// ) -> Array<(felt252, T, T)> nopanic;
/// ```
pub fn build_into_entries<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureAndTypeConcreteLibfunc,
) -> Result<()> {
    metadata.get_or_insert_with(|| ReallocBindingsMeta::new(context, helper));

    let dict_ptr = entry.arg(0)?;

    // Get the size for the array (prefix + data)
    let (array_ty, array_layout) = registry.build_type_with_layout(
        context,
        helper,
        metadata,
        &info.branch_signatures()[0].vars[0].ty,
    )?;
    // Alloc the necessary memory
    let array_ptr =
        helper
            .init_block()
            .alloca1(context, location, array_ty, array_layout.align())?;

    // Runtime function that creates the array with its content
    metadata
        .get_mut::<RuntimeBindingsMeta>()
        .ok_or(Error::MissingMetadata)?
        .dict_into_entries(context, helper, entry, dict_ptr, array_ptr, location)?;

    // Extract the array from the pointer
    let entries_array = entry.load(context, location, array_ptr, array_ty)?;

    helper.br(entry, 0, &[entries_array], location)
}

#[cfg(test)]
mod test {
    use crate::{jit_enum, jit_struct, load_cairo, utils::testing::run_program, Value};
    use cairo_lang_sierra::program::Program;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref INTO_ENTRIES_EMPTY_DICT: (String, Program) = load_cairo! {
            use core::dict::{Felt252Dict, Felt252DictEntryTrait, SquashedFelt252DictTrait};

            fn into_entries_empty_dict() -> Array<(felt252, u8, u8)> {
                let mut dict: Felt252Dict<u8> = Default::default();
                dict.squash().into_entries()
            }
        };
        static ref INTO_ENTRIES_U8_VALUES: (String, Program) = load_cairo! {
            use core::dict::{Felt252Dict, Felt252DictEntryTrait, SquashedFelt252DictTrait};

            fn into_entries_u8_values() -> Array<(felt252, u8, u8)> {
                let mut dict: Felt252Dict<u8> = Default::default();
                dict.insert(0, 0);
                dict.insert(1, 1);
                dict.insert(2, 2);
                dict.squash().into_entries()
            }
        };
        static ref INTO_ENTRIES_U32_VALUES: (String, Program) = load_cairo! {
            use core::dict::{Felt252Dict, Felt252DictEntryTrait, SquashedFelt252DictTrait};

            fn into_entries_u32_values() -> Array<(felt252, u32, u32)> {
                let mut dict: Felt252Dict<u32> = Default::default();
                dict.insert(0, 0);
                dict.insert(1, 1);
                dict.insert(2, 2);
                dict.squash().into_entries()
            }
        };
        static ref INTO_ENTRIES_U128_VALUES: (String, Program) = load_cairo! {
            use core::dict::{Felt252Dict, Felt252DictEntryTrait, SquashedFelt252DictTrait};

            fn into_entries_u128_values() -> Array<(felt252, u128, u128)> {
                let mut dict: Felt252Dict<u128> = Default::default();
                dict.insert(0, 0);
                dict.insert(1, 1);
                dict.insert(2, 2);
                dict.squash().into_entries()
            }
        };
        static ref INTO_ENTRIES_FELT252_VALUES: (String, Program) = load_cairo! {
            use core::dict::{Felt252Dict, Felt252DictEntryTrait, SquashedFelt252DictTrait};

            fn into_entries_felt252_values() -> Array<(felt252, felt252, felt252)> {
                let mut dict: Felt252Dict<felt252> = Default::default();
                dict.insert(0, 0);
                dict.insert(1, 1);
                dict.insert(2, 2);
                dict.squash().into_entries()
            }
        };
        static ref INTO_ENTRIES_ARRAY_VALUES: (String, Program) = load_cairo! {
            use core::nullable::NullableTrait;
            use core::dict::{Felt252Dict, Felt252DictEntryTrait, SquashedFelt252DictTrait};

            fn into_entries_array_values() -> Array<(felt252, Nullable<Array<u8>>, Nullable<Array<u8>>)> {
                let arr0 = array![1, 2, 3];
                let arr1 = array![4, 5, 6];
                let arr2 = array![7, 8, 9];

                let mut dict: Felt252Dict<Nullable<Array<u8>>> = Default::default();
                dict.insert(0, NullableTrait::new(arr0));
                dict.insert(1, NullableTrait::new(arr1));
                dict.insert(2, NullableTrait::new(arr2));

                dict.squash().into_entries()
            }
        };
        static ref INTO_ENTRIES_USE_ARRAY_VALUES: (String, Program) = load_cairo! {
            use core::nullable::NullableTrait;
            use core::dict::{Felt252Dict, Felt252DictEntryTrait, SquashedFelt252DictTrait};

            fn into_entries_use_array_values() -> u8 {
                let arr0 = array![1, 2, 3];
                let arr1 = array![4, 5, 6];
                let arr2 = array![7, 8, 9];

                let mut dict: Felt252Dict<Nullable<Array<u8>>> = Default::default();
                dict.insert(0, NullableTrait::new(arr0));
                dict.insert(1, NullableTrait::new(arr1));
                dict.insert(2, NullableTrait::new(arr2));

                let mut entries_array = dict.squash().into_entries();

                let (_key, _prev, value) = entries_array.pop_front().unwrap();
                let mut arr = value.deref();
                let one = arr.pop_front().unwrap();
                one
            }
        };
        static ref INTO_ENTRIES_WITH_SNAPSHOT: (String, Program) = load_cairo! {
            use core::nullable::NullableTrait;
            use core::dict::{Felt252Dict, Felt252DictEntryTrait, SquashedFelt252DictTrait};

            fn into_entries_with_snapshot() -> Array<(felt252, Nullable<Array<u8>>, Nullable<Array<u8>>)> {
                let arr0 = array![1, 2, 3];

                let mut dict: Felt252Dict<Nullable<Array<u8>>> = Default::default();
                dict.insert(0, NullableTrait::new(arr0));

                let dict_snapshot = @dict;
                let dict_squash = dict.squash().into_entries();
                drop_snapshot(dict_snapshot);

                dict_squash
            }

            #[inline(never)]
            // Force dict_snapshot to stay alive until after squash
            fn drop_snapshot<T, +Drop<T>>(_x: T) {}
        };
    }

    #[test]
    fn test_into_entries_empty_dict() {
        let result =
            run_program(&INTO_ENTRIES_EMPTY_DICT, "into_entries_empty_dict", &[]).return_value;
        if let Value::Array(arr) = result {
            assert_eq!(arr.len(), 0);
        }
    }

    #[test]
    fn test_into_entries_u8_values() {
        let result =
            run_program(&INTO_ENTRIES_U8_VALUES, "into_entries_u8_values", &[]).return_value;
        if let Value::Array(arr) = result {
            assert_eq!(
                arr[0],
                jit_struct!(Value::Felt252(0.into()), Value::Uint8(0), Value::Uint8(0))
            );
            assert_eq!(
                arr[1],
                jit_struct!(Value::Felt252(1.into()), Value::Uint8(0), Value::Uint8(1))
            );
            assert_eq!(
                arr[2],
                jit_struct!(Value::Felt252(2.into()), Value::Uint8(0), Value::Uint8(2))
            );
        }
    }

    #[test]
    fn test_into_entries_u32_values() {
        let result =
            run_program(&INTO_ENTRIES_U32_VALUES, "into_entries_u32_values", &[]).return_value;
        if let Value::Array(arr) = result {
            assert_eq!(
                arr[0],
                jit_struct!(Value::Felt252(0.into()), Value::Uint32(0), Value::Uint32(0))
            );
            assert_eq!(
                arr[1],
                jit_struct!(Value::Felt252(1.into()), Value::Uint32(0), Value::Uint32(1))
            );
            assert_eq!(
                arr[2],
                jit_struct!(Value::Felt252(2.into()), Value::Uint32(0), Value::Uint32(2))
            );
        }
    }

    #[test]
    fn test_into_entries_u128_values() {
        let result =
            run_program(&INTO_ENTRIES_U128_VALUES, "into_entries_u128_values", &[]).return_value;
        if let Value::Array(arr) = result {
            assert_eq!(
                arr[0],
                jit_struct!(
                    Value::Felt252(0.into()),
                    Value::Uint128(0),
                    Value::Uint128(0)
                )
            );
            assert_eq!(
                arr[1],
                jit_struct!(
                    Value::Felt252(1.into()),
                    Value::Uint128(0),
                    Value::Uint128(1)
                )
            );
            assert_eq!(
                arr[2],
                jit_struct!(
                    Value::Felt252(2.into()),
                    Value::Uint128(0),
                    Value::Uint128(2)
                )
            );
        }
    }

    #[test]
    fn test_into_entries_felt252_values() {
        let result = run_program(
            &INTO_ENTRIES_FELT252_VALUES,
            "into_entries_felt252_values",
            &[],
        )
        .return_value;
        if let Value::Array(arr) = result {
            assert_eq!(
                arr[0],
                jit_struct!(
                    Value::Felt252(0.into()),
                    Value::Felt252(0.into()),
                    Value::Felt252(0.into())
                )
            );
            assert_eq!(
                arr[1],
                jit_struct!(
                    Value::Felt252(1.into()),
                    Value::Felt252(0.into()),
                    Value::Felt252(1.into())
                )
            );
            assert_eq!(
                arr[2],
                jit_struct!(
                    Value::Felt252(2.into()),
                    Value::Felt252(0.into()),
                    Value::Felt252(2.into())
                )
            );
        }
    }

    #[test]
    fn test_into_entries_u8_array_values() {
        let result =
            run_program(&INTO_ENTRIES_ARRAY_VALUES, "into_entries_array_values", &[]).return_value;
        if let Value::Array(arr) = result {
            assert_eq!(
                arr[0],
                jit_struct!(
                    Value::Felt252(0.into()),
                    Value::Null,
                    Value::Array(vec![Value::Uint8(1), Value::Uint8(2), Value::Uint8(3)])
                )
            );
            assert_eq!(
                arr[1],
                jit_struct!(
                    Value::Felt252(1.into()),
                    Value::Null,
                    Value::Array(vec![Value::Uint8(4), Value::Uint8(5), Value::Uint8(6)])
                )
            );
            assert_eq!(
                arr[2],
                jit_struct!(
                    Value::Felt252(2.into()),
                    Value::Null,
                    Value::Array(vec![Value::Uint8(7), Value::Uint8(8), Value::Uint8(9)])
                )
            );
        }
    }

    #[test]
    fn test_into_entries_use_values_from_array() {
        let result = run_program(
            &INTO_ENTRIES_USE_ARRAY_VALUES,
            "into_entries_use_array_values",
            &[],
        )
        .return_value;
        assert_eq!(result, jit_enum!(0, jit_struct!(Value::Uint8(1))));
    }

    #[test]
    /// Primarily to see if there are no double frees when dropping a snapshot of the same dictionary
    fn test_into_entries_with_snapshot() {
        let result = run_program(
            &INTO_ENTRIES_WITH_SNAPSHOT,
            "into_entries_with_snapshot",
            &[],
        )
        .return_value;
        if let Value::Array(arr) = result {
            assert_eq!(
                arr[0],
                jit_struct!(
                    Value::Felt252(0.into()),
                    Value::Null,
                    Value::Array(vec![Value::Uint8(1), Value::Uint8(2), Value::Uint8(3)])
                )
            );
        }
    }
}
