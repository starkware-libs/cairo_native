//! A sierra-emu-backed contract executor exposing the same `run` shape as
//! [`AotContractExecutor::run`](crate::executor::AotContractExecutor::run), so a caller
//! can swap the two behind a feature flag without changing call-site code.
//!
//! Both executors share the same `H: StarknetSyscallHandler` -- sierra-emu and
//! cairo-native re-export the trait from `cairo-native-syscalls`, so no adapter is
//! needed.

use cairo_lang_sierra::program::Program;
use cairo_lang_starknet_classes::compiler_version::VersionId;
use cairo_lang_starknet_classes::contract_class::ContractEntryPoints;
use starknet_types_core::felt::Felt;
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::execution_result::ContractExecutionResult;
use crate::starknet::StarknetSyscallHandler;
use crate::utils::BuiltinCosts;

/// Runs contract entry points through the sierra-emu interpreter.
///
/// Holds the program + entry points + sierra version triple that
/// `sierra_emu::VirtualMachine::run_contract` requires; the `Arc<Program>` is shared
/// across invocations rather than cloned per call.
#[derive(Debug, Clone)]
pub struct EmuContractExecutor {
    pub program: Arc<Program>,
    pub entry_points: ContractEntryPoints,
    pub sierra_version: VersionId,
}

impl EmuContractExecutor {
    /// Run the contract entry point identified by `selector`.
    ///
    /// Mirrors [`AotContractExecutor::run`](crate::executor::AotContractExecutor::run) so
    /// the two executor types are interchangeable at the call site.
    pub fn run(
        &self,
        selector: Felt,
        args: &[Felt],
        gas: u64,
        builtin_costs: Option<BuiltinCosts>,
        mut syscall_handler: impl StarknetSyscallHandler,
    ) -> Result<ContractExecutionResult> {
        // `run_contract` returns `None` when the VM never produced a final state --
        // propagate as an error rather than aborting the host.
        let result = sierra_emu::VirtualMachine::run_contract(
            Arc::clone(&self.program),
            &self.entry_points,
            self.sierra_version,
            selector,
            gas,
            args.to_vec(),
            builtin_costs.map(convert_builtin_costs),
            &mut syscall_handler,
        )
        .ok_or_else(|| {
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
}

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
