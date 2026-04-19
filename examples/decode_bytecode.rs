//! Decode a CSV of decimal felt252 values (one per line) into readable Sierra text.
//!
//! Usage: cargo run --example decode_bytecode -- <input.csv> [output.sierra]

use cairo_lang_sierra::ids::{ConcreteLibfuncId, ConcreteTypeId};
use cairo_lang_sierra::program::{GenericArg, Program};
use cairo_lang_starknet_classes::contract_class::ContractClass;
use num_bigint::BigUint;
use std::collections::HashMap;
use std::env;
use std::fs;

fn main() {
    let mut args = env::args().skip(1);
    let input = args.next().expect("missing input path");
    let output = args.next();

    let raw = fs::read_to_string(&input).expect("failed to read input");
    let felts: Vec<BigUint> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<BigUint>()
                .unwrap_or_else(|_| panic!("could not parse felt: {s:?}"))
        })
        .collect();

    eprintln!("Parsed {} felts", felts.len());

    let hex_strings: Vec<String> = felts.iter().map(|b| format!("0x{b:x}")).collect();

    let contract_json = serde_json::json!({
        "sierra_program": hex_strings,
        "sierra_program_debug_info": null,
        "contract_class_version": "0.1.0",
        "entry_points_by_type": {
            "EXTERNAL": [],
            "L1_HANDLER": [],
            "CONSTRUCTOR": []
        },
        "abi": null,
    });

    let contract: ContractClass =
        serde_json::from_value(contract_json).expect("failed to construct ContractClass");
    let extracted = contract
        .extract_sierra_program(true)
        .expect("sierra_from_felt252s failed");

    eprintln!("Sierra version:   {:?}", extracted.sierra_version);
    eprintln!("Compiler version: {:?}", extracted.compiler_version);

    let mut program = extracted.program;
    populate_structural_names(&mut program);

    let text = program.to_string();
    match output {
        Some(path) => {
            fs::write(&path, &text).expect("failed to write output");
            eprintln!("Wrote readable Sierra to {path}");
        }
        none => {
            let _ = none;
            println!("{text}");
        }
    }
}

/// Fill in `debug_name` for every type / libfunc concrete id using the structural
/// long-id form (e.g. `Array<felt252>`, `array_slice<u64>`). The raw bytecode does
/// not carry source-level names, but the long ids are enough to produce a reading
/// equivalent to what `replace_ids` gives when compiling from source.
fn populate_structural_names(program: &mut Program) {
    let mut type_names: HashMap<u64, String> = HashMap::new();
    let mut libfunc_names: HashMap<u64, String> = HashMap::new();

    // Types can forward-reference each other, so resolve to a fixed point: on each
    // pass, patch every arg with the current best-known name and re-derive each
    // declaration's name from its long id. Stops when nothing changes.
    loop {
        let mut changed = false;
        for decl in &mut program.type_declarations {
            patch_generic_args(&mut decl.long_id.generic_args, &type_names, &libfunc_names);
            let name = decl.long_id.to_string();
            if type_names.get(&decl.id.id) != Some(&name) {
                type_names.insert(decl.id.id, name.clone());
                decl.id.debug_name = Some(name.into());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    for decl in &mut program.libfunc_declarations {
        patch_generic_args(&mut decl.long_id.generic_args, &type_names, &libfunc_names);
        let name = decl.long_id.to_string();
        libfunc_names.insert(decl.id.id, name.clone());
        decl.id.debug_name = Some(name.into());
    }

    for statement in &mut program.statements {
        if let cairo_lang_sierra::program::GenStatement::Invocation(inv) = statement {
            rename_libfunc(&mut inv.libfunc_id, &libfunc_names);
        }
    }
    for func in &mut program.funcs {
        for param in &mut func.params {
            rename_type(&mut param.ty, &type_names);
        }
        for ty in &mut func.signature.param_types {
            rename_type(ty, &type_names);
        }
        for ty in &mut func.signature.ret_types {
            rename_type(ty, &type_names);
        }
    }
}

fn patch_generic_args(
    args: &mut [GenericArg],
    type_names: &HashMap<u64, String>,
    libfunc_names: &HashMap<u64, String>,
) {
    for arg in args {
        match arg {
            GenericArg::Type(id) => rename_type(id, type_names),
            GenericArg::Libfunc(id) => rename_libfunc(id, libfunc_names),
            _ => {}
        }
    }
}

fn rename_type(id: &mut ConcreteTypeId, type_names: &HashMap<u64, String>) {
    if let Some(name) = type_names.get(&id.id) {
        id.debug_name = Some(name.clone().into());
    }
}

fn rename_libfunc(id: &mut ConcreteLibfuncId, libfunc_names: &HashMap<u64, String>) {
    if let Some(name) = libfunc_names.get(&id.id) {
        id.debug_name = Some(name.clone().into());
    }
}
