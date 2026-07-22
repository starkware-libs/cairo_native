use crate::common::run_native_program;
use cairo_native::{
    starknet::{Secp256k1Point, Secp256r1Point, StarknetSyscallHandler, SyscallResult, U256},
    utils::testing::load_program_and_runner,
    Value,
};
use pretty_assertions_sorted::assert_eq;
use starknet_types_core::felt::Felt;
use std::collections::VecDeque;

#[derive(Debug, Default)]
struct SyscallHandler {
    secp256k1_new: (VecDeque<(U256, U256)>, VecDeque<Option<Secp256k1Point>>),
    secp256k1_add: (
        VecDeque<(Secp256k1Point, Secp256k1Point)>,
        VecDeque<Secp256k1Point>,
    ),
    secp256k1_mul: (VecDeque<(Secp256k1Point, U256)>, VecDeque<Secp256k1Point>),
    secp256k1_get_point_from_x: (VecDeque<(U256, bool)>, VecDeque<Option<Secp256k1Point>>),
    secp256k1_get_xy: (VecDeque<Secp256k1Point>, VecDeque<(U256, U256)>),

    secp256r1_new: (VecDeque<(U256, U256)>, VecDeque<Option<Secp256r1Point>>),
    secp256r1_add: (
        VecDeque<(Secp256r1Point, Secp256r1Point)>,
        VecDeque<Secp256r1Point>,
    ),
    secp256r1_mul: (VecDeque<(Secp256r1Point, U256)>, VecDeque<Secp256r1Point>),
    secp256r1_get_point_from_x: (VecDeque<(U256, bool)>, VecDeque<Option<Secp256r1Point>>),
    secp256r1_get_xy: (VecDeque<Secp256r1Point>, VecDeque<(U256, U256)>),
}

