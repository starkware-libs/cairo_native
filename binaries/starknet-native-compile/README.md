# Starknet Native Compile

A binary for compiling Sierra contracts with [Cairo Native](https://starkware-libs.github.io/cairo_native/cairo_native/).

## Requirements

See [Cairo Native's instructions](https://starkware-libs.github.io/cairo_native/cairo_native/#getting-started) on how to build this binary properly.

## Usage

First, compile a sierra contract class with starknet-native-compile. The contract class format should be compatible with [cairo_lang_starknet_classes::contract_class::ContractClass](https://docs.rs/cairo-lang-starknet-classes/*/cairo_lang_starknet_classes/contract_class/struct.ContractClass.html).

```bash
starknet-native-compile <CONTRACT_CLASS> <OUTPUT_LIBRARY>
```

Then, load and execute the shared library with [cairo_native::executor::AotContractExecutor](https://starkware-libs.github.io/cairo_native/cairo_native/executor/struct.AotContractExecutor.html).
