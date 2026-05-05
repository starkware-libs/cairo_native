use crate::common::run_native_program;
use cairo_native::{
    starknet::{
        BlockInfo, ExecutionInfo, ExecutionInfoV2, ExecutionInfoV3, Secp256k1Point, Secp256r1Point,
        StarknetSyscallHandler, SyscallResult, TxInfo, TxV2Info, TxV3Info, U256,
    },
    utils::testing::load_program_and_runner,
    Value,
};
use pretty_assertions_sorted::{assert_eq, assert_eq_sorted};
use starknet_types_core::felt::Felt;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

type Log = (Vec<Felt>, Vec<Felt>);
type L2ToL1Message = (Felt, Vec<Felt>);

#[derive(Debug, Default)]
struct ContractLogs {
    events: VecDeque<Log>,
    l2_to_l1_messages: VecDeque<L2ToL1Message>,
}

#[derive(Debug, Default)]
struct TestingState {
    sequencer_address: Felt,
    caller_address: Felt,
    contract_address: Felt,
    account_contract_address: Felt,
    transaction_hash: Felt,
    nonce: Felt,
    chain_id: Felt,
    version: Felt,
    max_fee: u64,
    block_number: u64,
    block_timestamp: u64,
    signature: Vec<Felt>,
    logs: HashMap<Felt, ContractLogs>,
}

struct SyscallHandler {
    /// Arc<Mutex> Is needed to test that the valures are set correct after the execution
    testing_state: Arc<Mutex<TestingState>>,
}

impl SyscallHandler {
    fn new() -> Self {
        Self {
            testing_state: Arc::new(Mutex::new(TestingState::default())),
        }
    }

    fn with(state: Arc<Mutex<TestingState>>) -> Self {
        Self {
            testing_state: state,
        }
    }
}

impl StarknetSyscallHandler for SyscallHandler {
    fn get_block_hash(
        &mut self,
        _block_number: u64,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        Ok(Felt::from_dec_str(
            "1158579293198495875788224011889333769139150068959598053296510642728083832673",
        )
        .unwrap())
    }

    fn get_execution_info(&mut self, _remaining_gas: &mut u64) -> SyscallResult<ExecutionInfo> {
        Ok(ExecutionInfo {
            block_info: BlockInfo {
                block_number: 10057862467973663535,
                block_timestamp: 13878668747512495966,
                sequencer_address: Felt::from_dec_str(
                    "1126241460712630201003776917997524449163698107789103849210792326381258973685",
                )
                .unwrap(),
            },
            tx_info: TxInfo {
                version: Felt::from_dec_str(
                    "1724985403142256920476849371638528834056988111202434162793214195754735917407",
                )
                .unwrap(),
                account_contract_address: Felt::from_dec_str(
                    "2419272378964094005143278046496347854926114240785059742454535261490265649110",
                )
                .unwrap(),
                max_fee: 67871905340377755668863509019681938001,
                signature: Vec::new(),
                transaction_hash: Felt::from_dec_str(
                    "2073267424102447009330753642820908998776456851902897601865334818765025369132",
                )
                .unwrap(),
                chain_id: Felt::from_dec_str(
                    "1727570805086347994328356733148206517040691113666039929118050093237140484117",
                )
                .unwrap(),
                nonce: Felt::from_dec_str(
                    "2223335940097352947792108259394621577330089800429182023415494612506457867705",
                )
                .unwrap(),
            },
            caller_address: Felt::from_dec_str(
                "2367044879643293830108311482898145302930693201376043522909298679498599559539",
            )
            .unwrap(),
            contract_address: Felt::from_dec_str(
                "2322490563038631685097154208793293355074547843057070254216662565231428808211",
            )
            .unwrap(),
            entry_point_selector: Felt::from_dec_str(
                "1501296828847480842982002010206952982741090100977850506550982801410247026532",
            )
            .unwrap(),
        })
    }

