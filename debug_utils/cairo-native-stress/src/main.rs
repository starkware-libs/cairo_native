//! A stress tester for Cairo Native
//!
//! See `StressTestCommand` for documentation on the CLI.
//!
//! ## Walkthrough
//!
//! Iterates through N rounds (specified as an argument), and in each round:
//! - Generates an unique dummy starknet contract.
//! - Compiles the program and inserts the compiled program into the cache.
//! - Executes the program.
//!
//! The cache is dropped at the end of the whole execution.
//!
//! For documentation on the specific cache used, see `NaiveAotCache`.

use cairo_lang_sierra::{
    ids::FunctionId,
    program::{GenericArg, Program},
    program_registry::ProgramRegistry,
};
use cairo_lang_starknet::compile::compile_path;
use cairo_native::{
    context::NativeContext,
    executor::AotNativeExecutor,
    metadata::gas::GasMetadata,
    module_to_object, object_to_shared_lib,
    starknet::DummySyscallHandler,
    utils::{find_entry_point_by_idx, SHARED_LIBRARY_EXT},
    OptLevel,
};
use clap::Parser;
use libloading::Library;
use num_bigint::BigInt;
use stats_alloc::{Region, StatsAlloc, INSTRUMENTED_SYSTEM};
use std::{
    alloc::System,
    collections::HashMap,
    fmt::{Debug, Display},
    fs::{self, create_dir_all, read_dir, OpenOptions},
    hash::Hash,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
use tracing::{debug, info, info_span, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

#[global_allocator]
static GLOBAL_ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

/// The directory used to store compiled native programs
const AOT_CACHE_DIR: &str = ".aot-cache";

/// An unique value hardcoded into the initial contract that it's
/// used as an anchor point to safely modify it.
/// It can be any value as long as it's unique in the contract.
const UNIQUE_CONTRACT_VALUE: u32 = 835;

/// A stress tester for Cairo Native
///
/// It compiles Sierra programs with Cairo Native, caches, and executes them with AOT runner.
/// The compiled dynamic libraries are stored in `AOT_CACHE_DIR` relative to the current working directory.
#[derive(Parser, Debug)]
struct StressTestCommand {
    /// Amount of rounds to execute
    rounds: u32,
    /// Output file for JSON formatted logs
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() {
    let args = StressTestCommand::parse();

    set_global_subscriber(&args);

    if !directory_is_empty(AOT_CACHE_DIR).expect("failed to open aot cache dir") {
        warn!("{AOT_CACHE_DIR} directory is not empty")
    }

    // Generate initial program
    let (entry_point, program) = {
        let before_generate = Instant::now();
        let initial_program = generate_starknet_contract(UNIQUE_CONTRACT_VALUE);
        let elapsed = before_generate.elapsed().as_millis();
        debug!(time = elapsed, "generated test program");
        initial_program
    };

    let global_region = Region::new(GLOBAL_ALLOC);
    let before_stress_test = Instant::now();

    // Initialize context and cache
    let native_context = NativeContext::new();
    let mut cache = NaiveAotCache::new(&native_context);

    info!("starting stress test");

    for round in 0..args.rounds {
        let _enter_round_span = info_span!("round", number = round).entered();

        let before_round = Instant::now();

        let program = modify_starknet_contract(program.clone(), UNIQUE_CONTRACT_VALUE, round);
        let hash = round;

        debug!(hash, "obtained test program");

        if cache.get(&hash).is_some() {
            panic!("all program keys should be different")
        }

        // Compiles and caches the program
        let executor = {
            let before_compile = Instant::now();
            let executor = cache.compile_and_insert(hash, &program, cairo_native::OptLevel::None);
            let elapsed = before_compile.elapsed().as_millis();
            debug!(time = elapsed, "compiled test program");
            executor
        };

        // Executes the program
        let execution_result = {
            let now = Instant::now();
            let execution_result = executor
                .invoke_contract_dynamic(&entry_point, &[], Some(u64::MAX), DummySyscallHandler)
                .expect("failed to execute contract");
            let elapsed = now.elapsed().as_millis();
            let result = execution_result.return_values[0];
            debug!(time = elapsed, result = %result, "executed test program");
            execution_result
        };

        assert!(
            !execution_result.failure_flag,
            "contract execution had failure flag set"
        );

        // Logs end of round
        let elapsed = before_round.elapsed().as_millis();
        let cache_disk_size =
            directory_get_size(AOT_CACHE_DIR).expect("failed to calculate cache disk size");
        let global_stats = global_region.change();
        let memory_used = global_stats.bytes_allocated - global_stats.bytes_deallocated;
        info!(
            time = elapsed,
            memory_used = memory_used,
            cache_disk_size = cache_disk_size,
            "finished round"
        );
    }

    let elapsed = before_stress_test.elapsed().as_millis();
    info!(time = elapsed, "finished stress test");
}

/// Generate a dummy starknet contract
///
/// The contract contains an external main function that returns `return_value`
fn generate_starknet_contract(
    return_value: u32,
) -> (FunctionId, cairo_lang_sierra::program::Program) {
    let program_str = format!(
        "\
#[starknet::contract]
mod Contract {{
    #[storage]
    struct Storage {{}}

    #[external(v0)]
    fn main(self: @ContractState) -> felt252 {{
        return {return_value};
    }}
}}
"
    );

    let mut program_file = tempfile::Builder::new()
        .prefix("test_")
        .suffix(".cairo")
        .tempfile()
        .expect("failed to create temporary file for cairo test program");
    fs::write(&mut program_file, program_str).expect("failed to write cairo test file");

    let contract_class = compile_path(
        program_file.path(),
        None,
        Default::default(),
        Default::default(),
    )
    .expect("failed to compile cairo contract");

    let program = contract_class
        .extract_sierra_program(false)
        .expect("failed to extract sierra program")
        .program;

    let entry_point_idx = contract_class
        .entry_points_by_type
        .external
        .first()
        .expect("contract should have at least one entrypoint")
        .function_idx;

    let entry_point = find_entry_point_by_idx(&program, entry_point_idx)
        .expect("failed to find entrypoint")
        .id
        .clone();

    (entry_point, program)
}

/// Modifies the given contract by replacing the `old_value` with `new_value` in any type declaration
///
/// The contract must only contain the value `old_value` once
fn modify_starknet_contract(mut program: Program, old_value: u32, new_value: u32) -> Program {
    let mut old_value_counter = 0;

    for type_declaration in &mut program.type_declarations {
        for generic_arg in &mut type_declaration.long_id.generic_args {
            let anchor = BigInt::from(old_value);

            match generic_arg {
                GenericArg::Value(return_value) if *return_value == anchor => {
                    *return_value = BigInt::from(new_value);
                    old_value_counter += 1;
                }
                _ => {}
            };
        }
    }

    assert!(
        old_value_counter == 1,
        "old_value was not found exactly once"
    );

    program
}

/// A naive implementation of an AOT Program Cache
///
/// For each contract:
/// - Compiles it to a shared library and stores it on disk (`AOT_CACHE_DIR`)
/// - Loads the shared library into an executor and stores it on a hashmap with the given key.
///
/// The shared libraries are unloaded when the executor gets drop, AKA: when the cache gets dropped.
/// The shared libraries on disk are never deleted
///
/// Possible improvements include:
/// - Keeping only some executors on memory, while storing the remaining compiled shared libraries only on disk.
/// - When restarting the program, reutilize already compiled programs from `AOT_CACHE_DIR`
struct NaiveAotCache<'a, K>
where
    K: PartialEq + Eq + Hash + Display,
{
    context: &'a NativeContext,
    cache: HashMap<K, Arc<AotNativeExecutor>>,
}

impl<'a, K> NaiveAotCache<'a, K>
where
    K: PartialEq + Eq + Hash + Display,
{
    pub fn new(context: &'a NativeContext) -> Self {
        Self {
            context,
            cache: Default::default(),
        }
    }

    pub fn get(&self, key: &K) -> Option<Arc<AotNativeExecutor>> {
        self.cache.get(key).cloned()
    }

    /// Compiles and inserts a given program into the cache
    ///
    /// The dynamic library is stored in `AOT_CACHE_DIR` directory
    pub fn compile_and_insert(
        &mut self,
        key: K,
        program: &Program,
        opt_level: OptLevel,
    ) -> Arc<AotNativeExecutor> {
        let native_module = self
            .context
            .compile(program, false, Some(Default::default()), None)
            .expect("failed to compile program");

        let registry = ProgramRegistry::new(program).expect("failed to get program registry");
        let metadata = native_module
            .metadata()
            .get::<GasMetadata>()
            .cloned()
            .expect("module should have gas metadata");

        let shared_library = {
            let object_data = module_to_object(native_module.module(), opt_level, None)
                .expect("failed to convert MLIR to object");

            let shared_library_dir = Path::new(AOT_CACHE_DIR);
            create_dir_all(shared_library_dir).expect("failed to create shared library directory");
            let shared_library_name = format!("lib{key}{SHARED_LIBRARY_EXT}");
            let shared_library_path = shared_library_dir.join(shared_library_name);

            object_to_shared_lib(&object_data, &shared_library_path, None)
                .expect("failed to link object into shared library");

            unsafe {
                Library::new(shared_library_path).expect("failed to load dynamic shared library")
            }
        };

        let executor = AotNativeExecutor::new(shared_library, registry, metadata);
        let executor = Arc::new(executor);

        self.cache.insert(key, executor.clone());

        executor
    }
}

/// Returns the size of a directory in bytes
fn directory_get_size(path: impl AsRef<Path>) -> io::Result<u64> {
    let mut dir = read_dir(path)?;

    dir.try_fold(0, |total_size, entry| {
        let entry = entry?;

        let size = match entry.metadata()? {
            data if data.is_dir() => directory_get_size(entry.path())?,
            data => data.len(),
        };

        Ok(total_size + size)
    })
}

fn directory_is_empty(path: impl AsRef<Path>) -> io::Result<bool> {
    let is_empty = match read_dir(path) {
        Ok(mut directory) => directory.next().is_none(),
        Err(error) => match error.kind() {
            io::ErrorKind::NotFound => true,
            _ => return Err(error),
        },
    };

    Ok(is_empty)
}

fn set_global_subscriber(args: &StressTestCommand) {
    let stdout = tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env());

    // Enables file logging conditionally, as Option<Layer> also implements Layer
    let file = args.output.as_ref().map(|path| {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("failed to open output file");

        tracing_subscriber::fmt::layer()
            .json()
            .with_writer(file)
            .with_filter(EnvFilter::from_default_env())
    });

    tracing_subscriber::Registry::default()
        .with(stdout)
        .with(file)
        .init();
}
