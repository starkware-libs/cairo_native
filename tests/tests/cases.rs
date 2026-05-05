use crate::common::{compare_inputless_program, run_native_starknet_contract, run_vm_contract};
use cairo_native::{starknet::DummySyscallHandler, utils::testing::load_contract};
use itertools::Itertools;
use pretty_assertions_sorted::assert_eq_sorted;
use test_case::test_case;

// Test cases for programs without input, it checks the outputs are correct automatically.

// felt tests
#[test_case("test_data_artifacts/programs/cases/felt_ops/add")]
#[test_case("test_data_artifacts/programs/cases/felt_ops/sub")]
#[test_case("test_data_artifacts/programs/cases/felt_ops/felt_is_zero")]
#[test_case("test_data_artifacts/programs/cases/felt_ops/mul")]
#[test_case("test_data_artifacts/programs/cases/felt_ops/negation")]
#[test_case("test_data_artifacts/programs/cases/felt_ops/div")]
// generic tests
#[test_case("test_data_artifacts/programs/cases/fib_counter")]
#[test_case("test_data_artifacts/programs/cases/fib_local")]
#[test_case("test_data_artifacts/programs/cases/pedersen_hash")]
#[test_case("test_data_artifacts/programs/cases/unwrap_non_zero")]
#[test_case("test_data_artifacts/programs/cases/poseidon")]
#[test_case("test_data_artifacts/programs/cases/panic_array")]
#[test_case("test_data_artifacts/programs/cases/generic_fn_loop")]
// enums
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_c_style")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_empty")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_multiple")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_nested_c_style")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_nested_empty")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_nested_multiple")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_nested_single_scalar")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_nested_single_struct")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_nested_single_tuple")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_single_scalar")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_single_struct")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init_single_tuple")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_init")]
#[test_case("test_data_artifacts/programs/cases/enums/single_value")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_match")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_snapshot_match_a")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_snapshot_match_b")]
#[test_case("test_data_artifacts/programs/cases/enums/enum_large_variants")]
// returns
#[test_case("test_data_artifacts/programs/cases/returns/enums")]
#[test_case("test_data_artifacts/programs/cases/returns/simple")]
#[test_case("test_data_artifacts/programs/cases/returns/tuple")]
// dict
#[test_case("test_data_artifacts/programs/cases/dict/insert_get")]
// uint
#[test_case("test_data_artifacts/programs/cases/uint/compare")]
#[test_case("test_data_artifacts/programs/cases/uint/consts")]
#[test_case("test_data_artifacts/programs/cases/uint/downcasts")]
#[test_case("test_data_artifacts/programs/cases/uint/safe_divmod")]
#[test_case("test_data_artifacts/programs/cases/uint/uint_addition")]
#[test_case("test_data_artifacts/programs/cases/uint/uint_subtraction")]
#[test_case("test_data_artifacts/programs/cases/uint/uint_try_from_felt")]
#[test_case("test_data_artifacts/programs/cases/uint/upcasts")]
#[test_case("test_data_artifacts/programs/cases/uint/wide_mul")]
#[test_case("test_data_artifacts/programs/cases/uint/u512_safe_divmod_by_u256")]
// sint
#[test_case("test_data_artifacts/programs/cases/sint/eq")]
#[test_case("test_data_artifacts/programs/cases/sint/to_from_felt252")]
// sint8
#[test_case("test_data_artifacts/programs/cases/sint/i8_diff")]
#[test_case("test_data_artifacts/programs/cases/sint/i8_add_sub")]
#[test_case("test_data_artifacts/programs/cases/sint/i8_wide_mul")]
// sint16
#[test_case("test_data_artifacts/programs/cases/sint/i16_diff")]
#[test_case("test_data_artifacts/programs/cases/sint/i16_add_sub")]
#[test_case("test_data_artifacts/programs/cases/sint/i16_wide_mul")]
// sint32
#[test_case("test_data_artifacts/programs/cases/sint/i32_diff")]
#[test_case("test_data_artifacts/programs/cases/sint/i32_add_sub")]
#[test_case("test_data_artifacts/programs/cases/sint/i32_wide_mul")]
// sint64
#[test_case("test_data_artifacts/programs/cases/sint/i64_diff")]
#[test_case("test_data_artifacts/programs/cases/sint/i64_add_sub")]
#[test_case("test_data_artifacts/programs/cases/sint/i64_wide_mul")]
// sint128
#[test_case("test_data_artifacts/programs/cases/sint/i128_diff")]
#[test_case("test_data_artifacts/programs/cases/sint/i128_add_sub")]
// structs
#[test_case("test_data_artifacts/programs/cases/structs/basic")]
#[test_case("test_data_artifacts/programs/cases/structs/bigger")]
#[test_case("test_data_artifacts/programs/cases/structs/enum_member")]
#[test_case("test_data_artifacts/programs/cases/structs/nested")]
#[test_case("test_data_artifacts/programs/cases/structs/struct_snapshot_deconstruct")]
// gas
#[test_case("test_data_artifacts/programs/cases/gas/available_gas")]
// bool
#[test_case("test_data_artifacts/programs/cases/bool/and")]
#[test_case("test_data_artifacts/programs/cases/bool/eq")]
#[test_case("test_data_artifacts/programs/cases/bool/not")]
#[test_case("test_data_artifacts/programs/cases/bool/or")]
#[test_case("test_data_artifacts/programs/cases/bool/to_felt252")]
#[test_case("test_data_artifacts/programs/cases/bool/xor")]
// bitwise
#[test_case("test_data_artifacts/programs/cases/bitwise/and")]
#[test_case("test_data_artifacts/programs/cases/bitwise/or")]
#[test_case("test_data_artifacts/programs/cases/bitwise/xor")]
// array
#[test_case("test_data_artifacts/programs/cases/array/append")]
#[test_case("test_data_artifacts/programs/cases/array/index_invalid")]
#[test_case("test_data_artifacts/programs/cases/array/pop_front_invalid")]
#[test_case("test_data_artifacts/programs/cases/array/pop_front_valid")]
#[test_case("test_data_artifacts/programs/cases/array/slice")]
// nullable
#[test_case("test_data_artifacts/programs/cases/nullable/nullable")]
// Programs copied from the cairo-vm
// https://github.com/lambdaclass/cairo-vm/tree/main/cairo_programs/cairo-1-programs
#[test_case("test_data_artifacts/programs/cases/cairo_vm/array_append")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/array_get")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/array_integer_tuple")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/bitwise")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/bytes31_ret")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/dict_with_struct")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/dictionaries")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/ecdsa_recover")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/enum_flow")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/enum_match")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/factorial")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/felt_dict")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/felt_dict_squash")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/felt_span")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/fibonacci")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/hello")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/my_rectangle")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/null_ret")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/nullable_box_vec")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/nullable_dict")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/ops")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/pedersen_example")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/poseidon")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/poseidon_pedersen")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/primitive_types2")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/print")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/recursion")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/sample")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/short_string")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/simple")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/simple_struct")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/struct_span_return")]
#[test_case("test_data_artifacts/programs/cases/cairo_vm/tensor_new")]
#[test_case("test_data_artifacts/programs/cases/brainfuck")]
// EVM related
#[test_case("test_data_artifacts/programs/cases/stack")]
fn test_program_cases(program_path: &str) {
    compare_inputless_program(program_path)
}