    fn get_execution_info_v2(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<ExecutionInfoV2> {
        Ok(ExecutionInfoV2 {
            block_info: BlockInfo {
                block_number: 10290342497028289173,
                block_timestamp: 8376161426686560326,
                sequencer_address: Felt::from_dec_str(
                    "1815189516202718271265591469295511271015058493881778555617445818147186579905",
                )
                .unwrap(),
            },
            tx_info: TxV2Info {
                version: Felt::from_dec_str(
                    "1946630339019864531118751968563861838541265142438690346764722398811248737786",
                )
                .unwrap(),
                account_contract_address: Felt::from_dec_str(
                    "2501333093425095943815772537228190103182643237630648877273495185321298605376",
                )
                .unwrap(),
                max_fee: 268753657614351187400966367706860329387,
                signature: Vec::new(),
                transaction_hash: Felt::from_dec_str(
                    "1123336726531770778820945049824733201592457249587063926479184903627272350002",
                )
                .unwrap(),
                chain_id: Felt::from_dec_str(
                    "2128916697180095451339935431635121484141376377516602728602049361615810538124",
                )
                .unwrap(),
                nonce: Felt::from_dec_str(
                    "3012936192361023209451741736298028332652992971202997279327088951248532774884",
                )
                .unwrap(),
                resource_bounds: Vec::new(),
                tip: 215444579144685671333997376989135077200,
                paymaster_data: Vec::new(),
                nonce_data_availability_mode: 140600095,
                fee_data_availability_mode: 988370659,
                account_deployment_data: Vec::new(),
            },
            caller_address: Felt::from_dec_str(
                "1185632056775552928459345712365014492063999606476424661067102766803470217687",
            )
            .unwrap(),
            contract_address: Felt::from_dec_str(
                "741063429140548584082645215539704615048011618665759826371923004739480130327",
            )
            .unwrap(),
            entry_point_selector: Felt::from_dec_str(
                "477501848519111015718660527024172361930966806556174677443839145770405114061",
            )
            .unwrap(),
        })
    }

    fn get_execution_info_v3(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<ExecutionInfoV3> {
        Ok(ExecutionInfoV3 {
            block_info: BlockInfo {
                block_number: 10290342497028289173,
                block_timestamp: 8376161426686560326,
                sequencer_address: Felt::from_dec_str(
                    "1815189516202718271265591469295511271015058493881778555617445818147186579905",
                )
                .unwrap(),
            },
            tx_info: TxV3Info {
                version: Felt::from_dec_str(
                    "1946630339019864531118751968563861838541265142438690346764722398811248737786",
                )
                .unwrap(),
                account_contract_address: Felt::from_dec_str(
                    "2501333093425095943815772537228190103182643237630648877273495185321298605376",
                )
                .unwrap(),
                max_fee: 268753657614351187400966367706860329387,
                signature: Vec::new(),
                transaction_hash: Felt::from_dec_str(
                    "1123336726531770778820945049824733201592457249587063926479184903627272350002",
                )
                .unwrap(),
                chain_id: Felt::from_dec_str(
                    "2128916697180095451339935431635121484141376377516602728602049361615810538124",
                )
                .unwrap(),
                nonce: Felt::from_dec_str(
                    "3012936192361023209451741736298028332652992971202997279327088951248532774884",
                )
                .unwrap(),
                resource_bounds: Vec::new(),
                tip: 215444579144685671333997376989135077200,
                paymaster_data: Vec::new(),
                nonce_data_availability_mode: 140600095,
                fee_data_availability_mode: 988370659,
                account_deployment_data: Vec::new(),
                proof_facts: vec![0.into(), 1.into(), 2.into()],
            },
            caller_address: Felt::from_dec_str(
                "1185632056775552928459345712365014492063999606476424661067102766803470217687",
            )
            .unwrap(),
            contract_address: Felt::from_dec_str(
                "741063429140548584082645215539704615048011618665759826371923004739480130327",
            )
            .unwrap(),
            entry_point_selector: Felt::from_dec_str(
                "477501848519111015718660527024172361930966806556174677443839145770405114061",
            )
            .unwrap(),
        })
    }

    fn deploy(
        &mut self,
        _class_hash: Felt,
        _contract_address_salt: Felt,
        _calldata: &[Felt],
        _deploy_from_zero: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(Felt, Vec<Felt>)> {
        Ok((
            Felt::from_dec_str(
                "1833707083418045616336697070784512826809940908236872124572250196391719980392",
            )
            .unwrap(),
            Vec::new(),
        ))
    }

    fn replace_class(&mut self, _class_hash: Felt, _remaining_gas: &mut u64) -> SyscallResult<()> {
        Ok(())
    }

    fn library_call(
        &mut self,
        _class_hash: Felt,
        _function_selector: Felt,
        _calldata: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        Ok(vec![
            Felt::from_dec_str(
                "3358892263739032253767642605669710712087178958719188919195252597609334880396",
            )
            .unwrap(),
            Felt::from_dec_str(
                "1104291043781086177955655234103730593173963850634630109574183288837411031513",
            )
            .unwrap(),
            Felt::from_dec_str(
                "3346377229881115874907650557159666001431249650068516742483979624047277128413",
            )
            .unwrap(),
        ])
    }

    fn call_contract(
        &mut self,
        _address: Felt,
        _entry_point_selector: Felt,
        _calldata: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        Ok(vec![
            Felt::from_dec_str(
                "3358892263739032253767642605669710712087178958719188919195252597609334880396",
            )
            .unwrap(),
            Felt::from_dec_str(
                "1104291043781086177955655234103730593173963850634630109574183288837411031513",
            )
            .unwrap(),
            Felt::from_dec_str(
                "3346377229881115874907650557159666001431249650068516742483979624047277128413",
            )
            .unwrap(),
        ])
    }

    fn storage_read(
        &mut self,
        _address_domain: u32,
        _address: Felt,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        Ok(Felt::from_dec_str(
            "1013181629378419652272218169322268188846114273878719855200100663863924329981",
        )
        .unwrap())
    }

    fn storage_write(
        &mut self,
        _address_domain: u32,
        _address: Felt,
        _value: Felt,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        Ok(())
    }

    fn emit_event(
        &mut self,
        _keys: &[Felt],
        _data: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        Ok(())
    }

    fn send_message_to_l1(
        &mut self,
        to_address: Felt,
        payload: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        assert_eq!(
            to_address,
            3.into(),
            "send_message_to_l1 to_address mismatch"
        );
        assert_eq!(payload, &[2.into()], "send_message_to_l1 payload mismatch");
        Ok(())
    }

    fn keccak(&mut self, _input: &[u64], _remaining_gas: &mut u64) -> SyscallResult<U256> {
        Ok(U256 {
            hi: 330939983442938156232262046592599923289,
            lo: 288102973244655531496349286021939642254,
        })
    }

    fn secp256k1_new(
        &mut self,
        _x: U256,
        _y: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256k1_add(
        &mut self,
        _p0: Secp256k1Point,
        _p1: Secp256k1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256k1_mul(
        &mut self,
        _p: Secp256k1Point,
        _m: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256k1_get_point_from_x(
        &mut self,
        _x: U256,
        _y_parity: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256k1_get_xy(
        &mut self,
        _p: Secp256k1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256r1_new(
        &mut self,
        _x: U256,
        _y: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256r1_add(
        &mut self,
        _p0: Secp256r1Point,
        _p1: Secp256r1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256r1_mul(
        &mut self,
        _p: Secp256r1Point,
        _m: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256r1_get_point_from_x(
        &mut self,
        _x: U256,
        _y_parity: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn secp256r1_get_xy(
        &mut self,
        _p: Secp256r1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        // Tested in `tests/tests/starknet/secp256.rs`.
        unimplemented!()
    }

    fn sha256_process_block(
        &mut self,
        _state: &mut [u32; 8],
        _block: &[u32; 16],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<()> {
        Ok(())
    }

    fn get_class_hash_at(
        &mut self,
        contract_address: Felt,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        Ok(contract_address)
    }

    fn meta_tx_v0(
        &mut self,
        _address: Felt,
        _entry_point_selector: Felt,
        _calldata: &[Felt],
        _signature: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        Ok(vec![
            Felt::from_dec_str(
                "3358892263739032253767642605669710712087178958719188919195252597609334880396",
            )
            .unwrap(),
            Felt::from_dec_str(
                "1104291043781086177955655234103730593173963850634630109574183288837411031513",
            )
            .unwrap(),
            Felt::from_dec_str(
                "3346377229881115874907650557159666001431249650068516742483979624047277128413",
            )
            .unwrap(),
        ])
    }

    fn cheatcode(&mut self, selector: Felt, input: &[Felt]) -> Vec<Felt> {
        let selector_bytes = selector.to_bytes_be();

        let selector = match std::str::from_utf8(&selector_bytes) {
            Ok(selector) => selector.trim_start_matches('\0'),
            Err(_) => return Vec::new(),
        };

        match selector {
            "set_sequencer_address" => {
                self.testing_state.lock().unwrap().sequencer_address = input[0];
                vec![]
            }
            "set_caller_address" => {
                self.testing_state.lock().unwrap().caller_address = input[0];
                vec![]
            }
            "set_contract_address" => {
                self.testing_state.lock().unwrap().contract_address = input[0];
                vec![]
            }
            "set_account_contract_address" => {
                self.testing_state.lock().unwrap().account_contract_address = input[0];
                vec![]
            }
            "set_transaction_hash" => {
                self.testing_state.lock().unwrap().transaction_hash = input[0];
                vec![]
            }
            "set_nonce" => {
                self.testing_state.lock().unwrap().nonce = input[0];
                vec![]
            }
            "set_version" => {
                self.testing_state.lock().unwrap().version = input[0];
                vec![]
            }
            "set_chain_id" => {
                self.testing_state.lock().unwrap().chain_id = input[0];
                vec![]
            }
            "set_max_fee" => {
                let max_fee = input[0].to_biguint().try_into().unwrap();
                self.testing_state.lock().unwrap().max_fee = max_fee;
                vec![]
            }
            "set_block_number" => {
                let block_number = input[0].to_biguint().try_into().unwrap();
                self.testing_state.lock().unwrap().block_number = block_number;
                vec![]
            }
            "set_block_timestamp" => {
                let block_timestamp = input[0].to_biguint().try_into().unwrap();
                self.testing_state.lock().unwrap().block_timestamp = block_timestamp;
                vec![]
            }
            "set_signature" => {
                self.testing_state.lock().unwrap().signature = input.to_vec();
                vec![]
            }
            "pop_log" => self
                .testing_state
                .lock()
                .unwrap()
                .logs
                .get_mut(&input[0])
                .and_then(|logs| logs.events.pop_front())
                .map(|mut log| {
                    let mut serialized_log = Vec::new();
                    serialized_log.push(log.0.len().into());
                    serialized_log.append(&mut log.0);
                    serialized_log.push(log.1.len().into());
                    serialized_log.append(&mut log.1);
                    serialized_log
                })
                .unwrap_or_default(),
            "pop_l2_to_l1_message" => self
                .testing_state
                .lock()
                .unwrap()
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
            _ => vec![],
        }
    }
}

#[test]
fn get_block_hash() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "get_block_hash",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Felt252(
                Felt::from_dec_str(
                    "1158579293198495875788224011889333769139150068959598053296510642728083832673",
                )
                .unwrap()
            )),
            debug_name: None,
        },
    );
}

#[test]
fn get_execution_info() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "get_execution_info",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![
                            Value::Uint64(10057862467973663535),
                            Value::Uint64(13878668747512495966),
                            Value::Felt252(Felt::from_dec_str(
                                "1126241460712630201003776917997524449163698107789103849210792326381258973685",
                            )
                            .unwrap()),
                        ],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![
                            Value::Felt252(Felt::from_dec_str(
                                "1724985403142256920476849371638528834056988111202434162793214195754735917407",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "2419272378964094005143278046496347854926114240785059742454535261490265649110",
                            )
                            .unwrap()),
                            Value::Uint128(67871905340377755668863509019681938001),
                            Value::Struct {
                                fields: vec![
                                    Value::Array(Vec::new()),
                                ],
                                debug_name: None
                            },
                            Value::Felt252(Felt::from_dec_str(
                                "2073267424102447009330753642820908998776456851902897601865334818765025369132",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "1727570805086347994328356733148206517040691113666039929118050093237140484117",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "2223335940097352947792108259394621577330089800429182023415494612506457867705",
                            )
                            .unwrap()),
                        ],
                        debug_name: None,
                    },
                    Value::Felt252(Felt::from_dec_str(
                        "2367044879643293830108311482898145302930693201376043522909298679498599559539",
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "2322490563038631685097154208793293355074547843057070254216662565231428808211",
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "1501296828847480842982002010206952982741090100977850506550982801410247026532",
                    )
                    .unwrap()),
                ],
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn get_execution_info_v2() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "get_execution_info_v2",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![
                            Value::Uint64(10290342497028289173),
                            Value::Uint64(8376161426686560326),
                            Value::Felt252(Felt::from_dec_str(
                                "1815189516202718271265591469295511271015058493881778555617445818147186579905",
                            )
                            .unwrap()),
                        ],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![
                            Value::Felt252(Felt::from_dec_str(
                                "1946630339019864531118751968563861838541265142438690346764722398811248737786",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "2501333093425095943815772537228190103182643237630648877273495185321298605376",
                            )
                            .unwrap()),
                            Value::Uint128(268753657614351187400966367706860329387),
                            Value::Struct {
                                fields: vec![Value::Array(Vec::new())],
                                debug_name: None,
                            },
                            Value::Felt252(Felt::from_dec_str(
                                "1123336726531770778820945049824733201592457249587063926479184903627272350002",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "2128916697180095451339935431635121484141376377516602728602049361615810538124",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "3012936192361023209451741736298028332652992971202997279327088951248532774884",
                            )
                            .unwrap()),
                            Value::Struct {
                                fields: vec![Value::Array(Vec::new())],
                                debug_name: None,
                            },
                            Value::Uint128(215444579144685671333997376989135077200),
                            Value::Struct {
                                fields: vec![Value::Array(Vec::new())],
                                debug_name: None,
                            },
                            Value::Uint32(140600095),
                            Value::Uint32(988370659),
                            Value::Struct {
                                fields: vec![Value::Array(Vec::new())],
                                debug_name: None,
                            },
                        ],
                        debug_name: None,
                    },
                    Value::Felt252(Felt::from_dec_str(
                        "1185632056775552928459345712365014492063999606476424661067102766803470217687",
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "741063429140548584082645215539704615048011618665759826371923004739480130327",
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "477501848519111015718660527024172361930966806556174677443839145770405114061",
                    )
                    .unwrap()),
                ],
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn get_execution_info_v3() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "get_execution_info_v3",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![
                            Value::Uint64(10290342497028289173),
                            Value::Uint64(8376161426686560326),
                            Value::Felt252(Felt::from_dec_str(
                                "1815189516202718271265591469295511271015058493881778555617445818147186579905",
                            )
                            .unwrap()),
                        ],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![
                            Value::Felt252(Felt::from_dec_str(
                                "1946630339019864531118751968563861838541265142438690346764722398811248737786",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "2501333093425095943815772537228190103182643237630648877273495185321298605376",
                            )
                            .unwrap()),
                            Value::Uint128(268753657614351187400966367706860329387),
                            Value::Struct {
                                fields: vec![Value::Array(Vec::new())],
                                debug_name: None,
                            },
                            Value::Felt252(Felt::from_dec_str(
                                "1123336726531770778820945049824733201592457249587063926479184903627272350002",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "2128916697180095451339935431635121484141376377516602728602049361615810538124",
                            )
                            .unwrap()),
                            Value::Felt252(Felt::from_dec_str(
                                "3012936192361023209451741736298028332652992971202997279327088951248532774884",
                            )
                            .unwrap()),
                            Value::Struct {
                                fields: vec![Value::Array(Vec::new())],
                                debug_name: None,
                            },
                            Value::Uint128(215444579144685671333997376989135077200),
                            Value::Struct {
                                fields: vec![Value::Array(Vec::new())],
                                debug_name: None,
                            },
                            Value::Uint32(140600095),
                            Value::Uint32(988370659),
                            Value::Struct {
                                fields: vec![Value::Array(Vec::new())],
                                debug_name: None,
                            },
                            Value::Struct {
                                fields: vec![Value::Array(vec![
                                    Value::Felt252(0.into()),
                                    Value::Felt252(1.into()),
                                    Value::Felt252(2.into()),
                                ])],
                                debug_name: None,
                            },
                        ],
                        debug_name: None,
                    },
                    Value::Felt252(Felt::from_dec_str(
                        "1185632056775552928459345712365014492063999606476424661067102766803470217687",
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "741063429140548584082645215539704615048011618665759826371923004739480130327",
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "477501848519111015718660527024172361930966806556174677443839145770405114061",
                    )
                    .unwrap()),
                ],
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn deploy() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "deploy",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Felt252(Felt::from_dec_str(
                        "1833707083418045616336697070784512826809940908236872124572250196391719980392",
                    )
                    .unwrap()),
                    Value::Struct {
                        fields: vec![Value::Array(Vec::new())],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        },
    )
}

#[test]
fn replace_class() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "replace_class",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: Vec::new(),
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn library_call() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "library_call",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![Value::Array(vec![
                    Value::Felt252(Felt::from_dec_str(
                        "3358892263739032253767642605669710712087178958719188919195252597609334880396"
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "1104291043781086177955655234103730593173963850634630109574183288837411031513"
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "3346377229881115874907650557159666001431249650068516742483979624047277128413"
                    )
                    .unwrap()),
                ])],
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn call_contract() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "call_contract",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![Value::Array(vec![
                    Value::Felt252(Felt::from_dec_str(
                        "3358892263739032253767642605669710712087178958719188919195252597609334880396"
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "1104291043781086177955655234103730593173963850634630109574183288837411031513"
                    )
                    .unwrap()),
                    Value::Felt252(Felt::from_dec_str(
                        "3346377229881115874907650557159666001431249650068516742483979624047277128413"
                    )
                    .unwrap()),
                ])],
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn storage_read() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "storage_read",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Felt252(Felt::from_dec_str(
                        "1013181629378419652272218169322268188846114273878719855200100663863924329981",
                    )
                    .unwrap()),
                ],
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn storage_write() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "storage_write",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![Value::Struct {
                    fields: Vec::new(),
                    debug_name: None,
                }],
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn emit_event() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "emit_event",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: Vec::new(),
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn send_message_to_l1() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "send_message_to_l1",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: Vec::new(),
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn keccak() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "keccak",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Uint128(288102973244655531496349286021939642254),
                    Value::Uint128(330939983442938156232262046592599923289),
                ],
                debug_name: None,
            }),
            debug_name: None,
        },
    );
}

