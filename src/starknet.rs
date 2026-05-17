//! Starknet related code for `cairo_native`

use crate::types::array::ArrayMetadata;
use serde::{Deserialize, Serialize};
use starknet_types_core::felt::Felt;

pub type SyscallResult<T> = std::result::Result<T, Vec<Felt>>;

#[repr(C)]
#[derive(Debug)]
pub struct ArrayAbi<T> {
    pub ptr: *mut *mut T,
    pub since: u32,
    pub until: u32,
    pub capacity: u32,
}

impl From<&ArrayAbi<Felt252Abi>> for Vec<Felt> {
    fn from(value: &ArrayAbi<Felt252Abi>) -> Self {
        unsafe {
            let since_offset = value.since as usize;
            let until_offset = value.until as usize;
            debug_assert!(since_offset <= until_offset);
            let len = until_offset - since_offset;
            match len {
                0 => &[],
                _ => {
                    // Access data through ArrayMetadata.data_ptr
                    let metadata = &*value.ptr.cast::<ArrayMetadata>();
                    let data_ptr = metadata.data_ptr.cast::<Felt252Abi>();
                    std::slice::from_raw_parts(data_ptr.add(since_offset), len)
                }
            }
        }
        .iter()
        .map(Felt::from)
        .collect()
    }
}

/// Binary representation of a `Felt` (in MLIR).
#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
pub struct Felt252Abi(pub [u8; 32]);

impl From<Felt252Abi> for Felt {
    fn from(mut value: Felt252Abi) -> Felt {
        value.0[31] &= 0x0F;
        Felt::from_bytes_le(&value.0)
    }
}

impl From<&Felt252Abi> for Felt {
    fn from(value: &Felt252Abi) -> Felt {
        let mut value = *value;
        value.0[31] &= 0x0F;
        Felt::from_bytes_le(&value.0)
    }
}

