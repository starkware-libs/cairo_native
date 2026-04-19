//! Reproduce the nested-`invoke_dynamic` arena-reset hazard.
//!
//! Outer program boxes a distinctive felt, calls `call_contract_syscall`, then
//! reads back the box.  A custom syscall handler re-enters `invoke_dynamic` on an
//! inner program from inside `call_contract`, which causes
//! `cairo_native__reset_box_arena()` to run while the outer box is still live.

use crate::common::load_cairo;
use cairo_lang_sierra::ids::FunctionId;
use cairo_native::{
    context::NativeContext,
    executor::JitNativeExecutor,
    starknet::{
        ExecutionInfo, ExecutionInfoV2, ExecutionInfoV3, Secp256k1Point, Secp256r1Point,
        StarknetSyscallHandler, SyscallResult, U256,
    },
    OptLevel, Value,
};
use starknet_types_core::felt::Felt;

const OUTER_PATTERN: &str = "0x1234567890abcdef1234567890abcdef";
const INNER_PATTERN: &str = "0xfeedfacefeedfacefeedfacefeedface";

struct ReentrantHandler<'m> {
    inner: &'m JitNativeExecutor<'m>,
    inner_entry: FunctionId,
}

impl<'m> StarknetSyscallHandler for &mut ReentrantHandler<'m> {
    fn call_contract(
        &mut self,
        _address: Felt,
        _entry_point_selector: Felt,
        _calldata: &[Felt],
        _remaining_gas: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        let result = self
            .inner
            .invoke_dynamic(&self.inner_entry, &[], Some(u64::MAX))
            .expect("inner invoke_dynamic failed");
        match result.return_value {
            Value::Felt252(f) => Ok(vec![f]),
            _ => Ok(vec![]),
        }
    }

    fn get_block_hash(&mut self, _: u64, _: &mut u64) -> SyscallResult<Felt> {
        unimplemented!()
    }
    fn get_execution_info(&mut self, _: &mut u64) -> SyscallResult<ExecutionInfo> {
        unimplemented!()
    }
    fn get_execution_info_v2(&mut self, _: &mut u64) -> SyscallResult<ExecutionInfoV2> {
        unimplemented!()
    }
    fn get_execution_info_v3(&mut self, _: &mut u64) -> SyscallResult<ExecutionInfoV3> {
        unimplemented!()
    }
    fn deploy(
        &mut self,
        _: Felt,
        _: Felt,
        _: &[Felt],
        _: bool,
        _: &mut u64,
    ) -> SyscallResult<(Felt, Vec<Felt>)> {
        unimplemented!()
    }
    fn replace_class(&mut self, _: Felt, _: &mut u64) -> SyscallResult<()> {
        unimplemented!()
    }
    fn library_call(
        &mut self,
        _: Felt,
        _: Felt,
        _: &[Felt],
        _: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        unimplemented!()
    }
    fn storage_read(&mut self, _: u32, _: Felt, _: &mut u64) -> SyscallResult<Felt> {
        unimplemented!()
    }
    fn storage_write(&mut self, _: u32, _: Felt, _: Felt, _: &mut u64) -> SyscallResult<()> {
        unimplemented!()
    }
    fn emit_event(&mut self, _: &[Felt], _: &[Felt], _: &mut u64) -> SyscallResult<()> {
        unimplemented!()
    }
    fn send_message_to_l1(&mut self, _: Felt, _: &[Felt], _: &mut u64) -> SyscallResult<()> {
        unimplemented!()
    }
    fn keccak(&mut self, _: &[u64], _: &mut u64) -> SyscallResult<U256> {
        unimplemented!()
    }
    fn secp256k1_new(
        &mut self,
        _: U256,
        _: U256,
        _: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        unimplemented!()
    }
    fn secp256k1_add(
        &mut self,
        _: Secp256k1Point,
        _: Secp256k1Point,
        _: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        unimplemented!()
    }
    fn secp256k1_mul(
        &mut self,
        _: Secp256k1Point,
        _: U256,
        _: &mut u64,
    ) -> SyscallResult<Secp256k1Point> {
        unimplemented!()
    }
    fn secp256k1_get_point_from_x(
        &mut self,
        _: U256,
        _: bool,
        _: &mut u64,
    ) -> SyscallResult<Option<Secp256k1Point>> {
        unimplemented!()
    }
    fn secp256k1_get_xy(
        &mut self,
        _: Secp256k1Point,
        _: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        unimplemented!()
    }
    fn secp256r1_new(
        &mut self,
        _: U256,
        _: U256,
        _: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        unimplemented!()
    }
    fn secp256r1_add(
        &mut self,
        _: Secp256r1Point,
        _: Secp256r1Point,
        _: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        unimplemented!()
    }
    fn secp256r1_mul(
        &mut self,
        _: Secp256r1Point,
        _: U256,
        _: &mut u64,
    ) -> SyscallResult<Secp256r1Point> {
        unimplemented!()
    }
    fn secp256r1_get_point_from_x(
        &mut self,
        _: U256,
        _: bool,
        _: &mut u64,
    ) -> SyscallResult<Option<Secp256r1Point>> {
        unimplemented!()
    }
    fn secp256r1_get_xy(
        &mut self,
        _: Secp256r1Point,
        _: &mut u64,
    ) -> SyscallResult<(U256, U256)> {
        unimplemented!()
    }
    fn sha256_process_block(
        &mut self,
        _: &mut [u32; 8],
        _: &[u32; 16],
        _: &mut u64,
    ) -> SyscallResult<()> {
        unimplemented!()
    }
    fn get_class_hash_at(&mut self, _: Felt, _: &mut u64) -> SyscallResult<Felt> {
        unimplemented!()
    }
    fn meta_tx_v0(
        &mut self,
        _: Felt,
        _: Felt,
        _: &[Felt],
        _: &[Felt],
        _: &mut u64,
    ) -> SyscallResult<Vec<Felt>> {
        unimplemented!()
    }
}

