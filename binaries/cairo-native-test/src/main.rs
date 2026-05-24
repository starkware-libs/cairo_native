use anyhow::bail;
use cairo_lang_compiler::project::check_compiler_path;
use cairo_lang_test_plugin::TestsCompilationConfig;
use cairo_lang_test_runner::TestCompiler;
use cairo_native_bin_utils::{
    test::{display_tests_summary, filter_test_cases, run_tests},
    RunArgs,
};
use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// Compiles a Cairo project and runs all the functions marked as `#[test]`.
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
    /// The filter for the tests, running only tests containing the filter string.
    #[arg(short, long, default_value_t = String::default())]
    filter: String,
    /// Should we run ignored tests as well.
    #[arg(long, default_value_t = false)]
    include_ignored: bool,
    /// Should we run only the ignored tests.
    #[arg(long, default_value_t = false)]
    ignored: bool,
    /// Should we add the starknet plugin to run the tests.
    #[arg(long, default_value_t = false)]
    starknet: bool,
    /// Optimization level, Valid: 0, 1, 2, 3. Values higher than 3 are considered as 3.
    #[arg(short = 'O', long, default_value_t = 0)]
    opt_level: u8,
    /// Compares test result with Cairo VM.
    #[arg(long, default_value_t = false)]
    compare_with_cairo_vm: bool,
    #[arg(short = 'j', long)]
    jobs: Option<u8>,
}

fn main() -> anyhow::Result<()> {
    // Parse command-line arguments.
    let args = Args::parse();

    // Configure logging and error handling.
    tracing::subscriber::set_global_default(
        FmtSubscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .finish(),
    )?;

    if let Some(num_jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(num_jobs as usize)
            .build_global()?;
    }

    check_compiler_path(args.single_file, &args.path)?;

    let compiler = TestCompiler::try_new(
        &args.path,
        args.allow_warnings,
        true,
        TestsCompilationConfig {
            starknet: args.starknet,
            add_statements_functions: false,
            add_statements_code_locations: false,
            add_functions_debug_info: false,
            add_type_names: false,
            contract_declarations: None,
            contract_crate_ids: None,
            executable_crate_ids: None,
            replace_ids: true,
        },
    )?;

    let build_test_compilation = compiler.build()?;

    let (compiled, filtered_out) = filter_test_cases(
        build_test_compilation,
        args.include_ignored,
        args.ignored,
        args.filter.clone(),
    );

    let summary = run_tests(
        compiled.metadata.named_tests,
        compiled.sierra_program.program,
        compiled.metadata.function_set_costs,
        compiled.metadata.contracts_info,
        RunArgs {
            opt_level: args.opt_level,
            compare_with_vm: args.compare_with_cairo_vm,
        },
    )?;

    display_tests_summary(&summary, filtered_out);

    if !summary.failed.is_empty() || !summary.mismatch.is_empty() {
        bail!("test failed")
    }

    Ok(())
}