#[test]
fn set_sequencer_address() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let address = Felt::THREE;
    let state = Arc::new(Mutex::new(TestingState::default()));

    let result = run_native_program(
        &program,
        "set_sequencer_address",
        &[Value::Felt252(address)],
        Some(u64::MAX),
        Some(SyscallHandler::with(state.clone())),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Struct {
            fields: vec![Value::Array(vec![])],
            debug_name: Some("core::array::Span::<core::felt252>".to_string())
        }
    );

    let actual_address = state.lock().unwrap().sequencer_address;
    assert_eq!(address, actual_address);
}

#[test]
fn set_max_fee() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let max_fee = 3;
    let state = Arc::new(Mutex::new(TestingState::default()));

    let result = run_native_program(
        &program,
        "set_max_fee",
        &[Value::Felt252(Felt::from(max_fee))],
        Some(u64::MAX),
        Some(SyscallHandler::with(state.clone())),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Struct {
            fields: vec![Value::Array(vec![])],
            debug_name: Some("core::array::Span::<core::felt252>".to_string())
        }
    );

    let actual_max_fee = state.lock().unwrap().max_fee;
    assert_eq!(max_fee, actual_max_fee);
}

#[test]
fn set_signature() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let signature = vec![Felt::ONE, Felt::TWO, Felt::THREE];

    let signature_jit = signature
        .clone()
        .into_iter()
        .map(Value::Felt252)
        .collect::<Vec<_>>();

    let state = Arc::new(Mutex::new(TestingState::default()));

    let result = run_native_program(
        &program,
        "set_signature",
        &[Value::Array(signature_jit)],
        Some(u64::MAX),
        Some(SyscallHandler::with(state.clone())),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Struct {
            fields: vec![Value::Array(vec![])],
            debug_name: Some("core::array::Span::<core::felt252>".to_string())
        }
    );

    let actual_signature = state.lock().unwrap().signature.clone();
    assert_eq_sorted!(signature, actual_signature);
}

