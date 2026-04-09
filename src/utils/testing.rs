#![cfg(any(test, feature = "testing"))]

use cairo_lang_compiler::CompilerConfig;
use cairo_lang_filesystem::{db::init_dev_corelib, ids::CrateInput};
use cairo_lang_lowering::utils::InliningStrategy;
use cairo_lang_runner::SierraCasmRunner;
use cairo_lang_sierra::{
    program::{Program, VersionedProgram},
    ProgramParser,
};
use cairo_lang_starknet::{compile::compile_contract_in_prepared_db, starknet_plugin_suite};
use itertools::Itertools;
use starknet_types_core::felt::Felt;
use std::{fs, path::Path, sync::Arc};

use crate::{
    context::NativeContext, execution_result::ExecutionResult, executor::JitNativeExecutor,
    starknet_stub::StubSyscallHandler, utils::*, values::Value,
};
use cairo_lang_compiler::{
    compile_prepared_db, db::RootDatabase, diagnostics::DiagnosticsReporter, project::setup_project,
};
use cairo_lang_starknet_classes::contract_class::ContractClass;
use std::env::var;

#[macro_export]
macro_rules! load_cairo {
        ( $( $program:tt )+ ) => {
            $crate::utils::testing::load_cairo_str(stringify!($($program)+))
        };
    }
#[macro_export]
macro_rules! load_starknet {
        ( $( $program:tt )+ ) => {
            $crate::utils::testing::load_starknet_str(stringify!($($program)+))
        };
    }
#[macro_export]
macro_rules! load_starknet_contract {
        ( $( $program:tt )+ ) => {
            $crate::utils::testing::load_starknet_contract_str(stringify!($($program)+))
        };
    }

// Helper macros for faster testing.
#[macro_export]
macro_rules! jit_struct {
        ($($y:expr),* $(,)? ) => {
            $crate::values::Value::Struct {
                fields: vec![$($y), *],
                debug_name: None
            }
        };
    }
#[macro_export]
macro_rules! jit_enum {
    ( $tag:expr, $value:expr ) => {
        $crate::values::Value::Enum {
            tag: $tag,
            value: Box::new($value),
            debug_name: None,
        }
    };
}
#[macro_export]
macro_rules! jit_dict {
        ( $($key:expr $(=>)+ $value:expr),* $(,)? ) => {
            $crate::values::Value::Felt252Dict {
                value: {
                    let mut map = std::collections::HashMap::new();
                    $(map.insert($key.into(), $value.into());)*
                    map
                },
                debug_name: None,
            }
        };
    }
#[macro_export]
macro_rules! jit_panic {
        ( $($value:expr)? ) => {
            $crate::jit_enum!(1, $crate::jit_struct!(
                $crate::jit_struct!(),
                [$($value), *].into()
            ))
        };
    }

#[macro_export]
macro_rules! jit_panic_byte_array {
    ( $value:expr ) => {
        $crate::jit_enum!(
            1,
            $crate::jit_struct!(
                $crate::jit_struct!(),
                $crate::utils::testing::panic_byte_array($value).into()
            )
        )
    };
}

/// Compile a cairo program found at the given path to sierra.
pub fn cairo_to_sierra(program: &Path) -> crate::error::Result<Arc<Program>> {
    if program
        .extension()
        .map(|x| {
            x.to_ascii_lowercase()
                .to_string_lossy()
                .eq_ignore_ascii_case("cairo")
        })
        .unwrap_or(false)
    {
        cairo_lang_compiler::compile_cairo_project_at_path(
            program,
            CompilerConfig {
                replace_ids: true,
                ..Default::default()
            },
            InliningStrategy::Default,
        )
        .map_err(|err| crate::error::Error::ProgramParser(err.to_string()))
    } else {
        let source = std::fs::read_to_string(program)?;
        ProgramParser::new()
            .parse(&source)
            .map_err(|err| crate::error::Error::ProgramParser(err.to_string()))
    }
    .map(Arc::new)
}

pub fn load_cairo_str(program_str: &str) -> (String, Program) {
    compile_program(program_str, RootDatabase::default())
}

pub fn load_starknet_str(program_str: &str) -> (String, Program) {
    compile_program(
        program_str,
        RootDatabase::builder()
            .with_default_plugin_suite(starknet_plugin_suite())
            .build()
            .unwrap(),
    )
}

pub fn load_starknet_contract_str(program_str: &str) -> (String, ContractClass) {
    compile_contract(
        program_str,
        RootDatabase::builder()
            .with_default_plugin_suite(starknet_plugin_suite())
            .build()
            .unwrap(),
    )
}

pub(crate) fn compile_contract(program_str: &str, mut db: RootDatabase) -> (String, ContractClass) {
    let mut program_file = tempfile::Builder::new()
        .prefix("test_")
        .suffix(".cairo")
        .tempfile()
        .unwrap();
    fs::write(&mut program_file, program_str).unwrap();

    init_dev_corelib(
        &mut db,
        Path::new(&var("CARGO_MANIFEST_DIR").unwrap()).join("corelib/src"),
    );
    let main_crate_ids = {
        let main_crate_inputs = setup_project(&mut db, program_file.path()).unwrap();
        CrateInput::into_crate_ids(&db, main_crate_inputs)
    };
    let contract = compile_contract_in_prepared_db(
        &db,
        None,
        main_crate_ids,
        CompilerConfig {
            diagnostics_reporter: DiagnosticsReporter::stderr(),
            replace_ids: true,
            ..Default::default()
        },
    )
    .unwrap();

    let module_name = program_file.path().with_extension("");
    let module_name = module_name.file_name().unwrap().to_str().unwrap();
    (module_name.to_string(), contract)
}