impl StarknetSyscallHandler for &mut SyscallHandler {
    fn get_block_hash(
        &mut self,
        _block_number: u64,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Felt> {
        unimplemented!()
    }

    fn get_execution_info(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<cairo_native::starknet::ExecutionInfo> {
        unimplemented!()
    }

    fn get_execution_info_v2(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<cairo_native::starknet::ExecutionInfoV2> {
        unimplemented!()
    }

    fn get_execution_info_v3(
        &mut self,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<cairo_native::starknet::ExecutionInfoV3> {
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

    fn deploy_v2(
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
        x: U256,
        y: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        let (args, rets) = &mut self.secp256k1_new;

        args.push_back((x, y));
        Ok(rets.pop_front().unwrap())
    }

    fn secp256k1_add(
        &mut self,
        p0: Secp256k1Point,
        p1: Secp256k1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        let (args, rets) = &mut self.secp256k1_add;

        args.push_back((p0, p1));
        Ok(rets.pop_front().unwrap())
    }

    fn secp256k1_mul(
        &mut self,
        p: Secp256k1Point,
        m: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        let (args, rets) = &mut self.secp256k1_mul;

        args.push_back((p, m));
        Ok(rets.pop_front().unwrap())
    }

    fn secp256k1_get_point_from_x(
        &mut self,
        x: U256,
        y_parity: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        let (args, rets) = &mut self.secp256k1_get_point_from_x;

        args.push_back((x, y_parity));
        Ok(rets.pop_front().unwrap())
    }

    fn secp256k1_get_xy(
        &mut self,
        p: Secp256k1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        let (args, rets) = &mut self.secp256k1_get_xy;

        args.push_back(p);
        Ok(rets.pop_front().unwrap())
    }

    fn secp256r1_new(
        &mut self,
        x: U256,
        y: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        let (args, rets) = &mut self.secp256r1_new;

        args.push_back((x, y));
        Ok(rets.pop_front().unwrap())
    }

    fn secp256r1_add(
        &mut self,
        p0: Secp256r1Point,
        p1: Secp256r1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        let (args, rets) = &mut self.secp256r1_add;

        args.push_back((p0, p1));
        Ok(rets.pop_front().unwrap())
    }

    fn secp256r1_mul(
        &mut self,
        p: Secp256r1Point,
        m: U256,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        let (args, rets) = &mut self.secp256r1_mul;

        args.push_back((p, m));
        Ok(rets.pop_front().unwrap())
    }

    fn secp256r1_get_point_from_x(
        &mut self,
        x: U256,
        y_parity: bool,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        let (args, rets) = &mut self.secp256r1_get_point_from_x;

        args.push_back((x, y_parity));
        Ok(rets.pop_front().unwrap())
    }

    fn secp256r1_get_xy(
        &mut self,
        p: Secp256r1Point,
        _remaining_gas: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        let (args, rets) = &mut self.secp256r1_get_xy;

        args.push_back(p);
        Ok(rets.pop_front().unwrap())
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

#[test]
fn secp256k1_new() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256k1_new: (
            VecDeque::from([]),
            VecDeque::from([
                None,
                Some(Secp256k1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                }),
                Some(Secp256k1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    y: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                }),
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256k1_new",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(0)],
                debug_name: None,
            },
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None
                }),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_new",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256K1Point(Secp256k1Point::default())),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_new",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256K1Point(Secp256k1Point::new(
                    u128::MAX,
                    u128::MAX,
                    u128::MAX,
                    u128::MAX,
                    false
                ))),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    assert_eq!(
        syscall_handler.secp256k1_new.0,
        [
            (U256 { hi: 0, lo: 0 }, U256 { hi: 0, lo: 0 }),
            (
                U256 {
                    lo: 0,
                    hi: u128::MAX
                },
                U256 {
                    lo: u128::MAX,
                    hi: 0
                }
            ),
            (
                U256 {
                    hi: u128::MAX,
                    lo: u128::MAX
                },
                U256 {
                    hi: u128::MAX,
                    lo: u128::MAX
                }
            ),
        ],
    );
    assert!(syscall_handler.secp256k1_new.1.is_empty());
}

#[test]
fn secp256k1_add() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256k1_add: (
            VecDeque::from([]),
            VecDeque::from([
                Secp256k1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                },
                Secp256k1Point {
                    x: U256 {
                        lo: u128::MAX,
                        hi: 0,
                    },
                    y: U256 {
                        lo: 0,
                        hi: u128::MAX,
                    },
                    is_infinity: false,
                },
                Secp256k1Point {
                    x: U256 {
                        lo: u128::MAX,
                        hi: u128::MAX,
                    },
                    y: U256 {
                        lo: u128::MAX,
                        hi: u128::MAX,
                    },
                    is_infinity: false,
                },
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256k1_add",
        &[
            Value::Secp256K1Point(Secp256k1Point::default()),
            Value::Secp256K1Point(Secp256k1Point::default()),
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256K1Point(Secp256k1Point::default())),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_add",
        &[
            Value::Secp256K1Point(Secp256k1Point::new(0, u128::MAX, u128::MAX, 0, false)),
            Value::Secp256K1Point(Secp256k1Point::new(u128::MAX, 0, 0, u128::MAX, false)),
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256K1Point(Secp256k1Point::new(
                u128::MAX,
                0,
                0,
                u128::MAX,
                false
            ))),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_add",
        &[
            Value::Secp256K1Point(Secp256k1Point::new(
                u128::MAX,
                u128::MAX,
                u128::MAX,
                u128::MAX,
                false,
            )),
            Value::Secp256K1Point(Secp256k1Point::new(
                u128::MAX,
                u128::MAX,
                u128::MAX,
                u128::MAX,
                false,
            )),
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256K1Point(Secp256k1Point::new(
                u128::MAX,
                u128::MAX,
                u128::MAX,
                u128::MAX,
                false
            ))),
            debug_name: None,
        },
    );

    assert_eq!(
        syscall_handler.secp256k1_add.0,
        [
            (
                Secp256k1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                },
                Secp256k1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                },
            ),
            (
                Secp256k1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: 0
                    },
                    y: U256 {
                        hi: 0,
                        lo: u128::MAX
                    },
                    is_infinity: false,
                },
                Secp256k1Point {
                    x: U256 {
                        hi: 0,
                        lo: u128::MAX
                    },
                    y: U256 {
                        hi: u128::MAX,
                        lo: 0
                    },
                    is_infinity: false,
                },
            ),
            (
                Secp256k1Point {
                    x: U256 {
                        lo: u128::MAX,
                        hi: u128::MAX
                    },
                    y: U256 {
                        lo: u128::MAX,
                        hi: u128::MAX
                    },
                    is_infinity: false,
                },
                Secp256k1Point {
                    x: U256 {
                        lo: u128::MAX,
                        hi: u128::MAX
                    },
                    y: U256 {
                        lo: u128::MAX,
                        hi: u128::MAX
                    },
                    is_infinity: false,
                },
            ),
        ],
    );
    assert!(syscall_handler.secp256k1_add.1.is_empty());
}