#[test]
fn pop_log() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let log_index = Felt::ONE;
    let mut log = (vec![Felt::ONE, Felt::TWO], vec![Felt::THREE]);
    let state = Arc::new(Mutex::new(TestingState::default()));

    let logs = ContractLogs {
        l2_to_l1_messages: VecDeque::new(),
        events: VecDeque::from(vec![log.clone()]),
    };

    state.lock().unwrap().logs.insert(log_index, logs);

    let result = run_native_program(
        &program,
        "pop_log",
        &[Value::Felt252(log_index)],
        Some(u64::MAX),
        Some(SyscallHandler::with(state.clone())),
    );

    let mut serialized_log = Vec::new();
    serialized_log.push(log.0.len().into());
    serialized_log.append(&mut log.0);
    serialized_log.push(log.1.len().into());
    serialized_log.append(&mut log.1);

    let serialized_log_jit = serialized_log
        .into_iter()
        .map(Value::Felt252)
        .collect::<Vec<_>>();

    assert_eq_sorted!(
        result.return_value,
        Value::Struct {
            fields: vec![Value::Array(serialized_log_jit)],
            debug_name: Some("core::array::Span::<core::felt252>".to_string())
        }
    );

    assert!(state
        .lock()
        .unwrap()
        .logs
        .get(&log_index)
        .unwrap()
        .events
        .is_empty());
}

