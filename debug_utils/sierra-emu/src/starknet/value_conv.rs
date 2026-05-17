//! Conversions between sierra-emu's `Value` representation and the syscall
//! types exported from `cairo-starknet-syscalls`.
//!
//! These helpers translate between the explicit Rust structs used by the syscall
//! handler trait (e.g. [`U256`], [`Secp256k1Point`], [`ExecutionInfoV2`]) and the
//! generic [`Value`] tree the sierra-emu VM stores variables in. Each
//! `*_into_value` lowers a struct to the Sierra wire shape; each `*_from_value`
//! inverts the lowering.
//!
//! The conversions are intentionally manual (rather than a generic derive) so the
//! field order and Sierra-level encoding remain explicit at call sites -- the
//! Sierra ABI is the source of truth here, not the Rust struct layout.

use cairo_lang_sierra::ids::ConcreteTypeId;
use cairo_starknet_syscalls::{
    BlockInfo, ExecutionInfo, ExecutionInfoV2, ResourceBounds, Secp256k1Point, Secp256r1Point,
    TxInfo, TxV2Info, U256,
};

use crate::Value;

/// Encode a [`U256`] as the Sierra-level `(lo: u128, hi: u128)` struct value.
pub fn u256_into_value(x: U256) -> Value {
    Value::Struct(vec![Value::U128(x.lo), Value::U128(x.hi)])
}

/// Decode a Sierra-level `(lo: u128, hi: u128)` struct value as a [`U256`].
///
/// Panics on a malformed `Value` -- the VM only produces these via the secp256
/// libfunc evaluators, which always emit the right shape.
pub fn u256_from_value(v: Value) -> U256 {
    let Value::Struct(v) = v else { panic!() };
    let Value::U128(lo) = v[0] else { panic!() };
    let Value::U128(hi) = v[1] else { panic!() };
    U256 { lo, hi }
}

/// Encode a [`Secp256k1Point`] as the Sierra-level `(x: U256, y: U256)` pair.
///
/// Sierra has no slot for `is_infinity`; the identity element is encoded as the
/// canonical `(0, 0)` sentinel so [`secp256k1_point_from_value`] can recover the
/// flag losslessly. `(0, 0)` is not on the secp256k1 curve, so this aliasing is
/// unambiguous.
pub fn secp256k1_point_into_value(p: Secp256k1Point) -> Value {
    let (x, y) = if p.is_infinity {
        (U256 { lo: 0, hi: 0 }, U256 { lo: 0, hi: 0 })
    } else {
        (p.x, p.y)
    };
    Value::Struct(vec![
        Value::Struct(vec![Value::U128(x.lo), Value::U128(x.hi)]),
        Value::Struct(vec![Value::U128(y.lo), Value::U128(y.hi)]),
    ])
}

/// Decode a Sierra-level `(x: U256, y: U256)` pair as a [`Secp256k1Point`].
///
/// Recovers `is_infinity` from the `(0, 0)` sentinel -- see
/// [`secp256k1_point_into_value`].
pub fn secp256k1_point_from_value(v: Value) -> Secp256k1Point {
    let Value::Struct(mut v) = v else { panic!() };
    let y = u256_from_value(v.remove(1));
    let x = u256_from_value(v.remove(0));
    let is_infinity = x.lo == 0 && x.hi == 0 && y.lo == 0 && y.hi == 0;
    Secp256k1Point { x, y, is_infinity }
}

/// Encode a [`Secp256r1Point`] as the Sierra-level `(x: U256, y: U256)` pair.
/// Identity-element handling mirrors [`secp256k1_point_into_value`].
pub fn secp256r1_point_into_value(p: Secp256r1Point) -> Value {
    let (x, y) = if p.is_infinity {
        (U256 { lo: 0, hi: 0 }, U256 { lo: 0, hi: 0 })
    } else {
        (p.x, p.y)
    };
    Value::Struct(vec![
        Value::Struct(vec![Value::U128(x.lo), Value::U128(x.hi)]),
        Value::Struct(vec![Value::U128(y.lo), Value::U128(y.hi)]),
    ])
}

/// Decode a Sierra-level `(x: U256, y: U256)` pair as a [`Secp256r1Point`].
/// `is_infinity` recovery mirrors [`secp256k1_point_from_value`].
pub fn secp256r1_point_from_value(v: Value) -> Secp256r1Point {
    let Value::Struct(mut v) = v else { panic!() };
    let y = u256_from_value(v.remove(1));
    let x = u256_from_value(v.remove(0));
    let is_infinity = x.lo == 0 && x.hi == 0 && y.lo == 0 && y.hi == 0;
    Secp256r1Point { x, y, is_infinity }
}

