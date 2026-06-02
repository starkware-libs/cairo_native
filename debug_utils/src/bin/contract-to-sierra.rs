use std::fs::File;

use cairo_lang_starknet_classes::contract_class::ContractClass;
use clap::Parser;

/// Extracts sierra program from contract
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    // Path to cairo contract
    contract_path: String,
}

fn main() {
    let args = Args::parse();

    let contract_file = File::open(args.contract_path).expect("failed to open contract file");

    let contract: ContractClass =
        serde_json::from_reader(contract_file).expect("failed to parse contract");

    print!(
        "{}",
        contract
            .extract_sierra_program(false)
            .expect("failed to extract sierra program")
            .program
    );
}
