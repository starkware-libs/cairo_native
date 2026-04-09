//! An usable implementation of the starknet syscall handler trait.
//!
//! Based of cairo-lang-runner syscall handler implementation.
//! - https://github.com/starkware-libs/cairo/blob/v2.16.0/crates/cairo-lang-runner/src/casm_run/mod.rs

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::Arc,
};

use crate::{
    error::Error,
    execution_result::BuiltinStats,
    executor::AotNativeExecutor,
    starknet::{
        ExecutionInfo, ExecutionInfoV2, Secp256k1Point, Secp256r1Point, StarknetSyscallHandler,
        SyscallResult, TxV2Info, U256,
    },
    Value,
};
use ark_ec::short_weierstrass::{Affine, Projective, SWCurveConfig};
use ark_ff::{BigInt, PrimeField};
use cairo_lang_sierra::ids::FunctionId;
use cairo_lang_starknet::contract::ContractInfo;
use cairo_lang_starknet_classes::casm_contract_class::ENTRY_POINT_COST;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use itertools::Itertools;
use num_bigint::BigUint;
use num_traits::Zero;
use sha2::digest::generic_array::GenericArray;
use starknet_types_core::{
    felt::{Felt, NonZeroFelt},
    hash::{Pedersen, StarkHash},
};
use tracing::instrument;

/// An usable implementation of the starknet syscall handler trait.
#[derive(Clone, Default, Debug)]
pub struct StubSyscallHandler {
    /// The Cairo Native executor
    pub executor: Option<Arc<AotNativeExecutor>>,
    /// The values of addresses in the simulated storage per contract.
    pub storage: HashMap<Felt, HashMap<Felt, Felt>>,
    /// A mapping from contract address to class hash.
    pub deployed_contracts: HashMap<Felt, Felt>,
    /// A mapping from contract address to logs.
    pub logs: HashMap<Felt, ContractLogs>,
    /// The simulated execution info.
    pub execution_info: ExecutionInfo,
    /// A mock history, mapping block number to the class hash.
    pub block_hash: HashMap<u64, Felt>,
    /// Mapping from class_hash to contract info.
    pub contracts_info: OrderedHashMap<Felt, ContractInfo>,
    /// Keep track of inner call builtin usage.
    pub builtin_counters: BuiltinStats,
}

/// Event emitted by the emit_event syscall.
#[derive(Debug, Clone)]
pub struct StubEvent {
    pub keys: Vec<Felt>,
    pub data: Vec<Felt>,
}

#[derive(Debug, Default, Clone)]
pub struct ContractLogs {
    pub events: VecDeque<StubEvent>,
    pub l2_to_l1_messages: VecDeque<L2ToL1Message>,
}

type L2ToL1Message = (Felt, Vec<Felt>);

#[derive(PartialEq, Clone, Copy)]
struct Secp256Point<Curve: SWCurveConfig>(Affine<Curve>);

impl<Curve: SWCurveConfig> fmt::Debug for Secp256Point<Curve> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Secp256Point").field(&self.0).finish()
    }
}

impl From<Secp256Point<ark_secp256k1::Config>> for Secp256k1Point {
    fn from(Secp256Point(Affine { x, y, infinity }): Secp256Point<ark_secp256k1::Config>) -> Self {
        Secp256k1Point {
            x: big4int_to_u256(x.into()),
            y: big4int_to_u256(y.into()),
            is_infinity: infinity,
        }
    }
}

impl From<Secp256Point<ark_secp256r1::Config>> for Secp256r1Point {
    fn from(Secp256Point(Affine { x, y, infinity }): Secp256Point<ark_secp256r1::Config>) -> Self {
        Secp256r1Point {
            x: big4int_to_u256(x.into()),
            y: big4int_to_u256(y.into()),
            is_infinity: infinity,
        }
    }
}

impl From<Secp256k1Point> for Secp256Point<ark_secp256k1::Config> {
    fn from(p: Secp256k1Point) -> Self {
        Secp256Point(Affine {
            x: u256_to_biguint(p.x).into(),
            y: u256_to_biguint(p.y).into(),
            infinity: p.is_infinity,
        })
    }
}

impl From<Secp256r1Point> for Secp256Point<ark_secp256r1::Config> {
    fn from(p: Secp256r1Point) -> Self {
        Secp256Point(Affine {
            x: u256_to_biguint(p.x).into(),
            y: u256_to_biguint(p.y).into(),
            infinity: p.is_infinity,
        })
    }
}

pub fn u256_to_biguint(u256: U256) -> BigUint {
    let lo = BigUint::from(u256.lo);
    let hi = BigUint::from(u256.hi);

    (hi << 128) + lo
}

pub fn big4int_to_u256(b_int: BigInt<4>) -> U256 {
    let [a, b, c, d] = b_int.0;

    let lo = u128::from(a) | (u128::from(b) << 64);
    let hi = u128::from(c) | (u128::from(d) << 64);

    U256 { lo, hi }
}

pub fn encode_str_as_felts(msg: &str) -> Vec<Felt> {
    const CHUNK_SIZE: usize = 32;

    let data = msg.as_bytes().chunks(CHUNK_SIZE - 1);
    let mut encoding = vec![Felt::default(); data.len()];
    for (i, data_chunk) in data.enumerate() {
        let mut chunk = [0_u8; CHUNK_SIZE];
        chunk[1..data_chunk.len() + 1].copy_from_slice(data_chunk);
        encoding[i] = Felt::from_bytes_be(&chunk);
    }
    encoding
}