#[test]
fn secp256k1_mul() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256k1_mul: (
            VecDeque::from([]),
            VecDeque::from([
                Secp256k1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                },
                Secp256k1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    y: U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                },
                Secp256k1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    y: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                },
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256k1_mul",
        &[
            Value::Secp256K1Point(Secp256k1Point::default()),
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256K1Point(Secp256k1Point::default())),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_mul",
        &[
            Value::Secp256K1Point(Secp256k1Point::new(u128::MAX, 0, 0, u128::MAX, false)),
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256K1Point(Secp256k1Point::new(
                0,
                u128::MAX,
                u128::MAX,
                0,
                false
            ))),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_mul",
        &[
            Value::Secp256K1Point(Secp256k1Point::new(u128::MAX, 0, 0, u128::MAX, false)),
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256K1Point(Secp256k1Point::new(
                u128::MAX,
                u128::MAX,
                u128::MAX,
                u128::MAX,
                false
            ))),
            debug_name: None,
        },
    );

    assert_eq!(
        syscall_handler.secp256k1_mul.0,
        [
            (Secp256k1Point::default(), U256 { hi: 0, lo: 0 },),
            (
                Secp256k1Point::new(u128::MAX, 0, 0, u128::MAX, false),
                U256 {
                    lo: u128::MAX,
                    hi: 0,
                },
            ),
            (
                Secp256k1Point::new(u128::MAX, 0, 0, u128::MAX, false),
                U256 {
                    hi: 0,
                    lo: u128::MAX,
                },
            ),
        ],
    );
    assert!(syscall_handler.secp256k1_mul.1.is_empty());
}

#[test]
fn secp256k1_get_point_from_x() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256k1_get_point_from_x: (
            VecDeque::from([]),
            VecDeque::from([
                None,
                Some(Secp256k1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                }),
                Some(Secp256k1Point {
                    x: U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    y: U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    is_infinity: false,
                }),
                Some(Secp256k1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    y: U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                }),
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256k1_get_point_from_x",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(0)],
                debug_name: None,
            },
            Value::Enum {
                tag: 0,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            }),
            debug_name: None
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_get_point_from_x",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
            Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256K1Point(Secp256k1Point::default())),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_get_point_from_x",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                debug_name: None,
            },
            Value::Enum {
                tag: 0,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256K1Point(Secp256k1Point::new(
                    u128::MAX,
                    0,
                    0,
                    u128::MAX,
                    false
                ))),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256k1_get_point_from_x",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
            Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256K1Point(Secp256k1Point::new(
                    0,
                    u128::MAX,
                    u128::MAX,
                    0,
                    false
                ))),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    assert_eq!(
        syscall_handler.secp256k1_get_point_from_x.0,
        [
            (U256 { hi: 0, lo: 0 }, false),
            (
                U256 {
                    lo: 0,
                    hi: u128::MAX,
                },
                true,
            ),
            (
                U256 {
                    lo: u128::MAX,
                    hi: 0,
                },
                false,
            ),
            (
                U256 {
                    hi: u128::MAX,
                    lo: u128::MAX,
                },
                true,
            ),
        ],
    );
    assert!(syscall_handler.secp256k1_get_point_from_x.1.is_empty());
}

