//! Regression test for the per-invocation box arena (`BoxArenaGuard`).
//!
//! A caller contract allocates two `Box<felt252>` values, then invokes a callee
//! contract via `call_contract_syscall`, then continues using its boxes after
//! the syscall returns. With a shared/global arena (the pre-fix design) the
//! callee's invocation-end reset would free the caller's live boxes; a
//! subsequent allocation in the caller would land on the same slot, producing
//! a use-after-free. With per-invocation arenas swapped in/out by
//! [`cairo_native::runtime::BoxArenaGuard`], the caller's boxes survive the
//! nested call intact.
//!
//! The contract-to-contract dispatch is handled by [`MultiContractHandler`],
//! a syscall handler that routes `call_contract` to a registered
//! [`AotContractExecutor`] by contract address.

use cairo_lang_starknet_classes::{
    casm_contract_class::ENTRY_POINT_COST, contract_class::ContractClass,
};
use cairo_native::{
    executor::AotContractExecutor,
    starknet::{
        ExecutionInfo, ExecutionInfoV2, ExecutionInfoV3, Secp256k1Point, Secp256r1Point,
        StarknetSyscallHandler, SyscallResult, U256,
    },
    utils::testing::load_contract,
    OptLevel,
};
use starknet_types_core::felt::Felt;
use std::{collections::HashMap, sync::Arc};

struct MultiContractHandler {
    contracts: HashMap<Felt, Arc<AotContractExecutor>>,
}

impl MultiContractHandler {
    fn new() -> Self {
        Self {
            contracts: HashMap::new(),
        }
    }

    fn register(&mut self, address: Felt, executor: AotContractExecutor) {
        self.contracts.insert(address, Arc::new(executor));
    }
}

impl StarknetSyscallHandler for &mut MultiContractHandler {
    fn call_contract(
        &mut self,
        address: Felt,
        entry_point_selector: Felt,
        calldata: &[Felt],
        remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        let executor = self
            .contracts
            .get(&address)
            .unwrap_or_else(|| panic!("no contract registered at address {address}"))
            .clone();

        let result = executor
            .run(
                entry_point_selector,
                calldata,
                *remaining_gas,
                None,
                &mut **self,
            )
            .expect("nested contract execution failed");

        *remaining_gas = result.remaining_gas;

        if result.failure_flag {
            return Err(result.return_values);
        }
        Ok(result.return_values)
    }

