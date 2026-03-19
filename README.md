<div align="center">

### ⚡ Cairo Native ⚡

A compiler to convert Cairo's intermediate representation "Sierra" code <br>
to machine code via MLIR and LLVM.

[Report Bug](https://github.com/starkware-libs/cairo_native/issues/new) · [Request Feature](https://github.com/starkware-libs/cairo_native/issues/new)

[![Telegram Chat][tg-badge]][tg-url]
[![rust](https://github.com/starkware-libs/cairo_native/actions/workflows/ci.yml/badge.svg)](https://github.com/starkware-libs/cairo_native/actions/workflows/ci.yml)
[![codecov](https://img.shields.io/codecov/c/github/starkware-libs/cairo_native)](https://codecov.io/gh/starkware-libs/cairo_native)
[![license](https://img.shields.io/github/license/starkware-libs/cairo_native)](/LICENSE)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](/LICENSE-MIT)
[![pr-welcome]](#-contributing)
[![Crates.io Version](https://img.shields.io/crates/v/cairo_native)](https://crates.io/crates/cairo-native)

[tg-badge]: https://img.shields.io/endpoint?url=https%3A%2F%2Ftg.sumanjay.workers.dev%2FLambdaStarkNet%2F&logo=telegram&label=chat&color=neon
[tg-url]: https://t.me/LambdaStarkNet
[pr-welcome]: https://img.shields.io/static/v1?color=orange&label=PRs&style=flat&message=welcome

</div>

## Table of Contents

- [Disclaimer](#disclaimer)
- [Getting Started](#getting-started)
  - [Dependencies](#dependencies)
  - [Setup](#setup)
  - [Make Targets](#make-targets)
- [Included Tools](#included-tools)
  - [`cairo-native-compile`](#cairo-native-compile)
  - [`cairo-native-dump`](#cairo-native-dump)
  - [`cairo-native-run`](#cairo-native-run)
  - [`cairo-native-test`](#cairo-native-test)
  - [`cairo-native-stress`](#cairo-native-stress)
  - [`scarb-native-dump`](#scarb-native-dump)
  - [`scarb-native-test`](#scarb-native-test)
- [Benchmarking](#benchmarking)
  - [Requirements](#requirements)
- [License](#license)

For in-depth documentation, see the [developer documentation][developer documentation].

## Disclaimer

🚧 Cairo Native is still being built therefore API breaking changes might happen
often so use it at your own risk. 🚧

For versions under `1.0` `cargo` doesn't comply with
[semver](https://semver.org/), so we advise to pin the version you
use. This can be done by adding `cairo-native = "0.9.0-rc.2"` to your Cargo.toml

## Getting Started

### Dependencies

- Linux or macOS (aarch64 included) only for now
- LLVM 19 with MLIR: On debian you can use [apt.llvm.org](https://apt.llvm.org/),
  on macOS you can use brew
- Rust 1.78.0 or later, since we make use of the u128
  [abi change](https://blog.rust-lang.org/2024/03/30/i128-layout-update.html).
- Python 3
- Git

### Setup

> This step applies to all operating systems.

Run the following make target to install the dependencies (**both Linux and macOS**):

```bash
make deps
```

#### Linux

Since Linux distributions change widely, you need to install LLVM 19 via your
package manager, compile it or check if the current release has a Linux binary.

If you are on Debian/Ubuntu, check out the repository https://apt.llvm.org/.
Then you can install with:

```bash
sudo apt-get install llvm-19 llvm-19-dev llvm-19-runtime clang-19 clang-tools-19 lld-19 libpolly-19-dev libmlir-19-dev mlir-19-tools
```

If you decide to build from source, here are some indications:

<details><summary>Install LLVM from source instructions</summary>

```bash
# Go to https://github.com/llvm/llvm-project/releases
# Download the latest LLVM 19 release:
# The blob to download is called llvm-project-19.x.x.src.tar.xz

# For example
wget https://github.com/llvm/llvm-project/releases/download/llvmorg-19.1.7/llvm-project-19.1.7.src.tar.xz
tar xf llvm-project-19.1.5.src.tar.xz

cd llvm-project-19.1.5.src.tar
mkdir build
cd build

# The following cmake command configures the build to be installed to /opt/llvm-19
cmake -G Ninja ../llvm \
   -DLLVM_ENABLE_PROJECTS="mlir;clang;clang-tools-extra;lld;polly" \
   -DLLVM_BUILD_EXAMPLES=OFF \
   -DLLVM_TARGETS_TO_BUILD="Native" \
   -DCMAKE_INSTALL_PREFIX=/opt/llvm-19 \
   -DCMAKE_BUILD_TYPE=RelWithDebInfo \
   -DLLVM_PARALLEL_LINK_JOBS=4 \
   -DLLVM_ENABLE_BINDINGS=OFF \
   -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DLLVM_ENABLE_LLD=ON \
   -DLLVM_ENABLE_ASSERTIONS=OFF

ninja install
```

</details>

Setup a environment variable called `MLIR_SYS_190_PREFIX`, `LLVM_SYS_191_PREFIX`
and `TABLEGEN_190_PREFIX` pointing to the llvm directory:

```bash
# For Debian/Ubuntu using the repository, the path will be /usr/lib/llvm-19
export MLIR_SYS_190_PREFIX=/usr/lib/llvm-19
export LLVM_SYS_191_PREFIX=/usr/lib/llvm-19
export TABLEGEN_190_PREFIX=/usr/lib/llvm-19
```

Alternatively, if installed from Debian/Ubuntu repository, then you can use
`env.sh` to automatically setup the environment variables.

```bash
source env.sh
```

#### MacOS

The makefile `deps` target (which you should have ran before) installs LLVM 19
with brew for you, afterwards you need to execute the `env.sh` script to setup
the needed environment variables.

```bash
source env.sh
```

#### Configure rust-analyzer for Vscode

If you are using vscode as your code editor, you'll need to add this to you settings.json:

```json
"rust-analyzer.cargo.extraEnv": {
  "MLIR_SYS_190_PREFIX": "<path-to-llvm-19>",
  "LLVM_SYS_191_PREFIX": "<path-to-llvm-19>",
  "TABLEGEN_190_PREFIX": "<path-to-llvm-19>",
}
```

if you are on MacOs, you'll need to add this extra line:

```json
"LIBRARY_PATH": "/opt/homebrew/lib",
```

Without this additional config, rust-analyzer won't be able to work properly

### Make Targets

Running `make` by itself will check whether the required LLVM installation and
corelib is found, and then list available targets.

```bash
% make
LLVM is correctly set at /opt/homebrew/opt/llvm.
./scripts/check-corelib-version.sh 2.12.3
Usage:
    deps:         Installs the necesary dependencies.
    build:        Builds the cairo-native library and binaries in release mode.
    build-native: Builds cairo-native with the target-cpu=native rust flag.
    build-dev:    Builds cairo-native under a development-optimized profile.
    check:        Checks format and lints.
    test:         Runs all tests.
    proptest:     Runs property tests.
    coverage:     Runs all tests and computes test coverage.
    doc:          Builds documentation.
    doc-open:     Builds and opens documentation in browser.
    bench:        Runs the hyperfine benchmark script.
    bench-ci:     Runs the criterion benchmarks for CI.
    install:      Invokes cargo to install the cairo-native tools.
    clean:        Cleans the built artifacts.
    stress-test   Runs a command which runs stress tests.
    stress-plot   Plots the results of the stress test command.
    stress-clean  Clean the cache of AOT compiled code of the stress test command.
```

## Included Tools

Aside from the compilation and execution engine library, Cairo Native includes a
few command-line tools to aid development, and some useful scripts.

These are:

- The contents of the `/scripts/` folder
- `cairo-native-compile`
- `cairo-native-dump`
- `cairo-native-run`
- `cairo-native-test`
- `cairo-native-stress`
- `scarb-native-dump`
- `scarb-native-test`

### `cairo-native-compile`

```bash
Compiles Cairo/Sierra to Native machine code.
Outputs the generated MLIR, and the final shared library.

Usage: cairo-native-compile [OPTIONS] <PATH> [OUTPUT_MLIR] [OUTPUT_LIBRARY]

Arguments:
  <PATH>            The input path to compile. By default, it is intrepreted as a Cairo project
  [OUTPUT_MLIR]     The output path for the generated MLIR [default: out.mlir]
  [OUTPUT_LIBRARY]  The output path for the shared library [default: out.dylib]

Options:
  -s, --single-file              Whether path is a single Cairo file
      --sierra-json              Whether path is a single Sierra JSON file
  -O, --opt-level <OPT_LEVEL>    Optimization level (Valid: 0, 1, 2, 3). Values higher than 3 are considered as 3 [default: 0]
      --stats-path <STATS_PATH>  The compilation statistics path
  -h, --help                     Print help (see more with '--help')
  -V, --version                  Print version
```

### `cairo-native-dump`

```bash
Usage: cairo-native-dump [OPTIONS] <INPUT>

Arguments:
  <INPUT>

Options:
  -o, --output <OUTPUT>  [default: -]
      --starknet         Compile a starknet contract
  -h, --help             Print help
```

### `cairo-native-run`

This tool allows to run programs using cairo-native, similar to the `cairo-run`
tool.

For example, the following calculates factorial of 1000.

```bash
cairo-native-run -- -s programs/recursion.cairo --available-gas 10000000
```

```bash
Exits with 1 if the compilation or run fails, otherwise 0.

Usage: cairo-native-run [OPTIONS] <PATH>

Arguments:
  <PATH>  The Cairo project path to compile and run its tests

Options:
  -s, --single-file                    Whether path is a single file
      --allow-warnings                 Allows the compilation to succeed with warnings
      --available-gas <AVAILABLE_GAS>  In cases where gas is available, the amount of provided gas
      --run-mode <RUN_MODE>            Run with JIT or AOT (compiled) [default: jit] [possible values: aot, jit]
  -O, --opt-level <OPT_LEVEL>          Optimization level, Valid: 0, 1, 2, 3. Values higher than 3 are considered as 3 [default: 0]
  -h, --help                           Print help
  -V, --version                        Print version
```

### `cairo-native-test`

This tool mimics the `cairo-test`
[tool](https://github.com/starkware-libs/cairo/tree/main/crates/cairo-lang-test-runner)
and is identical to it in interface. The only feature it doesn't have is the
profiler.

```bash
Compiles a Cairo project and runs all the functions marked as `#[test]`.
Exits with 1 if the compilation or run fails, otherwise 0.

Usage: cairo-native-test [OPTIONS] <PATH>

Arguments:
  <PATH>  The Cairo project path to compile and run its tests

Options:
  -s, --single-file            Whether path is a single file
      --allow-warnings         Allows the compilation to succeed with warnings
  -f, --filter <FILTER>        The filter for the tests, running only tests containing the filter string [default: ]
      --include-ignored        Should we run ignored tests as well
      --ignored                Should we run only the ignored tests
      --starknet               Should we add the starknet plugin to run the tests
  -O, --opt-level <OPT_LEVEL>  Optimization level, Valid: 0, 1, 2, 3. Values higher than 3 are considered as 3 [default: 0]
      --compare-with-cairo-vm  Compares test result with Cairo VM
  -h, --help                   Print help
  -V, --version                Print version
```

For single files, you can use the `-s, --single-file` option. For a project, it
needs to have a `cairo_project.toml` specifying the `crate_roots`.

For example, the following tests the corelib.

```bash
cairo-native-test ./corelib/
```

### `cairo-native-stress`

This tool runs a stress test on Cairo Native.

```bash
A stress tester for Cairo Native

Usage: cairo-native-stress [OPTIONS] <ROUNDS>

Arguments:
  <ROUNDS>  Amount of rounds to execute

Options:
  -o, --output <OUTPUT>  Output file for JSON formatted logs
  -h, --help             Print help (see more with '--help')
```

To quickly run a stress test and save logs as json, run:

```bash
make stress-test
```

This takes a lot of time to finish (it will probably crash first), you can kill
the program at any time.

To plot the results, run:

```bash
make stress-plot
```

To clear the cache directory, run:

```bash
make stress-clean
```

### `scarb-native-dump`

This tool mimics the `scarb build`
[command](https://github.com/software-mansion/scarb/tree/main/extensions/scarb-cairo-test).

This tool should be run at the directory where a `Scarb.toml` file is and it
will behave like `scarb build`, leaving the MLIR files under the `target/`
folder besides the generated JSON sierra files.

For example, the following compiles the alexandria project:

```bash
cd tests/alexandria
scarb-native-dump
file target/dev/alexandria.mlir
```

### `scarb-native-test`

This tool mimics the `scarb test`
[command](https://github.com/software-mansion/scarb/tree/main/extensions/scarb-cairo-test).

```bash
Compiles all packages from a Scarb project matching `packages_filter` and
runs all functions marked with `#[test]`. Exits with 1 if the compilation
or run fails, otherwise 0.

Usage: scarb-native-test [OPTIONS]

Options:
  -p, --package <SPEC>         Packages to run this command on, can be a concrete package name (`foobar`) or a prefix glob (`foo*`) [env: SCARB_PACKAGES_FILTER=] [default: *]
  -w, --workspace              Run for all packages in the workspace
  -f, --filter <FILTER>        Run only tests whose name contain FILTER [default: ]
      --include-ignored        Run ignored and not ignored tests
      --ignored                Run only ignored tests
  -t, --test-kind <TEST_KIND>  Choose test kind to run [possible values: unit, integration, all]
  -O, --opt-level <OPT_LEVEL>  Optimization level, Valid: 0, 1, 2, 3. Values higher than 3 are considered as 3 [default: 0]
      --compare-with-cairo-vm  Compares test result with Cairo VM
  -h, --help                   Print help
  -V, --version                Print version
```

## Benchmarking

### Requirements

In addition to the general requirements, you need:

- [hyperfine](https://github.com/sharkdp/hyperfine): `cargo install hyperfine`

You can then run the `bench` makefile target:

```bash
make bench
```

The `bench` target will run the `./scripts/bench-hyperfine.sh` script. This
script runs hyperfine commands to compare the execution time of programs in
the `./programs/benches/` folder. Each program is compiled and executed via the
execution engine with the `cairo-native-run` command and via the cairo-vm with
the `cairo-run` command provided by the `deps` Makefile target.

## License

This project is dual-licensed under Apache 2.0 and MIT. You may choose either license.

See [Apache 2.0 License](/LICENSE) or [MIT License](/LICENSE-MIT) for more information.

[developer documentation]: https://starkware-libs.github.io/cairo_native/cairo_native/docs/index.html