pub fn decode_felts_as_str(encoding: &[Felt]) -> String {
    let bytes_err: Vec<_> = encoding
        .iter()
        .flat_map(|felt| felt.to_bytes_be()[1..32].to_vec())
        .collect();

    match String::from_utf8(bytes_err) {
        Ok(s) => s.trim_matches('\0').to_owned(),
        Err(_) => {
            let err_msgs = encoding
                .iter()
                .map(
                    |felt| match String::from_utf8(felt.to_bytes_be()[1..32].to_vec()) {
                        Ok(s) => format!("{} ({})", s.trim_matches('\0'), felt),
                        Err(_) => felt.to_string(),
                    },
                )
                .join(", ");
            format!("[{}]", err_msgs)
        }
    }
}

impl<Curve: SWCurveConfig> Secp256Point<Curve>
where
    Curve::BaseField: PrimeField, // constraint for get_point_by_id
{
    // Given a (x,y) pair it will
    // - return the point at infinity for (0,0)
    // - Err if either x or y is outside of the modulus
    // - Ok(None) if (x,y) are within the modules but not on the curve
    // - Ok(Some(Point)) if (x,y) are on the curve
    fn new(x: U256, y: U256) -> Result<Option<Self>, Vec<Felt>> {
        let x = u256_to_biguint(x);
        let y = u256_to_biguint(y);
        let modulos = Curve::BaseField::MODULUS.into();

        if x >= modulos || y >= modulos {
            let error = Felt::from_hex(
                "0x00000000000000000000000000000000496e76616c696420617267756d656e74",
            ) // INVALID_ARGUMENT
            .map_err(|err| encode_str_as_felts(&err.to_string()))?;

            return Err(vec![error]);
        }

        Ok(maybe_affine(x.into(), y.into()))
    }

    fn add(p0: Self, p1: Self) -> Self {
        let result: Projective<Curve> = p0.0 + p1.0;
        Secp256Point(result.into())
    }

    fn mul(p: Self, m: U256) -> Self {
        let result = p.0 * Curve::ScalarField::from(u256_to_biguint(m));
        Secp256Point(result.into())
    }

    fn get_point_from_x(x: U256, y_parity: bool) -> Result<Option<Self>, Vec<Felt>> {
        let modulos = Curve::BaseField::MODULUS.into();
        let x = u256_to_biguint(x);

        if x >= modulos {
            let error = Felt::from_hex(
                "0x00000000000000000000000000000000496e76616c696420617267756d656e74",
            ) // INVALID_ARGUMENT
            .map_err(|err| encode_str_as_felts(&err.to_string()))?;

            return Err(vec![error]);
        }

        let x = x.into();
        let maybe_ec_point = Affine::<Curve>::get_ys_from_x_unchecked(x)
            .map(|(smaller, greater)| {
                // Return the correct y coordinate based on the parity.
                if ark_ff::BigInteger::is_odd(&smaller.into_bigint()) == y_parity {
                    smaller
                } else {
                    greater
                }
            })
            .map(|y| Affine::<Curve>::new_unchecked(x, y))
            .filter(|p| p.is_in_correct_subgroup_assuming_on_curve());

        Ok(maybe_ec_point.map(Secp256Point))
    }
}

/// Variation on [`Affine<Curve>::new`] that doesn't panic and maps (x,y) = (0,0) -> infinity
fn maybe_affine<Curve: SWCurveConfig>(
    x: Curve::BaseField,
    y: Curve::BaseField,
) -> Option<Secp256Point<Curve>> {
    let ec_point = if x.is_zero() && y.is_zero() {
        Affine::<Curve>::identity()
    } else {
        Affine::<Curve>::new_unchecked(x, y)
    };

    if ec_point.is_on_curve() && ec_point.is_in_correct_subgroup_assuming_on_curve() {
        Some(Secp256Point(ec_point))
    } else {
        None
    }
}

impl StubSyscallHandler {
    #[instrument(skip(self))]
    fn call_entry_point(
        &mut self,
        gas_counter: &mut u64,
        entry_point: &FunctionId,
        calldata: &[Felt],
    ) -> Result<Vec<Felt>, Vec<Felt>> {
        // The cost of the called syscall include `ENTRY_POINT_COST` so we need
        // to refund it here to avoid double charging.
        let inner_gas_counter = Some(*gas_counter + ENTRY_POINT_COST as u64);
        let inner_args = &[Value::Struct {
            fields: vec![Value::Array(
                calldata.iter().map(|x| Value::from(*x)).collect_vec(),
            )],
            debug_name: None,
        }];
        let concrete_result = self
            .executor
            .clone()
            .expect("calling contracts requires executor")
            .invoke_dynamic_with_syscall_handler(
                entry_point,
                inner_args,
                inner_gas_counter,
                &mut *self,
            )
            .expect("failed to execute inner contract");

        self.builtin_counters += concrete_result.builtin_stats;
        if let Some(remaining_gas) = concrete_result.remaining_gas {
            *gas_counter = remaining_gas;
        }

        read_contract_result(&concrete_result.return_value)
            .expect("return value was not a starknet panic result")
    }

    /// Replaces the addresses in the context.
    ///
    /// Called before `call_entry_point`.
    pub fn open_caller_context(
        &mut self,
        (new_contract_address, new_caller_address): (Felt, Felt),
    ) -> (Felt, Felt) {
        let old_contract_address = std::mem::replace(
            &mut self.execution_info.contract_address,
            new_contract_address,
        );
        let old_caller_address =
            std::mem::replace(&mut self.execution_info.caller_address, new_caller_address);
        (old_contract_address, old_caller_address)
    }