/// Encode a [`BlockInfo`] as the Sierra-level
/// `(block_number: u64, block_timestamp: u64, sequencer_address: felt252)` struct.
pub fn block_info_into_value(b: BlockInfo) -> Value {
    Value::Struct(vec![
        Value::U64(b.block_number),
        Value::U64(b.block_timestamp),
        Value::Felt(b.sequencer_address),
    ])
}

/// Encode a [`ResourceBounds`] as the Sierra-level
/// `(resource: felt252, max_amount: u64, max_price_per_unit: u128)` struct.
pub fn resource_bounds_into_value(r: ResourceBounds) -> Value {
    Value::Struct(vec![
        Value::Felt(r.resource),
        Value::U64(r.max_amount),
        Value::U128(r.max_price_per_unit),
    ])
}

/// Encode a [`TxInfo`] (the v1 transaction info) as its Sierra-level struct.
///
/// `felt252_ty` is the concrete type id used for the inner `Array<felt252>`
/// holding the signature.
pub fn tx_info_into_value(info: TxInfo, felt252_ty: ConcreteTypeId) -> Value {
    Value::Struct(vec![
        Value::Felt(info.version),
        Value::Felt(info.account_contract_address),
        Value::U128(info.max_fee),
        Value::Struct(vec![Value::Array {
            ty: felt252_ty,
            data: info.signature.into_iter().map(Value::Felt).collect(),
        }]),
        Value::Felt(info.transaction_hash),
        Value::Felt(info.chain_id),
        Value::Felt(info.nonce),
    ])
}

/// Encode a [`TxV2Info`] (the v2 transaction info, including resource bounds and
/// paymaster data) as its Sierra-level struct.
///
/// `felt252_ty` is the concrete type id for `Array<felt252>` (used by signature,
/// paymaster_data, account_deployment_data). `resource_bounds_ty` is the type id
/// for `Array<ResourceBounds>`. Both must be looked up from the Sierra program's
/// registry by the caller.
pub fn tx_v2_info_into_value(
    info: TxV2Info,
    felt252_ty: ConcreteTypeId,
    resource_bounds_ty: ConcreteTypeId,
) -> Value {
    Value::Struct(vec![
        Value::Felt(info.version),
        Value::Felt(info.account_contract_address),
        Value::U128(info.max_fee),
        Value::Struct(vec![Value::Array {
            ty: felt252_ty.clone(),
            data: info.signature.into_iter().map(Value::Felt).collect(),
        }]),
        Value::Felt(info.transaction_hash),
        Value::Felt(info.chain_id),
        Value::Felt(info.nonce),
        Value::Struct(vec![Value::Array {
            ty: resource_bounds_ty,
            data: info
                .resource_bounds
                .into_iter()
                .map(resource_bounds_into_value)
                .collect(),
        }]),
        Value::U128(info.tip),
        Value::Struct(vec![Value::Array {
            ty: felt252_ty.clone(),
            data: info.paymaster_data.into_iter().map(Value::Felt).collect(),
        }]),
        Value::U32(info.nonce_data_availability_mode),
        Value::U32(info.fee_data_availability_mode),
        Value::Struct(vec![Value::Array {
            ty: felt252_ty,
            data: info
                .account_deployment_data
                .into_iter()
                .map(Value::Felt)
                .collect(),
        }]),
    ])
}

/// Encode an [`ExecutionInfo`] (the v1 execution info) as its Sierra-level struct.
/// See [`tx_info_into_value`] for the `felt252_ty` parameter.
pub fn execution_info_into_value(info: ExecutionInfo, felt252_ty: ConcreteTypeId) -> Value {
    Value::Struct(vec![
        block_info_into_value(info.block_info),
        tx_info_into_value(info.tx_info, felt252_ty),
        Value::Felt(info.caller_address),
        Value::Felt(info.contract_address),
        Value::Felt(info.entry_point_selector),
    ])
}

/// Encode an [`ExecutionInfoV2`] as its Sierra-level struct.
/// See [`tx_v2_info_into_value`] for the type-id parameters.
pub fn execution_info_v2_into_value(
    info: ExecutionInfoV2,
    felt252_ty: ConcreteTypeId,
    resource_bounds_ty: ConcreteTypeId,
) -> Value {
    Value::Struct(vec![
        block_info_into_value(info.block_info),
        tx_v2_info_into_value(info.tx_info, felt252_ty, resource_bounds_ty),
        Value::Felt(info.caller_address),
        Value::Felt(info.contract_address),
        Value::Felt(info.entry_point_selector),
    ])
}
