use cairo_lang_compiler::CompilerConfig;
use cairo_lang_lowering::utils::InliningStrategy;
use cairo_lang_starknet::compile::compile_path;
use cairo_native::{
    context::NativeContext,
    executor::JitNativeExecutor,
    starknet::{
        BlockInfo, ExecutionInfo, ExecutionInfoV2, ResourceBounds, Secp256k1Point, Secp256r1Point,
        StarknetSyscallHandler, SyscallResult, TxInfo, TxV2Info, U256,
    },
    utils::find_entry_point_by_idx,
};
use starknet_types_core::felt::Felt;
use std::path::Path;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[derive(Debug)]
struct SyscallHandler;

impl StarknetSyscallHandler for SyscallHandler {
    fn get_block_hash(&mut self, block_number: u64, _gas: &mut u64) -> SyscallResult<Felt> {
        println!("Called `get_block_hash({block_number})` from MLIR.");
        Ok(Felt::from_bytes_be_slice(b"get_block_hash ok"))
    }

    fn get_execution_info(
        &mut self,
        _gas: &mut u64,
    ) -> SyscallResult<cairo_native::starknet::ExecutionInfo> {
        println!("Called `get_execution_info()` from MLIR.");
        Ok(ExecutionInfo {
            block_info: BlockInfo {
                block_number: 1234,
                block_timestamp: 2345,
                sequencer_address: 3456.into(),
            },
            tx_info: TxInfo {
                version: 4567.into(),
                account_contract_address: 5678.into(),
                max_fee: 6789,
                signature: vec![1248.into(), 2486.into()],
                transaction_hash: 9876.into(),
                chain_id: 8765.into(),
                nonce: 7654.into(),
            },
            caller_address: 6543.into(),
            contract_address: 5432.into(),
            entry_point_selector: 4321.into(),
        })
    }

    fn get_execution_info_v2(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<cairo_native::starknet::ExecutionInfoV2> {
        println!("Called `get_execution_info_v2()` from MLIR.");
        Ok(ExecutionInfoV2 {
            block_info: BlockInfo {
                block_number: 1234,
                block_timestamp: 2345,
                sequencer_address: 3456.into(),
            },
            tx_info: TxV2Info {
                version: 1.into(),
                account_contract_address: 1.into(),
                max_fee: 0,
                signature: vec![1.into()],
                transaction_hash: 1.into(),
                chain_id: 1.into(),
                nonce: 1.into(),
                tip: 1,
                paymaster_data: vec![1.into()],
                nonce_data_availability_mode: 0,
                fee_data_availability_mode: 0,
                account_deployment_data: vec![1.into()],
                resource_bounds: vec![ResourceBounds {
                    resource: 2.into(),
                    max_amount: 10,
                    max_price_per_unit: 20,
                }],
            },
            caller_address: 6543.into(),
            contract_address: 5432.into(),
            entry_point_selector: 4321.into(),
        })
    }

    fn get_execution_info_v3(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<cairo_native::starknet::ExecutionInfoV3> {
        println!("Called `get_execution_info_v3()` from MLIR.");
        todo!();
    }

    fn deploy(
        &mut self,
        class_hash: Felt,
        contract_address_salt: Felt,
        calldata: &[Felt],
        deploy_from_zero: bool,
        _gas: &mut u64,
    ) -> SyscallResult<(Felt, Vec<Felt>)> {
        println!("Called `deploy({class_hash}, {contract_address_salt}, {calldata:?}, {deploy_from_zero})` from MLIR.");
        Ok((
            class_hash + contract_address_salt,
            calldata.iter().map(|x| x + Felt::ONE).collect(),
        ))
    }

    fn replace_class(&mut self, class_hash: Felt, _gas: &mut u64) -> SyscallResult<()> {
        println!("Called `replace_class({class_hash})` from MLIR.");
        Ok(())
    }

    fn library_call(
        &mut self,
        class_hash: Felt,
        function_selector: Felt,
        calldata: &[Felt],
        _gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        println!(
            "Called `library_call({class_hash}, {function_selector}, {calldata:?})` from MLIR."
        );
        Ok(calldata.iter().map(|x| x * Felt::from(3)).collect())
    }

    fn call_contract(
        &mut self,
        address: Felt,
        entry_point_selector: Felt,
        calldata: &[Felt],
        _gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        println!(
            "Called `call_contract({address}, {entry_point_selector}, {calldata:?})` from MLIR."
        );
        Ok(calldata.iter().map(|x| x * Felt::from(3)).collect())
    }

    fn storage_read(
        &mut self,
        address_domain: u32,
        address: Felt,
        _gas: &mut u64,
    ) -> SyscallResult<Felt> {
        println!("Called `storage_read({address_domain}, {address})` from MLIR.");
        Ok(address * Felt::from(3))
    }

    fn storage_write(
        &mut self,
        address_domain: u32,
        address: Felt,
        value: Felt,
        _gas: &mut u64,
    ) -> SyscallResult<()> {
        println!("Called `storage_write({address_domain}, {address}, {value})` from MLIR.");
        Ok(())
    }

    fn emit_event(&mut self, keys: &[Felt], data: &[Felt], _gas: &mut u64) -> SyscallResult<()> {
        println!("Called `emit_event({keys:?}, {data:?})` from MLIR.");
        Ok(())
    }

    fn send_message_to_l1(
        &mut self,
        to_address: Felt,
        payload: &[Felt],
        _gas: &mut u64,
    ) -> SyscallResult<()> {
        println!("Called `send_message_to_l1({to_address}, {payload:?})` from MLIR.");
        Ok(())
    }

    fn keccak(
        &mut self,
        input: &[u64],
        _gas: &mut u64,
    ) -> SyscallResult<cairo_native::starknet::U256> {
        println!("Called `keccak({input:?})` from MLIR.");
        Ok(U256 {
            hi: 0,
            lo: 1234567890,
        })
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

    fn sha512_process_block(
        &mut self,
        _state: &mut [u64; 8],
        _block: &[u64; 16],
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

fn main() {
    // Configure logging and error handling.
    tracing::subscriber::set_global_default(
        FmtSubscriber::builder()
            .with_env_filter(EnvFilter::from_default_env())
            .finish(),
    )
    .unwrap();

    let path = Path::new("programs/erc20.cairo");

    let contract = compile_path(
        path,
        None,
        CompilerConfig {
            replace_ids: true,
            ..Default::default()
        },
        InliningStrategy::Default,
    )
    .unwrap();

    let entry_point = contract.entry_points_by_type.constructor.first().unwrap();
    let sierra_program = contract.extract_sierra_program(false).unwrap().program;

    let native_context = NativeContext::new();

    let native_program = native_context
        .compile(&sierra_program, false, Some(Default::default()), None)
        .unwrap();

    let entry_point_fn =
        find_entry_point_by_idx(&sierra_program, entry_point.function_idx).unwrap();
    let fn_id = &entry_point_fn.id;

    let native_executor =
        JitNativeExecutor::from_native_module(native_program, Default::default()).unwrap();

    let result = native_executor
        .invoke_contract_dynamic(
            fn_id,
            &[
                Felt::from_bytes_be_slice(b"name"),
                Felt::from_bytes_be_slice(b"symbol"),
                Felt::ZERO,
                Felt::from(i64::MAX),
                Felt::from(4),
                Felt::from(6),
            ],
            Some(u64::MAX),
            SyscallHandler,
        )
        .expect("failed to execute the given contract");

    println!();
    println!("Cairo program was compiled and executed successfully.");
    println!("{result:#?}");
}
