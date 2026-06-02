#![allow(dead_code)]

use anyhow::bail;
use cairo_lang_runner::{casm_run::format_next_item, RunResultValue};
use cairo_lang_sierra::program::{Function, Program};
use cairo_native::{
    execution_result::ExecutionResult,
    starknet::{Secp256k1Point, Secp256r1Point},
    Value,
};
use itertools::Itertools;
use starknet_types_core::felt::Felt;
use std::vec::IntoIter;

pub mod test;

pub struct RunArgs {
    pub opt_level: u8,
    pub compare_with_vm: bool,
}

/// Find the function ending with `name_suffix` in the program.
pub fn find_function<'a>(
    sierra_program: &'a Program,
    name_suffix: &str,
) -> anyhow::Result<&'a Function> {
    if let Some(x) = sierra_program.funcs.iter().find(|f| {
        if let Some(name) = &f.id.debug_name {
            name.ends_with(name_suffix)
        } else {
            false
        }
    }) {
        Ok(x)
    } else {
        bail!("test function not found")
    }
}

/// Formats the given felts as a panic string.
pub fn format_for_panic(mut felts: IntoIter<Felt>) -> String {
    let mut items = Vec::new();
    while let Some(item) = format_next_item(&mut felts) {
        items.push(item.quote_if_string());
    }
    let panic_values_string = if let [item] = &items[..] {
        item.clone()
    } else {
        format!("({})", items.join(", "))
    };
    format!("Panicked with {panic_values_string}.")
}

/// Convert the execution result to a run result.
pub fn result_to_runresult(result: &ExecutionResult) -> anyhow::Result<RunResultValue> {
    let is_success;
    let mut felts: Vec<Felt> = Vec::new();

    match &result.return_value {
        outer_value @ Value::Enum {
            tag,
            value,
            debug_name,
        } => {
            let debug_name = debug_name.as_ref().expect("missing debug name");
            if debug_name.starts_with("core::panics::PanicResult::")
                || debug_name.starts_with("Enum<ut@core::panics::PanicResult::")
            {
                is_success = *tag == 0;

                if !is_success {
                    match &**value {
                        Value::Struct { fields, .. } => {
                            for field in fields {
                                let felt = jitvalue_to_felt(field);
                                felts.extend(felt);
                            }
                        }
                        _ => bail!("unsuported return value in cairo-native"),
                    }
                } else {
                    felts.extend(jitvalue_to_felt(value));
                }
            } else {
                is_success = true;
                felts.extend(jitvalue_to_felt(outer_value));
            }
        }
        x => {
            is_success = true;
            felts.extend(jitvalue_to_felt(x));
        }
    }

    let return_values = felts
        .into_iter()
        .map(|x| x.to_bigint().into())
        .collect_vec();

    Ok(match is_success {
        true => RunResultValue::Success(return_values),
        false => RunResultValue::Panic(return_values),
    })
}

