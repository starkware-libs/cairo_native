//! Bridge between cairo-native's [`StarknetSyscallHandler`](crate::starknet::StarknetSyscallHandler)
//! and the sierra-emu VM's
//! [`StarknetSyscallHandler`](::sierra_emu::starknet::StarknetSyscallHandler).
//!
//! The sierra-emu interpreter expects a syscall handler implementing its own copy of the
//! `StarknetSyscallHandler` trait, while cairo-native callers normally provide a handler that
//! implements the cairo-native version. [`SierraEmuSyscallBridge`] adapts the latter to the
//! former, performing the small set of type-level translations between the two crates.
//!
//! Available under the `sierra-emu` feature.

#![cfg(feature = "sierra-emu")]

use sierra_emu::starknet as emu;
use starknet_types_core::felt::Felt;

use crate::starknet as native;

/// Wraps a cairo-native [`StarknetSyscallHandler`](native::StarknetSyscallHandler) so it can be
/// driven by the sierra-emu VM, which expects [`emu::StarknetSyscallHandler`].
pub struct SierraEmuSyscallBridge<'a, H>(pub &'a mut H);

impl<H: native::StarknetSyscallHandler> emu::StarknetSyscallHandler
    for SierraEmuSyscallBridge<'_, H>
{
    fn get_block_hash(
        &mut self,
        block_number: u64,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Felt> {
        self.0.get_block_hash(block_number, remaining_gas)
    }

    fn get_execution_info(
        &mut self,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<emu::ExecutionInfo> {
        self.0.get_execution_info(remaining_gas).map(convert_execution_info)
    }

    fn get_execution_info_v2(
        &mut self,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<emu::ExecutionInfoV2> {
        self.0.get_execution_info_v2(remaining_gas).map(convert_execution_info_v2)
    }

    fn deploy(
        &mut self,
        class_hash: Felt,
        contract_address_salt: Felt,
        calldata: Vec<Felt>,
        deploy_from_zero: bool,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<(Felt, Vec<Felt>)> {
        self.0.deploy(class_hash, contract_address_salt, &calldata, deploy_from_zero, remaining_gas)
    }

    fn replace_class(
        &mut self,
        class_hash: Felt,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<()> {
        self.0.replace_class(class_hash, remaining_gas)
    }

    fn library_call(
        &mut self,
        class_hash: Felt,
        function_selector: Felt,
        calldata: Vec<Felt>,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Vec<Felt>> {
        self.0.library_call(class_hash, function_selector, &calldata, remaining_gas)
    }

    fn call_contract(
        &mut self,
        address: Felt,
        entry_point_selector: Felt,
        calldata: Vec<Felt>,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Vec<Felt>> {
        self.0.call_contract(address, entry_point_selector, &calldata, remaining_gas)
    }

    fn storage_read(
        &mut self,
        address_domain: u32,
        address: Felt,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Felt> {
        self.0.storage_read(address_domain, address, remaining_gas)
    }

    fn storage_write(
        &mut self,
        address_domain: u32,
        address: Felt,
        value: Felt,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<()> {
        self.0.storage_write(address_domain, address, value, remaining_gas)
    }

    fn emit_event(
        &mut self,
        keys: Vec<Felt>,
        data: Vec<Felt>,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<()> {
        self.0.emit_event(&keys, &data, remaining_gas)
    }

    fn send_message_to_l1(
        &mut self,
        to_address: Felt,
        payload: Vec<Felt>,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<()> {
        self.0.send_message_to_l1(to_address, &payload, remaining_gas)
    }

    fn keccak(
        &mut self,
        input: Vec<u64>,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<emu::U256> {
        self.0.keccak(&input, remaining_gas).map(convert_u256)
    }

    fn secp256k1_new(
        &mut self,
        x: emu::U256,
        y: emu::U256,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Option<emu::Secp256k1Point>> {
        self.0
            .secp256k1_new(convert_from_u256(x), convert_from_u256(y), remaining_gas)
            .map(|opt| opt.map(convert_secp_256_k1_point))
    }

    fn secp256k1_add(
        &mut self,
        p0: emu::Secp256k1Point,
        p1: emu::Secp256k1Point,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<emu::Secp256k1Point> {
        self.0
            .secp256k1_add(
                convert_from_secp_256_k1_point(p0),
                convert_from_secp_256_k1_point(p1),
                remaining_gas,
            )
            .map(convert_secp_256_k1_point)
    }

    fn secp256k1_mul(
        &mut self,
        p: emu::Secp256k1Point,
        m: emu::U256,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<emu::Secp256k1Point> {
        self.0
            .secp256k1_mul(convert_from_secp_256_k1_point(p), convert_from_u256(m), remaining_gas)
            .map(convert_secp_256_k1_point)
    }

    fn secp256k1_get_point_from_x(
        &mut self,
        x: emu::U256,
        y_parity: bool,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Option<emu::Secp256k1Point>> {
        self.0
            .secp256k1_get_point_from_x(convert_from_u256(x), y_parity, remaining_gas)
            .map(|opt| opt.map(convert_secp_256_k1_point))
    }

    fn secp256k1_get_xy(
        &mut self,
        p: emu::Secp256k1Point,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<(emu::U256, emu::U256)> {
        self.0
            .secp256k1_get_xy(convert_from_secp_256_k1_point(p), remaining_gas)
            .map(|(x, y)| (convert_u256(x), convert_u256(y)))
    }

    fn secp256r1_new(
        &mut self,
        x: emu::U256,
        y: emu::U256,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Option<emu::Secp256r1Point>> {
        self.0
            .secp256r1_new(convert_from_u256(x), convert_from_u256(y), remaining_gas)
            .map(|opt| opt.map(convert_secp_256_r1_point))
    }

    fn secp256r1_add(
        &mut self,
        p0: emu::Secp256r1Point,
        p1: emu::Secp256r1Point,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<emu::Secp256r1Point> {
        self.0
            .secp256r1_add(
                convert_from_secp_256_r1_point(p0),
                convert_from_secp_256_r1_point(p1),
                remaining_gas,
            )
            .map(convert_secp_256_r1_point)
    }

    fn secp256r1_mul(
        &mut self,
        p: emu::Secp256r1Point,
        m: emu::U256,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<emu::Secp256r1Point> {
        self.0
            .secp256r1_mul(convert_from_secp_256_r1_point(p), convert_from_u256(m), remaining_gas)
            .map(convert_secp_256_r1_point)
    }

    fn secp256r1_get_point_from_x(
        &mut self,
        x: emu::U256,
        y_parity: bool,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Option<emu::Secp256r1Point>> {
        self.0
            .secp256r1_get_point_from_x(convert_from_u256(x), y_parity, remaining_gas)
            .map(|opt| opt.map(convert_secp_256_r1_point))
    }

    fn secp256r1_get_xy(
        &mut self,
        p: emu::Secp256r1Point,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<(emu::U256, emu::U256)> {
        self.0
            .secp256r1_get_xy(convert_from_secp_256_r1_point(p), remaining_gas)
            .map(|(x, y)| (convert_u256(x), convert_u256(y)))
    }

    fn sha256_process_block(
        &mut self,
        mut prev_state: [u32; 8],
        current_block: [u32; 16],
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<[u32; 8]> {
        self.0.sha256_process_block(&mut prev_state, &current_block, remaining_gas)?;
        Ok(prev_state)
    }

    fn meta_tx_v0(
        &mut self,
        address: Felt,
        entry_point_selector: Felt,
        calldata: Vec<Felt>,
        signature: Vec<Felt>,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Vec<Felt>> {
        self.0.meta_tx_v0(address, entry_point_selector, &calldata, &signature, remaining_gas)
    }

    fn get_class_hash_at(
        &mut self,
        contract_address: Felt,
        remaining_gas: &mut u64,
    ) -> emu::SyscallResult<Felt> {
        self.0.get_class_hash_at(contract_address, remaining_gas)
    }
}

// --- Type conversion helpers between cairo-native and sierra-emu syscall types. ---

fn convert_u256(x: native::U256) -> emu::U256 {
    emu::U256 { lo: x.lo, hi: x.hi }
}

fn convert_from_u256(x: emu::U256) -> native::U256 {
    native::U256 { lo: x.lo, hi: x.hi }
}

fn convert_secp_256_k1_point(x: native::Secp256k1Point) -> emu::Secp256k1Point {
    emu::Secp256k1Point { x: convert_u256(x.x), y: convert_u256(x.y) }
}

fn convert_from_secp_256_k1_point(x: emu::Secp256k1Point) -> native::Secp256k1Point {
    // sierra-emu has no `is_infinity` flag and represents the identity element as (0, 0).
    // Detect that here so cairo-native (which has the flag) treats it as infinity rather
    // than the literal point (0, 0), which is not on the curve.
    let is_infinity = is_u256_zero(&x.x) && is_u256_zero(&x.y);
    native::Secp256k1Point { x: convert_from_u256(x.x), y: convert_from_u256(x.y), is_infinity }
}

fn convert_secp_256_r1_point(x: native::Secp256r1Point) -> emu::Secp256r1Point {
    emu::Secp256r1Point { x: convert_u256(x.x), y: convert_u256(x.y) }
}

fn convert_from_secp_256_r1_point(x: emu::Secp256r1Point) -> native::Secp256r1Point {
    let is_infinity = is_u256_zero(&x.x) && is_u256_zero(&x.y);
    native::Secp256r1Point { x: convert_from_u256(x.x), y: convert_from_u256(x.y), is_infinity }
}

fn is_u256_zero(x: &emu::U256) -> bool {
    x.lo == 0 && x.hi == 0
}

fn convert_execution_info(x: native::ExecutionInfo) -> emu::ExecutionInfo {
    emu::ExecutionInfo {
        block_info: convert_block_info(x.block_info),
        tx_info: convert_tx_info(x.tx_info),
        caller_address: x.caller_address,
        contract_address: x.contract_address,
        entry_point_selector: x.entry_point_selector,
    }
}

fn convert_tx_info(x: native::TxInfo) -> emu::TxInfo {
    emu::TxInfo {
        version: x.version,
        account_contract_address: x.account_contract_address,
        max_fee: x.max_fee,
        signature: x.signature,
        transaction_hash: x.transaction_hash,
        chain_id: x.chain_id,
        nonce: x.nonce,
    }
}

fn convert_execution_info_v2(x: native::ExecutionInfoV2) -> emu::ExecutionInfoV2 {
    emu::ExecutionInfoV2 {
        block_info: convert_block_info(x.block_info),
        tx_info: convert_tx_v2_info(x.tx_info),
        caller_address: x.caller_address,
        contract_address: x.contract_address,
        entry_point_selector: x.entry_point_selector,
    }
}

fn convert_tx_v2_info(x: native::TxV2Info) -> emu::TxV2Info {
    emu::TxV2Info {
        version: x.version,
        account_contract_address: x.account_contract_address,
        max_fee: x.max_fee,
        signature: x.signature,
        transaction_hash: x.transaction_hash,
        chain_id: x.chain_id,
        nonce: x.nonce,
        resource_bounds: x.resource_bounds.into_iter().map(convert_resource_bounds).collect(),
        tip: x.tip,
        paymaster_data: x.paymaster_data,
        nonce_data_availability_mode: x.nonce_data_availability_mode,
        fee_data_availability_mode: x.fee_data_availability_mode,
        account_deployment_data: x.account_deployment_data,
    }
}

fn convert_resource_bounds(x: native::ResourceBounds) -> emu::ResourceBounds {
    emu::ResourceBounds {
        resource: x.resource,
        max_amount: x.max_amount,
        max_price_per_unit: x.max_price_per_unit,
    }
}

fn convert_block_info(x: native::BlockInfo) -> emu::BlockInfo {
    emu::BlockInfo {
        block_number: x.block_number,
        block_timestamp: x.block_timestamp,
        sequencer_address: x.sequencer_address,
    }
}
