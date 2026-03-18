use std::{collections::HashMap, fs::File, path::PathBuf};

use cairo_lang_runner::casm_run::{hint_to_hint_params, run_function, CairoHintProcessor};
use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
use cairo_vm::{
    cairo_run::{write_encoded_memory, write_encoded_trace},
    types::builtin_name::BuiltinName,
    vm::runners::cairo_runner::RunResources,
};
use clap::Parser;
use num_bigint::BigInt;
use starknet_types_core::felt::Felt;

#[derive(Debug, Parser)]
struct Args {
    contract_path: PathBuf,
    memory_path: PathBuf,
    trace_path: PathBuf,
}

pub fn main() {
    let cli_args = Args::parse();
    let contract_file = File::open(cli_args.contract_path).expect("failed to open contract path");
    let contract: CasmContractClass =
        serde_json::from_reader(contract_file).expect("failed to parse contract file");

    let bytecode: Vec<BigInt> = contract
        .bytecode
        .iter()
        .map(|x| BigInt::from(x.value.clone()))
        .collect();

    let program_builtins = contract
        .entry_points_by_type
        .external
        .iter()
        .find(|e| e.offset == 0)
        .unwrap()
        .builtins
        .iter()
        .map(|s| BuiltinName::from_str(s).expect("Invalid builtin name"))
        .collect::<Vec<_>>();

    let hints_dict: HashMap<usize, Vec<_>> = contract
        .hints
        .iter()
        .map(|(offset, hints)| (*offset, hints.iter().map(hint_to_hint_params).collect()))
        .collect();

    let string_to_hint: HashMap<String, cairo_lang_casm::hints::Hint> = contract
        .hints
        .iter()
        .flat_map(|(_, hints)| hints.iter().cloned())
        .map(|hint| (format!("{hint:?}"), hint))
        .collect();

    let mut hint_processor = CairoHintProcessor {
        runner: None,
        user_args: vec![],
        string_to_hint,
        starknet_state: Default::default(),
        run_resources: RunResources::new(usize::MAX),
        syscalls_used_resources: Default::default(),
        no_temporary_segments: false,
        markers: vec![],
        panic_traceback: vec![],
    };

    let result = run_function(
        bytecode.iter(),
        program_builtins,
        |_vm| Ok(()),
        &mut hint_processor,
        hints_dict,
    )
    .expect("failed to execute contract");

    println!("Return values:");
    let retdata: Vec<Felt> = result
        .memory
        .iter()
        .rev()
        .take(5)
        .filter_map(|v| *v)
        .collect();
    println!("{retdata:?}");

    // Write trace file.
    let trace_file = File::create(cli_args.trace_path).expect("failed to create trace file");
    let mut trace_writer = std::io::BufWriter::new(trace_file);
    write_encoded_trace(&result.relocated_trace, &mut trace_writer)
        .expect("failed to write trace");

    // Write memory file.
    let memory_file = File::create(cli_args.memory_path).expect("failed to create memory file");
    let mut memory_writer = std::io::BufWriter::new(memory_file);
    write_encoded_memory(&result.memory, &mut memory_writer).expect("failed to write memory");
}