/// Convert a JIT value to a felt.
fn jitvalue_to_felt(value: &Value) -> Vec<Felt> {
    let mut felts = Vec::new();
    match value {
        Value::Felt252(felt) => vec![*felt],
        Value::BoundedInt { value, .. } => vec![*value],
        Value::Array(fields) | Value::Struct { fields, .. } => {
            fields.iter().flat_map(jitvalue_to_felt).collect()
        }
        Value::Enum {
            value,
            tag,
            debug_name,
        } => {
            if let Some(debug_name) = debug_name {
                if debug_name == "core::bool" {
                    vec![(*tag == 1).into()]
                } else {
                    let mut felts = vec![(*tag).into()];
                    felts.extend(jitvalue_to_felt(value));
                    felts
                }
            } else {
                // Assume its a regular enum.
                let mut felts = vec![(*tag).into()];
                felts.extend(jitvalue_to_felt(value));
                felts
            }
        }
        Value::Felt252Dict { value, .. } => {
            for (key, value) in value {
                felts.push(*key);
                let felt = jitvalue_to_felt(value);
                felts.extend(felt);
            }

            felts
        }
        Value::Uint8(x) => vec![(*x).into()],
        Value::Uint16(x) => vec![(*x).into()],
        Value::Uint32(x) => vec![(*x).into()],
        Value::Uint64(x) => vec![(*x).into()],
        Value::Uint128(x) => vec![(*x).into()],
        Value::Sint8(x) => vec![(*x).into()],
        Value::Sint16(x) => vec![(*x).into()],
        Value::Sint32(x) => vec![(*x).into()],
        Value::Sint64(x) => vec![(*x).into()],
        Value::Sint128(x) => vec![(*x).into()],
        Value::Bytes31(bytes) => vec![Felt::from_bytes_le_slice(bytes)],
        Value::EcPoint(x, y) | Value::EcState(x, y) => {
            vec![*x, *y]
        }
        Value::QM31(a, b, c, d) => vec![
            Felt::from(*a),
            Felt::from(*b),
            Felt::from(*c),
            Felt::from(*d),
        ],
        Value::Secp256K1Point(Secp256k1Point {
            x,
            y,
            is_infinity: _,
        }) => {
            vec![x.lo.into(), x.hi.into(), y.lo.into(), y.hi.into()]
        }
        Value::Secp256R1Point(Secp256r1Point {
            x,
            y,
            is_infinity: _,
        }) => {
            vec![x.lo.into(), x.hi.into(), y.lo.into(), y.hi.into()]
        }
        Value::Null => vec![0.into()],
        Value::IntRange { x, y } => [jitvalue_to_felt(x), jitvalue_to_felt(y)].concat(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairo_lang_sierra::ProgramParser;
    use std::collections::HashMap;

    /// Check if subsequence is present in sequence
    fn is_subsequence<T: PartialEq>(subsequence: &[T], mut sequence: &[T]) -> bool {
        for search in subsequence {
            if let Some(index) = sequence.iter().position(|element| search == element) {
                sequence = &sequence[index + 1..];
            } else {
                return false;
            }
        }
        true
    }

    #[test]
    fn test_find_function() {
        // Parse a simple program containing a function named "Func2"
        let program = ProgramParser::new().parse("Func2@6() -> ();").unwrap();

        // Assert that the function "Func2" is found and returned correctly
        assert_eq!(
            find_function(&program, "Func2").unwrap(),
            program.funcs.first().unwrap()
        );

        // Assert that an error is returned when trying to find a non-existing function "Func3"
        assert!(find_function(&program, "Func3").is_err());

        // Assert that an error is returned when trying to find a function in an empty program
        assert!(find_function(&ProgramParser::new().parse("").unwrap(), "Func2").is_err());
    }

    #[test]
    fn test_result_to_runresult_enum_nonpanic() {
        // Tests the conversion of a non-panic enum result to a `RunResultValue::Success`.
        assert_eq!(
            result_to_runresult(&ExecutionResult {
                remaining_gas: None,
                return_value: Value::Enum {
                    tag: 34,
                    value: Value::Array(vec![
                        Value::Felt252(42.into()),
                        Value::Uint8(100),
                        Value::Uint128(1000),
                    ])
                    .into(),
                    debug_name: Some("debug_name".into()),
                },
                builtin_stats: Default::default(),
            })
            .unwrap(),
            RunResultValue::Success(vec![
                Felt::from(34),
                Felt::from(42),
                Felt::from(100),
                Felt::from(1000)
            ])
        );
    }

    #[test]
    fn test_result_to_runresult_success() {
        // Tests the conversion of a success enum result to a `RunResultValue::Success`.
        assert_eq!(
            result_to_runresult(&ExecutionResult {
                remaining_gas: None,
                return_value: Value::Enum {
                    tag: 0,
                    value: Value::Uint64(24).into(),
                    debug_name: Some("core::panics::PanicResult::Test".into()),
                },
                builtin_stats: Default::default(),
            })
            .unwrap(),
            RunResultValue::Success(vec![Felt::from(24)])
        );
    }

    #[test]
    #[should_panic(expected = "unsuported return value in cairo-native")]
    fn test_result_to_runresult_panic() {
        // Tests the conversion with unsuported return value.
        let _ = result_to_runresult(&ExecutionResult {
            remaining_gas: None,
            return_value: Value::Enum {
                tag: 10,
                value: Value::Uint64(24).into(),
                debug_name: Some("core::panics::PanicResult::Test".into()),
            },
            builtin_stats: Default::default(),
        })
        .unwrap();
    }

    #[test]
    #[should_panic(expected = "missing debug name")]
    fn test_result_to_runresult_missing_debug_name() {
        // Tests the conversion with no debug name.
        let _ = result_to_runresult(&ExecutionResult {
            remaining_gas: None,
            return_value: Value::Enum {
                tag: 10,
                value: Value::Uint64(24).into(),
                debug_name: None,
            },
            builtin_stats: Default::default(),
        })
        .unwrap();
    }

    #[test]
    fn test_result_to_runresult_return() {
        // Tests the conversion of a panic enum result with non-zero tag to a `RunResultValue::Panic`.
        assert_eq!(
            result_to_runresult(&ExecutionResult {
                remaining_gas: None,
                return_value: Value::Enum {
                    tag: 10,
                    value: Value::Struct {
                        fields: vec![
                            Value::Felt252(42.into()),
                            Value::Uint8(100),
                            Value::Uint128(1000),
                        ],
                        debug_name: Some("debug_name".into()),
                    }
                    .into(),
                    debug_name: Some("core::panics::PanicResult::Test".into()),
                },
                builtin_stats: Default::default(),
            })
            .unwrap(),
            RunResultValue::Panic(vec![Felt::from(42), Felt::from(100), Felt::from(1000)])
        );
    }

    #[test]
    fn test_result_to_runresult_non_enum() {
        // Tests the conversion of a non-enum result to a `RunResultValue::Success`.
        assert_eq!(
            result_to_runresult(&ExecutionResult {
                remaining_gas: None,
                return_value: Value::Uint8(10),
                builtin_stats: Default::default(),
            })
            .unwrap(),
            RunResultValue::Success(vec![Felt::from(10)])
        );
    }

    #[test]
    fn test_jitvalue_to_felt_felt252() {
        let felt_value: Felt = 42.into();

        assert_eq!(
            jitvalue_to_felt(&Value::Felt252(felt_value)),
            vec![felt_value]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_array() {
        assert_eq!(
            jitvalue_to_felt(&Value::Array(vec![
                Value::Felt252(42.into()),
                Value::Uint8(100),
                Value::Uint128(1000),
            ])),
            vec![Felt::from(42), Felt::from(100), Felt::from(1000)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_struct() {
        assert_eq!(
            jitvalue_to_felt(&Value::Struct {
                fields: vec![
                    Value::Felt252(42.into()),
                    Value::Uint8(100),
                    Value::Uint128(1000)
                ],
                debug_name: Some("debug_name".into())
            }),
            vec![Felt::from(42), Felt::from(100), Felt::from(1000)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_enum() {
        // With debug name
        assert_eq!(
            jitvalue_to_felt(&Value::Enum {
                tag: 34,
                value: Value::Array(vec![
                    Value::Felt252(42.into()),
                    Value::Uint8(100),
                    Value::Uint128(1000),
                ])
                .into(),
                debug_name: Some("debug_name".into())
            }),
            vec![
                Felt::from(34),
                Felt::from(42),
                Felt::from(100),
                Felt::from(1000)
            ]
        );

        // With core::bool debug name and tag 1
        assert_eq!(
            jitvalue_to_felt(&Value::Enum {
                tag: 1,
                value: Value::Uint128(1000).into(),
                debug_name: Some("core::bool".into())
            }),
            vec![Felt::ONE]
        );

        // With core::bool debug name and tag not 1
        assert_eq!(
            jitvalue_to_felt(&Value::Enum {
                tag: 10,
                value: Value::Uint128(1000).into(),
                debug_name: Some("core::bool".into())
            }),
            vec![Felt::ZERO]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_u8() {
        assert_eq!(jitvalue_to_felt(&Value::Uint8(10)), vec![Felt::from(10)]);
    }

    #[test]
    fn test_jitvalue_to_felt_u16() {
        assert_eq!(jitvalue_to_felt(&Value::Uint16(100)), vec![Felt::from(100)]);
    }

    #[test]
    fn test_jitvalue_to_felt_u32() {
        assert_eq!(
            jitvalue_to_felt(&Value::Uint32(1000)),
            vec![Felt::from(1000)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_u64() {
        assert_eq!(
            jitvalue_to_felt(&Value::Uint64(10000)),
            vec![Felt::from(10000)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_u128() {
        assert_eq!(
            jitvalue_to_felt(&Value::Uint128(100000)),
            vec![Felt::from(100000)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_sint8() {
        assert_eq!(jitvalue_to_felt(&Value::Sint8(-10)), vec![Felt::from(-10)]);
    }

    #[test]
    fn test_jitvalue_to_felt_sint16() {
        assert_eq!(
            jitvalue_to_felt(&Value::Sint16(-100)),
            vec![Felt::from(-100)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_sint32() {
        assert_eq!(
            jitvalue_to_felt(&Value::Sint32(-1000)),
            vec![Felt::from(-1000)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_sint64() {
        assert_eq!(
            jitvalue_to_felt(&Value::Sint64(-10000)),
            vec![Felt::from(-10000)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_sint128() {
        assert_eq!(
            jitvalue_to_felt(&Value::Sint128(-100000)),
            vec![Felt::from(-100000)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_null() {
        assert_eq!(jitvalue_to_felt(&Value::Null), vec![Felt::ZERO]);
    }

    #[test]
    fn test_jitvalue_to_felt_felt252_dict() {
        let result = jitvalue_to_felt(&Value::Felt252Dict {
            value: HashMap::from([
                (Felt::ONE, Value::Felt252(Felt::from(101))),
                (Felt::TWO, Value::Felt252(Felt::from(102))),
            ]),
            debug_name: None,
        });

        let first_dict_entry = vec![Felt::from(1), Felt::from(101)];
        let second_dict_entry = vec![Felt::from(2), Felt::from(102)];

        // Check that the two Key, value pairs are in the result
        assert!(is_subsequence(&first_dict_entry, &result));
        assert!(is_subsequence(&second_dict_entry, &result));
    }

    #[test]
    fn test_jitvalue_to_felt_felt252_dict_with_array() {
        let result = jitvalue_to_felt(&Value::Felt252Dict {
            value: HashMap::from([
                (
                    Felt::ONE,
                    Value::Array(Vec::from([
                        Value::Felt252(Felt::from(101)),
                        Value::Felt252(Felt::from(102)),
                    ])),
                ),
                (
                    Felt::TWO,
                    Value::Array(Vec::from([
                        Value::Felt252(Felt::from(201)),
                        Value::Felt252(Felt::from(202)),
                    ])),
                ),
            ]),
            debug_name: None,
        });

        let first_dict_entry = vec![Felt::from(1), Felt::from(101), Felt::from(102)];
        let second_dict_entry = vec![Felt::from(2), Felt::from(201), Felt::from(202)];

        // Check that the two Key, value pairs are in the result
        assert!(is_subsequence(&first_dict_entry, &result));
        assert!(is_subsequence(&second_dict_entry, &result));
    }
    #[test]
    fn test_jitvalue_to_felt_ec_point() {
        assert_eq!(
            jitvalue_to_felt(&Value::EcPoint(Felt::ONE, Felt::TWO,)),
            vec![Felt::ONE, Felt::TWO,]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_ec_state() {
        assert_eq!(
            jitvalue_to_felt(&Value::EcState(Felt::ONE, Felt::TWO,)),
            vec![Felt::ONE, Felt::TWO,]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_secp256_k1_point() {
        assert_eq!(
            jitvalue_to_felt(&Value::Secp256K1Point(Secp256k1Point::new(
                1, 2, 3, 4, false
            ))),
            vec![Felt::ONE, Felt::TWO, Felt::THREE, Felt::from(4)]
        );
    }

    #[test]
    fn test_jitvalue_to_felt_secp256_r1_point() {
        assert_eq!(
            jitvalue_to_felt(&Value::Secp256R1Point(Secp256r1Point::new(
                1, 2, 3, 4, false
            ))),
            vec![Felt::ONE, Felt::TWO, Felt::THREE, Felt::from(4)]
        );
    }
}
