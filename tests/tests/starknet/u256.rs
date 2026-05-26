use crate::common::run_native_starknet_contract;
use cairo_native::starknet_stub::StubSyscallHandler;
use cairo_native::utils::testing::load_contract;
use starknet_types_core::felt::Felt;

#[test]
fn u256_test() {
    let contract = load_contract("contracts/u256_order.contract.json");

    let entry_point = contract.entry_points_by_type.external.first().unwrap();

    let program = contract.extract_sierra_program(false).unwrap().program;
    let result = run_native_starknet_contract(
        &program,
        entry_point.function_idx,
        &[],
        &mut StubSyscallHandler::default(),
    );

    assert!(!result.failure_flag);
    assert_eq!(
        result.return_values,
        vec![Felt::from_hex("0xf70cba9bb86caa97b086fdfa3df602ed").unwrap()]
    );
    assert_eq!(result.remaining_gas, 18446744073709353675);
}