// Contracts copied from the cairo-vm
// https://github.com/lambdaclass/cairo-vm/tree/main/cairo_programs/cairo-1-contracts
#[test_case("test_data_artifacts/contracts/cairo_vm/alloc_segment.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/assert_le_find_small_arcs.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/dict_test.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/divmod.contract.json", &[100, 10])]
#[test_case("test_data_artifacts/contracts/cairo_vm/factorial.contract.json", &[10])]
#[test_case("test_data_artifacts/contracts/cairo_vm/felt252_dict_entry_init.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/felt252_dict_entry_update.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/felt_252_dict.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/fib.contract.json", &[10, 10, 10])]
#[test_case("test_data_artifacts/contracts/cairo_vm/get_segment_arena_index.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/init_squash_data.contract.json", &[10])]
#[test_case("test_data_artifacts/contracts/cairo_vm/linear_split.contract.json", &[10])]
#[test_case("test_data_artifacts/contracts/cairo_vm/should_skip_squash_loop.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/less_than.contract.json", &[10])]
#[test_case("test_data_artifacts/contracts/cairo_vm/u128_sqrt.contract.json", &[100])]
#[test_case("test_data_artifacts/contracts/cairo_vm/u16_sqrt.contract.json", &[100])]
#[test_case("test_data_artifacts/contracts/cairo_vm/u256_sqrt.contract.json", &[100])]
#[test_case("test_data_artifacts/contracts/cairo_vm/u32_sqrt.contract.json", &[100])]
#[test_case("test_data_artifacts/contracts/cairo_vm/u64_sqrt.contract.json", &[100])]
#[test_case("test_data_artifacts/contracts/cairo_vm/u8_sqrt.contract.json", &[100])]
#[test_case("test_data_artifacts/contracts/cairo_vm/uint512_div_mod.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/widemul128.contract.json", &[100, 100])]
#[test_case("test_data_artifacts/contracts/cairo_vm/field_sqrt.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/random_ec_point.contract.json", &[])]
#[test_case("test_data_artifacts/contracts/cairo_vm/alloc_constant_size.contract.json", &[10, 10, 10])]
#[test_case("test_data_artifacts/contracts/cairo_vm/heavy_blake2s.contract.json", &[])]
fn test_contract_cases(name: &str, args: &[u128]) {
    let args = args.iter().map(|&arg| arg.into()).collect_vec();

    let contract = load_contract(name);

    let entrypoint = contract
        .entry_points_by_type
        .external
        .first()
        .expect("contract should have at least one external entrypoint");

    let program = contract
        .extract_sierra_program(false)
        .expect("contract bytes should be a valid sierra program")
        .program;

    let native_result = run_native_starknet_contract(
        &program,
        entrypoint.function_idx,
        &args,
        DummySyscallHandler,
    );

    assert!(
        !native_result.failure_flag,
        "native contract execution failed"
    );

    let native_output = native_result.return_values;

    let vm_output = run_vm_contract(&contract, &entrypoint.selector, &args);

    assert_eq_sorted!(vm_output, native_output);
}

/// Test that the blake builtin counter accurately tracks the number of blake operations.
/// The heavy_blake2s contract calls blake2s_compress 6 times and blake2s_finalize once,
/// for a total of 7 blake operations.
#[test]
fn test_blake_builtin_counter() {
    let contract =
        load_contract("test_data_artifacts/contracts/cairo_vm/heavy_blake2s.contract.json");

    let entrypoint = contract
        .entry_points_by_type
        .external
        .first()
        .expect("contract should have at least one external entrypoint");

    let program = contract
        .extract_sierra_program(false)
        .expect("contract bytes should be a valid sierra program")
        .program;

    let native_result =
        run_native_starknet_contract(&program, entrypoint.function_idx, &[], DummySyscallHandler);

    assert!(
        !native_result.failure_flag,
        "native contract execution failed"
    );

    // heavy_blake2s: 6 blake2s_compress + 1 blake2s_finalize = 7 blake operations
    assert_eq!(
        native_result.builtin_stats.blake, 7,
        "blake counter should be exactly 7 (6 compress + 1 finalize)"
    );
}
