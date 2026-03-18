//! This module contains common code used by all integration tests, which use proptest to compare various outputs based on the inputs
//! The general idea is to have a test for each libfunc if possible.

#![allow(dead_code)]

use ark_ff::One;
use cairo_lang_compiler::{
    compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig,
};
use cairo_lang_filesystem::{db::init_dev_corelib, ids::CrateInput};
use cairo_lang_runner::{
    Arg, RunResultStarknet, RunResultValue, RunnerError, SierraCasmRunner, StarknetState,
};
use cairo_lang_sierra::{
    extensions::{
        circuit::CircuitTypeConcrete,
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        starknet::StarknetTypeConcrete,
        utils::Range,
        ConcreteType,
    },
    ids::{ConcreteTypeId, FunctionId},
    program::Program,
    program_registry::ProgramRegistry,
};
use cairo_lang_sierra_generator::replace_ids::{DebugReplacer, SierraIdReplacer};
use cairo_lang_starknet::{
    compile::compile_contract_in_prepared_db,
    contract::{find_contracts, get_contracts_info},
    starknet_plugin_suite,
};
use cairo_lang_starknet_classes::{
    casm_contract_class::ENTRY_POINT_COST, contract_class::ContractClass,
};
use cairo_native::{
    context::NativeContext,
    execution_result::{ContractExecutionResult, ExecutionResult},
    executor::{AotContractExecutor, JitNativeExecutor},
    starknet::{DummySyscallHandler, StarknetSyscallHandler},
    utils::{find_entry_point_by_idx, testing::load_program_and_runner, HALF_PRIME, PRIME},
    OptLevel, Value,
};
use lambdaworks_math::{
    field::{
        element::FieldElement, fields::montgomery_backed_prime_fields::MontgomeryBackendPrimeField,
    },
    unsigned_integer::element::UnsignedInteger,
};
use num_bigint::{BigInt, BigUint, Sign};
use pretty_assertions_sorted::assert_eq_sorted;
use proptest::{strategy::Strategy, test_runner::TestCaseError};
use starknet_types_core::felt::Felt;
use std::{collections::HashMap, env::var, fs, ops::Neg, path::Path};

#[allow(unused_macros)]
macro_rules! load_cairo {
    ( $( $program:tt )+ ) => {
        $crate::common::load_cairo_str(stringify!($($program)+))
    };
}

#[allow(unused_imports)]
pub(crate) use load_cairo;
use num_traits::ToPrimitive;

pub const DEFAULT_GAS: u64 = u64::MAX;

// Parse numeric string into felt, wrapping negatives around the prime modulo.
pub fn felt(value: &str) -> [u32; 8] {
    let value = value.parse::<BigInt>().unwrap();
    let value = match value.sign() {
        Sign::Minus => &*PRIME - value.neg().to_biguint().unwrap(),
        _ => value.to_biguint().unwrap(),
    };

    let mut u32_digits = value.to_u32_digits();
    u32_digits.resize(8, 0);
    u32_digits.try_into().unwrap()
}

/// Parse any type that can be a bigint to a felt that can be used in the cairo-native input.
pub fn feltn(value: impl Into<BigInt>) -> [u32; 8] {
    let value: BigInt = value.into();
    let value = match value.sign() {
        Sign::Minus => &*PRIME - value.neg().to_biguint().unwrap(),
        _ => value.to_biguint().unwrap(),
    };

    let mut u32_digits = value.to_u32_digits();
    u32_digits.resize(8, 0);
    u32_digits.try_into().unwrap()
}

/// Converts a casm variant to sierra.
pub const fn casm_variant_to_sierra(idx: i64, num_variants: i64) -> i64 {
    num_variants - 1 - (idx >> 1)
}

pub fn get_run_result(r: &RunResultValue) -> Vec<String> {
    match r {
        RunResultValue::Success(x) | RunResultValue::Panic(x) => {
            x.iter().map(ToString::to_string).collect()
        }
    }
}

