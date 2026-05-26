# Overview

This crate is a compiler and JIT engine that transforms Sierra (or Cairo)
sources into MLIR, which can be
[JIT-executed](https://en.wikipedia.org/wiki/Just-in-time_compilation) or further
compiled into a binary
[ahead of time](https://en.wikipedia.org/wiki/Ahead-of-time_compilation).

## Getting started as a developer

First make sure you have a working environment and are able to compile the
project without issues. Make sure to follow the [setup](/README.md#setup) guide
on steps on how to do this.

It is generally recommended to use the `optimized-dev` cargo profile when
testing or running programs, the make target `make build-dev` will be useful for
this.

### Other tools

In addition to the tools included in Cairo Native, it is also recommended you
have `cairo-compile` and `cairo-run` installed to check how the generated sierra
code looks like, and to compare results manually (when required) which will help
greatly when implementing functionality into Cairo Native.

You can check the [cairo](https://github.com/starkware-libs/cairo) repository
for more info on how to get those tools.

## Basic Workflow

After having implemented your desired feature or bug fix, you should check it
passes all tests and lints, also make sure to add any needed test cases for the
added code.

```bash
# Check it passes all lints
make check

# Check it passes all tests
make test
```

Then you are free to go and make a PR!

## High level project overview

This will explain how the project is structured, without going into much details
yet:

### Project dependencies

The major dependencies of the project are the following:

- Melior: This is the crate that abstracts away most of the interfacing with
  MLIR, our compilation target, it uses mlir-sys and tries to safely
  abstract MLIR in Rust.
- Cairo: We use the cairo crates to keep a close tie to the API contracts
  of the language, they provide a really nice way to know what features the
  language has and aids with codegen. For example, most library functions
  are under enumerations, and thanks to Rust exhaustive pattern matching we
  can't miss any.
- Runtime: The JIT runner and compiler depend on a "runtime" that lives on
  this repository too, it aids with more complex stuff like `pedersen`,
  `keccak` and dictionaries that would be quite complex to implement from
  the ground up in MLIR (Basically would be like coding a complex hash
  function in pseudo assembly).

### Common definitions

Within this project there are lots of functions with the same signature.
As their arguments have all the same meaning, they are documented here:

- `context: NativeContext`: The MLIR context.
- `module: &NativeModule`: The compiled MLIR program, with other relevant
  information such as program registry and metadata.
- `program: &Program`: The Sierra input program.
- `registry: &ProgramRegistry<TType, TLibfunc>`: The registry extracted
  from the program.
- `metadata: &mut MetadataStorage`: Current compiler metadata.

## Project layout

The code is laid out in the following sections:

```txt
 src
 ├─ arch.rs             Trampoline assembly for calling functions with dynamic signatures.
 ├─ arch/               Architecture-specific code for the trampoline.
 ├─ block_ext.rs        A melior (MLIR) block trait extension to write less code.
 ├─ cache.rs            Types and implementations of compiled program caches.
 ├─ compiler.rs         The glue code of the compiler, has the codegen for
                        the function signatures and calls the libfunc
                        codegen implementations.
 ├─ context.rs          The MLIR context wrapper, provides the compile method.
 ├─ debug.rs
 ├─ docs.rs             Documentation modules.
 ├─ error.rs            Error handling,
 ├─ execution_result.rs Program result parsing.
 ├─ executor.rs         The executor & related code,
 ├─ ffi.cpp             Missing FFI C wrappers,
 ├─ ffi.rs              Missing FFI C wrappers, rust side.
 ├─ lib.rs              The main lib file.
 ├─ libfuncs.rs         Cairo Sierra libfunc glue code & implementations,
 ├─ metadata.rs         Metadata injector to use within the compilation process.
 ├─ module.rs           The MLIR module wrapper.
 ├─ starknet.rs         Starknet syscall handler glue code.
 ├─ starknet_stub.rs
 ├─ types.rs            Cairo to MLIR type information,
 ├─ utils.rs            Internal utilities.
 └─ values.rs           JIT serialization.
```

### Library functions

Path: `src/libfuncs`

Here are stored all the library function implementations in MLIR, this
contains the majority of the code.

To store information about the different types of library functions sierra
has, we divide them into the following using the enum `SierraLibFunc`:

- **Branching**: These functions are implemented inline, adding blocks and
  jumping as necessary based on given conditions.
- **Constant**: A constant value, this isn't represented as a function and
  is inserted inline.
- **Function**: Any other function.
- **InlineDataFlow**: Functions that can be implemented inline without much
  problem. For example: `dup`, `store_temp`

### Statements

Path: `src/statements`

Here is the code that processes the statements of non-library functions.
It handles dataflow, branching, function calls, variable storage and also
has implementations for the inline library functions.

### User functions

These are extra utility functions unrelated to sierra that aid in the
development, such as wrapping return values and printing them.

## Basic API usage example

The API contains two structs, `NativeContext` and `NativeExecutor`.
The main purpose of `NativeContext` is MLIR initialization, compilation and
lowering to LLVM.
`NativeExecutor` in the other hand is responsible of executing MLIR
compiled sierra programs from an entrypoint. Programs and JIT states can be
cached in contexts where their execution will be done multiple times.

```rust
use cairo_native::{
    context::NativeContext, executor::JitNativeExecutor, utils::testing::cairo_to_sierra, Value,
};
use starknet_types_core::felt::Felt;
use std::path::Path;

fn main() {
    let program_path = Path::new("programs/examples/hello.cairo");

    // Instantiate a Cairo Native MLIR context. This data structure is responsible for the MLIR
    // initialization and compilation of sierra programs into a MLIR module.
    let native_context = NativeContext::new();

    // Compile the cairo program to sierra.
    let sierra_program = cairo_to_sierra(program_path).unwrap();

    // Compile the sierra program into a MLIR module.
    let native_program = native_context
        .compile(&sierra_program, false, Some(Default::default()), None)
        .unwrap();

    // The parameters of the entry point.
    let params = &[Value::Felt252(Felt::from_bytes_be_slice(b"user"))];

    // Find the entry point id by its name.
    let entry_point = "hello::hello::greet";
    let entry_point_id = cairo_native::utils::find_function_id(&sierra_program, entry_point)
        .expect("entry point not found");

    // Instantiate the executor.
    let native_executor =
        JitNativeExecutor::from_native_module(native_program, Default::default()).unwrap();

    // Execute the program.
    let result = native_executor
        .invoke_dynamic(entry_point_id, params, None)
        .unwrap();

    println!("Cairo program was compiled and executed successfully.");
    println!("{:?}", result);
}
```

## Running a Cairo program

This is a usage example using the API for an easy Cairo program that
requires the least setup to get running. It allows you to compile and
execute a program using the JIT.

Example code to run a program:

```rust
use cairo_native::{
    context::NativeContext, executor::JitNativeExecutor, utils::find_entry_point, Value,
};
use std::path::Path;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

fn main() {
    let program_path = Path::new("programs/echo.cairo");

    // Compile the cairo program to sierra.
    let sierra_program = cairo_native::utils::testing::cairo_to_sierra(program_path).unwrap();

    // Instantiate a Cairo Native MLIR context. This data structure is responsible for the MLIR
    // initialization and compilation of sierra programs into a MLIR module.
    let native_context = NativeContext::new();

    // Compile the sierra program into a MLIR module.
    let native_program = native_context
        .compile(&sierra_program, false, Some(Default::default()), None)
        .unwrap();

    // Find the entry point id by its name.
    let entry_point_fn = find_entry_point(&sierra_program, "echo::echo::main").unwrap();
    let fn_id = &entry_point_fn.id;

    // Instantiate the executor.
    let native_executor =
        JitNativeExecutor::from_native_module(native_program, Default::default()).unwrap();

    // Execute the program.
    let output = native_executor.invoke_dynamic(fn_id, &[Value::Felt252(1.into())], None);

    println!();
    println!("Cairo program was compiled and executed successfully.");
    println!("{output:#?}");
}
```

## Running a Starknet contract

Example code to run a Starknet contract:

```rust
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

/// To run a starknet contract, we need to use a syscall handler.
#[derive(Debug, Default)]
struct SyscallHandler;

fn main() {
    let path = Path::new("programs/examples/hello_starknet.cairo");

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

    let entry_point = contract.entry_points_by_type.external.first().unwrap();
    let sierra_program = contract.extract_sierra_program(false).unwrap().program;

    let native_context = NativeContext::new();

    let native_program = native_context
        .compile(&sierra_program, false, Some(Default::default()), None)
        .unwrap();

    // Call the echo function from the contract using the generated wrapper.
    let entry_point_fn =
        find_entry_point_by_idx(&sierra_program, entry_point.function_idx).unwrap();

    let fn_id = &entry_point_fn.id;

    let native_executor =
        JitNativeExecutor::from_native_module(native_program, Default::default()).unwrap();

    let result = native_executor
        .invoke_contract_dynamic(fn_id, &[Felt::ONE], Some(u64::MAX), SyscallHandler)
        .expect("failed to execute the given contract");

    println!();
    println!("Cairo program was compiled and executed successfully.");
    println!("{result:#?}");
}

// Implement an example syscall handler.
impl StarknetSyscallHandler for SyscallHandler {
    fn get_block_hash(&mut self, block_number: u64, _gas: &mut u64) -> SyscallResult<Felt> {
        println!("Called `get_block_hash({block_number})` from MLIR.");
        Ok(Felt::from_bytes_be_slice(b"get_block_hash ok"))
    }

    fn get_execution_info(&mut self, _gas: &mut u64) -> SyscallResult<ExecutionInfo> {
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

    fn get_execution_info_v2(&mut self, _remaining_gas: &mut u64) -> SyscallResult<ExecutionInfoV2> {
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
        unimplemented!()
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

    fn keccak(&mut self, input: &[u64], _gas: &mut u64) -> SyscallResult<U256> {
        println!("Called `keccak({input:?})` from MLIR.");
        Ok(U256 { hi: 0, lo: 1234567890 })
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
```

For more examples, check out the `examples/` directory.
