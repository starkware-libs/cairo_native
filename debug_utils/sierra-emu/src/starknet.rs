use std::{
    collections::{BTreeMap, VecDeque},
    iter::once,
};

pub use cairo_native_syscalls::{
    BlockInfo, ExecutionInfo, ExecutionInfoV2, ExecutionInfoV3, ResourceBounds, Secp256k1Point,
    Secp256r1Point, StarknetSyscallHandler, SyscallResult, TxInfo, TxV2Info, TxV3Info, U256,
};
use k256::elliptic_curve::{
    generic_array::GenericArray,
    sec1::{FromEncodedPoint, ToEncodedPoint},
};
use sec1::point::Coordinates;
use serde::Serialize;
use starknet_types_core::felt::Felt;

pub mod value_conv;

/// A (somewhat) usable implementation of the starknet syscall handler trait.
///
/// Currently gas is not deducted.
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
pub struct StubSyscallHandler {
    pub storage: BTreeMap<(u32, Felt), Felt>,
    pub events: Vec<StubEvent>,
    pub execution_info: ExecutionInfoV2,
    pub logs: BTreeMap<Felt, ContractLogs>,
}

/// Event emitted by the emit_event syscall.
#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
pub struct StubEvent {
    pub keys: Vec<Felt>,
    pub data: Vec<Felt>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize)]
pub struct ContractLogs {
    pub events: VecDeque<StubEvent>,
    pub l2_to_l1_messages: VecDeque<L2ToL1Message>,
}

type L2ToL1Message = (Felt, Vec<Felt>);

impl Default for StubSyscallHandler {
    fn default() -> Self {
        Self {
            storage: BTreeMap::new(),
            events: Vec::new(),
            execution_info: ExecutionInfoV2 {
                block_info: BlockInfo {
                    block_number: 0,
                    block_timestamp: 0,
                    sequencer_address: 666.into(),
                },
                tx_info: TxV2Info {
                    version: 1.into(),
                    account_contract_address: 1.into(),
                    max_fee: 0,
                    signature: vec![1.into()],
                    transaction_hash: 1.into(),
                    chain_id: 1.into(),
                    nonce: 0.into(),
                    resource_bounds: vec![],
                    tip: 0,
                    paymaster_data: vec![],
                    nonce_data_availability_mode: 0,
                    fee_data_availability_mode: 0,
                    account_deployment_data: vec![],
                },
                caller_address: 2.into(),
                contract_address: 3.into(),
                entry_point_selector: 4.into(),
            },
            logs: BTreeMap::new(),
        }
    }
}

impl StarknetSyscallHandler for StubSyscallHandler {
    fn get_block_hash(
        &mut self,
        block_number: u64,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        Ok(block_number.into())
    }

    fn get_execution_info(&mut self, _remaining_gas: &mut u64) -> SyscallResult<ExecutionInfo> {
        Ok(ExecutionInfo {
            block_info: self.execution_info.block_info,
            tx_info: TxInfo {
                version: self.execution_info.tx_info.version,
                account_contract_address: self.execution_info.tx_info.account_contract_address,
                max_fee: self.execution_info.tx_info.max_fee,
                signature: self.execution_info.tx_info.signature.clone(),
                transaction_hash: self.execution_info.tx_info.transaction_hash,
                chain_id: self.execution_info.tx_info.chain_id,
                nonce: self.execution_info.tx_info.nonce,
            },
            caller_address: self.execution_info.caller_address,
            contract_address: self.execution_info.contract_address,
            entry_point_selector: self.execution_info.entry_point_selector,
        })
    }