#[test]
fn secp256k1_get_xy() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256k1_get_xy: (
            VecDeque::from([]),
            VecDeque::from([
                (U256 { hi: 0, lo: 0 }, U256 { hi: 0, lo: 0 }),
                (
                    U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                ),
                (
                    U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                ),
                (
                    U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                ),
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256k1_get_xy",
        &[Value::Secp256K1Point(Secp256k1Point::default())],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![Value::Uint128(0), Value::Uint128(0)],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![Value::Uint128(0), Value::Uint128(0)],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        }
    );

    let result = run_native_program(
        &program,
        "secp256k1_get_xy",
        &[Value::Secp256K1Point(Secp256k1Point::new(
            0,
            u128::MAX,
            u128::MAX,
            0,
            false,
        ))],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        }
    );

    let result = run_native_program(
        &program,
        "secp256k1_get_xy",
        &[Value::Secp256K1Point(Secp256k1Point::new(
            u128::MAX,
            0,
            0,
            u128::MAX,
            false,
        ))],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        }
    );

    let result = run_native_program(
        &program,
        "secp256k1_get_xy",
        &[Value::Secp256K1Point(Secp256k1Point::new(
            u128::MAX,
            u128::MAX,
            u128::MAX,
            u128::MAX,
            false,
        ))],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        }
    );

    assert_eq!(
        syscall_handler.secp256k1_get_xy.0,
        [
            Secp256k1Point {
                x: U256 { hi: 0, lo: 0 },
                y: U256 { hi: 0, lo: 0 },
                is_infinity: false
            },
            Secp256k1Point {
                x: U256 {
                    lo: 0,
                    hi: u128::MAX,
                },
                y: U256 {
                    lo: u128::MAX,
                    hi: 0,
                },
                is_infinity: false
            },
            Secp256k1Point {
                x: U256 {
                    lo: u128::MAX,
                    hi: 0,
                },
                y: U256 {
                    lo: 0,
                    hi: u128::MAX,
                },
                is_infinity: false
            },
            Secp256k1Point {
                x: U256 {
                    hi: u128::MAX,
                    lo: u128::MAX,
                },
                y: U256 {
                    hi: u128::MAX,
                    lo: u128::MAX,
                },
                is_infinity: false
            },
        ],
    );
    assert!(syscall_handler.secp256k1_get_xy.1.is_empty());
}

#[test]
fn secp256r1_new() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256r1_new: (
            VecDeque::from([]),
            VecDeque::from([
                None,
                Some(Secp256r1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                }),
                Some(Secp256r1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    y: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                }),
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256r1_new",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(0)],
                debug_name: None,
            },
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None
                }),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_new",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256R1Point(Secp256r1Point::default())),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_new",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256R1Point(Secp256r1Point::new(
                    u128::MAX,
                    u128::MAX,
                    u128::MAX,
                    u128::MAX,
                    false
                ))),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    assert_eq!(
        syscall_handler.secp256r1_new.0,
        [
            (U256 { hi: 0, lo: 0 }, U256 { hi: 0, lo: 0 }),
            (
                U256 {
                    hi: u128::MAX,
                    lo: 0
                },
                U256 {
                    hi: 0,
                    lo: u128::MAX
                }
            ),
            (
                U256 {
                    lo: u128::MAX,
                    hi: u128::MAX
                },
                U256 {
                    hi: u128::MAX,
                    lo: u128::MAX
                }
            ),
        ],
    );
    assert!(syscall_handler.secp256r1_new.1.is_empty());
}

#[test]
fn secp256r1_add() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256r1_add: (
            VecDeque::from([]),
            VecDeque::from([
                Secp256r1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                },
                Secp256r1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    y: U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                },
                Secp256r1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    y: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                },
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256r1_add",
        &[
            Value::Secp256R1Point(Secp256r1Point::default()),
            Value::Secp256R1Point(Secp256r1Point::default()),
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256R1Point(Secp256r1Point::default())),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_add",
        &[
            Value::Secp256R1Point(Secp256r1Point::new(u128::MAX, 0, 0, u128::MAX, false)),
            Value::Secp256R1Point(Secp256r1Point::new(0, u128::MAX, u128::MAX, 0, false)),
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256R1Point(Secp256r1Point::new(
                0,
                u128::MAX,
                u128::MAX,
                0,
                false
            ))),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_add",
        &[
            Value::Secp256R1Point(Secp256r1Point::new(
                u128::MAX,
                u128::MAX,
                u128::MAX,
                u128::MAX,
                false,
            )),
            Value::Secp256R1Point(Secp256r1Point::new(
                u128::MAX,
                u128::MAX,
                u128::MAX,
                u128::MAX,
                false,
            )),
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256R1Point(Secp256r1Point::new(
                u128::MAX,
                u128::MAX,
                u128::MAX,
                u128::MAX,
                false
            )),),
            debug_name: None,
        },
    );

    assert_eq!(
        syscall_handler.secp256r1_add.0,
        [
            (Secp256r1Point::default(), Secp256r1Point::default(),),
            (
                Secp256r1Point::new(u128::MAX, 0, 0, u128::MAX, false),
                Secp256r1Point::new(0, u128::MAX, u128::MAX, 0, false),
            ),
            (
                Secp256r1Point::new(u128::MAX, u128::MAX, u128::MAX, u128::MAX, false),
                Secp256r1Point::new(u128::MAX, u128::MAX, u128::MAX, u128::MAX, false),
            ),
        ],
    );
    assert!(syscall_handler.secp256r1_add.1.is_empty());
}

