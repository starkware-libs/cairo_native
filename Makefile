# Environment detection.

UNAME := $(shell uname)
SCARB_VERSION = 2.16.0
CAIRO_2_VERSION = 2.16.0

# Usage is the default target for newcomers running `make`.
.PHONY: usage
usage: check-llvm check-corelib check-cairo2
	@echo "Usage:"
	@echo "    deps:         Installs the necesary dependencies."
	@echo "    build:        Builds the cairo-native library and binaries in release mode."
	@echo "    build-native: Builds cairo-native with the target-cpu=native rust flag."
	@echo "    build-dev:    Builds cairo-native under a development-optimized profile."
	@echo "    check:        Checks format and lints."
	@echo "    test:         Runs all tests."
	@echo "    test-cairo:   Runs all Cairo tests."
	@echo "    proptest:     Runs property tests."
	@echo "    coverage:     Runs all tests and computes test coverage."
	@echo "    doc:          Builds documentation."
	@echo "    doc-open:     Builds and opens documentation in browser."
	@echo "    bench:        Runs the hyperfine benchmark script."
	@echo "    bench-ci:     Runs the criterion benchmarks for CI."
	@echo "    install:      Invokes cargo to install cairo-native tools."
	@echo "    clean:        Cleans the built artifacts."
	@echo "    stress-test   Runs a command which runs stress tests."
	@echo "    stress-plot   Plots the results of the stress test command."
	@echo "    stress-clean  Clean the cache of AOT compiled code of the stress test command."

.PHONY: check-llvm
check-llvm:
ifndef MLIR_SYS_190_PREFIX
	$(error Could not find a suitable LLVM 19 toolchain (mlir), please set MLIR_SYS_190_PREFIX env pointing to the LLVM 19 dir)
endif
ifndef TABLEGEN_190_PREFIX
	$(error Could not find a suitable LLVM 19 toolchain (tablegen), please set TABLEGEN_190_PREFIX env pointing to the LLVM 19 dir)
endif
	@echo "LLVM is correctly set at $(MLIR_SYS_190_PREFIX)."

.PHONY: check-corelib
check-corelib:
	./scripts/check-corelib-version.sh $(CAIRO_2_VERSION)

.PHONY: check-cairo2
check-cairo2:
	./scripts/check-cairo2-version.sh $(CAIRO_2_VERSION)

.PHONY: build
build: check-llvm
	cargo build --release --workspace

.PHONY: build-natives
build-native: check-llvm
	RUSTFLAGS="-C target-cpu=native" cargo build --release

.PHONY: build-dev
build-dev: check-llvm
	cargo build --profile optimized-dev

.PHONY: check
check: check-llvm
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings

.PHONY: test
test: check-llvm
	cargo nextest run --cargo-profile ci --features=with-cheatcode,with-debug-utils,testing

.PHONY: test-cairo
test-cairo: check-llvm
	bash ./scripts/test_cairo.sh

.PHONY: proptest
proptest: check-llvm
	cargo nextest run --cargo-profile ci --features=with-cheatcode,with-debug-utils,testing proptest

.PHONY: test-cli
test-ci: check-llvm
	cargo nextest run --cargo-profile ci --features=with-cheatcode,with-debug-utils,testing

.PHONY: proptest-cli
proptest-ci: check-llvm
	cargo nextest run --cargo-profile ci --features=with-cheatcode,with-debug-utils,testing proptest

.PHONY: coverage
coverage: check-llvm
	cargo llvm-cov nextest --verbose --profile ci --features=with-cheatcode,with-debug-utils,testing --workspace --lcov --output-path lcov.info
	cargo llvm-cov run --verbose --profile ci --features=with-cheatcode,with-debug-utils,testing --lcov --output-path lcov-test.info --package cairo-native-test -- corelib

.PHONY: doc
doc: check-llvm
	cargo doc --all-features --no-deps --workspace

.PHONY: doc-open
doc-open: check-llvm
	cargo doc --all-features --no-deps --workspace --open