pub fn load_cairo_str(program_str: &str) -> (String, Program, SierraCasmRunner) {
    let mut program_file = tempfile::Builder::new()
        .prefix("test_")
        .suffix(".cairo")
        .tempfile()
        .unwrap();
    fs::write(&mut program_file, program_str).unwrap();

    let mut db = RootDatabase::builder().detect_corelib().build().unwrap();
    let main_crate_ids = {
        let main_crate_inputs =
            setup_project(&mut db, program_file.as_ref()).expect("failed to setup project");
        CrateInput::into_crate_ids(&db, main_crate_inputs)
    };
    let sierra_program_with_dbg = compile_prepared_db(
        &db,
        main_crate_ids.clone(),
        CompilerConfig {
            replace_ids: true,
            ..Default::default()
        },
    )
    .unwrap();
    let program = sierra_program_with_dbg.program;

    let module_name = program_file.path().with_extension("");
    let module_name = module_name.file_name().unwrap().to_str().unwrap();

    let replacer = DebugReplacer { db: &db };

    let contracts = find_contracts(&db, &main_crate_ids);
    let contracts_info = get_contracts_info(&db, contracts, &replacer).unwrap();

    let runner = SierraCasmRunner::new(
        program.clone(),
        Some(Default::default()),
        contracts_info,
        None,
    )
    .unwrap();

    (module_name.to_string(), program, runner)
}

