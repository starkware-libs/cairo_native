use std::{error::Error, fs::File, path::PathBuf};

use cairo_lang_test_plugin::TestsCompilationConfig;
use cairo_lang_test_runner::TestCompiler;
use clap::Parser;

/// Compiles Cairo tests to Sierra
#[derive(Parser, Debug)]
#[command()]
struct Args {
    /// Path to input Cairo program
    cairo_path: PathBuf,
    /// Path to output Sierra test compilation
    sierra_tests_path: PathBuf,
    /// Whether to compile with the starknet plugin
    #[arg(long, default_value_t = false)]
    starknet: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let compiler = TestCompiler::try_new(
        &args.cairo_path,
        true,
        true,
        TestsCompilationConfig {
            starknet: args.starknet,
            contract_declarations: None,
            contract_crate_ids: None,
            executable_crate_ids: None,
            add_statements_functions: false,
            add_statements_code_locations: false,
            add_functions_debug_info: false,
            add_type_names: false,
            replace_ids: false,
        },
    )?;
    let compilation = compiler.build()?;

    let sierra_tests_file = File::create(&args.sierra_tests_path)?;
    serde_json::to_writer_pretty(sierra_tests_file, &compilation)?;

    Ok(())
}
