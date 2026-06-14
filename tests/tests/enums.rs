use crate::common::{compare_outputs, run_native_program, run_vm_program, DEFAULT_GAS};
use cairo_lang_runner::Arg;
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::Value;
use starknet_types_core::felt::Felt;

#[test]
fn single_variant_enum_in_array_matches_vm() {
    let program = &load_program_and_runner("programs/enum_single_variant_in_array");

    let values = [Felt::from(10), Felt::from(20), Felt::from(30)];

    let mut vm_array = Vec::new();
    for v in values {
        vm_array.push(Arg::Value(Felt::from(0)));
        vm_array.push(Arg::Value(v));
    }
    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![Arg::Array(vm_array)],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();

    let result_native = run_native_program(
        program,
        "run_test",
        &[Value::Array(
            values
                .iter()
                .copied()
                .map(|v| Value::Enum {
                    tag: 0,
                    value: Box::new(Value::Felt252(v)),
                    debug_name: None,
                })
                .collect(),
        )],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("run_test").unwrap().id,
        &result_vm,
        &result_native,
    )
    .expect("single-variant enum in array must agree between VM and native");
}