    fn get_block_hash(
        &mut self,
        _block_number: u64,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        unimplemented!()
    }
    fn get_execution_info(&mut self, _remaining_gas: &mut u64) -> SyscallResult<ExecutionInfo> {
        unimplemented!()
    }
    fn get_execution_info_v2(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<ExecutionInfoV2> {
        unimplemented!()
    }
    fn get_execution_info_v3(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<ExecutionInfoV3> {
        unimplemented!()
    }
    fn deploy(
        &mut self,
        _class_hash: Felt,
        _contract_address_salt: Felt,
        _calldata: &[Felt],
        _deploy_from_zero: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(Felt, Vec<Felt>)> {
        unimplemented!()
    }
    fn replace_class(&mut self, _class_hash: Felt, _remaining_gas: &mut u64) -> SyscallResult<()> {
        unimplemented!()
    }
    fn library_call(
        &mut self,
        _class_hash: Felt,
        _function_selector: Felt,
        _calldata: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        unimplemented!()
    }
    fn storage_read(
        &mut self,
        _address_domain: u32,
        _address: Felt,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        unimplemented!()
    }
    fn storage_write(
        &mut self,
        _address_domain: u32,
        _address: Felt,
        _value: Felt,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        unimplemented!()
    }
    fn emit_event(
        &mut self,
        _keys: &[Felt],
        _data: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        unimplemented!()
    }
    fn send_message_to_l1(
        &mut self,
        _to_address: Felt,
        _payload: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        unimplemented!()
    }
    fn keccak(&mut self, _input: &[u64], _remaining_gas: &mut u64) -> SyscallResult<U256> {
        unimplemented!()
    }
    fn secp256k1_new(
        &mut self,
        _x: U256,
        _y: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        unimplemented!()
    }
    fn secp256k1_add(
        &mut self,
        _p0: Secp256k1Point,
        _p1: Secp256k1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        unimplemented!()
    }
    fn secp256k1_mul(
        &mut self,
        _p: Secp256k1Point,
        _m: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        unimplemented!()
    }
    fn secp256k1_get_point_from_x(
        &mut self,
        _x: U256,
        _y_parity: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        unimplemented!()
    }
    fn secp256k1_get_xy(
        &mut self,
        _p: Secp256k1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        unimplemented!()
    }
    fn secp256r1_new(
        &mut self,
        _x: U256,
        _y: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        unimplemented!()
    }
    fn secp256r1_add(
        &mut self,
        _p0: Secp256r1Point,
        _p1: Secp256r1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        unimplemented!()
    }
    fn secp256r1_mul(
        &mut self,
        _p: Secp256r1Point,
        _m: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        unimplemented!()
    }
    fn secp256r1_get_point_from_x(
        &mut self,
        _x: U256,
        _y_parity: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        unimplemented!()
    }
    fn secp256r1_get_xy(
        &mut self,
        _p: Secp256r1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        unimplemented!()
    }
    fn sha256_process_block(
        &mut self,
        _state: &mut [u32; 8],
        _block: &[u32; 16],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        unimplemented!()
    }
    fn get_class_hash_at(
        &mut self,
        _contract_address: Felt,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        unimplemented!()
    }
    fn meta_tx_v0(
        &mut self,
        _address: Felt,
        _entry_point_selector: Felt,
        _calldata: &[Felt],
        _signature: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        unimplemented!()
    }
}

fn build_executor(contract: &ContractClass) -> (AotContractExecutor, Felt) {
    let extracted = contract
        .extract_sierra_program(false)
        .expect("failed to extract sierra program");

    let executor = AotContractExecutor::new(
        &extracted.program,
        &contract.entry_points_by_type,
        extracted.sierra_version,
        OptLevel::Default,
        None,
    )
    .expect("failed to build AotContractExecutor");

    let selector = Felt::from(
        &contract
            .entry_points_by_type
            .external
            .first()
            .expect("contract should have an external entry point")
            .selector,
    );

    (executor, selector)
}

#[test]
fn test_boxes_stay_valid_across_contract_calls() {
    let callee = load_contract("test_data_artifacts/contracts/box_arena/callee.contract.json");
    let caller = load_contract("test_data_artifacts/contracts/box_arena/caller.contract.json");

    let (callee_executor, add_one_selector) = build_executor(&callee);
    let (caller_executor, proxy_selector) = build_executor(&caller);

    let callee_address = Felt::from(0x42);
    let mut handler = MultiContractHandler::new();
    handler.register(callee_address, callee_executor);

    let input = Felt::from(5);
    let result = caller_executor
        .run(
            proxy_selector,
            &[callee_address, add_one_selector, input],
            u64::MAX - ENTRY_POINT_COST as u64,
            None,
            &mut handler,
        )
        .expect("caller contract execution failed");

    assert!(
        !result.failure_flag,
        "caller execution panicked: {:?}",
        result.error_msg
    );
    // Caller returns (recovered, clobber_val).
    // Under correct arena scoping: recovered == x == 5 (the box survived the syscall).
    // Under the current flat-reset design: recovered == *result[0] == 6 (UAF — the
    // box's slot was reused by `clobber` after the callee reset the arena).
    assert_eq!(
        result.return_values,
        vec![Felt::from(5), Felt::from(6)],
        "UAF: caller's pre-syscall box was freed by the nested callee's reset_box_arena"
    );
}