.PHONY: bench
bench: check-llvm check-cairo2
	cargo b --release --package cairo-native-run
	cargo b --release --package cairo-native-compile
	./scripts/bench-hyperfine.sh

.PHONY: bench-ci
bench-ci: check-llvm
	cargo criterion --features=with-cheatcode,with-debug-utils,testing

.PHONY: stress-test
stress-test: check-llvm
	RUST_LOG=cairo_native_stress=DEBUG cargo run --package cairo-native-stress 1000000 --output cairo-native-stress-logs.jsonl

.PHONY: stress-plot
stress-plot:
	python3 debug_utils/cairo-native-stress/plotter.py cairo-native-stress-logs.jsonl

.PHONY: stress-clean
stress-clean:
	rm -rf .aot-cache

.PHONY: install
install: check-llvm
	RUSTFLAGS="-C target-cpu=native" cargo install --features=with-cheatcode --locked --path .

.PHONY: clean
clean: stress-clean
	cargo clean

.PHONY: deps
deps:
ifeq ($(UNAME), Linux)
deps: build-cairo-2-compiler install-scarb
endif
ifeq ($(UNAME), Darwin)
deps: deps-macos
endif
	$(MAKE) compile-test-data

.PHONY: deps-macos
deps-macos: build-cairo-2-compiler-macos install-scarb-macos
	-brew install llvm@19 --quiet
	@echo "You can execute the env-macos.sh script to setup the needed env variables."

# CI use only
.PHONY: deps-ci-linux build-cairo-2-compiler install-scarb
deps-ci-linux:
ifeq ($(UNAME), Linux)
	-wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && sudo ./llvm.sh 19
endif

cairo-repo-2-dir = cairo2
cairo-repo-2-dir-macos = cairo2-macos

build-cairo-2-compiler-macos: | $(cairo-repo-2-dir-macos)

$(cairo-repo-2-dir-macos): cairo-${CAIRO_2_VERSION}-macos.tar
	$(MAKE) decompress-cairo SOURCE=$< TARGET=cairo2/

build-cairo-2-compiler: | $(cairo-repo-2-dir)

$(cairo-repo-2-dir): cairo-${CAIRO_2_VERSION}.tar
	$(MAKE) decompress-cairo SOURCE=$< TARGET=cairo2/

.PHONY: decompress-cairo
decompress-cairo:
	rm -rf $(TARGET) \
	&& tar -xzvf $(SOURCE) \
	&& mv cairo/ $(TARGET)

cairo-%-macos.tar:
	curl -L -o "$@" "https://github.com/starkware-libs/cairo/releases/download/v$*/release-aarch64-apple-darwin.tar"

cairo-%.tar:
	curl -L -o "$@" "https://github.com/starkware-libs/cairo/releases/download/v$*/release-x86_64-unknown-linux-musl.tar.gz"

.PHONY: install-scarb
install-scarb:
	curl --proto '=https' --tlsv1.2 -sSf https://docs.swmansion.com/scarb/install.sh | \
	sed 's/bash_completion_block/bash_completions_block/g' | \
	sed 's/fish_completion_block/fish_completions_block/g' | \
	sed 's/zsh_completion_block/zsh_completions_block/g' | \
	sh -s -- --no-modify-path --version $(SCARB_VERSION)

.PHONY: install-scarb-macos
install-scarb-macos:
	curl --proto '=https' --tlsv1.2 -sSf https://docs.swmansion.com/scarb/install.sh | \
	sed 's/bash_completion_block/bash_completions_block/g' | \
	sed 's/fish_completion_block/fish_completions_block/g' | \
	sed 's/zsh_completion_block/zsh_completions_block/g' | \
	sh -s -- --version $(SCARB_VERSION)

.PHONY: pull-external-projects
pull-external-projects:
	python3 ./scripts/pull_external_projects.py v${CAIRO_2_VERSION}

test_data/e2e_sierra: test_data/e2e_libfuncs_raw
	python3 ./scripts/extract_e2e_sierra.py


.PHONY: compile-test-data
compile-test-data: test_data/e2e_sierra
	python3 ./scripts/compile_test_data.py