    /// Restores the addresses in the context.
    ///
    /// Called after `call_entry_point`.
    pub fn close_caller_context(
        &mut self,
        (old_contract_address, old_caller_address): (Felt, Felt),
    ) {
        self.execution_info.contract_address = old_contract_address;
        self.execution_info.caller_address = old_caller_address;
    }
}

/// Reads a contract entrypoint result.
///
/// The value should be of type `PanicResult<(Span<Felt>,)>`.
/// Returns `Ok(Ok(felts))` for success, `Ok(Err(felts))` for panic.
fn read_contract_result(value: &Value) -> Result<Result<Vec<Felt>, Vec<Felt>>, Error> {
    let unexpected_value_error = Err(Error::UnexpectedValue(String::from(
        "PanicResult<(Span<Felt>,)>",
    )));

    // The value should be of type: Enum<Struct<Struct<Span<Felt>>>, Struct<Panic,Array<Felt>>>
    let Value::Enum { tag, value, .. } = value else {
        return unexpected_value_error;
    };
    match tag {
        0 => {
            // The value should be of type: Struct<Struct<Span<Felt>>>
            let Value::Struct { fields: values, .. } = value.as_ref() else {
                return unexpected_value_error;
            };
            let value = if values.len() != 1 {
                return unexpected_value_error;
            } else {
                &values[0]
            };
            // The value should be of type: Struct<Span<Felt>>
            let Value::Struct { fields: values, .. } = value else {
                return unexpected_value_error;
            };
            let value = if values.len() != 1 {
                return unexpected_value_error;
            } else {
                &values[0]
            };
            // The value should be of type: Span<Felt>
            let Value::Array(values) = value else {
                return unexpected_value_error;
            };
            // The values should be of type: Felt
            let Some(values) = values
                .iter()
                .map(|value| {
                    if let Value::Felt252(value) = value {
                        Some(*value)
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<Felt>>>()
            else {
                return unexpected_value_error;
            };
            Ok(Ok(values))
        }
        1 => {
            // The value should be of type: Struct<Panic,Array<Felt>>
            let Value::Struct { fields: values, .. } = value.as_ref() else {
                return unexpected_value_error;
            };
            let value = if values.len() != 2 {
                return unexpected_value_error;
            } else {
                &values[1]
            };
            // The value should be of type: Array<Felt>
            let Value::Array(values) = value else {
                return unexpected_value_error;
            };
            // The values should be of type: Felt
            let Some(values) = values
                .iter()
                .map(|value| {
                    if let Value::Felt252(value) = value {
                        Some(*value)
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<Felt>>>()
            else {
                return unexpected_value_error;
            };
            Ok(Err(values))
        }
        _ => unexpected_value_error,
    }
}

impl StarknetSyscallHandler for &mut StubSyscallHandler {
    #[instrument(skip(self))]
    fn get_block_hash(
        &mut self,
        block_number: u64,
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<Felt> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::GET_BLOCK_HASH)?;

        if let Some(block_hash) = self.block_hash.get(&block_number) {
            Ok(*block_hash)
        } else {
            Err(vec![Felt::from_bytes_be_slice(b"GET_BLOCK_HASH_NOT_SET")])
        }
    }

    #[instrument(skip(self))]
    fn get_execution_info(
        &mut self,
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<crate::starknet::ExecutionInfo> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::GET_EXECUTION_INFO)?;
        Ok(self.execution_info.clone())
    }

    #[instrument(skip(self))]
    fn get_execution_info_v2(
        &mut self,
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<crate::starknet::ExecutionInfoV2> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::GET_EXECUTION_INFO)?;
        Ok(ExecutionInfoV2 {
            block_info: self.execution_info.block_info,
            tx_info: TxV2Info {
                version: self.execution_info.tx_info.version,
                account_contract_address: self.execution_info.tx_info.account_contract_address,
                max_fee: self.execution_info.tx_info.max_fee,
                signature: self.execution_info.tx_info.signature.clone(),
                transaction_hash: self.execution_info.tx_info.transaction_hash,
                chain_id: self.execution_info.tx_info.chain_id,
                nonce: self.execution_info.tx_info.nonce,
                ..TxV2Info::default()
            },
            caller_address: self.execution_info.caller_address,
            contract_address: self.execution_info.contract_address,
            entry_point_selector: self.execution_info.entry_point_selector,
        })
    }

    #[instrument(skip(self))]
    fn get_execution_info_v3(
        &mut self,
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<crate::starknet::ExecutionInfoV3> {
        todo!();
    }

    #[instrument(skip(self))]
    fn deploy(
        &mut self,
        class_hash: Felt,
        contract_address_salt: Felt,
        calldata: &[Felt],
        deploy_from_zero: bool,
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<(Felt, Vec<Felt>)> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::DEPLOY)?;

        /// Max value for a contract address: 2**251 - 256.
        const CONTRACT_ADDRESS_BOUND: NonZeroFelt =
            NonZeroFelt::from_felt_unchecked(Felt::from_hex_unwrap(
                "0x7ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00",
            ));
        /// Cairo string for "STARKNET_CONTRACT_ADDRESS"
        const CONTRACT_ADDRESS_PREFIX: Felt =
            Felt::from_hex_unwrap("0x535441524b4e45545f434f4e54524143545f41444452455353");

        let deployer_address = if deploy_from_zero {
            Felt::zero()
        } else {
            self.execution_info.contract_address
        };
        let deployed_contract_address = {
            let constructor_calldata_hash = Pedersen::hash_array(calldata);
            Pedersen::hash_array(&[
                CONTRACT_ADDRESS_PREFIX,
                deployer_address,
                contract_address_salt,
                class_hash,
                constructor_calldata_hash,
            ])
            .mod_floor(&CONTRACT_ADDRESS_BOUND)
        };

        let Some(contract_info) = self.contracts_info.get(&class_hash) else {
            return Err(vec![Felt::from_bytes_be_slice(b"CLASS_HASH_NOT_FOUND")]);
        };

        if self
            .deployed_contracts
            .insert(deployed_contract_address, class_hash)
            .is_some()
        {
            return Err(vec![Felt::from_bytes_be_slice(
                b"CONTRACT_ALREADY_DEPLOYED",
            )]);
        }

        if let Some(constructor) = contract_info.constructor.clone() {
            let old_addrs = self.open_caller_context((deployed_contract_address, deployer_address));
            let res = self.call_entry_point(remaining_gas, &constructor, calldata);
            self.close_caller_context(old_addrs);
            match res {
                Ok(res) => Ok((deployed_contract_address, res)),
                Err(mut res) => {
                    res.push(Felt::from_bytes_be_slice(b"CONSTRUCTOR_FAILED"));
                    Err(res)
                }
            }
        } else if calldata.is_empty() {
            Ok((deployed_contract_address, vec![]))
        } else {
            // Remove the contract from the deployed contracts,
            // since it failed to deploy.
            self.deployed_contracts.remove(&deployed_contract_address);
            Err(vec![Felt::from_bytes_be_slice(b"INVALID_CALLDATA_LEN")])
        }
    }

    #[instrument(skip(self))]
    fn replace_class(
        &mut self,
        class_hash: Felt,
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<()> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::REPLACE_CLASS)?;

        if !self.contracts_info.contains_key(&class_hash) {
            return Err(vec![Felt::from_bytes_be_slice(b"CLASS_HASH_NOT_FOUND")]);
        };
        self.deployed_contracts
            .insert(self.execution_info.contract_address, class_hash);
        Ok(())
    }

    #[instrument(skip(self))]
    fn library_call(
        &mut self,
        class_hash: Felt,
        function_selector: Felt,
        calldata: &[Felt],
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<Vec<Felt>> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::LIBRARY_CALL)?;

        let Some(contract_info) = self.contracts_info.get(&class_hash).cloned() else {
            return Err(vec![Felt::from_bytes_be_slice(b"CLASS_HASH_NOT_DECLARED")]);
        };
        let Some(entry_point) = contract_info
            .externals
            .iter()
            .find(|(k, _)| k.to_bytes_le() == function_selector.to_bytes_le())
            .map(|(_, v)| v)
        else {
            return Err(vec![
                Felt::from_bytes_be_slice(b"ENTRYPOINT_NOT_FOUND"),
                Felt::from_bytes_be_slice(b"ENTRYPOINT_FAILED"),
            ]);
        };

        match self.call_entry_point(remaining_gas, entry_point, calldata) {
            Ok(res) => Ok(res),
            Err(mut err) => {
                err.push(Felt::from_bytes_be_slice(b"ENTRYPOINT_FAILED"));
                Err(err)
            }
        }
    }

    #[instrument(skip(self))]
    fn call_contract(
        &mut self,
        address: Felt,
        entry_point_selector: Felt,
        calldata: &[Felt],
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<Vec<Felt>> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::CALL_CONTRACT)?;

        let Some(class_hash) = self.deployed_contracts.get(&address) else {
            return Err(vec![
                Felt::from_bytes_be_slice(b"CONTRACT_NOT_DEPLOYED"),
                Felt::from_bytes_be_slice(b"ENTRYPOINT_FAILED"),
            ]);
        };
        let contract_info = self
            .contracts_info
            .get(class_hash)
            .expect("Deployed contract not found in registry.")
            .clone();
        let Some(entry_point) = contract_info
            .externals
            .iter()
            .find(|(k, _)| k.to_bytes_le() == entry_point_selector.to_bytes_le())
            .map(|(_, v)| v)
        else {
            return Err(vec![
                Felt::from_bytes_be_slice(b"ENTRYPOINT_NOT_FOUND"),
                Felt::from_bytes_be_slice(b"ENTRYPOINT_FAILED"),
            ]);
        };

        let old_addrs = self.open_caller_context((address, self.execution_info.contract_address));
        let res = self.call_entry_point(remaining_gas, entry_point, calldata);
        self.close_caller_context(old_addrs);
        match res {
            Ok(res) => Ok(res),
            Err(mut res) => {
                res.push(Felt::from_bytes_be_slice(b"ENTRYPOINT_FAILED"));
                Err(res)
            }
        }
    }

    fn storage_read(
        &mut self,
        address_domain: u32,
        address: Felt,
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<Felt> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::STORAGE_READ)?;

        if !address_domain.is_zero() {
            // Only address_domain 0 is currently supported.
            return Err(vec![Felt::from_bytes_be_slice(
                b"Unsupported address domain",
            )]);
        }
        let value = self
            .storage
            .get(&self.execution_info.contract_address)
            .and_then(|contract_storage| contract_storage.get(&address))
            .cloned()
            .unwrap_or_else(|| Felt::from(0));
        Ok(value)
    }

    #[instrument(skip(self))]
    fn storage_write(
        &mut self,
        address_domain: u32,
        address: Felt,
        value: Felt,
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<()> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::STORAGE_WRITE)?;

        if !address_domain.is_zero() {
            // Only address_domain 0 is currently supported.
            return Err(vec![Felt::from_bytes_be_slice(
                b"Unsupported address domain",
            )]);
        }
        self.storage
            .entry(self.execution_info.contract_address)
            .or_default()
            .insert(address, value);
        Ok(())
    }

    #[instrument(skip(self))]
    fn emit_event(
        &mut self,
        keys: &[Felt],
        data: &[Felt],
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<()> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::EMIT_EVENT)?;
        let contract = self.execution_info.contract_address;
        self.logs
            .entry(contract)
            .or_default()
            .events
            .push_back(StubEvent {
                keys: keys.to_vec(),
                data: data.to_vec(),
            });
        Ok(())
    }

    #[instrument(skip(self))]
    fn send_message_to_l1(
        &mut self,
        to_address: Felt,
        payload: &[Felt],
        remaining_gas: &mut u64,
    ) -> crate::starknet::SyscallResult<()> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SEND_MESSAGE_TO_L1)?;
        let contract = self.execution_info.contract_address;
        self.logs
            .entry(contract)
            .or_default()
            .l2_to_l1_messages
            .push_back((to_address, payload.to_vec()));
        Ok(())
    }

    #[instrument(skip(self))]
    fn keccak(&mut self, input: &[u64], gas: &mut u64) -> SyscallResult<U256> {
        tracing::debug!("called");
        deduct_gas(gas, gas_costs::KECCAK)?;

        const KECCAK_FULL_RATE_IN_WORDS: usize = 17;

        let length = input.len();
        let (_n_rounds, remainder) = num_integer::div_rem(length, KECCAK_FULL_RATE_IN_WORDS);

        if remainder != 0 {
            // In VM this error is wrapped into `SyscallExecutionError::SyscallError`
            return Err(vec![Felt::from_hex(
                "0x000000000000000000000000496e76616c696420696e707574206c656e677468",
            )
            .unwrap()]);
        }

        let mut state = [0u64; 25];
        for chunk in input.chunks(KECCAK_FULL_RATE_IN_WORDS) {
            deduct_gas(gas, gas_costs::KECCAK_ROUND_COST)?;
            for (i, val) in chunk.iter().enumerate() {
                state[i] ^= val;
            }
            keccak::f1600(&mut state)
        }

        Ok(U256 {
            hi: u128::from(state[2]) | (u128::from(state[3]) << 64),
            lo: u128::from(state[0]) | (u128::from(state[1]) << 64),
        })
    }

    #[instrument(skip(self))]
    fn secp256k1_new(
        &mut self,
        x: U256,
        y: U256,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256K1_NEW)?;
        Secp256Point::new(x, y).map(|op| op.map(|p| p.into()))
    }

    #[instrument(skip(self))]
    fn secp256k1_add(
        &mut self,
        p0: Secp256k1Point,
        p1: Secp256k1Point,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256K1_ADD)?;
        Ok(Secp256Point::add(p0.into(), p1.into()).into())
    }

    #[instrument(skip(self))]
    fn secp256k1_mul(
        &mut self,
        p: Secp256k1Point,
        m: U256,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256K1_MUL)?;
        Ok(Secp256Point::mul(p.into(), m).into())
    }

    #[instrument(skip(self))]
    fn secp256k1_get_point_from_x(
        &mut self,
        x: U256,
        y_parity: bool,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256K1_GET_POINT_FROM_X)?;
        Secp256Point::get_point_from_x(x, y_parity).map(|op| op.map(|p| p.into()))
    }

    #[instrument(skip(self))]
    fn secp256k1_get_xy(
        &mut self,
        p: Secp256k1Point,
        remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256K1_GET_XY)?;
        Ok((p.x, p.y))
    }

    #[instrument(skip(self))]
    fn secp256r1_new(
        &mut self,
        x: U256,
        y: U256,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256R1_NEW)?;
        Secp256Point::new(x, y).map(|op| op.map(|p| p.into()))
    }

    #[instrument(skip(self))]
    fn secp256r1_add(
        &mut self,
        p0: Secp256r1Point,
        p1: Secp256r1Point,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256R1_ADD)?;
        Ok(Secp256Point::add(p0.into(), p1.into()).into())
    }

    #[instrument(skip(self))]
    fn secp256r1_mul(
        &mut self,
        p: Secp256r1Point,
        m: U256,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256R1_MUL)?;
        Ok(Secp256Point::mul(p.into(), m).into())
    }

    #[instrument(skip(self))]
    fn secp256r1_get_point_from_x(
        &mut self,
        x: U256,
        y_parity: bool,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256R1_GET_POINT_FROM_X)?;
        Secp256Point::get_point_from_x(x, y_parity).map(|op| op.map(|p| p.into()))
    }

    #[instrument(skip(self))]
    fn secp256r1_get_xy(
        &mut self,
        p: Secp256r1Point,
        remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SECP256R1_GET_XY)?;
        Ok((p.x, p.y))
    }

    #[instrument(skip(self))]
    fn meta_tx_v0(
        &mut self,
        address: Felt,
        entry_point_selector: Felt,
        calldata: &[Felt],
        signature: &[Felt],
        remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        todo!("Implement meta_tx_v0 syscall");
    }

    #[cfg(feature = "with-cheatcode")]
    #[instrument(skip(self))]
    fn cheatcode(&mut self, selector: Felt, input: &[Felt]) -> Vec<Felt> {
        tracing::debug!("called");
        let selector_bytes = selector.to_bytes_be();

        let selector = match std::str::from_utf8(&selector_bytes) {
            Ok(selector) => selector.trim_start_matches('\0'),
            Err(_) => return Vec::new(),
        };

        match selector {
            "set_sequencer_address" => {
                self.execution_info.block_info.sequencer_address = input[0];
                vec![]
            }
            "set_caller_address" => {
                self.execution_info.caller_address = input[0];
                vec![]
            }
            "set_contract_address" => {
                self.execution_info.contract_address = input[0];
                vec![]
            }
            "set_account_contract_address" => {
                self.execution_info.tx_info.account_contract_address = input[0];
                vec![]
            }
            "set_transaction_hash" => {
                self.execution_info.tx_info.transaction_hash = input[0];
                vec![]
            }
            "set_nonce" => {
                self.execution_info.tx_info.nonce = input[0];
                vec![]
            }
            "set_version" => {
                self.execution_info.tx_info.version = input[0];
                vec![]
            }
            "set_chain_id" => {
                self.execution_info.tx_info.chain_id = input[0];
                vec![]
            }
            "set_max_fee" => {
                let max_fee = input[0].to_biguint().try_into().unwrap();
                self.execution_info.tx_info.max_fee = max_fee;
                vec![]
            }
            "set_block_number" => {
                let block_number = input[0].to_biguint().try_into().unwrap();
                self.execution_info.block_info.block_number = block_number;
                vec![]
            }
            "set_block_timestamp" => {
                let block_timestamp = input[0].to_biguint().try_into().unwrap();
                self.execution_info.block_info.block_timestamp = block_timestamp;
                vec![]
            }
            "set_block_hash" => {
                let block_number = input[0].to_biguint().try_into().unwrap();
                let block_hash = input[1];
                self.block_hash.insert(block_number, block_hash);
                vec![]
            }
            "set_signature" => {
                self.execution_info.tx_info.signature = input.to_vec();
                vec![]
            }
            "pop_log" => self
                .logs
                .get_mut(&input[0])
                .and_then(|logs| logs.events.pop_front())
                .map(|mut log| {
                    let mut serialized_log = Vec::new();
                    serialized_log.push(log.keys.len().into());
                    serialized_log.append(&mut log.keys);
                    serialized_log.push(log.data.len().into());
                    serialized_log.append(&mut log.data);
                    serialized_log
                })
                .unwrap_or_default(),
            "pop_l2_to_l1_message" => self
                .logs
                .get_mut(&input[0])
                .and_then(|logs| logs.l2_to_l1_messages.pop_front())
                .map(|mut log| {
                    let mut serialized_log = Vec::new();
                    serialized_log.push(log.0);
                    serialized_log.push(log.1.len().into());
                    serialized_log.append(&mut log.1);
                    serialized_log
                })
                .unwrap_or_default(),
            _ => todo!("Implement cheatcode: {}", selector),
        }
    }

    // Reference implementation:
    // https://github.com/starkware-libs/cairo/blob/v2.16.0/crates/cairo-lang-runner/src/casm_run/mod.rs#L1455
    fn sha256_process_block(
        &mut self,
        state: &mut [u32; 8],
        block: &[u32; 16],
        remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::SHA256_PROCESS_BLOCK)?;
        let data_as_bytes =
            GenericArray::from_exact_iter(block.iter().flat_map(|x| x.to_be_bytes())).unwrap();
        sha2::compress256(state, &[data_as_bytes]);
        Ok(())
    }

    fn get_class_hash_at(
        &mut self,
        contract_address: Felt,
        remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        tracing::debug!("called");
        deduct_gas(remaining_gas, gas_costs::GET_CLASS_HASH_AT)?;
        let class_hash = self
            .deployed_contracts
            .get(&contract_address)
            .cloned()
            .unwrap_or_else(Felt::zero);
        Ok(class_hash)
    }
}

