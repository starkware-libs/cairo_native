use std::{
    error::Error,
    fs::{self, File},
    path::PathBuf,
};

use cairo_lang_compiler::{
    compile_prepared_db_program_artifact, db::RootDatabase, diagnostics::DiagnosticsReporter,
    project::setup_project, CompilerConfig,
};
use cairo_lang_filesystem::ids::CrateInput;
use cairo_lang_lowering::optimizations::config::Optimizations;
use cairo_lang_sierra::program::VersionedProgram;
use clap::Parser;

/// Compiles Cairo program to Sierra
#[derive(Parser, Debug)]
#[command()]
struct Args {
    /// Path to input Cairo program
    cairo_path: PathBuf,
    /// Path to output Sierra json
    sierra_json_path: PathBuf,
    /// Path to output Sierra text
    sierra_text_path: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut db = {
        let mut dbb = RootDatabase::builder();
        dbb.detect_corelib();
        dbb.with_optimizations(Optimizations::Disabled);
        dbb.build()?
    };
    let crate_inputs = setup_project(&mut db, &args.cairo_path)?;
    let diagnostics_reporter = DiagnosticsReporter::stderr()
        .with_crates(&crate_inputs)
        .allow_warnings();
    let crate_ids = CrateInput::into_crate_ids(&db, crate_inputs);

    let program = compile_prepared_db_program_artifact(
        &db,
        crate_ids,
        CompilerConfig {
            replace_ids: true,
            diagnostics_reporter,
            ..CompilerConfig::default()
        },
    )?;

    fs::write(&args.sierra_text_path, program.to_string())?;
    let versioned_program = VersionedProgram::from(program);
    let sierra_json_file = File::create(&args.sierra_json_path)?;
    serde_json::to_writer_pretty(sierra_json_file, &versioned_program)?;

    Ok(())
}
