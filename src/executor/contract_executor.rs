//! Dispatch enum that lets a single call site choose between AOT-compiled execution and
//! sierra-emu interpretation, without changing call-site code.
//!
//! The `Emu` variant is gated on the `sierra-emu` feature. Both variants share the same
//! `H: StarknetSyscallHandler` — sierra-emu and cairo-native re-export the trait from
//! `cairo-native-syscalls`, so no adapter is needed.

#[cfg(any(feature = "sierra-emu", feature = "with-libfunc-profiling"))]
use cairo_lang_sierra::program::Program;
#[cfg(feature = "sierra-emu")]
use cairo_lang_starknet_classes::compiler_version::VersionId;
#[cfg(feature = "sierra-emu")]
use cairo_lang_starknet_classes::contract_class::ContractEntryPoints;
use starknet_types_core::felt::Felt;
#[cfg(any(feature = "sierra-emu", feature = "with-libfunc-profiling"))]
use std::sync::Arc;

#[cfg(feature = "sierra-emu")]
use crate::error::Error;
use crate::error::Result;
use crate::execution_result::ContractExecutionResult;
use crate::executor::AotContractExecutor;
#[cfg(feature = "with-libfunc-profiling")]
use crate::metadata::profiler::Profile;
use crate::starknet::StarknetSyscallHandler;
use crate::utils::BuiltinCosts;

/// Runtime selection between cairo-native's AOT executor and the sierra-emu interpreter.
///
/// `Emu` is constructed from the program + entry points + sierra version triple that
/// `sierra_emu::VirtualMachine::new_starknet` requires; the `Arc<Program>` is shared across
/// invocations rather than cloned per call.
#[derive(Debug)]
pub enum ContractExecutor {
    Aot(AotContractExecutor),
    #[cfg(feature = "sierra-emu")]
    Emu(EmuContractInfo),
    #[cfg(feature = "with-libfunc-profiling")]
    AotWithProgram(AotWithProgram),
}

/// Inputs required to construct a `sierra_emu::VirtualMachine` for the `Emu` variant.
#[cfg(feature = "sierra-emu")]
#[derive(Debug, Clone)]
pub struct EmuContractInfo {
    pub program: Arc<Program>,
    pub entry_points: ContractEntryPoints,
    pub sierra_version: VersionId,
}

/// AOT executor paired with the Sierra program it was built from. Required by
/// [`ContractExecutor::run_with_profile`] so libfunc samples can be resolved against
/// the program's declarations.
#[cfg(feature = "with-libfunc-profiling")]
#[derive(Debug)]
pub struct AotWithProgram {
    pub executor: AotContractExecutor,
    pub program: Arc<Program>,
}

impl From<AotContractExecutor> for ContractExecutor {
    fn from(value: AotContractExecutor) -> Self {
        Self::Aot(value)
    }
}

#[cfg(feature = "sierra-emu")]
impl From<EmuContractInfo> for ContractExecutor {
    fn from(value: EmuContractInfo) -> Self {
        Self::Emu(value)
    }
}

#[cfg(feature = "with-libfunc-profiling")]
impl From<AotWithProgram> for ContractExecutor {
    fn from(value: AotWithProgram) -> Self {
        Self::AotWithProgram(value)
    }
}

impl ContractExecutor {
    /// Run the contract entry point identified by `selector`.
    ///
    /// Dispatches to [`AotContractExecutor::run`] for the `Aot` variant and to a
    /// [`sierra_emu::VirtualMachine`] for the `Emu` variant. The same `syscall_handler`
    /// flows through both paths unchanged — its trait is shared across the two crates.
    pub fn run<H: StarknetSyscallHandler>(
        &self,
        selector: Felt,
        args: &[Felt],
        gas: u64,
        builtin_costs: Option<BuiltinCosts>,
        #[cfg_attr(not(feature = "sierra-emu"), allow(unused_mut))] mut syscall_handler: H,
    ) -> Result<ContractExecutionResult> {
        match self {
            ContractExecutor::Aot(aot) => {
                aot.run(selector, args, gas, builtin_costs, syscall_handler)
            }
            #[cfg(feature = "sierra-emu")]
            ContractExecutor::Emu(EmuContractInfo {
                program,
                entry_points,
                sierra_version,
            }) => {
                let mut virtual_machine = sierra_emu::VirtualMachine::new_starknet(
                    Arc::clone(program),
                    entry_points,
                    *sierra_version,
                );

                let emu_builtin_costs = builtin_costs.map(convert_builtin_costs);

                virtual_machine.call_contract(selector, gas, args.to_vec(), emu_builtin_costs);

                // `VirtualMachine::run` returns `None` when the VM never produced a
                // final state — propagate as an error rather than aborting the host.
                let result = virtual_machine.run(&mut syscall_handler).ok_or_else(|| {
                    Error::UnexpectedValue("sierra-emu VM produced no final state".to_string())
                })?;

                Ok(ContractExecutionResult {
                    remaining_gas: result.remaining_gas,
                    failure_flag: result.failure_flag,
                    return_values: result.return_values,
                    error_msg: result.error_msg,
                    builtin_stats: Default::default(),
                })
            }
            #[cfg(feature = "with-libfunc-profiling")]
            ContractExecutor::AotWithProgram(AotWithProgram { executor, program }) => executor
                .run_with_libfunc_profile(
                    program,
                    selector,
                    args,
                    gas,
                    builtin_costs,
                    syscall_handler,
                    // Profile is collected and dropped on this path. Use
                    // `run_with_profile` to capture it.
                    |_profile| {},
                ),
        }
    }

    /// Like [`Self::run`] but, for the `AotWithProgram` variant, hands the captured
    /// libfunc profile to `on_profile` after the call returns successfully. For other
    /// variants this is identical to `run` and `on_profile` is never invoked.
    #[cfg(feature = "with-libfunc-profiling")]
    pub fn run_with_profile<H, F>(
        &self,
        selector: Felt,
        args: &[Felt],
        gas: u64,
        builtin_costs: Option<BuiltinCosts>,
        syscall_handler: H,
        on_profile: F,
    ) -> Result<ContractExecutionResult>
    where
        H: StarknetSyscallHandler,
        F: FnOnce(Profile),
    {
        match self {
            ContractExecutor::AotWithProgram(AotWithProgram { executor, program }) => executor
                .run_with_libfunc_profile(
                    program,
                    selector,
                    args,
                    gas,
                    builtin_costs,
                    syscall_handler,
                    on_profile,
                ),
            _ => self.run(selector, args, gas, builtin_costs, syscall_handler),
        }
    }
}

#[cfg(feature = "sierra-emu")]
fn convert_builtin_costs(builtin_costs: BuiltinCosts) -> sierra_emu::BuiltinCosts {
    sierra_emu::BuiltinCosts {
        r#const: builtin_costs.r#const,
        pedersen: builtin_costs.pedersen,
        bitwise: builtin_costs.bitwise,
        ecop: builtin_costs.ecop,
        poseidon: builtin_costs.poseidon,
        add_mod: builtin_costs.add_mod,
        mul_mod: builtin_costs.mul_mod,
        blake: builtin_costs.blake,
    }
}
