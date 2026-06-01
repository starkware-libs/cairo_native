use crate::common::{compare_outputs, run_native_program, run_vm_program, DEFAULT_GAS};
use cairo_lang_runner::Arg;
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::Value;
use starknet_types_core::felt::Felt;

#[test]
fn felt252_to_i8_downcast_neg_one_matches_vm() {
    let program = &load_program_and_runner("programs/cast_downcast_felt_neg");
    let neg_one = Felt::from(-1);

    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![Arg::Value(neg_one)],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[Value::Felt252(neg_one)],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("run_test").unwrap().id,
        &result_vm,
        &result_native,
    )
    .expect("felt252 -> i8 downcast of -1 must agree between VM and native");
}

#[test]
fn nonneg_to_i8_downcast_matches_vm() {
    let program = &load_program_and_runner("programs/cast_downcast_nonneg_i8");

    // `TestInput` is a 6-variant enum, each carrying a unit
    // payload. Native takes a `Value::Enum` whose `tag` is the variant index. The cairo VM,
    // however, encodes an enum value by its *variant selector*, not its index.
    const N_VARIANTS: usize = 6;
    for index in 0usize..N_VARIANTS {
        let selector = (N_VARIANTS - index) * 2 - 1;
        let result_vm = run_vm_program(
            program,
            "run_test",
            vec![Arg::Value(Felt::from(selector as u64))],
            Some(DEFAULT_GAS as usize),
        )
        .unwrap();

        let result_native = run_native_program(
            program,
            "run_test",
            &[Value::Enum {
                tag: index,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            }],
            Some(DEFAULT_GAS),
            Option::<DummySyscallHandler>::None,
        );

        compare_outputs(
            &program.1,
            &program.2.find_function("run_test").unwrap().id,
            &result_vm,
            &result_native,
        )
        .unwrap_or_else(|e| {
            panic!("nonneg -> i8 downcast of variant {index} diverges between VM and native: {e:?}")
        });
    }
}
