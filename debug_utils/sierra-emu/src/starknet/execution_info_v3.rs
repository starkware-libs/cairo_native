use super::{BlockInfo, TxV3Info};
use serde::Serialize;
use starknet_types_core::felt::Felt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ExecutionInfoV3 {
    pub block_info: BlockInfo,
    pub tx_info: TxV3Info,
    pub caller_address: Felt,
    pub contract_address: Felt,
    pub entry_point_selector: Felt,
}