pub fn deduct_gas(gas: &mut u64, price: u64) -> Result<(), Vec<Felt>> {
    if *gas < price {
        Err(vec![Felt::from_bytes_be_slice(b"Syscall out of gas")])
    } else {
        *gas -= price;
        Ok(())
    }
}

/// Gas costs for syscalls.
///
/// Taken from cairo-lang-runner syscall handler implementation.
mod gas_costs {
    use cairo_lang_starknet_classes::casm_contract_class::ENTRY_POINT_COST;

    const STEP: u64 = 100;
    const RANGE_CHECK: u64 = 70;
    const BITWISE: u64 = 594;
    const ENTRY_POINT: u64 = ENTRY_POINT_COST as u64 + 500 * STEP;

    // Gas cost for each syscall, minus the precharged base amount.
    pub const CALL_CONTRACT: u64 = 10 * STEP + ENTRY_POINT;
    pub const DEPLOY: u64 = 200 * STEP + ENTRY_POINT;
    pub const EMIT_EVENT: u64 = 10 * STEP;
    pub const GET_BLOCK_HASH: u64 = 50 * STEP;
    pub const GET_EXECUTION_INFO: u64 = 10 * STEP;
    pub const GET_CLASS_HASH_AT: u64 = 50 * STEP;
    pub const KECCAK: u64 = 0;
    pub const KECCAK_ROUND_COST: u64 = 180000;
    pub const SHA256_PROCESS_BLOCK: u64 = 1852 * STEP + 65 * RANGE_CHECK + 1115 * BITWISE;
    pub const LIBRARY_CALL: u64 = CALL_CONTRACT;
    pub const REPLACE_CLASS: u64 = 50 * STEP;
    pub const SECP256K1_ADD: u64 = 254 * STEP + 29 * RANGE_CHECK;
    pub const SECP256K1_GET_POINT_FROM_X: u64 = 260 * STEP + 29 * RANGE_CHECK;
    pub const SECP256K1_GET_XY: u64 = 24 * STEP + 9 * RANGE_CHECK;
    pub const SECP256K1_MUL: u64 = 121810 * STEP + 10739 * RANGE_CHECK;
    pub const SECP256K1_NEW: u64 = 340 * STEP + 36 * RANGE_CHECK;
    pub const SECP256R1_ADD: u64 = 254 * STEP + 29 * RANGE_CHECK;
    pub const SECP256R1_GET_POINT_FROM_X: u64 = 260 * STEP + 29 * RANGE_CHECK;
    pub const SECP256R1_GET_XY: u64 = 24 * STEP + 9 * RANGE_CHECK;
    pub const SECP256R1_MUL: u64 = 121810 * STEP + 10739 * RANGE_CHECK;
    pub const SECP256R1_NEW: u64 = 340 * STEP + 36 * RANGE_CHECK;
    pub const SEND_MESSAGE_TO_L1: u64 = 50 * STEP;
    pub const STORAGE_READ: u64 = 50 * STEP;
    pub const STORAGE_WRITE: u64 = 50 * STEP;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secp256k1_get_xy() {
        let p = Secp256k1Point {
            x: U256 {
                hi: 331229800296699308591929724809569456681,
                lo: 240848751772479376198639683648735950585,
            },
            y: U256 {
                hi: 75181762170223969696219813306313470806,
                lo: 134255467439736302886468555755295925874,
            },
            is_infinity: false,
        };

        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        assert_eq!(
            test_syscall_handler.secp256k1_get_xy(p, &mut gas).unwrap(),
            (
                U256 {
                    hi: 331229800296699308591929724809569456681,
                    lo: 240848751772479376198639683648735950585,
                },
                U256 {
                    hi: 75181762170223969696219813306313470806,
                    lo: 134255467439736302886468555755295925874,
                }
            )
        )
    }