pub fn load_cairo_path(program_path: &str) -> (String, Program, SierraCasmRunner) {
    let program_file = Path::new(program_path);

    let mut db = RootDatabase::default();
    init_dev_corelib(
        &mut db,
        Path::new(&var("CARGO_MANIFEST_DIR").unwrap()).join("corelib/src"),
    );
    let main_crate_ids = {
        let main_crate_inputs =
            setup_project(&mut db, program_file).expect("failed to setup project");
        CrateInput::into_crate_ids(&db, main_crate_inputs)
    };
    let sierra_program_with_dbg = compile_prepared_db(
        &db,
        main_crate_ids.clone(),
        CompilerConfig {
            replace_ids: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut program = sierra_program_with_dbg.program;

    let module_name = program_file.with_extension("");
    let module_name = module_name.file_name().unwrap().to_str().unwrap();

    let replacer = DebugReplacer { db: &db };
    replacer.enrich_function_names(&mut program);
    let contracts = find_contracts(&db, &main_crate_ids);
    let contracts_info = get_contracts_info(&db, contracts, &replacer).unwrap();

    let program = replacer.apply(&program);

    let runner = SierraCasmRunner::new(
        program.clone(),
        Some(Default::default()),
        contracts_info,
        None,
    )
    .unwrap();

    (module_name.to_string(), program, runner)
}

/// Compiles a cairo starknet contract from the given path
pub fn load_cairo_contract_path(path: &str) -> ContractClass {
    let mut db = RootDatabase::builder()
        .detect_corelib()
        .with_default_plugin_suite(starknet_plugin_suite())
        .build()
        .expect("failed to build database");

    let main_crate_ids = {
        let main_crate_inputs = setup_project(&mut db, path.as_ref())
            .expect("path should be a valid cairo project or file");
        CrateInput::into_crate_ids(&db, main_crate_inputs)
    };

    compile_contract_in_prepared_db(
        &db,
        None,
        main_crate_ids.clone(),
        CompilerConfig {
            replace_ids: true,
            ..Default::default()
        },
    )
    .expect("path should contain a single valid contract")
}

pub fn run_native_program(
    program: &(String, Program, SierraCasmRunner),
    entry_point: &str,
    args: &[Value],
    gas: Option<u64>,
    syscall_handler: Option<impl StarknetSyscallHandler>,
) -> ExecutionResult {
    let entry_point = format!("{0}::{0}::{1}", program.0, entry_point);
    let program = &program.1;

    let entry_point_id = &program
        .funcs
        .iter()
        .find(|x| x.id.debug_name.as_deref() == Some(&entry_point))
        .expect("Test program entry point not found.")
        .id;

    let context = NativeContext::new();

    let module = context
        .compile(program, false, Some(Default::default()), None)
        .expect("Could not compile test program to MLIR.");

    assert!(
        module.module().as_operation().verify(),
        "Test program generated invalid MLIR:\n{}",
        module.module().as_operation()
    );

    // FIXME: There are some bugs with non-zero LLVM optimization levels.
    let executor = JitNativeExecutor::from_native_module(module, OptLevel::None).unwrap();
    match syscall_handler {
        Some(syscall_handler) => executor
            .invoke_dynamic_with_syscall_handler(entry_point_id, args, gas, syscall_handler)
            .unwrap(),
        None => executor.invoke_dynamic(entry_point_id, args, gas).unwrap(),
    }
}

/// Runs the program on the cairo-vm
pub fn run_vm_program(
    program: &(String, Program, SierraCasmRunner),
    entry_point: &str,
    args: Vec<Arg>,
    gas: Option<usize>,
) -> Result<RunResultStarknet, RunnerError> {
    let runner = &program.2;
    runner.run_function_with_starknet_context(
        runner.find_function(entry_point).unwrap(),
        args,
        gas,
        StarknetState::default(),
    )
}

/// Runs a contract on the cairo-vm via SierraCasmRunner (from cairo-lang-runner).
///
/// This is used for cross-validation: comparing the VM output against the native output
/// to ensure correctness.
pub fn run_vm_contract(
    cairo_contract: &ContractClass,
    selector: &BigUint,
    args: &[Felt],
) -> Vec<Felt> {
    let extracted = cairo_contract
        .extract_sierra_program(true)
        .expect("failed to extract sierra program from contract");
    let mut program = extracted.program;

    // Workaround: populate UserType debug names in generic_args.
    // The library's `DebugInfo::populate` skips `GenericArg::UserType`, but
    // `SierraCasmRunner::inner_type_from_panic_wrapper` assumes they are set.
    // In Sierra, `Enum<ut@TypeName, ...>` / `Struct<ut@TypeName, ...>` means
    // the UserType name matches the parent type declaration's debug name.
    {
        use cairo_lang_sierra::program::GenericArg;
        for decl in &mut program.type_declarations {
            if let Some(debug_name) = decl.id.debug_name.clone() {
                for arg in &mut decl.long_id.generic_args {
                    if let GenericArg::UserType(ut) = arg {
                        if ut.debug_name.is_none() {
                            ut.debug_name = Some(debug_name.clone());
                        }
                    }
                }
            }
        }
    }

    // Find the entry point by selector
    let entrypoint = cairo_contract
        .entry_points_by_type
        .external
        .iter()
        .find(|e| e.selector == *selector)
        .expect("entry point with given selector not found");

    // Find the function's debug name using the function index
    let func = find_entry_point_by_idx(&program, entrypoint.function_idx)
        .expect("entry point function not found in sierra program");

    let func_name = func
        .id
        .debug_name
        .as_ref()
        .expect("function should have a debug name")
        .to_string();

    let runner = SierraCasmRunner::new(program, Some(Default::default()), Default::default(), None)
        .expect("failed to create SierraCasmRunner");

    let result = runner
        .run_function_with_starknet_context(
            runner.find_function(&func_name).unwrap(),
            vec![Arg::Array(args.iter().cloned().map(Arg::Value).collect())],
            Some(usize::MAX),
            StarknetState::default(),
        )
        .expect("failed to run contract on VM");

    match result.value {
        RunResultValue::Success(values) => {
            // The VM returns raw values for the inner type of PanicResult.
            // For contracts, the inner type is (Span<felt252>,) which is represented
            // as two felt252 values: (start_ptr, end_ptr) into VM memory.
            // We need to dereference these pointers to get the actual array elements.
            assert!(
                values.len() == 2,
                "expected Span<felt252> (2 values: start, end), got {} values",
                values.len()
            );
            let start = values[0]
                .to_usize()
                .expect("span start pointer should fit in usize");
            let end = values[1]
                .to_usize()
                .expect("span end pointer should fit in usize");

            result.memory[start..end]
                .iter()
                .map(|cell| cell.expect("memory cell in span range should be initialized"))
                .collect()
        }
        RunResultValue::Panic(values) => {
            panic!("VM contract execution panicked: {:?}", values)
        }
    }
}

pub fn compare_inputless_program(program_path: &str) {
    let program: (String, Program, SierraCasmRunner) = load_program_and_runner(program_path);

    let result_vm = run_vm_program(&program, "main", vec![], Some(DEFAULT_GAS as usize)).unwrap();
    let result_native = run_native_program(
        &program,
        "main",
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("main").unwrap().id,
        &result_vm,
        &result_native,
    )
    .expect("compare error");
}

/// Runs the program using cairo-native JIT.
pub fn run_native_starknet_contract(
    sierra_program: &Program,
    entry_point_function_idx: usize,
    args: &[Felt],
    handler: impl StarknetSyscallHandler,
) -> ContractExecutionResult {
    let native_context = NativeContext::new();

    let native_program = native_context
        .compile(sierra_program, false, Some(Default::default()), None)
        .unwrap();

    let entry_point_fn = find_entry_point_by_idx(sierra_program, entry_point_function_idx).unwrap();
    let entry_point_id = &entry_point_fn.id;

    let native_executor =
        JitNativeExecutor::from_native_module(native_program, Default::default()).unwrap();
    native_executor
        .invoke_contract_dynamic(entry_point_id, args, u64::MAX.into(), handler)
        .expect("failed to execute the given contract")
}

pub fn run_native_starknet_aot_contract(
    contract: &ContractClass,
    selector: &BigUint,
    args: &[Felt],
    handler: impl StarknetSyscallHandler,
) -> ContractExecutionResult {
    let extracted = contract.extract_sierra_program(false).unwrap();
    let native_executor = AotContractExecutor::new(
        &extracted.program,
        &contract.entry_points_by_type,
        extracted.sierra_version,
        Default::default(),
        None,
    )
    .unwrap();
    native_executor
        // substract ENTRY_POINT_COST so gas matches
        .run(
            Felt::from(selector),
            args,
            u64::MAX - ENTRY_POINT_COST as u64,
            None,
            handler,
        )
        .expect("failed to execute the given contract")
}

/// Given the result of the cairo-vm and cairo-native of the same program, it compares
/// the results automatically, triggering a proptest assert if there is a mismatch.
///
/// Left of report of the assert is the cairo vm result, right side is cairo native
pub fn compare_outputs(
    program: &Program,
    entry_point: &FunctionId,
    vm_result: &RunResultStarknet,
    native_result: &ExecutionResult,
) -> Result<(), TestCaseError> {
    let registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(program).unwrap();
    let function = registry.get_function(entry_point).unwrap();

    fn map_vm_sizes(
        size_cache: &mut HashMap<ConcreteTypeId, usize>,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
        ty: &ConcreteTypeId,
    ) -> usize {
        match size_cache.get(ty) {
            Some(&type_size) => type_size,
            None => {
                let type_size = match registry.get_type(ty).unwrap() {
                    CoreTypeConcrete::Array(_) | CoreTypeConcrete::EcPoint(_) => 2,
                    CoreTypeConcrete::Felt252(_)
                    | CoreTypeConcrete::Uint128(_)
                    | CoreTypeConcrete::Uint64(_)
                    | CoreTypeConcrete::Uint32(_)
                    | CoreTypeConcrete::Uint16(_)
                    | CoreTypeConcrete::Uint8(_)
                    | CoreTypeConcrete::Sint128(_)
                    | CoreTypeConcrete::Sint64(_)
                    | CoreTypeConcrete::Sint32(_)
                    | CoreTypeConcrete::Sint16(_)
                    | CoreTypeConcrete::Sint8(_)
                    | CoreTypeConcrete::Box(_)
                    | CoreTypeConcrete::BoundedInt(_)
                    | CoreTypeConcrete::Circuit(CircuitTypeConcrete::U96Guarantee(_))
                    | CoreTypeConcrete::Nullable(_) => 1,
                    CoreTypeConcrete::Enum(info) => {
                        1 + info
                            .variants
                            .iter()
                            .map(|variant_ty| map_vm_sizes(size_cache, registry, variant_ty))
                            .max()
                            .unwrap_or_default()
                    }
                    CoreTypeConcrete::Struct(info) => info
                        .members
                        .iter()
                        .map(|member_ty| map_vm_sizes(size_cache, registry, member_ty))
                        .sum(),
                    CoreTypeConcrete::NonZero(info) => map_vm_sizes(size_cache, registry, &info.ty),
                    CoreTypeConcrete::EcState(_) => 4,
                    CoreTypeConcrete::Snapshot(info) => {
                        map_vm_sizes(size_cache, registry, &info.ty)
                    }
                    CoreTypeConcrete::SquashedFelt252Dict(_) => 2,
                    x => todo!("vm size not yet implemented: {:?}", x.info()),
                };
                size_cache.insert(ty.clone(), type_size);

                type_size
            }
        }
    }

    fn map_vm_values(
        size_cache: &mut HashMap<ConcreteTypeId, usize>,
        registry: &ProgramRegistry<CoreType, CoreLibfunc>,
        memory: &[Option<Felt>],
        mut values: &[Felt],
        ty: &ConcreteTypeId,
    ) -> Value {
        match registry.get_type(ty).unwrap() {
            CoreTypeConcrete::Array(info) => {
                assert_eq!(values.len(), 2);
                let since_ptr = values[0].to_usize().unwrap();
                let until_ptr = values[1].to_usize().unwrap();

                let total_len = until_ptr - since_ptr;
                let elem_size = map_vm_sizes(size_cache, registry, &info.ty);
                assert_eq!(total_len % elem_size, 0);

                Value::Array(
                    memory[since_ptr..until_ptr]
                        .chunks(elem_size)
                        .map(|data| data.iter().cloned().map(Option::unwrap).collect::<Vec<_>>())
                        .map(|data| map_vm_values(size_cache, registry, memory, &data, &info.ty))
                        .collect(),
                )
            }
            CoreTypeConcrete::Felt252(_) => {
                Value::Felt252(Felt::from_bytes_le(&values[0].to_bytes_le()))
            }
            CoreTypeConcrete::Uint128(_) => Value::Uint128(values[0].to_u128().unwrap()),
            CoreTypeConcrete::Uint64(_) => Value::Uint64(values[0].to_u64().unwrap()),
            CoreTypeConcrete::Uint32(_) => Value::Uint32(values[0].to_u32().unwrap()),
            CoreTypeConcrete::Uint16(_) => Value::Uint16(values[0].to_u16().unwrap()),
            CoreTypeConcrete::Uint8(_) => Value::Uint8(values[0].to_u8().unwrap()),
            CoreTypeConcrete::Sint128(_) => {
                Value::Sint128(if values[0].to_biguint() >= *HALF_PRIME {
                    -(&*PRIME - &values[0].to_biguint()).to_i128().unwrap()
                } else {
                    values[0].to_biguint().to_i128().unwrap()
                })
            }
            CoreTypeConcrete::Sint64(_) => {
                Value::Sint64(if values[0].to_biguint() >= *HALF_PRIME {
                    -(&*PRIME - &values[0].to_biguint()).to_i64().unwrap()
                } else {
                    values[0].to_biguint().to_i64().unwrap()
                })
            }
            CoreTypeConcrete::Sint32(_) => {
                Value::Sint32(if values[0].to_biguint() >= *HALF_PRIME {
                    -(&*PRIME - &values[0].to_biguint()).to_i32().unwrap()
                } else {
                    values[0].to_biguint().to_i32().unwrap()
                })
            }
            CoreTypeConcrete::Sint16(_) => {
                Value::Sint16(if values[0].to_biguint() >= *HALF_PRIME {
                    -(&*PRIME - &values[0].to_biguint()).to_i16().unwrap()
                } else {
                    values[0].to_biguint().to_i16().unwrap()
                })
            }
            CoreTypeConcrete::Sint8(_) => Value::Sint8(if values[0].to_biguint() >= *HALF_PRIME {
                -(&*PRIME - &values[0].to_biguint()).to_i8().unwrap()
            } else {
                values[0].to_biguint().to_i8().unwrap()
            }),
            CoreTypeConcrete::BoundedInt(info) => Value::BoundedInt {
                value: values[0],
                range: info.range.clone(),
            },
            CoreTypeConcrete::Circuit(CircuitTypeConcrete::U96Guarantee(_)) => Value::BoundedInt {
                value: values[0],
                range: Range {
                    lower: BigInt::ZERO,
                    upper: BigInt::one() << 96,
                },
            },
            CoreTypeConcrete::Enum(info) => {
                let enum_size = map_vm_sizes(size_cache, registry, ty);
                assert_eq!(values.len(), enum_size);

                let (tag, data);
                (tag, values) = values.split_first().unwrap();

                let mut tag = tag.to_usize().unwrap();
                if info.variants.len() > 2 {
                    tag = info.variants.len() - ((tag + 1) >> 1);
                }
                assert!(tag <= info.variants.len());
                data = &values[enum_size - size_cache[&info.variants[tag]] - 1..];

                Value::Enum {
                    tag,
                    value: Box::new(map_vm_values(
                        size_cache,
                        registry,
                        memory,
                        data,
                        &info.variants[tag],
                    )),
                    debug_name: ty.debug_name.as_deref().map(String::from),
                }
            }
            CoreTypeConcrete::Struct(info) => Value::Struct {
                fields: info
                    .members
                    .iter()
                    .map(|member_ty| {
                        let data;
                        (data, values) =
                            values.split_at(map_vm_sizes(size_cache, registry, member_ty));

                        map_vm_values(size_cache, registry, memory, data, member_ty)
                    })
                    .collect(),
                debug_name: ty.debug_name.as_deref().map(String::from),
            },
            CoreTypeConcrete::SquashedFelt252Dict(info) => Value::Felt252Dict {
                value: (values[0].to_usize().unwrap()..values[1].to_usize().unwrap())
                    .step_by(3)
                    .map(|index| {
                        (
                            Felt::from_bytes_le(&memory[index].unwrap().to_bytes_le()),
                            match &info.info.long_id.generic_args[0] {
                                cairo_lang_sierra::program::GenericArg::Type(ty) => map_vm_values(
                                    size_cache,
                                    registry,
                                    memory,
                                    &[memory[index + 2].unwrap()],
                                    ty,
                                ),
                                _ => unimplemented!("unsupported dict value type"),
                            },
                        )
                    })
                    .collect(),
                debug_name: ty.debug_name.as_deref().map(String::from),
            },
            CoreTypeConcrete::Snapshot(info) => {
                map_vm_values(size_cache, registry, memory, values, &info.ty)
            }
            CoreTypeConcrete::Nullable(info) => {
                assert_eq!(values.len(), 1);

                let ty_size = map_vm_sizes(size_cache, registry, &info.ty);
                match values[0].to_usize().unwrap() {
                    0 => Value::Null,
                    ptr if ty_size == 0 => {
                        assert_eq!(ptr, 1);
                        map_vm_values(size_cache, registry, memory, &[], &info.ty)
                    }
                    ptr => map_vm_values(
                        size_cache,
                        registry,
                        memory,
                        &memory[ptr..ptr + ty_size]
                            .iter()
                            .cloned()
                            .map(Option::unwrap)
                            .collect::<Vec<_>>(),
                        &info.ty,
                    ),
                }
            }
            CoreTypeConcrete::Box(info) => {
                assert_eq!(values.len(), 1);

                let ty_size = map_vm_sizes(size_cache, registry, &info.ty);
                match values[0].to_usize().unwrap() {
                    ptr if ty_size == 0 => {
                        assert_eq!(ptr, 1);
                        map_vm_values(size_cache, registry, memory, &[], &info.ty)
                    }
                    ptr => map_vm_values(
                        size_cache,
                        registry,
                        memory,
                        &memory[ptr..ptr + ty_size]
                            .iter()
                            .cloned()
                            .map(Option::unwrap)
                            .collect::<Vec<_>>(),
                        &info.ty,
                    ),
                }
            }
            CoreTypeConcrete::NonZero(info) => {
                map_vm_values(size_cache, registry, memory, values, &info.ty)
            }
            CoreTypeConcrete::EcPoint(_) => {
                assert_eq!(values.len(), 2);

                Value::EcPoint(
                    Felt::from_bytes_le(&values[0].to_bytes_le()),
                    Felt::from_bytes_le(&values[1].to_bytes_le()),
                )
            }
            CoreTypeConcrete::EcState(_) => {
                assert_eq!(values.len(), 4);

                Value::EcState(
                    Felt::from_bytes_le(&values[0].to_bytes_le()),
                    Felt::from_bytes_le(&values[1].to_bytes_le()),
                    Felt::from_bytes_le(&values[2].to_bytes_le()),
                    Felt::from_bytes_le(&values[3].to_bytes_le()),
                )
            }
            CoreTypeConcrete::Bytes31(_) => {
                let mut bytes = values[0].to_bytes_le().to_vec();
                bytes.pop();
                Value::Bytes31(bytes.try_into().unwrap())
            }
            CoreTypeConcrete::Coupon(_) => todo!(),
            CoreTypeConcrete::Bitwise(_) => unreachable!(),
            CoreTypeConcrete::Const(_) => unreachable!(),
            CoreTypeConcrete::EcOp(_) => unreachable!(),
            CoreTypeConcrete::GasBuiltin(_) => unreachable!(),
            CoreTypeConcrete::BuiltinCosts(_) => unreachable!(),
            CoreTypeConcrete::RangeCheck(_) => unreachable!(),
            CoreTypeConcrete::Pedersen(_) => unreachable!(),
            CoreTypeConcrete::Poseidon(_) => unreachable!(),
            CoreTypeConcrete::SegmentArena(_) => unreachable!(),
            x => {
                todo!("vm value not yet implemented: {:?}", x.info())
            }
        }
    }

    let mut size_cache = HashMap::new();
    let ty = function.signature.ret_types.last();
    let is_builtin = ty.is_some_and(|ty| {
        matches!(
            registry.get_type(ty).unwrap(),
            CoreTypeConcrete::Bitwise(_)
                | CoreTypeConcrete::EcOp(_)
                | CoreTypeConcrete::GasBuiltin(_)
                | CoreTypeConcrete::BuiltinCosts(_)
                | CoreTypeConcrete::RangeCheck(_)
                | CoreTypeConcrete::RangeCheck96(_)
                | CoreTypeConcrete::Pedersen(_)
                | CoreTypeConcrete::Poseidon(_)
                | CoreTypeConcrete::Coupon(_)
                | CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_))
                | CoreTypeConcrete::SegmentArena(_)
                | CoreTypeConcrete::Circuit(CircuitTypeConcrete::AddMod(_))
                | CoreTypeConcrete::Circuit(CircuitTypeConcrete::MulMod(_))
        )
    });
    let returns_panic = ty.is_some_and(|ty| {
        ty.debug_name
            .as_ref()
            .map(|x| x.starts_with("core::panics::PanicResult"))
            .unwrap_or(false)
    });
    assert_eq!(
        vm_result
            .gas_counter
            .unwrap_or_else(|| Felt::from(0))
            .to_bigint(),
        Felt::from(native_result.remaining_gas.unwrap_or(0)).to_bigint(),
        "gas mismatch"
    );

    let native_builtins = {
        let mut native_builtins = HashMap::new();
        native_builtins.insert("range_check", native_result.builtin_stats.range_check);
        native_builtins.insert("pedersen", native_result.builtin_stats.pedersen);
        native_builtins.insert("bitwise", native_result.builtin_stats.bitwise);
        native_builtins.insert("ec_op", native_result.builtin_stats.ec_op);
        native_builtins.insert("poseidon", native_result.builtin_stats.poseidon);
        // don't include the segment arena builtin, as its not included in the VM output either.
        native_builtins.insert("range_check96", native_result.builtin_stats.range_check96);
        native_builtins.insert("add_mod", native_result.builtin_stats.add_mod);
        native_builtins.insert("mul_mod", native_result.builtin_stats.mul_mod);
        // Note: blake is not included here because the VM tracks it as an opcode
        // (OpcodeExtension::Blake), not as a builtin in builtin_instance_counter.
        // Blake counter accuracy is validated separately in test_blake_builtin_counter.
        native_builtins.retain(|_, &mut v| v != 0);
        native_builtins
    };

    let vm_builtins: HashMap<&str, usize> = vm_result
        .used_resources
        .basic_resources
        .filter_unused_builtins()
        .builtin_instance_counter
        .iter()
        .map(|(k, v)| (k.to_str(), *v))
        .collect();

    assert_eq_sorted!(vm_builtins, native_builtins, "builtin mismatch",);

    let vm_result = match &vm_result.value {
        RunResultValue::Success(values) if !values.is_empty() | returns_panic => {
            if returns_panic {
                let inner_ty = match registry.get_type(ty.unwrap())? {
                    CoreTypeConcrete::Enum(info) => &info.variants[0],
                    _ => unreachable!(),
                };
                Value::Enum {
                    tag: 0,
                    value: Box::new(map_vm_values(
                        &mut size_cache,
                        &registry,
                        &vm_result.memory,
                        values,
                        inner_ty,
                    )),
                    debug_name: None,
                }
            } else if !is_builtin {
                map_vm_values(
                    &mut size_cache,
                    &registry,
                    &vm_result.memory,
                    values,
                    ty.unwrap(),
                )
            } else {
                Value::Struct {
                    fields: Vec::new(),
                    debug_name: None,
                }
            }
        }
        RunResultValue::Panic(values) => Value::Enum {
            tag: 1,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: Vec::new(),
                        debug_name: None,
                    },
                    Value::Array(
                        values
                            .iter()
                            .map(|value| Felt::from_bytes_le(&value.to_bytes_le()))
                            .map(Value::Felt252)
                            .collect(),
                    ),
                ],
                debug_name: None,
            }),
            debug_name: None,
        },
        _ => Value::Struct {
            fields: vec![],
            debug_name: None,
        },
    };

    pretty_assertions_sorted::assert_eq!(
        native_result.return_value,
        vm_result,
        "return value mismatch"
    );
    Ok(())
}