#[test]
fn secp256r1_mul() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256r1_mul: (
            VecDeque::from([]),
            VecDeque::from([
                Secp256r1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                },
                Secp256r1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    y: U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                },
                Secp256r1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    y: U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                },
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256r1_mul",
        &[
            Value::Secp256R1Point(Secp256r1Point::default()),
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256R1Point(Secp256r1Point::default())),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_mul",
        &[
            Value::Secp256R1Point(Secp256r1Point::new(u128::MAX, 0, 0, u128::MAX, false)),
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256R1Point(Secp256r1Point::new(
                0,
                u128::MAX,
                u128::MAX,
                0,
                false
            ))),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_mul",
        &[
            Value::Secp256R1Point(Secp256r1Point::new(0, u128::MAX, u128::MAX, 0, false)),
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Secp256R1Point(Secp256r1Point::new(
                u128::MAX,
                u128::MAX,
                u128::MAX,
                u128::MAX,
                false
            ))),
            debug_name: None,
        },
    );

    assert_eq!(
        syscall_handler.secp256r1_mul.0,
        [
            (Secp256r1Point::default(), U256 { hi: 0, lo: 0 },),
            (
                Secp256r1Point::new(u128::MAX, 0, 0, u128::MAX, false),
                U256 {
                    lo: u128::MAX,
                    hi: 0,
                },
            ),
            (
                Secp256r1Point::new(0, u128::MAX, u128::MAX, 0, false),
                U256 {
                    lo: 0,
                    hi: u128::MAX,
                },
            ),
        ],
    );
    assert!(syscall_handler.secp256r1_mul.1.is_empty());
}

#[test]
fn secp256r1_get_point_from_x() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256r1_get_point_from_x: (
            VecDeque::from([]),
            VecDeque::from([
                None,
                Some(Secp256r1Point {
                    x: U256 { hi: 0, lo: 0 },
                    y: U256 { hi: 0, lo: 0 },
                    is_infinity: false,
                }),
                Some(Secp256r1Point {
                    x: U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    y: U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    is_infinity: false,
                }),
                Some(Secp256r1Point {
                    x: U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    y: U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    is_infinity: false,
                }),
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256r1_get_point_from_x",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(0)],
                debug_name: None,
            },
            Value::Enum {
                tag: 0,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            }),
            debug_name: None
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_get_point_from_x",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
            Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256R1Point(Secp256r1Point::default())),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_get_point_from_x",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                debug_name: None,
            },
            Value::Enum {
                tag: 0,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256R1Point(Secp256r1Point::new(
                    u128::MAX,
                    0,
                    0,
                    u128::MAX,
                    false
                ))),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    let result = run_native_program(
        &program,
        "secp256r1_get_point_from_x",
        &[
            Value::Struct {
                fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                debug_name: None,
            },
            Value::Enum {
                tag: 1,
                value: Box::new(Value::Struct {
                    fields: vec![],
                    debug_name: None,
                }),
                debug_name: None,
            },
        ],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Enum {
                tag: 0,
                value: Box::new(Value::Secp256R1Point(Secp256r1Point::new(
                    0,
                    u128::MAX,
                    u128::MAX,
                    0,
                    false
                ))),
                debug_name: None,
            }),
            debug_name: None,
        },
    );

    assert_eq!(
        syscall_handler.secp256r1_get_point_from_x.0,
        [
            (U256 { hi: 0, lo: 0 }, false),
            (
                U256 {
                    lo: 0,
                    hi: u128::MAX,
                },
                true,
            ),
            (
                U256 {
                    lo: u128::MAX,
                    hi: 0,
                },
                false,
            ),
            (
                U256 {
                    hi: u128::MAX,
                    lo: u128::MAX,
                },
                true,
            ),
        ],
    );
    assert!(syscall_handler.secp256r1_get_point_from_x.1.is_empty());
}

