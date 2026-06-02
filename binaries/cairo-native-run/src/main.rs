use anyhow::Context;
use cairo_lang_compiler::{
    compile_prepared_db, db::RootDatabase, project::setup_project, CompilerConfig,
};
use cairo_lang_filesystem::ids::CrateInput;
use cairo_lang_runner::short_string::as_cairo_short_string;
#[cfg(feature = "with-libfunc-profiling")]
use cairo_lang_sierra::ids::ConcreteLibfuncId;
use cairo_lang_sierra_to_casm::metadata::MetadataComputationConfig;
use cairo_lang_sierra_type_size::ProgramRegistryInfo;
#[cfg(feature = "with-libfunc-profiling")]
use cairo_native::metadata::profiler::LibfuncProfileData;
use cairo_native::{
    context::NativeContext,
    executor::{AotNativeExecutor, JitNativeExecutor},
    metadata::gas::GasMetadata,
    starknet_stub::StubSyscallHandler,
};
use cairo_native_bin_utils::{find_function, result_to_runresult};
use clap::{Parser, ValueEnum};
#[cfg(feature = "with-libfunc-profiling")]
use std::collections::HashMap;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Clone, Debug, ValueEnum)]
enum RunMode {
    Aot,
    Jit,
}

/// Command line args parser.
/// Exits with 1 if the compilation or run fails, otherwise 0.
#[derive(Parser, Debug)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// The Cairo project path to compile and run its tests.
    path: PathBuf,
    /// Whether path is a single file.
    #[arg(short, long)]
    single_file: bool,
    /// Allows the compilation to succeed with warnings.
    #[arg(long)]
    allow_warnings: bool,
    /// In cases where gas is available, the amount of provided gas.
    #[arg(long)]
    available_gas: Option<u64>,
    /// Run with JIT or AOT (compiled).
    #[arg(long, value_enum, default_value_t = RunMode::Jit)]
    run_mode: RunMode,
    /// Optimization level, Valid: 0, 1, 2, 3. Values higher than 3 are considered as 3.
    #[arg(short = 'O', long, default_value_t = 0)]
    opt_level: u8,

    #[cfg(feature = "with-libfunc-profiling")]
    #[arg(long)]
    /// The output path for the libfunc profilling results
    profiler_output: Option<PathBuf>,

    #[cfg(feature = "with-trace-dump")]
    #[arg(long)]
    /// The output path for the execution trace
    trace_output: Option<PathBuf>,

    #[cfg(feature = "with-trace-dump")]
    #[arg(long)]
    /// The output path for the compiled sierra code
    sierra_output: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    // Configure logging and error handling.
    tracing::subscriber::set_global_default(
        FmtSubscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .finish(),
    )?;

    let args = Args::parse();

    let mut db = RootDatabase::builder().detect_corelib().build()?;
    let main_crate_ids = {
        let main_crate_inputs =
            setup_project(&mut db, &args.path).expect("failed to setup project");
        CrateInput::into_crate_ids(&db, main_crate_inputs)
    };

    let sierra_program = compile_prepared_db(
        &db,
        main_crate_ids,
        CompilerConfig {
            replace_ids: true,
            ..Default::default()
        },
    )?
    .program;

    #[cfg(feature = "with-trace-dump")]
    if let Some(sierra_output) = args.sierra_output {
        use std::fs::File;
        use std::io::Write;
        let mut file = File::create(sierra_output).unwrap();
        write!(file, "{}", &sierra_program).unwrap();
    }

    let native_context = NativeContext::new();

    // Compile the sierra program into a MLIR module.
    let native_module = native_context
        .compile(&sierra_program, false, Some(Default::default()), None)
        .unwrap();

    let native_executor: Box<dyn Fn(_, _, _, &mut StubSyscallHandler) -> _> = match args.run_mode {
        RunMode::Aot => {
            let executor =
                AotNativeExecutor::from_native_module(native_module, args.opt_level.into())?;

            #[cfg(feature = "with-trace-dump")]
            {
                use cairo_native::metadata::trace_dump::TraceBinding;
                if let Some(trace_id) = executor.find_symbol_ptr(TraceBinding::TraceId.symbol()) {
                    let trace_id = trace_id.cast::<u64>();
                    unsafe { *trace_id = 0 };
                }
            }

            Box::new(move |function_id, args, gas, syscall_handler| {
                executor.invoke_dynamic_with_syscall_handler(
                    function_id,
                    args,
                    gas,
                    syscall_handler,
                )
            })
        }
        RunMode::Jit => {
            let executor =
                JitNativeExecutor::from_native_module(native_module, args.opt_level.into())?;

            #[cfg(feature = "with-trace-dump")]
            {
                use cairo_native::metadata::trace_dump::TraceBinding;
                if let Some(trace_id) = executor.find_symbol_ptr(TraceBinding::TraceId.symbol()) {
                    let trace_id = trace_id.cast::<u64>();
                    unsafe { *trace_id = 0 };
                }
            }

            #[cfg(feature = "with-libfunc-profiling")]
            {
                use cairo_native::metadata::profiler::ProfilerBinding;

                if let Some(trace_id) =
                    executor.find_symbol_ptr(ProfilerBinding::ProfileId.symbol())
                {
                    let trace_id = trace_id.cast::<u64>();
                    unsafe { *trace_id = 0 };
                }
            }

            Box::new(move |function_id, args, gas, syscall_handler| {
                executor.invoke_dynamic_with_syscall_handler(
                    function_id,
                    args,
                    gas,
                    syscall_handler,
                )
            })
        }
    };

    #[cfg(feature = "with-trace-dump")]
    {
        use cairo_lang_sierra::program_registry::ProgramRegistry;
        use cairo_native::metadata::trace_dump::trace_dump_runtime::{TraceDump, TRACE_DUMP};

        TRACE_DUMP.lock().unwrap().insert(
            0,
            TraceDump::new(ProgramRegistry::new(&sierra_program).unwrap()),
        );
    }

    #[cfg(feature = "with-libfunc-profiling")]
    {
        use cairo_native::metadata::profiler::{ProfilerImpl, LIBFUNC_PROFILE};

        LIBFUNC_PROFILE
            .lock()
            .unwrap()
            .insert(0, ProfilerImpl::new());
    }

    let gas_metadata = GasMetadata::new(
        &sierra_program,
        &ProgramRegistryInfo::new(&sierra_program)?,
        Some(MetadataComputationConfig::default()),
    )
    .unwrap();

    let func = find_function(&sierra_program, "::main")?;

    let initial_gas = gas_metadata
        .get_initial_available_gas(&func.id, args.available_gas)
        .with_context(|| "not enough gas to run")?;

    let mut syscall_handler = StubSyscallHandler::default();

    let result = native_executor(&func.id, &[], Some(initial_gas), &mut syscall_handler)
        .with_context(|| "Failed to run the function.")?;

    let run_result = result_to_runresult(&result)?;

    match run_result {
        cairo_lang_runner::RunResultValue::Success(values) => {
            println!("Run completed successfully, returning {values:?}")
        }
        cairo_lang_runner::RunResultValue::Panic(values) => {
            print!("Run panicked with [");
            for value in &values {
                match as_cairo_short_string(value) {
                    Some(as_string) => print!("{value} ('{as_string}'), "),
                    None => print!("{value}, "),
                }
            }
            println!("].")
        }
    }
    if let Some(gas) = result.remaining_gas {
        println!("Remaining gas: {gas}");
    }

    #[cfg(feature = "with-libfunc-profiling")]
    {
        use std::{fs::File, io::Write};

        let profile = cairo_native::metadata::profiler::LIBFUNC_PROFILE
            .lock()
            .unwrap();

        assert_eq!(profile.values().len(), 1);

        let profile = profile.values().next().unwrap();

        if let Some(profiler_output_path) = args.profiler_output {
            let mut output = File::create(profiler_output_path)?;

            let raw_profile = profile.get_profile(&sierra_program);
            let mut processed_profile = process_profile(raw_profile);

            processed_profile.sort_by_key(|LibfuncProfileSummary { libfunc_idx, .. }| {
                sierra_program
                    .libfunc_declarations
                    .iter()
                    .enumerate()
                    .find_map(|(i, x)| (x.id == *libfunc_idx).then_some(i))
                    .unwrap()
            });

            for LibfuncProfileSummary {
                libfunc_idx,
                samples,
                total_time,
                average_time,
                std_deviation,
                quartiles,
            } in processed_profile
            {
                writeln!(output, "{libfunc_idx}")?;
                writeln!(output, "    Total Samples:          {samples}")?;

                let (Some(total_time), Some(average_time), Some(std_deviation), Some(quartiles)) =
                    (total_time, average_time, std_deviation, quartiles)
                else {
                    writeln!(output, "    Total Execution Time:   none")?;
                    writeln!(output, "    Average Execution Time: none")?;
                    writeln!(output, "    Standard Deviation:     none")?;
                    writeln!(output, "    Quartiles:              none")?;
                    writeln!(output)?;

                    continue;
                };

                writeln!(output, "    Total Execution Time:   {total_time}")?;
                writeln!(output, "    Average Execution Time: {average_time}")?;
                writeln!(output, "    Standard Deviation:     {std_deviation}")?;
                writeln!(output, "    Quartiles:              {quartiles:?}")?;
                writeln!(output)?;
            }
        }
    }

    #[cfg(feature = "with-trace-dump")]
    if let Some(trace_output) = args.trace_output {
        let traces = cairo_native::metadata::trace_dump::trace_dump_runtime::TRACE_DUMP
            .lock()
            .unwrap();
        assert_eq!(traces.len(), 1);

        let trace_dump = traces.values().next().unwrap();
        serde_json::to_writer_pretty(
            std::fs::File::create(trace_output).unwrap(),
            &trace_dump.trace,
        )
        .unwrap();
    }

    Ok(())
}

