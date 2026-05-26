use crate::common::{compare_outputs, run_native_program, run_vm_program, DEFAULT_GAS};
use cairo_lang_runner::Arg;
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;
use cairo_native::Value;
use starknet_types_core::felt::Felt;
use test_case::test_case;

// `int_range_try_new` over `BoundedInt<-10, 10>` (5-bit biased representation,
// bias = +10) must compare biased values with an unsigned predicate. A signed
// predicate misreads any biased value with MSB set as negative; e.g. `(10, 0)`
// -> biased `(20, 10)` is read as `(-12, 10)` and the invalid range is
// incorrectly accepted, while `(-10, 10)` -> biased `(0, 20)` is read as
// `(0, -12)` and the valid range is rejected.
#[test_case(0, 5; "valid_zero_to_five")]
#[test_case(-10, -5; "valid_neg_ten_to_neg_five")]
#[test_case(-10, 10; "valid_full_range")]
#[test_case(10, 10; "valid_empty_at_upper")]
#[test_case(0, -5; "invalid_descending_zero")]
#[test_case(10, 0; "invalid_msb_biased_lhs")]
#[test_case(10, -10; "invalid_max_to_min")]
fn int_range_try_new_bounded_int_negative_lower_vm_equivalence(lhs: i64, rhs: i64) {
    let program = &load_program_and_runner(
        "test_data_artifacts/programs/libfuncs/int_range_try_new_bounded_int",
    );

    let result_vm = run_vm_program(
        program,
        "run_test",
        vec![Arg::Value(Felt::from(lhs)), Arg::Value(Felt::from(rhs))],
        Some(DEFAULT_GAS as usize),
    )
    .unwrap();
    let result_native = run_native_program(
        program,
        "run_test",
        &[
            Value::Felt252(Felt::from(lhs)),
            Value::Felt252(Felt::from(rhs)),
        ],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("run_test").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}