pub(crate) fn compile_program(program_str: &str, mut db: RootDatabase) -> (String, Program) {
    let mut program_file = tempfile::Builder::new()
        .prefix("test_")
        .suffix(".cairo")
        .tempfile()
        .unwrap();
    fs::write(&mut program_file, program_str).unwrap();

    init_dev_corelib(
        &mut db,
        Path::new(&var("CARGO_MANIFEST_DIR").unwrap()).join("corelib/src"),
    );
    let main_crate_ids = {
        let main_crate_inputs = setup_project(&mut db, program_file.path()).unwrap();
        CrateInput::into_crate_ids(&db, main_crate_inputs)
    };
    let sierra_program_with_dbg = compile_prepared_db(
        &db,
        main_crate_ids,
        CompilerConfig {
            diagnostics_reporter: DiagnosticsReporter::stderr(),
            replace_ids: true,
            ..Default::default()
        },
    )
    .unwrap();

    let module_name = program_file.path().with_extension("");
    let module_name = module_name.file_name().unwrap().to_str().unwrap();
    (module_name.to_string(), sierra_program_with_dbg.program)
}

pub fn run_program(
    program: &(String, Program),
    entry_point: &str,
    args: &[Value],
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

    let executor = JitNativeExecutor::from_native_module(module, OptLevel::Less).unwrap();
    executor
        .invoke_dynamic_with_syscall_handler(
            entry_point_id,
            args,
            Some(u64::MAX),
            &mut StubSyscallHandler::default(),
        )
        .unwrap()
}

#[track_caller]
pub fn run_program_assert_output(
    program: &(String, Program),
    entry_point: &str,
    args: &[Value],
    output: Value,
) {
    let result = run_program(program, entry_point, args);
    assert_eq!(result.return_value, output);
}

/// Serializes a message into a vector of felts, the same way that Cairo
/// serializes byte arrays. Used for asserting panic message on tests.
///
/// https://github.com/starkware-libs/cairo/tree/v2.12.3/corelib/src/debug.cairo#L142
pub fn panic_byte_array(message: &str) -> Vec<Felt> {
    // Prepend byte array magic, used to identify serialized `ByteArray` variables.
    // https://github.com/starkware-libs/cairo/tree/v2.12.3/corelib/src/byte_array.cairo#L64
    let mut array = vec![Felt::from_hex_unwrap(
        "0x46a6158a16a947e5916b2a2ca68501a45e93d7110e81aa2d6438b1c57c879a3",
    )];

    let chunk_iter = message.bytes().chunks(31);
    let mut chunks = chunk_iter.into_iter().collect_vec();

    // Take last word as its serialized differently.
    let pending = chunks
        .pop()
        .map(|pendign| pendign.collect_vec())
        .unwrap_or_default();

    // Serialize length of the byte array.
    array.push(chunks.len().into());

    // Serialize each byte array element.
    for chunk in chunks {
        let chunk = chunk.collect_vec();
        array.push(Felt::from_bytes_be_slice(&chunk));
    }

    // Serialize last word with its length.
    array.extend_from_slice(&[Felt::from_bytes_be_slice(&pending), pending.len().into()]);

    array
}

// TODO: Eventually this function will be changed or will be replaced in a future refactor. Right now
// so as to not make a big unrelated change we keep it like this.
pub fn load_program_and_runner(name: &str) -> (String, Program, SierraCasmRunner) {
    let program = load_program(name);
    let runner = SierraCasmRunner::new(
        program.clone(),
        Some(Default::default()),
        Default::default(),
        None,
    )
    .unwrap();
    let entrypoint = name
        .split("/")
        .collect::<Vec<&str>>()
        .last()
        .unwrap()
        .to_string();
    (entrypoint, program, runner)
}

// TODO: Same as load_program_and_runner(). Both of the functions will be changed in a rafactor.
pub fn get_compiled_program(name: &str) -> (String, Program) {
    let program_path = format!("{}/{}.sierra.json", env!("CARGO_MANIFEST_DIR"), name);
    let program_content = fs::read_to_string(program_path)
        .expect("Failed to read the content of the program into a String");
    let versioned_program =
        serde_json::from_str::<cairo_lang_sierra::program::VersionedProgram>(&program_content)
            .unwrap();
    let program = versioned_program.into_v1().unwrap().program;
    let entrypoint = name
        .split("/")
        .collect::<Vec<&str>>()
        .last()
        .unwrap()
        .to_string();
    (entrypoint, program)
}

#[macro_export]
macro_rules! include_program {
    ( $path:expr ) => {
        serde_json::from_str::<cairo_lang_sierra::program::VersionedProgram>(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/",
            $path
        )))
        .unwrap()
    };
}
#[macro_export]
macro_rules! include_contract {
    ( $path:expr ) => {
        serde_json::from_str::<cairo_lang_starknet_classes::contract_class::ContractClass>(
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $path)),
        )
        .unwrap()
    };
}

pub fn load_contract(path: &str) -> ContractClass {
    serde_json::from_str::<ContractClass>(
        &fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path)).unwrap(),
    )
    .unwrap()
}

pub fn load_program(path: &str) -> Program {
    let versioned_program = serde_json::from_str::<VersionedProgram>(
        &fs::read_to_string(format!(
            "{}/{}.sierra.json",
            env!("CARGO_MANIFEST_DIR"),
            path
        ))
        .unwrap(),
    )
    .unwrap();
    versioned_program.into_v1().unwrap().program
}