#[cfg(feature = "with-libfunc-profiling")]
struct LibfuncProfileSummary {
    pub libfunc_idx: ConcreteLibfuncId,
    pub samples: u64,
    pub total_time: Option<u64>,
    pub average_time: Option<f64>,
    pub std_deviation: Option<f64>,
    pub quartiles: Option<[u64; 5]>,
}

#[cfg(feature = "with-libfunc-profiling")]
fn process_profile(
    profiles: HashMap<ConcreteLibfuncId, LibfuncProfileData>,
) -> Vec<LibfuncProfileSummary> {
    profiles
        .into_iter()
        .map(
            |(
                libfunc_idx,
                LibfuncProfileData {
                    mut deltas,
                    extra_counts,
                },
            )| {
                // if no deltas were registered, we only return the libfunc's calls amount
                if deltas.is_empty() {
                    return LibfuncProfileSummary {
                        libfunc_idx,
                        samples: extra_counts,
                        total_time: None,
                        average_time: None,
                        std_deviation: None,
                        quartiles: None,
                    };
                }

                deltas.sort();

                // Drop outliers.
                {
                    let q1 = deltas[deltas.len() / 4];
                    let q3 = deltas[3 * deltas.len() / 4];
                    let iqr = q3 - q1;

                    let q1_thr = q1.saturating_sub(iqr + iqr / 2);
                    let q3_thr = q3 + (iqr + iqr / 2);

                    deltas.retain(|x| *x >= q1_thr && *x <= q3_thr);
                }

                // Compute the quartiles.
                let quartiles = [
                    *deltas.first().unwrap(),
                    deltas[deltas.len() / 4],
                    deltas[deltas.len() / 2],
                    deltas[3 * deltas.len() / 4],
                    *deltas.last().unwrap(),
                ];

                // Compute the average.
                let average = deltas.iter().copied().sum::<u64>() as f64 / deltas.len() as f64;

                // Compute the standard deviation.
                let std_dev = {
                    let sum = deltas
                        .iter()
                        .copied()
                        .map(|x| x as f64)
                        .map(|x| x - average)
                        .map(|x| x * x)
                        .sum::<f64>();
                    sum / (deltas.len() as u64 + extra_counts) as f64
                };

                LibfuncProfileSummary {
                    libfunc_idx,
                    samples: deltas.len() as u64 + extra_counts,
                    total_time: Some(
                        deltas.iter().sum::<u64>() + (extra_counts as f64 * average).round() as u64,
                    ),
                    average_time: Some(average),
                    std_deviation: Some(std_dev),
                    quartiles: Some(quartiles),
                }
            },
        )
        .collect::<Vec<_>>()
}