#[test]
fn secp256r1_get_xy() {
    let program = load_program_and_runner("programs/starknet/secp256");
    let mut syscall_handler = SyscallHandler {
        secp256r1_get_xy: (
            VecDeque::from([]),
            VecDeque::from([
                (U256 { hi: 0, lo: 0 }, U256 { hi: 0, lo: 0 }),
                (
                    U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                    U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                ),
                (
                    U256 {
                        hi: u128::MAX,
                        lo: 0,
                    },
                    U256 {
                        hi: 0,
                        lo: u128::MAX,
                    },
                ),
                (
                    U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                    U256 {
                        hi: u128::MAX,
                        lo: u128::MAX,
                    },
                ),
            ]),
        ),
        ..Default::default()
    };

    let result = run_native_program(
        &program,
        "secp256r1_get_xy",
        &[Value::Secp256R1Point(Secp256r1Point::default())],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![Value::Uint128(0), Value::Uint128(0)],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![Value::Uint128(0), Value::Uint128(0)],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        }
    );

    let result = run_native_program(
        &program,
        "secp256r1_get_xy",
        &[Value::Secp256R1Point(Secp256r1Point::new(
            0,
            u128::MAX,
            u128::MAX,
            0,
            false,
        ))],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        }
    );

    let result = run_native_program(
        &program,
        "secp256r1_get_xy",
        &[Value::Secp256R1Point(Secp256r1Point::new(
            u128::MAX,
            0,
            0,
            u128::MAX,
            false,
        ))],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![Value::Uint128(0), Value::Uint128(u128::MAX)],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![Value::Uint128(u128::MAX), Value::Uint128(0)],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        }
    );

    let result = run_native_program(
        &program,
        "secp256r1_get_xy",
        &[Value::Secp256R1Point(Secp256r1Point::new(
            u128::MAX,
            u128::MAX,
            u128::MAX,
            u128::MAX,
            false,
        ))],
        Some(u64::MAX),
        Some(&mut syscall_handler),
    );
    assert_eq!(
        result.return_value,
        Value::Enum {
            tag: 0,
            value: Box::new(Value::Struct {
                fields: vec![
                    Value::Struct {
                        fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                        debug_name: None,
                    },
                    Value::Struct {
                        fields: vec![Value::Uint128(u128::MAX), Value::Uint128(u128::MAX)],
                        debug_name: None,
                    },
                ],
                debug_name: None,
            }),
            debug_name: None,
        }
    );

    assert_eq!(
        syscall_handler.secp256r1_get_xy.0,
        [
            Secp256r1Point {
                x: U256 { hi: 0, lo: 0 },
                y: U256 { hi: 0, lo: 0 },
                is_infinity: false
            },
            Secp256r1Point {
                x: U256 {
                    lo: 0,
                    hi: u128::MAX,
                },
                y: U256 {
                    lo: u128::MAX,
                    hi: 0,
                },
                is_infinity: false
            },
            Secp256r1Point {
                x: U256 {
                    lo: u128::MAX,
                    hi: 0,
                },
                y: U256 {
                    lo: 0,
                    hi: u128::MAX,
                },
                is_infinity: false
            },
            Secp256r1Point {
                x: U256 {
                    hi: u128::MAX,
                    lo: u128::MAX,
                },
                y: U256 {
                    hi: u128::MAX,
                    lo: u128::MAX,
                },
                is_infinity: false
            },
        ],
    );
    assert!(syscall_handler.secp256r1_get_xy.1.is_empty());
}