#[test]
fn nested_invoke_dynamic_must_not_clobber_outer_box() {
    // --- Inner program: allocates its own box, returns its value. ---
    let inner_src = format!(
        r#"
            use core::box::BoxTrait;
            fn run() -> felt252 {{
                let b: Box<felt252> = BoxTrait::new({INNER_PATTERN});
                b.unbox()
            }}
        "#
    );
    let (_inner_name, inner_program, _) = crate::common::load_cairo_str(&inner_src);
    let inner_ctx = NativeContext::new();
    let inner_module = inner_ctx
        .compile(&inner_program, false, Some(Default::default()), None)
        .expect("compile inner");
    let inner_executor =
        JitNativeExecutor::from_native_module(inner_module, OptLevel::None).expect("jit inner");
    let inner_entry = inner_program
        .funcs
        .iter()
        .find(|f| {
            f.id.debug_name
                .as_deref()
                .map(|s| s.ends_with("::run"))
                .unwrap_or(false)
        })
        .expect("inner run")
        .id
        .clone();

    // --- Outer program: box a felt, do a syscall (which re-enters), read the box back. ---
    let outer_src = format!(
        r#"
            use core::box::BoxTrait;
            use starknet::syscalls::call_contract_syscall;
            use starknet::SyscallResultTrait;
            use starknet::ContractAddress;
            fn run() -> felt252 {{
                let b: Box<felt252> = BoxTrait::new({OUTER_PATTERN});
                let _r = call_contract_syscall(
                    0_felt252.try_into().unwrap(),
                    0,
                    array![].span(),
                ).unwrap_syscall();
                b.unbox()
            }}
        "#
    );
    let outer = load_cairo_str(&outer_src);
    let (_outer_name, outer_program, _) = outer;
    let outer_ctx = NativeContext::new();
    let outer_module = outer_ctx
        .compile(&outer_program, false, Some(Default::default()), None)
        .expect("compile outer");
    let outer_executor =
        JitNativeExecutor::from_native_module(outer_module, OptLevel::None).expect("jit outer");
    let outer_entry = outer_program
        .funcs
        .iter()
        .find(|f| {
            f.id.debug_name
                .as_deref()
                .map(|s| s.ends_with("::run"))
                .unwrap_or(false)
        })
        .expect("outer run")
        .id
        .clone();

    let mut handler = ReentrantHandler {
        inner: &inner_executor,
        inner_entry,
    };

    let result = outer_executor
        .invoke_dynamic_with_syscall_handler(&outer_entry, &[], Some(u64::MAX), &mut handler)
        .expect("outer invoke");

    let expected = Felt::from_hex(OUTER_PATTERN).unwrap();
    println!("outer returned: {:#x}", {
        match &result.return_value {
            Value::Felt252(f) => f,
            other => panic!("unexpected return: {:?}", other),
        }
    });
    assert_eq!(
        result.return_value,
        Value::Felt252(expected),
        "outer box was clobbered by nested invoke_dynamic's arena reset"
    );
}

use crate::common::load_cairo_str;
