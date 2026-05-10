use super::ResourceBounds;
use serde::Serialize;
use starknet_types_core::felt::Felt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
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