    fn get_execution_info_v2(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<ExecutionInfoV2> {
        Ok(self.execution_info.clone())
    }

    fn get_execution_info_v3(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<ExecutionInfoV3> {
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
        address_domain: u32,
        address: Felt,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        if let Some(value) = self.storage.get(&(address_domain, address)) {
            Ok(*value)
        } else {
            Err(vec![Felt::from_bytes_be_slice(b"address not found")])
        }
    }

    fn storage_write(
        &mut self,
        address_domain: u32,
        address: Felt,
        value: Felt,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        self.storage.insert((address_domain, address), value);
        Ok(())
    }

    fn emit_event(
        &mut self,
        keys: &[Felt],
        data: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        self.events.push(StubEvent {
            keys: keys.to_vec(),
            data: data.to_vec(),
        });
        Ok(())
    }

    fn send_message_to_l1(
        &mut self,
        _to_address: Felt,
        _payload: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        unimplemented!()
    }

    fn keccak(&mut self, input: &[u64], gas: &mut u64) -> SyscallResult<U256> {
        let length = input.len();

        if length % 17 != 0 {
            let error_msg = b"Invalid keccak input size";
            let felt_error = Felt::from_bytes_be_slice(error_msg);
            return Err(vec![felt_error]);
        }

        let n_chunks = length / 17;
        let mut state = [0u64; 25];

        const KECCAK_ROUND_COST: u64 = 180000;
        for i in 0..n_chunks {
            if *gas < KECCAK_ROUND_COST {
                let error_msg = b"Syscall out of gas";
                let felt_error = Felt::from_bytes_be_slice(error_msg);
                return Err(vec![felt_error]);
            }

            *gas -= KECCAK_ROUND_COST;
            let chunk = &input[i * 17..(i + 1) * 17]; //(request.input_start + i * 17)?;
            for (i, val) in chunk.iter().enumerate() {
                state[i] ^= val;
            }
            keccak::f1600(&mut state)
        }

        // state[0] and state[1] conform the hash_high (u128)
        // state[2] and state[3] conform the hash_low (u128)
        SyscallResult::Ok(U256 {
            lo: state[0] as u128 | ((state[1] as u128) << 64),
            hi: state[2] as u128 | ((state[3] as u128) << 64),
        })
    }

    fn secp256k1_new(
        &mut self,
        x: U256,
        y: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        // The following unwraps should be unreachable because the iterator we provide has the
        // expected number of bytes.
        let point = k256::ProjectivePoint::from_encoded_point(
            &k256::EncodedPoint::from_affine_coordinates(
                &GenericArray::from_exact_iter(
                    x.hi.to_be_bytes().into_iter().chain(x.lo.to_be_bytes()),
                )
                .unwrap(),
                &GenericArray::from_exact_iter(
                    y.hi.to_be_bytes().into_iter().chain(y.lo.to_be_bytes()),
                )
                .unwrap(),
                false,
            ),
        );

        if bool::from(point.is_some()) {
            Ok(Some(Secp256k1Point {
                x,
                y,
                is_infinity: false,
            }))
        } else {
            Ok(None)
        }
    }

    fn secp256k1_add(
        &mut self,
        p0: Secp256k1Point,
        p1: Secp256k1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        // The inner unwraps should be unreachable because the iterator we provide has the expected
        // number of bytes. The outer unwraps depend on the felt values, which should be valid since
        // they'll be provided by secp256 syscalls.
        let p0 = k256::ProjectivePoint::from_encoded_point(
            &k256::EncodedPoint::from_affine_coordinates(
                &GenericArray::from_exact_iter(
                    p0.x.hi
                        .to_be_bytes()
                        .into_iter()
                        .chain(p0.x.lo.to_be_bytes()),
                )
                .unwrap(),
                &GenericArray::from_exact_iter(
                    p0.y.hi
                        .to_be_bytes()
                        .into_iter()
                        .chain(p0.y.lo.to_be_bytes()),
                )
                .unwrap(),
                false,
            ),
        )
        .unwrap();
        let p1 = k256::ProjectivePoint::from_encoded_point(
            &k256::EncodedPoint::from_affine_coordinates(
                &GenericArray::from_exact_iter(
                    p1.x.hi
                        .to_be_bytes()
                        .into_iter()
                        .chain(p1.x.lo.to_be_bytes()),
                )
                .unwrap(),
                &GenericArray::from_exact_iter(
                    p1.y.hi
                        .to_be_bytes()
                        .into_iter()
                        .chain(p1.y.lo.to_be_bytes()),
                )
                .unwrap(),
                false,
            ),
        )
        .unwrap();

        let p = p0 + p1;

        let p = p.to_encoded_point(false);
        let (x, y) = match p.coordinates() {
            Coordinates::Uncompressed { x, y } => (x, y),
            Coordinates::Identity => {
                // P + (-P) yields the identity. Return the canonical (0, 0) +
                // is_infinity encoding (see Secp256k1Point::into_value).
                return Ok(Secp256k1Point {
                    x: U256 { lo: 0, hi: 0 },
                    y: U256 { lo: 0, hi: 0 },
                    is_infinity: true,
                });
            }
            _ => {
                // We explicitly asked for the uncompressed encoding.
                unreachable!()
            }
        };

        // The following two unwraps should be safe because the array always has 32 bytes. The other
        // four are definitely safe because the slicing guarantees its length to be the right one.
        let x: [u8; 32] = x.as_slice().try_into().unwrap();
        let y: [u8; 32] = y.as_slice().try_into().unwrap();
        Ok(Secp256k1Point {
            x: U256 {
                hi: u128::from_be_bytes(x[0..16].try_into().unwrap()),
                lo: u128::from_be_bytes(x[16..32].try_into().unwrap()),
            },
            y: U256 {
                hi: u128::from_be_bytes(y[0..16].try_into().unwrap()),
                lo: u128::from_be_bytes(y[16..32].try_into().unwrap()),
            },
            is_infinity: false,
        })
    }

    fn secp256k1_mul(
        &mut self,
        p: Secp256k1Point,
        m: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        // The inner unwrap should be unreachable because the iterator we provide has the expected
        // number of bytes. The outer unwrap depends on the felt values, which should be valid since
        // they'll be provided by secp256 syscalls.
        let p = k256::ProjectivePoint::from_encoded_point(
            &k256::EncodedPoint::from_affine_coordinates(
                &GenericArray::from_exact_iter(
                    p.x.hi.to_be_bytes().into_iter().chain(p.x.lo.to_be_bytes()),
                )
                .unwrap(),
                &GenericArray::from_exact_iter(
                    p.y.hi.to_be_bytes().into_iter().chain(p.y.lo.to_be_bytes()),
                )
                .unwrap(),
                false,
            ),
        )
        .unwrap();
        let m: k256::Scalar = k256::elliptic_curve::ScalarPrimitive::from_slice(&{
            let mut buf = [0u8; 32];
            buf[0..16].copy_from_slice(&m.hi.to_be_bytes());
            buf[16..32].copy_from_slice(&m.lo.to_be_bytes());
            buf
        })
        .map_err(|_| {
            vec![Felt::from_bytes_be(
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0invalid scalar",
            )]
        })?
        .into();

        let p = p * m;

        let p = p.to_encoded_point(false);
        let (x, y) = match p.coordinates() {
            Coordinates::Uncompressed { x, y } => (x, y),
            Coordinates::Identity => {
                // k * P can be the identity (e.g. k = ord(P)).
                return Ok(Secp256k1Point {
                    x: U256 { lo: 0, hi: 0 },
                    y: U256 { lo: 0, hi: 0 },
                    is_infinity: true,
                });
            }
            _ => {
                // We explicitly asked for the uncompressed encoding.
                unreachable!()
            }
        };

        // The following two unwraps should be safe because the array always has 32 bytes. The other
        // four are definitely safe because the slicing guarantees its length to be the right one.
        let x: [u8; 32] = x.as_slice().try_into().unwrap();
        let y: [u8; 32] = y.as_slice().try_into().unwrap();
        Ok(Secp256k1Point {
            x: U256 {
                hi: u128::from_be_bytes(x[0..16].try_into().unwrap()),
                lo: u128::from_be_bytes(x[16..32].try_into().unwrap()),
            },
            y: U256 {
                hi: u128::from_be_bytes(y[0..16].try_into().unwrap()),
                lo: u128::from_be_bytes(y[16..32].try_into().unwrap()),
            },
            is_infinity: false,
        })
    }

    fn secp256k1_get_point_from_x(
        &mut self,
        x: U256,
        y_parity: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        // The inner unwrap should be unreachable because the iterator we provide has the expected
        // number of bytes. The outer unwrap depends on the encoding format, which should be valid
        // since it's hardcoded..
        let point = k256::ProjectivePoint::from_encoded_point(
            &k256::EncodedPoint::from_bytes(
                k256::CompressedPoint::from_exact_iter(
                    once(0x02 | y_parity as u8)
                        .chain(x.hi.to_be_bytes())
                        .chain(x.lo.to_be_bytes()),
                )
                .unwrap(),
            )
            .unwrap(),
        );

        if bool::from(point.is_some()) {
            // This unwrap has already been checked in the `if` expression's condition.
            let p = point.unwrap();

            let p = p.to_encoded_point(false);
            let y = match p.coordinates() {
                Coordinates::Uncompressed { y, .. } => y,
                _ => {
                    // This should be unreachable because we explicitly asked for the uncompressed
                    // encoding.
                    unreachable!()
                }
            };

            // The following unwrap should be safe because the array always has 32 bytes. The other
            // two are definitely safe because the slicing guarantees its length to be the right
            // one.
            let y: [u8; 32] = y.as_slice().try_into().unwrap();
            Ok(Some(Secp256k1Point {
                x,
                y: U256 {
                    hi: u128::from_be_bytes(y[0..16].try_into().unwrap()),
                    lo: u128::from_be_bytes(y[16..32].try_into().unwrap()),
                },
                is_infinity: false,
            }))
        } else {
            Ok(None)
        }
    }

    fn secp256k1_get_xy(
        &mut self,
        p: Secp256k1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        Ok((p.x, p.y))
    }

    fn secp256r1_new(
        &mut self,
        x: U256,
        y: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        // The following unwraps should be unreachable because the iterator we provide has the
        // expected number of bytes.
        let point = p256::ProjectivePoint::from_encoded_point(
            &k256::EncodedPoint::from_affine_coordinates(
                &GenericArray::from_exact_iter(
                    x.hi.to_be_bytes().into_iter().chain(x.lo.to_be_bytes()),
                )
                .unwrap(),
                &GenericArray::from_exact_iter(
                    y.hi.to_be_bytes().into_iter().chain(y.lo.to_be_bytes()),
                )
                .unwrap(),
                false,
            ),
        );

        if bool::from(point.is_some()) {
            Ok(Some(Secp256r1Point {
                x,
                y,
                is_infinity: false,
            }))
        } else {
            Ok(None)
        }
    }

    fn secp256r1_add(
        &mut self,
        p0: Secp256r1Point,
        p1: Secp256r1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        // The inner unwraps should be unreachable because the iterator we provide has the expected
        // number of bytes. The outer unwraps depend on the felt values, which should be valid since
        // they'll be provided by secp256 syscalls.
        let p0 = p256::ProjectivePoint::from_encoded_point(
            &p256::EncodedPoint::from_affine_coordinates(
                &GenericArray::from_exact_iter(
                    p0.x.hi
                        .to_be_bytes()
                        .into_iter()
                        .chain(p0.x.lo.to_be_bytes()),
                )
                .unwrap(),
                &GenericArray::from_exact_iter(
                    p0.y.hi
                        .to_be_bytes()
                        .into_iter()
                        .chain(p0.y.lo.to_be_bytes()),
                )
                .unwrap(),
                false,
            ),
        )
        .unwrap();
        let p1 = p256::ProjectivePoint::from_encoded_point(
            &p256::EncodedPoint::from_affine_coordinates(
                &GenericArray::from_exact_iter(
                    p1.x.hi
                        .to_be_bytes()
                        .into_iter()
                        .chain(p1.x.lo.to_be_bytes()),
                )
                .unwrap(),
                &GenericArray::from_exact_iter(
                    p1.y.hi
                        .to_be_bytes()
                        .into_iter()
                        .chain(p1.y.lo.to_be_bytes()),
                )
                .unwrap(),
                false,
            ),
        )
        .unwrap();

        let p = p0 + p1;

        let p = p.to_encoded_point(false);
        let (x, y) = match p.coordinates() {
            Coordinates::Uncompressed { x, y } => (x, y),
            Coordinates::Identity => {
                // P + (-P) yields the identity. Return the canonical (0, 0) +
                // is_infinity encoding (see Secp256r1Point::into_value).
                return Ok(Secp256r1Point {
                    x: U256 { lo: 0, hi: 0 },
                    y: U256 { lo: 0, hi: 0 },
                    is_infinity: true,
                });
            }
            _ => {
                // We explicitly asked for the uncompressed encoding.
                unreachable!()
            }
        };

        // The following two unwraps should be safe because the array always has 32 bytes. The other
        // four are definitely safe because the slicing guarantees its length to be the right one.
        let x: [u8; 32] = x.as_slice().try_into().unwrap();
        let y: [u8; 32] = y.as_slice().try_into().unwrap();
        Ok(Secp256r1Point {
            x: U256 {
                hi: u128::from_be_bytes(x[0..16].try_into().unwrap()),
                lo: u128::from_be_bytes(x[16..32].try_into().unwrap()),
            },
            y: U256 {
                hi: u128::from_be_bytes(y[0..16].try_into().unwrap()),
                lo: u128::from_be_bytes(y[16..32].try_into().unwrap()),
            },
            is_infinity: false,
        })
    }

    fn secp256r1_mul(
        &mut self,
        p: Secp256r1Point,
        m: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        // The inner unwrap should be unreachable because the iterator we provide has the expected
        // number of bytes. The outer unwrap depends on the felt values, which should be valid since
        // they'll be provided by secp256 syscalls.
        let p = p256::ProjectivePoint::from_encoded_point(
            &p256::EncodedPoint::from_affine_coordinates(
                &GenericArray::from_exact_iter(
                    p.x.hi.to_be_bytes().into_iter().chain(p.x.lo.to_be_bytes()),
                )
                .unwrap(),
                &GenericArray::from_exact_iter(
                    p.y.hi.to_be_bytes().into_iter().chain(p.y.lo.to_be_bytes()),
                )
                .unwrap(),
                false,
            ),
        )
        .unwrap();
        let m: p256::Scalar = p256::elliptic_curve::ScalarPrimitive::from_slice(&{
            let mut buf = [0u8; 32];
            buf[0..16].copy_from_slice(&m.hi.to_be_bytes());
            buf[16..32].copy_from_slice(&m.lo.to_be_bytes());
            buf
        })
        .map_err(|_| {
            vec![Felt::from_bytes_be(
                b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0invalid scalar",
            )]
        })?
        .into();

        let p = p * m;
        let p = p.to_encoded_point(false);
        let (x, y) = match p.coordinates() {
            Coordinates::Uncompressed { x, y } => (x, y),
            Coordinates::Identity => {
                // k * P can be the identity (e.g. k = ord(P)).
                return Ok(Secp256r1Point {
                    x: U256 { lo: 0, hi: 0 },
                    y: U256 { lo: 0, hi: 0 },
                    is_infinity: true,
                });
            }
            _ => {
                // We explicitly asked for the uncompressed encoding.
                unreachable!()
            }
        };

        // The following two unwraps should be safe because the array always has 32 bytes. The other
        // four are definitely safe because the slicing guarantees its length to be the right one.
        let x: [u8; 32] = x.as_slice().try_into().unwrap();
        let y: [u8; 32] = y.as_slice().try_into().unwrap();
        Ok(Secp256r1Point {
            x: U256 {
                hi: u128::from_be_bytes(x[0..16].try_into().unwrap()),
                lo: u128::from_be_bytes(x[16..32].try_into().unwrap()),
            },
            y: U256 {
                hi: u128::from_be_bytes(y[0..16].try_into().unwrap()),
                lo: u128::from_be_bytes(y[16..32].try_into().unwrap()),
            },
            is_infinity: false,
        })
    }

    fn secp256r1_get_point_from_x(
        &mut self,
        x: U256,
        y_parity: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        let point = p256::ProjectivePoint::from_encoded_point(
            &p256::EncodedPoint::from_bytes(
                p256::CompressedPoint::from_exact_iter(
                    once(0x02 | y_parity as u8)
                        .chain(x.hi.to_be_bytes())
                        .chain(x.lo.to_be_bytes()),
                )
                .unwrap(),
            )
            .unwrap(),
        );

        if bool::from(point.is_some()) {
            let p = point.unwrap();

            let p = p.to_encoded_point(false);
            let y = match p.coordinates() {
                Coordinates::Uncompressed { y, .. } => y,
                _ => unreachable!(),
            };

            let y: [u8; 32] = y.as_slice().try_into().unwrap();
            Ok(Some(Secp256r1Point {
                x,
                y: U256 {
                    hi: u128::from_be_bytes(y[0..16].try_into().unwrap()),
                    lo: u128::from_be_bytes(y[16..32].try_into().unwrap()),
                },
                is_infinity: false,
            }))
        } else {
            Ok(None)
        }
    }

    fn secp256r1_get_xy(
        &mut self,
        p: Secp256r1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        Ok((p.x, p.y))
    }

    fn sha256_process_block(
        &mut self,
        prev_state: &mut [u32; 8],
        current_block: &[u32; 16],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        let data_as_bytes = sha2::digest::generic_array::GenericArray::from_exact_iter(
            current_block.iter().flat_map(|x| x.to_be_bytes()),
        )
        .unwrap();
        sha2::compress256(prev_state, &[data_as_bytes]);
        Ok(())
    }
}