#[test]
fn pop_log_empty() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let log_index = Felt::ONE;
    let state = Arc::new(Mutex::new(TestingState::default()));

    let result = run_native_program(
        &program,
        "pop_log",
        &[Value::Felt252(log_index)],
        Some(u64::MAX),
        Some(SyscallHandler::with(state.clone())),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Struct {
            fields: vec![Value::Array(Vec::new())],
            debug_name: Some("core::array::Span::<core::felt252>".to_string())
        }
    );
}

#[test]
fn pop_l2_to_l1_message() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let log_index = Felt::ONE;
    let mut message = (Felt::ONE, vec![Felt::TWO, Felt::THREE]);
    let state = Arc::new(Mutex::new(TestingState::default()));

    let logs = ContractLogs {
        l2_to_l1_messages: VecDeque::from(vec![message.clone()]),
        events: VecDeque::new(),
    };

    state.lock().unwrap().logs.insert(log_index, logs);

    let result = run_native_program(
        &program,
        "pop_l2_to_l1_message",
        &[Value::Felt252(log_index)],
        Some(u64::MAX),
        Some(SyscallHandler::with(state.clone())),
    );

    let mut serialized_message = Vec::new();
    serialized_message.push(message.0);
    serialized_message.push(message.1.len().into());
    serialized_message.append(&mut message.1);

    let serialized_message_jit = serialized_message
        .into_iter()
        .map(Value::Felt252)
        .collect::<Vec<_>>();

    assert_eq_sorted!(
        result.return_value,
        Value::Struct {
            fields: vec![Value::Array(serialized_message_jit)],
            debug_name: Some("core::array::Span::<core::felt252>".to_string())
        }
    );

    assert!(state
        .lock()
        .unwrap()
        .logs
        .get(&log_index)
        .unwrap()
        .events
        .is_empty());
}

#[test]
fn sha256_process() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "sha256_process",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![Value::Struct {
                    fields: vec![
                        1779033703_u32.into(),
                        3144134277_u32.into(),
                        1013904242_u32.into(),
                        2773480762_u32.into(),
                        1359893119_u32.into(),
                        2600822924_u32.into(),
                        528734635_u32.into(),
                        1541459225_u32.into(),
                    ],
                    debug_name: None
                }],
                debug_name: None
            }),
            debug_name: None,
        },
    );
}

#[test]
fn get_class_hash_at() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "get_class_hash_at",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Felt252(2.into())),
            debug_name: None,
        },
    );
}

#[test]
fn multi_sha256_flow() {
    let program = load_program_and_runner("test_data_artifacts/programs/starknet/syscalls");
    let result = run_native_program(
        &program,
        "multi_sha256_flow",
        &[],
        Some(u64::MAX),
        Some(SyscallHandler::new()),
    );

    assert_eq_sorted!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![Value::Enum {
                    tag: 1,
                    value: Box::new(Value::Struct {
                        fields: vec![],
                        debug_name: Some("Unit".to_string()),
                    }),
                    debug_name: Some("core::bool".to_string()),
                }],
                debug_name: None
            }),
            debug_name: None,
        },
    );
}