/// Binary representation of a `u256` (in MLIR).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    Default,
)]
#[repr(C, align(16))]
pub struct U256 {
    pub lo: u128,
    pub hi: u128,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct ExecutionInfo {
    pub block_info: BlockInfo,
    pub tx_info: TxInfo,
    pub caller_address: Felt,
    pub contract_address: Felt,
    pub entry_point_selector: Felt,
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ExecutionInfoV2 {
    pub block_info: BlockInfo,
    pub tx_info: TxV2Info,
    pub caller_address: Felt,
    pub contract_address: Felt,
    pub entry_point_selector: Felt,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct TxV2Info {
    pub version: Felt,
    pub account_contract_address: Felt,
    pub max_fee: u128,
    pub signature: Vec<Felt>,
    pub transaction_hash: Felt,
    pub chain_id: Felt,
    pub nonce: Felt,
    pub resource_bounds: Vec<ResourceBounds>,
    pub tip: u128,
    pub paymaster_data: Vec<Felt>,
    pub nonce_data_availability_mode: u32,
    pub fee_data_availability_mode: u32,
    pub account_deployment_data: Vec<Felt>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ExecutionInfoV3 {
    pub block_info: BlockInfo,
    pub tx_info: TxV3Info,
    pub caller_address: Felt,
    pub contract_address: Felt,
    pub entry_point_selector: Felt,
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TxV3Info {
    pub version: Felt,
    pub account_contract_address: Felt,
    pub max_fee: u128,
    pub signature: Vec<Felt>,
    pub transaction_hash: Felt,
    pub chain_id: Felt,
    pub nonce: Felt,
    pub resource_bounds: Vec<ResourceBounds>,
    pub tip: u128,
    pub paymaster_data: Vec<Felt>,
    pub nonce_data_availability_mode: u32,
    pub fee_data_availability_mode: u32,
    pub account_deployment_data: Vec<Felt>,
    pub proof_facts: Vec<Felt>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct ResourceBounds {
    pub resource: Felt,
    pub max_amount: u64,
    pub max_price_per_unit: u128,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct BlockInfo {
    pub block_number: u64,
    pub block_timestamp: u64,
    pub sequencer_address: Felt,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct TxInfo {
    pub version: Felt,
    pub account_contract_address: Felt,
    pub max_fee: u128,
    pub signature: Vec<Felt>,
    pub transaction_hash: Felt,
    pub chain_id: Felt,
    pub nonce: Felt,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize, Default)]
#[repr(C, align(16))]
pub struct Secp256k1Point {
    pub x: U256,
    pub y: U256,
    pub is_infinity: bool,
}

impl Secp256k1Point {
    pub const fn new(x_lo: u128, x_hi: u128, y_lo: u128, y_hi: u128, is_infinity: bool) -> Self {
        Self {
            x: U256 { lo: x_lo, hi: x_hi },
            y: U256 { lo: y_lo, hi: y_hi },
            is_infinity,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize, Default)]
#[repr(C, align(16))]
pub struct Secp256r1Point {
    pub x: U256,
    pub y: U256,
    pub is_infinity: bool,
}

impl Secp256r1Point {
    pub const fn new(x_lo: u128, x_hi: u128, y_lo: u128, y_hi: u128, is_infinity: bool) -> Self {
        Self {
            x: U256 { lo: x_lo, hi: x_hi },
            y: U256 { lo: y_lo, hi: y_hi },
            is_infinity,
        }
    }
}

pub trait StarknetSyscallHandler {
    fn get_block_hash(&mut self, block_number: u64, remaining_gas: &mut u64)
        -> SyscallResult<Felt>;

    fn get_execution_info(&mut self, remaining_gas: &mut u64) -> SyscallResult<ExecutionInfo>;

    fn get_execution_info_v2(&mut self, remaining_gas: &mut u64) -> SyscallResult<ExecutionInfoV2>;

    fn get_execution_info_v3(&mut self, remaining_gas: &mut u64) -> SyscallResult<ExecutionInfoV3>;

    fn deploy(
        &mut self,
        class_hash: Felt,
        contract_address_salt: Felt,
        calldata: &[Felt],
        deploy_from_zero: bool,
        remaining_gas: &mut u64,
    ) -> SyscallResult<(Felt, Vec<Felt>)>;
    fn replace_class(&mut self, class_hash: Felt, remaining_gas: &mut u64) -> SyscallResult<()>;

    fn library_call(
        &mut self,
        class_hash: Felt,
        function_selector: Felt,
        calldata: &[Felt],
        remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>>;

    fn call_contract(
        &mut self,
        address: Felt,
        entry_point_selector: Felt,
        calldata: &[Felt],
        remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>>;

    fn storage_read(
        &mut self,
        address_domain: u32,
        address: Felt,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Felt>;

    fn storage_write(
        &mut self,
        address_domain: u32,
        address: Felt,
        value: Felt,
        remaining_gas: &mut u64,
    ) -> SyscallResult<()>;

    fn emit_event(
        &mut self,
        keys: &[Felt],
        data: &[Felt],
        remaining_gas: &mut u64,
    ) -> SyscallResult<()>;

    fn send_message_to_l1(
        &mut self,
        to_address: Felt,
        payload: &[Felt],
        remaining_gas: &mut u64,
    ) -> SyscallResult<()>;

    fn keccak(&mut self, input: &[u64], remaining_gas: &mut u64) -> SyscallResult<U256>;

    fn secp256k1_new(
        &mut self,
        x: U256,
        y: U256,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>>;

    fn secp256k1_add(
        &mut self,
        p0: Secp256k1Point,
        p1: Secp256k1Point,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point>;

    fn secp256k1_mul(
        &mut self,
        p: Secp256k1Point,
        m: U256,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point>;

    fn secp256k1_get_point_from_x(
        &mut self,
        x: U256,
        y_parity: bool,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>>;

    fn secp256k1_get_xy(
        &mut self,
        p: Secp256k1Point,
        remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)>;

    fn secp256r1_new(
        &mut self,
        x: U256,
        y: U256,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>>;

    fn secp256r1_add(
        &mut self,
        p0: Secp256r1Point,
        p1: Secp256r1Point,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point>;

    fn secp256r1_mul(
        &mut self,
        p: Secp256r1Point,
        m: U256,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point>;

    fn secp256r1_get_point_from_x(
        &mut self,
        x: U256,
        y_parity: bool,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>>;

    fn secp256r1_get_xy(
        &mut self,
        p: Secp256r1Point,
        remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)>;

    fn sha256_process_block(
        &mut self,
        state: &mut [u32; 8],
        block: &[u32; 16],
        remaining_gas: &mut u64,
    ) -> SyscallResult<()>;

    fn get_class_hash_at(
        &mut self,
        contract_address: Felt,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Felt>;

    fn meta_tx_v0(
        &mut self,
        address: Felt,
        entry_point_selector: Felt,
        calldata: &[Felt],
        signature: &[Felt],
        remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>>;

    fn cheatcode(&mut self, _selector: Felt, _input: &[Felt]) -> Vec<Felt> {
        unimplemented!();
    }
}

pub struct DummySyscallHandler;

impl StarknetSyscallHandler for DummySyscallHandler {
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

    fn call_contract(
        &mut self,
        _address: Felt,
        _entry_point_selector: Felt,
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

// TODO: Move to the correct place or remove if unused. See: https://github.com/starkware-libs/cairo_native/issues/1222
pub(crate) mod handler {
    use super::*;
    use crate::{
        types::array::ArrayMetadata,
        utils::{libc_free, libc_malloc},
    };
    use std::{
        alloc::Layout,
        fmt::Debug,
        mem::{size_of, ManuallyDrop, MaybeUninit},
        ptr::{null_mut, NonNull},
    };

    macro_rules! field_offset {
        ( $ident:path, $field:ident ) => {
            unsafe {
                let value_ptr = std::mem::MaybeUninit::<$ident>::uninit().as_ptr();
                let field_ptr: *const u8 = std::ptr::addr_of!((*value_ptr).$field) as *const u8;
                field_ptr.offset_from(value_ptr as *const u8) as usize
            }
        };
    }

    #[repr(C)]
    pub union SyscallResultAbi<T> {
        pub ok: ManuallyDrop<SyscallResultAbiOk<T>>,
        pub err: ManuallyDrop<SyscallResultAbiErr>,
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct SyscallResultAbiOk<T> {
        pub tag: u8,
        pub payload: ManuallyDrop<T>,
    }

    #[repr(C)]
    #[derive(Debug)]
    pub struct SyscallResultAbiErr {
        pub tag: u8,
        pub payload: ArrayAbi<Felt252Abi>,
    }

    #[repr(C)]
    struct ExecutionInfoAbi {
        block_info: NonNull<BlockInfoAbi>,
        tx_info: NonNull<TxInfoAbi>,
        caller_address: Felt252Abi,
        contract_address: Felt252Abi,
        entry_point_selector: Felt252Abi,
    }

    #[repr(C)]
    struct ExecutionInfoV2Abi {
        block_info: NonNull<BlockInfoAbi>,
        tx_info: NonNull<TxInfoV2Abi>,
        caller_address: Felt252Abi,
        contract_address: Felt252Abi,
        entry_point_selector: Felt252Abi,
    }

    #[repr(C)]
    struct TxInfoV2Abi {
        version: Felt252Abi,
        account_contract_address: Felt252Abi,
        max_fee: u128,
        signature: ArrayAbi<Felt252Abi>,
        transaction_hash: Felt252Abi,
        chain_id: Felt252Abi,
        nonce: Felt252Abi,
        resource_bounds: ArrayAbi<ResourceBoundsAbi>,
        tip: u128,
        paymaster_data: ArrayAbi<Felt252Abi>,
        nonce_data_availability_mode: u32,
        fee_data_availability_mode: u32,
        account_deployment_data: ArrayAbi<Felt252Abi>,
    }

    #[repr(C)]
    struct ExecutionInfoV3Abi {
        block_info: NonNull<BlockInfoAbi>,
        tx_info: NonNull<TxInfoV3Abi>,
        caller_address: Felt252Abi,
        contract_address: Felt252Abi,
        entry_point_selector: Felt252Abi,
    }

    #[repr(C)]
    struct TxInfoV3Abi {
        version: Felt252Abi,
        account_contract_address: Felt252Abi,
        max_fee: u128,
        signature: ArrayAbi<Felt252Abi>,
        transaction_hash: Felt252Abi,
        chain_id: Felt252Abi,
        nonce: Felt252Abi,
        resource_bounds: ArrayAbi<ResourceBoundsAbi>,
        tip: u128,
        paymaster_data: ArrayAbi<Felt252Abi>,
        nonce_data_availability_mode: u32,
        fee_data_availability_mode: u32,
        account_deployment_data: ArrayAbi<Felt252Abi>,
        proof_facts: ArrayAbi<Felt252Abi>,
    }

    #[repr(C)]
    #[derive(Debug, Clone)]
    struct ResourceBoundsAbi {
        resource: Felt252Abi,
        max_amount: u64,
        max_price_per_unit: u128,
    }

    #[repr(C)]
    struct BlockInfoAbi {
        block_number: u64,
        block_timestamp: u64,
        sequencer_address: Felt252Abi,
    }

    #[repr(C)]
    struct TxInfoAbi {
        version: Felt252Abi,
        account_contract_address: Felt252Abi,
        max_fee: u128,
        signature: ArrayAbi<Felt252Abi>,
        transaction_hash: Felt252Abi,
        chain_id: Felt252Abi,
        nonce: Felt252Abi,
    }

    /// A C ABI Wrapper around the StarknetSyscallHandler
    ///
    /// It contains pointers to functions which can be called through MLIR based on the field offset.
    /// The functions convert C ABI structures to the Rust equivalent and calls the wrapped implementation.
    ///
    /// Unlike runtime functions, the callback table is generic to the StarknetSyscallHandler,
    /// which allows the user to specify the desired implementation to use during the execution.
    #[repr(C)]
    #[derive(Debug)]
    pub struct StarknetSyscallHandlerCallbacks<'a, T> {
        self_ptr: &'a mut T,

        get_block_hash: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<Felt252Abi>,
            ptr: &mut T,
            gas: &mut u64,
            block_number: u64,
        ),
        get_execution_info: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<NonNull<ExecutionInfoAbi>>,
            ptr: &mut T,
            gas: &mut u64,
        ),
        get_execution_info_v2: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<NonNull<ExecutionInfoV2Abi>>,
            ptr: &mut T,
            gas: &mut u64,
        ),
        get_execution_info_v3: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<NonNull<ExecutionInfoV3Abi>>,
            ptr: &mut T,
            gas: &mut u64,
        ),
        deploy: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<(Felt252Abi, ArrayAbi<Felt252Abi>)>,
            ptr: &mut T,
            gas: &mut u64,
            class_hash: &Felt252Abi,
            contract_address_salt: &Felt252Abi,
            calldata: &ArrayAbi<Felt252Abi>,
            deploy_from_zero: bool,
        ),
        replace_class: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<()>,
            ptr: &mut T,
            _gas: &mut u64,
            class_hash: &Felt252Abi,
        ),
        library_call: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<ArrayAbi<Felt252Abi>>,
            ptr: &mut T,
            gas: &mut u64,
            class_hash: &Felt252Abi,
            function_selector: &Felt252Abi,
            calldata: &ArrayAbi<Felt252Abi>,
        ),
        call_contract: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<ArrayAbi<Felt252Abi>>,
            ptr: &mut T,
            gas: &mut u64,
            address: &Felt252Abi,
            entry_point_selector: &Felt252Abi,
            calldata: &ArrayAbi<Felt252Abi>,
        ),
        storage_read: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<Felt252Abi>,
            ptr: &mut T,
            gas: &mut u64,
            address_domain: u32,
            address: &Felt252Abi,
        ),
        storage_write: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<()>,
            ptr: &mut T,
            gas: &mut u64,
            address_domain: u32,
            address: &Felt252Abi,
            value: &Felt252Abi,
        ),
        emit_event: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<()>,
            ptr: &mut T,
            gas: &mut u64,
            keys: &ArrayAbi<Felt252Abi>,
            data: &ArrayAbi<Felt252Abi>,
        ),
        send_message_to_l1: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<()>,
            ptr: &mut T,
            gas: &mut u64,
            to_address: &Felt252Abi,
            data: &ArrayAbi<Felt252Abi>,
        ),
        keccak: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<U256>,
            ptr: &mut T,
            gas: &mut u64,
            input: &ArrayAbi<u64>,
        ),

        secp256k1_new: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<(u8, MaybeUninit<Secp256k1Point>)>,
            ptr: &mut T,
            gas: &mut u64,
            x: &U256,
            y: &U256,
        ),
        secp256k1_add: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<Secp256k1Point>,
            ptr: &mut T,
            gas: &mut u64,
            p0: &Secp256k1Point,
            p1: &Secp256k1Point,
        ),
        secp256k1_mul: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<Secp256k1Point>,
            ptr: &mut T,
            gas: &mut u64,
            p: &Secp256k1Point,
            scalar: &U256,
        ),
        secp256k1_get_point_from_x: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<(u8, MaybeUninit<Secp256k1Point>)>,
            ptr: &mut T,
            gas: &mut u64,
            x: &U256,
            y_parity: &bool,
        ),
        secp256k1_get_xy: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<(U256, U256)>,
            ptr: &mut T,
            gas: &mut u64,
            p: &Secp256k1Point,
        ),

        secp256r1_new: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<(u8, MaybeUninit<Secp256r1Point>)>,
            ptr: &mut T,
            gas: &mut u64,
            x: &U256,
            y: &U256,
        ),
        secp256r1_add: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<Secp256r1Point>,
            ptr: &mut T,
            gas: &mut u64,
            p0: &Secp256r1Point,
            p1: &Secp256r1Point,
        ),
        secp256r1_mul: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<Secp256r1Point>,
            ptr: &mut T,
            gas: &mut u64,
            p: &Secp256r1Point,
            scalar: &U256,
        ),
        secp256r1_get_point_from_x: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<(u8, MaybeUninit<Secp256r1Point>)>,
            ptr: &mut T,
            gas: &mut u64,
            x: &U256,
            y_parity: &bool,
        ),
        secp256r1_get_xy: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<(U256, U256)>,
            ptr: &mut T,
            gas: &mut u64,
            p: &Secp256r1Point,
        ),
        sha256_process_block: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<*mut [u32; 8]>,
            ptr: &mut T,
            gas: &mut u64,
            state: *mut [u32; 8],
            block: &[u32; 16],
        ),
        get_class_hash_at: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<Felt252Abi>,
            ptr: &mut T,
            gas: &mut u64,
            contract_address: &Felt252Abi,
        ),

        meta_tx_v0: extern "C" fn(
            result_ptr: &mut SyscallResultAbi<ArrayAbi<Felt252Abi>>,
            ptr: &mut T,
            gas: &mut u64,
            address: &Felt252Abi,
            entry_point_selector: &Felt252Abi,
            calldata: &ArrayAbi<Felt252Abi>,
            signature: &ArrayAbi<Felt252Abi>,
        ),

        // testing syscalls
        #[cfg(feature = "with-cheatcode")]
        pub cheatcode: extern "C" fn(
            result_ptr: &mut ArrayAbi<Felt252Abi>,
            ptr: &mut T,
            selector: &Felt252Abi,
            input: &ArrayAbi<Felt252Abi>,
        ),
    }

    impl<'a, T> StarknetSyscallHandlerCallbacks<'a, T>
    where
        T: 'a,
    {
        // Callback field indices.
        pub const CALL_CONTRACT: usize = field_offset!(Self, call_contract) >> 3;
        pub const DEPLOY: usize = field_offset!(Self, deploy) >> 3;
        pub const EMIT_EVENT: usize = field_offset!(Self, emit_event) >> 3;
        pub const GET_BLOCK_HASH: usize = field_offset!(Self, get_block_hash) >> 3;
        pub const GET_EXECUTION_INFO: usize = field_offset!(Self, get_execution_info) >> 3;
        pub const GET_EXECUTION_INFOV2: usize = field_offset!(Self, get_execution_info_v2) >> 3;
        pub const GET_EXECUTION_INFOV3: usize = field_offset!(Self, get_execution_info_v3) >> 3;
        pub const KECCAK: usize = field_offset!(Self, keccak) >> 3;
        pub const LIBRARY_CALL: usize = field_offset!(Self, library_call) >> 3;
        pub const REPLACE_CLASS: usize = field_offset!(Self, replace_class) >> 3;
        pub const SEND_MESSAGE_TO_L1: usize = field_offset!(Self, send_message_to_l1) >> 3;
        pub const STORAGE_READ: usize = field_offset!(Self, storage_read) >> 3;
        pub const STORAGE_WRITE: usize = field_offset!(Self, storage_write) >> 3;

        pub const SECP256K1_NEW: usize = field_offset!(Self, secp256k1_new) >> 3;
        pub const SECP256K1_ADD: usize = field_offset!(Self, secp256k1_add) >> 3;
        pub const SECP256K1_MUL: usize = field_offset!(Self, secp256k1_mul) >> 3;
        pub const SECP256K1_GET_POINT_FROM_X: usize =
            field_offset!(Self, secp256k1_get_point_from_x) >> 3;
        pub const SECP256K1_GET_XY: usize = field_offset!(Self, secp256k1_get_xy) >> 3;
        pub const SECP256R1_NEW: usize = field_offset!(Self, secp256r1_new) >> 3;
        pub const SECP256R1_ADD: usize = field_offset!(Self, secp256r1_add) >> 3;
        pub const SECP256R1_MUL: usize = field_offset!(Self, secp256r1_mul) >> 3;
        pub const SECP256R1_GET_POINT_FROM_X: usize =
            field_offset!(Self, secp256r1_get_point_from_x) >> 3;
        pub const SECP256R1_GET_XY: usize = field_offset!(Self, secp256r1_get_xy) >> 3;

        pub const SHA256_PROCESS_BLOCK: usize = field_offset!(Self, sha256_process_block) >> 3;

        pub const GET_CLASS_HASH_AT: usize = field_offset!(Self, get_class_hash_at) >> 3;

        pub const META_TX_V0: usize = field_offset!(Self, meta_tx_v0) >> 3;
    }

    #[allow(unused_variables)]
    impl<'a, T> StarknetSyscallHandlerCallbacks<'a, T>
    where
        T: StarknetSyscallHandler + 'a,
    {
        pub fn new(handler: &'a mut T) -> Self {
            Self {
                self_ptr: handler,
                get_block_hash: Self::wrap_get_block_hash,
                get_execution_info: Self::wrap_get_execution_info,
                get_execution_info_v2: Self::wrap_get_execution_info_v2,
                get_execution_info_v3: Self::wrap_get_execution_info_v3,
                deploy: Self::wrap_deploy,
                replace_class: Self::wrap_replace_class,
                library_call: Self::wrap_library_call,
                call_contract: Self::wrap_call_contract,
                storage_read: Self::wrap_storage_read,
                storage_write: Self::wrap_storage_write,
                emit_event: Self::wrap_emit_event,
                send_message_to_l1: Self::wrap_send_message_to_l1,
                keccak: Self::wrap_keccak,
                secp256k1_new: Self::wrap_secp256k1_new,
                secp256k1_add: Self::wrap_secp256k1_add,
                secp256k1_mul: Self::wrap_secp256k1_mul,
                secp256k1_get_point_from_x: Self::wrap_secp256k1_get_point_from_x,
                secp256k1_get_xy: Self::wrap_secp256k1_get_xy,
                secp256r1_new: Self::wrap_secp256r1_new,
                secp256r1_add: Self::wrap_secp256r1_add,
                secp256r1_mul: Self::wrap_secp256r1_mul,
                secp256r1_get_point_from_x: Self::wrap_secp256r1_get_point_from_x,
                secp256r1_get_xy: Self::wrap_secp256r1_get_xy,
                sha256_process_block: Self::wrap_sha256_process_block,
                get_class_hash_at: Self::wrap_get_class_hash_at,
                meta_tx_v0: Self::wrap_meta_tx_v0,
                #[cfg(feature = "with-cheatcode")]
                cheatcode: Self::wrap_cheatcode,
            }
        }

        unsafe fn alloc_mlir_array<E: Clone>(data: &[E]) -> ArrayAbi<E> {
            match data.len() {
                0 => ArrayAbi {
                    ptr: null_mut(),
                    since: 0,
                    until: 0,
                    capacity: 0,
                },
                _ => {
                    let len: u32 = data.len().try_into().unwrap();

                    // Allocate data (no prefix)
                    let data_ptr =
                        libc_malloc(Layout::array::<E>(data.len()).unwrap().size()) as *mut E;

                    for (i, val) in data.iter().enumerate() {
                        data_ptr.add(i).write(val.clone());
                    }

                    let metadata_ptr =
                        libc_malloc(size_of::<ArrayMetadata>()) as *mut ArrayMetadata;
                    metadata_ptr.write(ArrayMetadata {
                        refcount: 1,
                        max_len: len,
                        data_ptr: data_ptr.cast::<u8>(),
                    });

                    ArrayAbi {
                        ptr: metadata_ptr.cast(),
                        since: 0,
                        until: len,
                        capacity: len,
                    }
                }
            }
        }

        unsafe fn drop_mlir_array<E>(data: &ArrayAbi<E>) {
            if data.ptr.is_null() {
                return;
            }

            let metadata_ptr = data.ptr.cast::<ArrayMetadata>();
            let metadata = &mut *metadata_ptr;

            match metadata.refcount {
                1 => {
                    // Last reference - free data and metadata
                    libc_free(metadata.data_ptr.cast());
                    libc_free(metadata_ptr.cast());
                }
                n => {
                    // Decrement refcount
                    metadata.refcount = n - 1;
                }
            }
        }

        fn wrap_error<E>(e: &[Felt]) -> SyscallResultAbi<E> {
            SyscallResultAbi {
                err: ManuallyDrop::new(SyscallResultAbiErr {
                    tag: 1u8,
                    payload: unsafe {
                        let data: Vec<_> = e.iter().map(|x| Felt252Abi(x.to_bytes_le())).collect();
                        Self::alloc_mlir_array(&data)
                    },
                }),
            }
        }

        extern "C" fn wrap_get_block_hash(
            result_ptr: &mut SyscallResultAbi<Felt252Abi>,
            ptr: &mut T,
            gas: &mut u64,
            block_number: u64,
        ) {
            let result = ptr.get_block_hash(block_number, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(Felt252Abi(x.to_bytes_le())),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_get_execution_info(
            result_ptr: &mut SyscallResultAbi<NonNull<ExecutionInfoAbi>>,
            ptr: &mut T,
            gas: &mut u64,
        ) {
            let result = ptr.get_execution_info(gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: unsafe {
                            let mut block_info_ptr = NonNull::new(libc_malloc(
                                size_of::<BlockInfoAbi>(),
                            )
                                as *mut BlockInfoAbi)
                            .unwrap();
                            block_info_ptr.as_mut().block_number = x.block_info.block_number;
                            block_info_ptr.as_mut().block_timestamp = x.block_info.block_timestamp;
                            block_info_ptr.as_mut().sequencer_address =
                                Felt252Abi(x.block_info.sequencer_address.to_bytes_le());

                            let mut tx_info_ptr =
                                NonNull::new(libc_malloc(size_of::<TxInfoAbi>()) as *mut TxInfoAbi)
                                    .unwrap();
                            tx_info_ptr.as_mut().version =
                                Felt252Abi(x.tx_info.version.to_bytes_le());
                            tx_info_ptr.as_mut().account_contract_address =
                                Felt252Abi(x.tx_info.account_contract_address.to_bytes_le());
                            tx_info_ptr.as_mut().max_fee = x.tx_info.max_fee;
                            tx_info_ptr.as_mut().signature = Self::alloc_mlir_array(
                                &x.tx_info
                                    .signature
                                    .into_iter()
                                    .map(|x| Felt252Abi(x.to_bytes_le()))
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().transaction_hash =
                                Felt252Abi(x.tx_info.transaction_hash.to_bytes_le());
                            tx_info_ptr.as_mut().chain_id =
                                Felt252Abi(x.tx_info.chain_id.to_bytes_le());
                            tx_info_ptr.as_mut().nonce = Felt252Abi(x.tx_info.nonce.to_bytes_le());

                            let mut execution_info_ptr =
                                NonNull::new(libc_malloc(size_of::<ExecutionInfoAbi>())
                                    as *mut ExecutionInfoAbi)
                                .unwrap();
                            execution_info_ptr.as_mut().block_info = block_info_ptr;
                            execution_info_ptr.as_mut().tx_info = tx_info_ptr;
                            execution_info_ptr.as_mut().caller_address =
                                Felt252Abi(x.caller_address.to_bytes_le());
                            execution_info_ptr.as_mut().contract_address =
                                Felt252Abi(x.contract_address.to_bytes_le());
                            execution_info_ptr.as_mut().entry_point_selector =
                                Felt252Abi(x.entry_point_selector.to_bytes_le());

                            ManuallyDrop::new(execution_info_ptr)
                        },
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_get_execution_info_v2(
            result_ptr: &mut SyscallResultAbi<NonNull<ExecutionInfoV2Abi>>,
            ptr: &mut T,
            gas: &mut u64,
        ) {
            let result = ptr.get_execution_info_v2(gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: unsafe {
                            let mut execution_info_ptr =
                                NonNull::new(libc_malloc(size_of::<ExecutionInfoV2Abi>())
                                    as *mut ExecutionInfoV2Abi)
                                .unwrap();

                            let mut block_info_ptr = NonNull::new(libc_malloc(
                                size_of::<BlockInfoAbi>(),
                            )
                                as *mut BlockInfoAbi)
                            .unwrap();
                            block_info_ptr.as_mut().block_number = x.block_info.block_number;
                            block_info_ptr.as_mut().block_timestamp = x.block_info.block_timestamp;
                            block_info_ptr.as_mut().sequencer_address =
                                Felt252Abi(x.block_info.sequencer_address.to_bytes_le());

                            let mut tx_info_ptr = NonNull::new(
                                libc_malloc(size_of::<TxInfoV2Abi>()) as *mut TxInfoV2Abi,
                            )
                            .unwrap();
                            tx_info_ptr.as_mut().version =
                                Felt252Abi(x.tx_info.version.to_bytes_le());
                            tx_info_ptr.as_mut().signature = Self::alloc_mlir_array(
                                &x.tx_info
                                    .signature
                                    .into_iter()
                                    .map(|x| Felt252Abi(x.to_bytes_le()))
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().max_fee = x.tx_info.max_fee;
                            tx_info_ptr.as_mut().transaction_hash =
                                Felt252Abi(x.tx_info.transaction_hash.to_bytes_le());
                            tx_info_ptr.as_mut().chain_id =
                                Felt252Abi(x.tx_info.chain_id.to_bytes_le());
                            tx_info_ptr.as_mut().nonce = Felt252Abi(x.tx_info.nonce.to_bytes_le());
                            tx_info_ptr.as_mut().resource_bounds = Self::alloc_mlir_array(
                                &x.tx_info
                                    .resource_bounds
                                    .into_iter()
                                    .map(|x| ResourceBoundsAbi {
                                        resource: Felt252Abi(x.resource.to_bytes_le()),
                                        max_amount: x.max_amount,
                                        max_price_per_unit: x.max_price_per_unit,
                                    })
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().tip = x.tx_info.tip;
                            tx_info_ptr.as_mut().paymaster_data = Self::alloc_mlir_array(
                                &x.tx_info
                                    .paymaster_data
                                    .into_iter()
                                    .map(|x| Felt252Abi(x.to_bytes_le()))
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().nonce_data_availability_mode =
                                x.tx_info.nonce_data_availability_mode;
                            tx_info_ptr.as_mut().fee_data_availability_mode =
                                x.tx_info.fee_data_availability_mode;
                            tx_info_ptr.as_mut().account_deployment_data = Self::alloc_mlir_array(
                                &x.tx_info
                                    .account_deployment_data
                                    .into_iter()
                                    .map(|x| Felt252Abi(x.to_bytes_le()))
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().account_contract_address =
                                Felt252Abi(x.tx_info.account_contract_address.to_bytes_le());

                            execution_info_ptr.as_mut().block_info = block_info_ptr;
                            execution_info_ptr.as_mut().tx_info = tx_info_ptr;
                            execution_info_ptr.as_mut().caller_address =
                                Felt252Abi(x.caller_address.to_bytes_le());
                            execution_info_ptr.as_mut().contract_address =
                                Felt252Abi(x.contract_address.to_bytes_le());
                            execution_info_ptr.as_mut().entry_point_selector =
                                Felt252Abi(x.entry_point_selector.to_bytes_le());

                            ManuallyDrop::new(execution_info_ptr)
                        },
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_get_execution_info_v3(
            result_ptr: &mut SyscallResultAbi<NonNull<ExecutionInfoV3Abi>>,
            ptr: &mut T,
            gas: &mut u64,
        ) {
            let result = ptr.get_execution_info_v3(gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: unsafe {
                            let mut execution_info_ptr =
                                NonNull::new(libc_malloc(size_of::<ExecutionInfoV3Abi>())
                                    as *mut ExecutionInfoV3Abi)
                                .unwrap();

                            let mut block_info_ptr = NonNull::new(libc_malloc(
                                size_of::<BlockInfoAbi>(),
                            )
                                as *mut BlockInfoAbi)
                            .unwrap();
                            block_info_ptr.as_mut().block_number = x.block_info.block_number;
                            block_info_ptr.as_mut().block_timestamp = x.block_info.block_timestamp;
                            block_info_ptr.as_mut().sequencer_address =
                                Felt252Abi(x.block_info.sequencer_address.to_bytes_le());

                            let mut tx_info_ptr = NonNull::new(
                                libc_malloc(size_of::<TxInfoV3Abi>()) as *mut TxInfoV3Abi,
                            )
                            .unwrap();
                            tx_info_ptr.as_mut().version =
                                Felt252Abi(x.tx_info.version.to_bytes_le());
                            tx_info_ptr.as_mut().signature = Self::alloc_mlir_array(
                                &x.tx_info
                                    .signature
                                    .into_iter()
                                    .map(|x| Felt252Abi(x.to_bytes_le()))
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().max_fee = x.tx_info.max_fee;
                            tx_info_ptr.as_mut().transaction_hash =
                                Felt252Abi(x.tx_info.transaction_hash.to_bytes_le());
                            tx_info_ptr.as_mut().chain_id =
                                Felt252Abi(x.tx_info.chain_id.to_bytes_le());
                            tx_info_ptr.as_mut().nonce = Felt252Abi(x.tx_info.nonce.to_bytes_le());
                            tx_info_ptr.as_mut().resource_bounds = Self::alloc_mlir_array(
                                &x.tx_info
                                    .resource_bounds
                                    .into_iter()
                                    .map(|x| ResourceBoundsAbi {
                                        resource: Felt252Abi(x.resource.to_bytes_le()),
                                        max_amount: x.max_amount,
                                        max_price_per_unit: x.max_price_per_unit,
                                    })
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().tip = x.tx_info.tip;
                            tx_info_ptr.as_mut().paymaster_data = Self::alloc_mlir_array(
                                &x.tx_info
                                    .paymaster_data
                                    .into_iter()
                                    .map(|x| Felt252Abi(x.to_bytes_le()))
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().nonce_data_availability_mode =
                                x.tx_info.nonce_data_availability_mode;
                            tx_info_ptr.as_mut().fee_data_availability_mode =
                                x.tx_info.fee_data_availability_mode;
                            tx_info_ptr.as_mut().account_deployment_data = Self::alloc_mlir_array(
                                &x.tx_info
                                    .account_deployment_data
                                    .into_iter()
                                    .map(|x| Felt252Abi(x.to_bytes_le()))
                                    .collect::<Vec<_>>(),
                            );
                            tx_info_ptr.as_mut().account_contract_address =
                                Felt252Abi(x.tx_info.account_contract_address.to_bytes_le());
                            tx_info_ptr.as_mut().proof_facts = Self::alloc_mlir_array(
                                &x.tx_info
                                    .proof_facts
                                    .into_iter()
                                    .map(|x| Felt252Abi(x.to_bytes_le()))
                                    .collect::<Vec<_>>(),
                            );

                            execution_info_ptr.as_mut().block_info = block_info_ptr;
                            execution_info_ptr.as_mut().tx_info = tx_info_ptr;
                            execution_info_ptr.as_mut().caller_address =
                                Felt252Abi(x.caller_address.to_bytes_le());
                            execution_info_ptr.as_mut().contract_address =
                                Felt252Abi(x.contract_address.to_bytes_le());
                            execution_info_ptr.as_mut().entry_point_selector =
                                Felt252Abi(x.entry_point_selector.to_bytes_le());

                            ManuallyDrop::new(execution_info_ptr)
                        },
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_deploy(
            result_ptr: &mut SyscallResultAbi<(Felt252Abi, ArrayAbi<Felt252Abi>)>,
            ptr: &mut T,
            gas: &mut u64,
            class_hash: &Felt252Abi,
            contract_address_salt: &Felt252Abi,
            calldata: &ArrayAbi<Felt252Abi>,
            deploy_from_zero: bool,
        ) {
            let class_hash = Felt::from(class_hash);
            let contract_address_salt = Felt::from(contract_address_salt);

            let calldata_vec: Vec<_> = calldata.into();
            unsafe {
                Self::drop_mlir_array(calldata);
            }

            let result = ptr.deploy(
                class_hash,
                contract_address_salt,
                &calldata_vec,
                deploy_from_zero,
                gas,
            );

            *result_ptr = match result {
                Ok(x) => {
                    let felts: Vec<_> = x.1.iter().map(|x| Felt252Abi(x.to_bytes_le())).collect();
                    let felts_ptr = unsafe { Self::alloc_mlir_array(&felts) };
                    SyscallResultAbi {
                        ok: ManuallyDrop::new(SyscallResultAbiOk {
                            tag: 0u8,
                            payload: ManuallyDrop::new((Felt252Abi(x.0.to_bytes_le()), felts_ptr)),
                        }),
                    }
                }
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_replace_class(
            result_ptr: &mut SyscallResultAbi<()>,
            ptr: &mut T,
            gas: &mut u64,
            class_hash: &Felt252Abi,
        ) {
            let class_hash = Felt::from(class_hash);
            let result = ptr.replace_class(class_hash, gas);

            *result_ptr = match result {
                Ok(_) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(()),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_library_call(
            result_ptr: &mut SyscallResultAbi<ArrayAbi<Felt252Abi>>,
            ptr: &mut T,
            gas: &mut u64,
            class_hash: &Felt252Abi,
            function_selector: &Felt252Abi,
            calldata: &ArrayAbi<Felt252Abi>,
        ) {
            let class_hash = Felt::from(class_hash);
            let function_selector = Felt::from(function_selector);

            let calldata_vec: Vec<Felt> = calldata.into();
            unsafe {
                Self::drop_mlir_array(calldata);
            }

            let result = ptr.library_call(class_hash, function_selector, &calldata_vec, gas);

            *result_ptr = match result {
                Ok(x) => {
                    let felts: Vec<_> = x.iter().map(|x| Felt252Abi(x.to_bytes_le())).collect();
                    let felts_ptr = unsafe { Self::alloc_mlir_array(&felts) };
                    SyscallResultAbi {
                        ok: ManuallyDrop::new(SyscallResultAbiOk {
                            tag: 0u8,
                            payload: ManuallyDrop::new(felts_ptr),
                        }),
                    }
                }
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_call_contract(
            result_ptr: &mut SyscallResultAbi<ArrayAbi<Felt252Abi>>,
            ptr: &mut T,
            gas: &mut u64,
            address: &Felt252Abi,
            entry_point_selector: &Felt252Abi,
            calldata: &ArrayAbi<Felt252Abi>,
        ) {
            let address = Felt::from(address);
            let entry_point_selector = Felt::from(entry_point_selector);

            let calldata_vec: Vec<Felt> = calldata.into();
            unsafe {
                Self::drop_mlir_array(calldata);
            }

            let result = ptr.call_contract(address, entry_point_selector, &calldata_vec, gas);

            *result_ptr = match result {
                Ok(x) => {
                    let felts: Vec<_> = x.iter().map(|x| Felt252Abi(x.to_bytes_le())).collect();
                    let felts_ptr = unsafe { Self::alloc_mlir_array(&felts) };
                    SyscallResultAbi {
                        ok: ManuallyDrop::new(SyscallResultAbiOk {
                            tag: 0u8,
                            payload: ManuallyDrop::new(felts_ptr),
                        }),
                    }
                }
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_storage_read(
            result_ptr: &mut SyscallResultAbi<Felt252Abi>,
            ptr: &mut T,
            gas: &mut u64,
            address_domain: u32,
            address: &Felt252Abi,
        ) {
            let address = Felt::from(address);
            let result = ptr.storage_read(address_domain, address, gas);

            *result_ptr = match result {
                Ok(res) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(Felt252Abi(res.to_bytes_le())),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_storage_write(
            result_ptr: &mut SyscallResultAbi<()>,
            ptr: &mut T,
            gas: &mut u64,
            address_domain: u32,
            address: &Felt252Abi,
            value: &Felt252Abi,
        ) {
            let address = Felt::from(address);
            let value = Felt::from(value);
            let result = ptr.storage_write(address_domain, address, value, gas);

            *result_ptr = match result {
                Ok(_) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(()),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_emit_event(
            result_ptr: &mut SyscallResultAbi<()>,
            ptr: &mut T,
            gas: &mut u64,
            keys: &ArrayAbi<Felt252Abi>,
            data: &ArrayAbi<Felt252Abi>,
        ) {
            let keys_vec: Vec<_> = keys.into();
            let data_vec: Vec<_> = data.into();

            unsafe {
                Self::drop_mlir_array(keys);
                Self::drop_mlir_array(data);
            }

            let result = ptr.emit_event(&keys_vec, &data_vec, gas);

            *result_ptr = match result {
                Ok(_) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(()),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_send_message_to_l1(
            result_ptr: &mut SyscallResultAbi<()>,
            ptr: &mut T,
            gas: &mut u64,
            to_address: &Felt252Abi,
            payload: &ArrayAbi<Felt252Abi>,
        ) {
            let to_address = Felt::from(to_address);
            let payload_vec: Vec<_> = payload.into();

            unsafe {
                Self::drop_mlir_array(payload);
            }

            let result = ptr.send_message_to_l1(to_address, &payload_vec, gas);

            *result_ptr = match result {
                Ok(_) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(()),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_keccak(
            result_ptr: &mut SyscallResultAbi<U256>,
            ptr: &mut T,
            gas: &mut u64,
            input: &ArrayAbi<u64>,
        ) {
            let input_vec = unsafe {
                let since_offset = input.since as usize;
                let until_offset = input.until as usize;
                debug_assert!(since_offset <= until_offset);
                let len = until_offset - since_offset;
                match len {
                    0 => &[],
                    _ => {
                        // Access data through ArrayMetadata.data_ptr
                        let metadata = &*input.ptr.cast::<ArrayMetadata>();
                        let data_ptr = metadata.data_ptr.cast::<u64>();
                        std::slice::from_raw_parts(data_ptr.add(since_offset), len)
                    }
                }
            };

            let result = ptr.keccak(input_vec, gas);
            unsafe {
                Self::drop_mlir_array(input);
            }

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(x),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256k1_new(
            result_ptr: &mut SyscallResultAbi<(u8, MaybeUninit<Secp256k1Point>)>,
            ptr: &mut T,
            gas: &mut u64,
            x: &U256,
            y: &U256,
        ) {
            let x = *x;
            let y = *y;
            let result = ptr.secp256k1_new(x, y, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(match x {
                            Some(x) => (0, MaybeUninit::new(x)),
                            None => (1, MaybeUninit::uninit()),
                        }),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256k1_add(
            result_ptr: &mut SyscallResultAbi<Secp256k1Point>,
            ptr: &mut T,
            gas: &mut u64,
            p0: &Secp256k1Point,
            p1: &Secp256k1Point,
        ) {
            let p0 = *p0;
            let p1 = *p1;
            let result = ptr.secp256k1_add(p0, p1, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(x),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256k1_mul(
            result_ptr: &mut SyscallResultAbi<Secp256k1Point>,
            ptr: &mut T,
            gas: &mut u64,
            p: &Secp256k1Point,
            scalar: &U256,
        ) {
            // Seems like it's important to dereference and create a local instead of at call site directly.
            let scalar = *scalar;
            let p = *p;
            let result = ptr.secp256k1_mul(p, scalar, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(x),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256k1_get_point_from_x(
            result_ptr: &mut SyscallResultAbi<(u8, MaybeUninit<Secp256k1Point>)>,
            ptr: &mut T,
            gas: &mut u64,
            x: &U256,
            y_parity: &bool,
        ) {
            let x = *x;
            let y_parity = *y_parity;
            let result = ptr.secp256k1_get_point_from_x(x, y_parity, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(match x {
                            Some(x) => (0, MaybeUninit::new(x)),
                            None => (1, MaybeUninit::uninit()),
                        }),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256k1_get_xy(
            result_ptr: &mut SyscallResultAbi<(U256, U256)>,
            ptr: &mut T,
            gas: &mut u64,
            p: &Secp256k1Point,
        ) {
            let p = *p;
            let result = ptr.secp256k1_get_xy(p, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(x),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256r1_new(
            result_ptr: &mut SyscallResultAbi<(u8, MaybeUninit<Secp256r1Point>)>,
            ptr: &mut T,
            gas: &mut u64,
            x: &U256,
            y: &U256,
        ) {
            let x = *x;
            let y = *y;
            let result = ptr.secp256r1_new(x, y, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(match x {
                            Some(x) => (0, MaybeUninit::new(x)),
                            None => (1, MaybeUninit::uninit()),
                        }),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256r1_add(
            result_ptr: &mut SyscallResultAbi<Secp256r1Point>,
            ptr: &mut T,
            gas: &mut u64,
            p0: &Secp256r1Point,
            p1: &Secp256r1Point,
        ) {
            let p0 = *p0;
            let p1 = *p1;
            let result = ptr.secp256r1_add(p0, p1, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(x),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256r1_mul(
            result_ptr: &mut SyscallResultAbi<Secp256r1Point>,
            ptr: &mut T,
            gas: &mut u64,
            p: &Secp256r1Point,
            scalar: &U256,
        ) {
            let scalar = *scalar;
            let p = *p;
            let result = ptr.secp256r1_mul(p, scalar, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(x),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256r1_get_point_from_x(
            result_ptr: &mut SyscallResultAbi<(u8, MaybeUninit<Secp256r1Point>)>,
            ptr: &mut T,
            gas: &mut u64,
            x: &U256,
            y_parity: &bool,
        ) {
            let x = *x;
            let y_parity = *y_parity;
            let result = ptr.secp256r1_get_point_from_x(x, y_parity, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(match x {
                            Some(x) => (0, MaybeUninit::new(x)),
                            None => (1, MaybeUninit::uninit()),
                        }),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_secp256r1_get_xy(
            result_ptr: &mut SyscallResultAbi<(U256, U256)>,
            ptr: &mut T,
            gas: &mut u64,
            p: &Secp256r1Point,
        ) {
            let p = *p;
            let result = ptr.secp256r1_get_xy(p, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(x),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_sha256_process_block(
            result_ptr: &mut SyscallResultAbi<*mut [u32; 8]>,
            ptr: &mut T,
            gas: &mut u64,
            state: *mut [u32; 8],
            block: &[u32; 16],
        ) {
            let state_ref = unsafe { state.as_mut().unwrap() };
            let result = ptr.sha256_process_block(state_ref, block, gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(state),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_get_class_hash_at(
            result_ptr: &mut SyscallResultAbi<Felt252Abi>,
            ptr: &mut T,
            gas: &mut u64,
            contract_address: &Felt252Abi,
        ) {
            let result = ptr.get_class_hash_at(contract_address.into(), gas);

            *result_ptr = match result {
                Ok(x) => SyscallResultAbi {
                    ok: ManuallyDrop::new(SyscallResultAbiOk {
                        tag: 0u8,
                        payload: ManuallyDrop::new(Felt252Abi(x.to_bytes_le())),
                    }),
                },
                Err(e) => Self::wrap_error(&e),
            };
        }

        extern "C" fn wrap_meta_tx_v0(
            result_ptr: &mut SyscallResultAbi<ArrayAbi<Felt252Abi>>,
            ptr: &mut T,
            gas: &mut u64,
            address: &Felt252Abi,
            entry_point_selector: &Felt252Abi,
            calldata: &ArrayAbi<Felt252Abi>,
            signature: &ArrayAbi<Felt252Abi>,
        ) {
            let address = Felt::from(address);
            let entry_point_selector = Felt::from(entry_point_selector);

            let calldata_vec: Vec<Felt> = calldata.into();
            unsafe {
                Self::drop_mlir_array(calldata);
            }
            let signature_vec: Vec<Felt> = signature.into();
            unsafe {
                Self::drop_mlir_array(signature);
            }

            let result = ptr.meta_tx_v0(
                address,
                entry_point_selector,
                &calldata_vec,
                &signature_vec,
                gas,
            );

            *result_ptr = match result {
                Ok(x) => {
                    let felts: Vec<_> = x.iter().map(|x| Felt252Abi(x.to_bytes_le())).collect();
                    let felts_ptr = unsafe { Self::alloc_mlir_array(&felts) };
                    SyscallResultAbi {
                        ok: ManuallyDrop::new(SyscallResultAbiOk {
                            tag: 0u8,
                            payload: ManuallyDrop::new(felts_ptr),
                        }),
                    }
                }
                Err(e) => Self::wrap_error(&e),
            };
        }

        #[cfg(feature = "with-cheatcode")]
        extern "C" fn wrap_cheatcode(
            result_ptr: &mut ArrayAbi<Felt252Abi>,
            ptr: &mut T,
            selector: &Felt252Abi,
            input: &ArrayAbi<Felt252Abi>,
        ) {
            let selector = Felt::from(selector);
            let input_vec: Vec<_> = input.into();

            unsafe {
                Self::drop_mlir_array(input);
            }

            let result = ptr
                .cheatcode(selector, &input_vec)
                .into_iter()
                .map(|x| Felt252Abi(x.to_bytes_le()))
                .collect::<Vec<_>>();

            *result_ptr = unsafe { Self::alloc_mlir_array(&result) };
        }
    }
}

#[cfg(feature = "with-cheatcode")]
thread_local!(pub static SYSCALL_HANDLER_VTABLE: std::cell::Cell<*mut ()> = const { std::cell::Cell::new(std::ptr::null_mut()) });

#[allow(non_snake_case)]
#[cfg(feature = "with-cheatcode")]
/// Runtime function that calls the `cheatcode` syscall
///
/// The Cairo compiler doesn't specify that the cheatcode syscall needs the syscall handler,
/// so a pointer to `StarknetSyscallHandlerCallbacks` is stored as a `thread::LocalKey` and accesed in runtime by this function.
pub extern "C" fn cairo_native__vtable_cheatcode(
    result_ptr: &mut ArrayAbi<Felt252Abi>,
    selector: &Felt252Abi,
    input: &ArrayAbi<Felt252Abi>,
) {
    let ptr = SYSCALL_HANDLER_VTABLE.with(|ptr| ptr.get());
    assert!(!ptr.is_null());

    let callbacks_ptr = ptr as *mut handler::StarknetSyscallHandlerCallbacks<DummySyscallHandler>;
    let callbacks = unsafe { callbacks_ptr.as_mut().expect("should not be null") };

    // The `StarknetSyscallHandler` is stored as a reference in the first field of `StarknetSyscalLHandlerCallbacks`,
    // so we can interpret `ptr` as a double pointer to the handler.
    let handler_ptr_ptr = ptr as *mut *mut DummySyscallHandler;
    let handler = unsafe { (*handler_ptr_ptr).as_mut().expect("should not be null") };

    (callbacks.cheatcode)(result_ptr, handler, selector, input);
}
