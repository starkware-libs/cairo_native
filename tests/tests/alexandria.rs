use crate::common::{compare_outputs, run_native_program, run_vm_program, DEFAULT_GAS};
use cairo_lang_runner::SierraCasmRunner;
use cairo_lang_sierra::program::Program;
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;
use test_case::test_case;

#[track_caller]
fn compare_inputless_function(function_name: &str) {
    let program: (String, Program, SierraCasmRunner) =
        load_program_and_runner("scarb/alexandria/alexandria");
    let result_vm =
        run_vm_program(&program, function_name, vec![], Some(DEFAULT_GAS as usize)).unwrap();
    let result_native = run_native_program(
        &program,
        function_name,
        &[],
        Some(DEFAULT_GAS),
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function(function_name).unwrap().id,
        &result_vm,
        &result_native,
    )
    .expect("compare error");
}

// alexandria_math
#[test_case("fib")]
#[test_case("karatsuba")]
#[test_case("armstrong_number")]
#[test_case("collatz_sequence")]
#[test_case("aliquot_sum")]
#[test_case("extended_euclidean_algorithm")]
// alexandria_data_structures
#[test_case("vec")]
#[test_case("stack")]
#[test_case("queue")]
#[test_case("bit_array")]
// alexandria_encoding
#[test_case("base64_encode")]
#[test_case("reverse_bits")]
#[test_case("reverse_bytes")]
fn test_cases(function_name: &str) {
    compare_inputless_function(function_name)
}