pub const FIELD_HIGH: u128 = (1 << 123) + (17 << 64); // this is equal to 10633823966279327296825105735305134080
pub const FIELD_LOW: u128 = 1;

/// Returns a [`Strategy`] that generates any valid Felt
pub fn any_felt() -> impl Strategy<Value = Felt> {
    use proptest::prelude::*;

    (0..=FIELD_HIGH)
        // turn range into `impl Strategy`
        .prop_map(|x| x)
        // choose second 128-bit limb capped by first one
        .prop_flat_map(|high| {
            let low = if high == FIELD_HIGH {
                (0..FIELD_LOW).prop_map(|x| x).sboxed()
            } else {
                any::<u128>().sboxed()
            };
            (Just(high), low)
        })
        // turn (u128, u128) into limbs array and then into Felt
        .prop_map(|(high, low)| {
            let limbs = [
                (high >> 64) as u64,
                (high & ((1 << 64) - 1)) as u64,
                (low >> 64) as u64,
                (low & ((1 << 64) - 1)) as u64,
            ];
            FieldElement::new(UnsignedInteger::from_limbs(limbs))
        })
        .prop_map(|value: FieldElement<MontgomeryBackendPrimeField<_, 4>>| {
            Felt::from_bytes_be(&value.to_bytes_be())
        })
}

/// Returns a [`Strategy`] that generates any nonzero Felt
pub fn nonzero_felt() -> impl Strategy<Value = Felt> {
    any_felt().prop_filter("is zero", |x| x != &Felt::ZERO)
}
