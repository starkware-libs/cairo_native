use crate::common::{run_native_starknet_aot_contract, run_native_starknet_contract};
use cairo_native::{starknet_stub::StubSyscallHandler, utils::testing::load_contract};

#[test]
fn keccak_test() {
    let contract = load_contract("contracts/keccak.contract.json");

    let entry_point = contract.entry_points_by_type.external.first().unwrap();

    let program = contract.extract_sierra_program(false).unwrap().program;
    let result = run_native_starknet_contract(
        &program,
        entry_point.function_idx,
        &[],
        &mut StubSyscallHandler::default(),
    );

    assert!(!result.failure_flag);
    assert_eq!(result.remaining_gas, 18446744073709326285);
    assert_eq!(result.return_values, vec![1.into()]);

    let result_aot_ct = run_native_starknet_aot_contract(
        &contract,
        &entry_point.selector,
        &[],
        &mut StubSyscallHandler::default(),
    );

    assert!(!result_aot_ct.failure_flag);
    assert_eq!(result_aot_ct.remaining_gas, result.remaining_gas);
    assert_eq!(result_aot_ct.return_values, vec![1.into()]);
}