    #[test]
    fn test_secp256k1_secp256k1_new() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let x = U256 {
            lo: 330631467365974629050427735731901850225,
            hi: 97179038819393695679,
        };
        let y = U256 {
            lo: 68974579539311638391577168388077592842,
            hi: 26163136114030451075775058782541084873,
        };

        assert_eq!(
            test_syscall_handler.secp256k1_new(x, y, &mut gas).unwrap(),
            Some(Secp256k1Point {
                x,
                y,
                is_infinity: false
            })
        );
    }

    #[test]
    fn test_secp256k1_secp256k1_new_none() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let x = U256 {
            hi: 330631467365974629050427735731901850225,
            lo: 97179038819393695679,
        };
        let y = U256 { hi: 0, lo: 0 };

        assert!(test_syscall_handler
            .secp256k1_new(x, y, &mut gas)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_secp256k1_ssecp256k1_add() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let p1 = Secp256k1Point {
            x: U256 {
                lo: 3468390537006497937951914270391801752,
                hi: 161825202758953104525843685720298294023,
            },
            y: U256 {
                lo: 336417762351022071123394393598455764152,
                hi: 96009999919712310848645357523629574312,
            },
            is_infinity: false,
        };

        let p2 = p1;

        // 2 * P1
        let p3 = test_syscall_handler
            .secp256k1_add(p1, p2, &mut gas)
            .unwrap();

        let p1_double = Secp256k1Point {
            x: U256 {
                lo: 122909745026270932982812610085084241637,
                hi: 263210499965038831386353541518668627160,
            },
            y: U256 {
                lo: 329597642124196932058042157271922763050,
                hi: 35730324229579385338853513728577301230,
            },
            is_infinity: false,
        };
        assert_eq!(p3, p1_double);
        assert_eq!(
            test_syscall_handler
                .secp256k1_mul(p1, U256 { lo: 2, hi: 0 }, &mut gas)
                .unwrap(),
            p1_double
        );

        // 3 * P1
        let three_p1 = Secp256k1Point {
            x: U256 {
                lo: 240848751772479376198639683648735950585,
                hi: 331229800296699308591929724809569456681,
            },
            y: U256 {
                lo: 134255467439736302886468555755295925874,
                hi: 75181762170223969696219813306313470806,
            },
            is_infinity: false,
        };
        assert_eq!(
            test_syscall_handler
                .secp256k1_add(p1, p3, &mut gas)
                .unwrap(),
            three_p1
        );
        assert_eq!(
            test_syscall_handler
                .secp256k1_mul(p1, U256 { lo: 3, hi: 0 }, &mut gas)
                .unwrap(),
            three_p1
        );
    }

    #[test]
    fn test_secp256k1_get_point_from_x_false_yparity() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        assert_eq!(
            test_syscall_handler
                .secp256k1_get_point_from_x(
                    U256 {
                        lo: 330631467365974629050427735731901850225,
                        hi: 97179038819393695679,
                    },
                    false,
                    &mut gas
                )
                .unwrap()
                .unwrap(),
            Secp256k1Point {
                x: U256 {
                    lo: 330631467365974629050427735731901850225,
                    hi: 97179038819393695679,
                },
                y: U256 {
                    lo: 68974579539311638391577168388077592842,
                    hi: 26163136114030451075775058782541084873,
                },
                is_infinity: false
            }
        );
    }

    #[test]
    fn test_secp256k1_get_point_from_x_true_yparity() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        assert_eq!(
            test_syscall_handler
                .secp256k1_get_point_from_x(
                    U256 {
                        lo: 330631467365974629050427735731901850225,
                        hi: 97179038819393695679,
                    },
                    true,
                    &mut gas
                )
                .unwrap()
                .unwrap(),
            Secp256k1Point {
                x: U256 {
                    lo: 330631467365974629050427735731901850225,
                    hi: 97179038819393695679,
                },
                y: U256 {
                    lo: 271307787381626825071797439039395650341,
                    hi: 314119230806908012387599548649227126582,
                },
                is_infinity: false
            }
        );
    }

    #[test]
    fn test_secp256k1_get_point_from_x_none() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        assert!(test_syscall_handler
            .secp256k1_get_point_from_x(U256 { hi: 0, lo: 0 }, true, &mut gas)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_secp256r1_new() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let x = U256 {
            lo: 330631467365974629050427735731901850225,
            hi: 97179038819393695679,
        };
        let y = U256 {
            lo: 111045440647474106186537215379882575585,
            hi: 118910939004298029402109603132816090461,
        };

        assert_eq!(
            test_syscall_handler
                .secp256r1_new(x, y, &mut gas)
                .unwrap()
                .unwrap(),
            Secp256r1Point {
                x,
                y,
                is_infinity: false
            }
        );
    }

    #[test]
    fn test_secp256r1_new_infinity() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let x = U256 { hi: 0, lo: 0 };
        let y = U256 { hi: 0, lo: 0 };

        assert!(
            test_syscall_handler
                .secp256r1_new(x, y, &mut gas)
                .unwrap()
                .unwrap()
                .is_infinity
        );
    }

    #[test]
    fn test_secp256r1_add() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let p1 = Secp256r1Point {
            x: U256 {
                lo: 330631467365974629050427735731901850225,
                hi: 97179038819393695679,
            },
            y: U256 {
                lo: 111045440647474106186537215379882575585,
                hi: 118910939004298029402109603132816090461,
            },
            is_infinity: false,
        };

        let p2 = p1;

        // 2 * P1
        let p3 = test_syscall_handler
            .secp256r1_add(p1, p2, &mut gas)
            .unwrap();

        let p1_double = Secp256r1Point {
            x: U256 {
                lo: 309339945874468445579793098896656960879,
                hi: 280079427190737520201067412903899817878,
            },
            y: U256 {
                lo: 231570843221643745062297421862629788481,
                hi: 84249534056490759701994051847937833933,
            },
            is_infinity: false,
        };
        assert_eq!(p3, p1_double);
        assert_eq!(
            test_syscall_handler
                .secp256r1_mul(p1, U256 { lo: 2, hi: 0 }, &mut gas)
                .unwrap(),
            p1_double
        );

        // 3 * P1
        let three_p1 = Secp256r1Point {
            x: U256 {
                lo: 195259625777021303662291420857740525307,
                hi: 23850518908906170876551962912581992002,
            },
            y: U256 {
                lo: 282344931843342117515389970197013120959,
                hi: 178681203065513270100417145499857169664,
            },
            is_infinity: false,
        };
        assert_eq!(
            test_syscall_handler
                .secp256r1_add(p1, p3, &mut gas)
                .unwrap(),
            three_p1
        );
        assert_eq!(
            test_syscall_handler
                .secp256r1_mul(p1, U256 { lo: 3, hi: 0 }, &mut gas)
                .unwrap(),
            three_p1
        );
    }

    #[test]
    fn test_secp256r1_get_point_from_x_true_yparity() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let x = U256 {
            lo: 330631467365974629050427735731901850225,
            hi: 97179038819393695679,
        };

        let y = U256 {
            lo: 111045440647474106186537215379882575585,
            hi: 118910939004298029402109603132816090461,
        };

        assert_eq!(
            test_syscall_handler
                .secp256r1_get_point_from_x(x, true, &mut gas)
                .unwrap()
                .unwrap(),
            Secp256r1Point {
                x,
                y,
                is_infinity: false
            }
        );
    }

    #[test]
    fn test_secp256r1_get_point_from_x_false_yparity() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let x = U256 {
            lo: 330631467365974629050427735731901850225,
            hi: 97179038819393695679,
        };

        let y = U256 {
            lo: 229236926352692519791101729645429586206,
            hi: 221371427837412271565447410779117722274,
        };

        assert_eq!(
            test_syscall_handler
                .secp256r1_get_point_from_x(x, false, &mut gas)
                .unwrap()
                .unwrap(),
            Secp256r1Point {
                x,
                y,
                is_infinity: false
            }
        );
    }

    #[test]
    fn test_secp256r1_get_point_from_x_none() {
        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        let x = U256 { lo: 0, hi: 10 };

        assert!(test_syscall_handler
            .secp256r1_get_point_from_x(x, true, &mut gas)
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_secp256r1_get_xy() {
        let p = Secp256r1Point {
            x: U256 {
                lo: 97179038819393695679,
                hi: 330631467365974629050427735731901850225,
            },
            y: U256 {
                lo: 221371427837412271565447410779117722274,
                hi: 229236926352692519791101729645429586206,
            },
            is_infinity: false,
        };

        let mut test_syscall_handler = StubSyscallHandler::default();
        let mut test_syscall_handler = &mut test_syscall_handler;
        let mut gas = u64::MAX;

        assert_eq!(
            test_syscall_handler.secp256r1_get_xy(p, &mut gas).unwrap(),
            (
                U256 {
                    lo: 97179038819393695679,
                    hi: 330631467365974629050427735731901850225,
                },
                U256 {
                    lo: 221371427837412271565447410779117722274,
                    hi: 229236926352692519791101729645429586206,
                }
            )
        )
    }
}
