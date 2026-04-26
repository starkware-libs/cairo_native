//! Dispatch enum that lets a single call site choose between AOT-compiled execution and
//! sierra-emu interpretation, without changing call-site code.
//!
//! The `Emu` variant is gated on the `sierra-emu` feature and uses
//! [`crate::sierra_emu_bridge::SierraEmuSyscallBridge`] to thread a cairo-native syscall
//! handler through the sierra-emu VM.

#[cfg(feature = "sierra-emu")]
use cairo_lang_sierra::program::Program;
#[cfg(feature = "sierra-emu")]
use cairo_lang_starknet_classes::compiler_version::VersionId;
#[cfg(feature = "sierra-emu")]
use cairo_lang_starknet_classes::contract_class::ContractEntryPoints;
use starknet_types_core::felt::Felt;
#[cfg(feature = "sierra-emu")]
use std::sync::Arc;

use crate::error::Result;
use crate::execution_result::ContractExecutionResult;
use crate::executor::AotContractExecutor;
#[cfg(feature = "sierra-emu")]
use crate::sierra_emu_bridge::SierraEmuSyscallBridge;
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
}

/// Inputs required to construct a `sierra_emu::VirtualMachine` for the `Emu` variant.
#[cfg(feature = "sierra-emu")]
#[derive(Debug, Clone)]
pub struct EmuContractInfo {
    pub program: Arc<Program>,
    pub entry_points: ContractEntryPoints,
    pub sierra_version: VersionId,
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

impl ContractExecutor {
    /// Run the contract entry point identified by `selector`.
    ///
    /// Dispatches to [`AotContractExecutor::run`] for the `Aot` variant and to a
    /// [`sierra_emu::VirtualMachine`] for the `Emu` variant. The same `syscall_handler`
    /// is reused across both paths; for the `Emu` variant it's wrapped in a
    /// [`SierraEmuSyscallBridge`] so it satisfies the sierra-emu trait.
    pub fn run<H: StarknetSyscallHandler>(
        &self,
        selector: Felt,
        args: &[Felt],
        gas: u64,
        builtin_costs: Option<BuiltinCosts>,
        #[cfg_attr(not(feature = "sierra-emu"), allow(unused_mut))] mut syscall_handler: H,
    ) -> Result<ContractExecutionResult> {
        match self {
            ContractExecutor::Aot(aot) => aot.run(selector, args, gas, builtin_costs, syscall_handler),
            #[cfg(feature = "sierra-emu")]
            ContractExecutor::Emu(EmuContractInfo { program, entry_points, sierra_version }) => {
                let mut virtual_machine = sierra_emu::VirtualMachine::new_starknet(
                    Arc::clone(program),
                    entry_points,
                    *sierra_version,
                );

                let emu_builtin_costs = builtin_costs.map(convert_builtin_costs);

                virtual_machine.call_contract(selector, gas, args.to_vec(), emu_builtin_costs);

                let mut bridge = SierraEmuSyscallBridge(&mut syscall_handler);
                let result = virtual_machine
                    .run(&mut bridge)
                    .expect("sierra-emu VM run failed");

                Ok(ContractExecutionResult {
                    remaining_gas: result.remaining_gas,
                    failure_flag: result.failure_flag,
                    return_values: result.return_values,
                    error_msg: result.error_msg,
                    builtin_stats: Default::default(),
                })
            }
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
