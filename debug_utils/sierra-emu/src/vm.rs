use crate::{
    debug::libfunc_to_name,
    gas::{BuiltinCosts, GasMetadata},
    starknet::StarknetSyscallHandler,
    ContractExecutionResult, ProgramTrace, StateDump, Value,
};
use cairo_lang_sierra::{
    edit_state::EditState,
    extensions::{
        circuit::CircuitTypeConcrete,
        core::{CoreConcreteLibfunc, CoreLibfunc, CoreType, CoreTypeConcrete},
        gas::CostTokenType,
        starknet::StarknetTypeConcrete,
        ConcreteLibfunc, ConcreteType,
    },
    ids::{ConcreteLibfuncId, FunctionId, VarId},
    program::{GenFunction, GenStatement, Invocation, Program, StatementIdx},
    program_registry::ProgramRegistry,
};
use cairo_lang_sierra_to_casm::metadata::MetadataComputationConfig;
use cairo_lang_sierra_type_size::ProgramRegistryInfo;
use cairo_lang_starknet_classes::{
    casm_contract_class::ENTRY_POINT_COST, compiler_version::VersionId,
    contract_class::ContractEntryPoints,
};
use cairo_lang_utils::{ordered_hash_map::OrderedHashMap, small_ordered_map::SmallOrderedMap};
use smallvec::{smallvec, SmallVec};
use starknet_types_core::felt::Felt;
use std::{cmp::Ordering, fmt::Debug, sync::Arc};
use tracing::{debug, trace};

mod ap_tracking;
mod array;
mod bool;
mod bounded_int;
mod r#box;
mod branch_align;
mod bytes31;
mod cast;
mod circuit;
mod r#const;
mod coupon;
mod drop;
mod dup;
mod ec;
mod r#enum;
mod felt252;
mod felt252_dict;
mod felt252_dict_entry;
mod function_call;
mod gas;
mod gas_reserve;
mod int;
mod int_range;
mod jump;
mod mem;
mod nullable;
mod pedersen;
mod poseidon;
mod snapshot_take;
mod starknet;
mod r#struct;
mod uint256;
mod uint512;

#[derive(Clone)]
pub struct VirtualMachine {
    pub program: Arc<Program>,
    pub registry: Arc<ProgramRegistry<CoreType, CoreLibfunc>>,
    frames: Vec<SierraFrame>,
    pub gas: GasMetadata,
    entry_points: Option<ContractEntryPoints>,
    builtin_costs: BuiltinCosts,
}

impl Debug for VirtualMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualMachine")
            .field("frames", &self.frames)
            .field("gas", &self.gas)
            .finish_non_exhaustive()
    }
}

impl VirtualMachine {
    pub fn new(program: Arc<Program>) -> Self {
        let registry = ProgramRegistry::new(&program).unwrap();
        let program_info = ProgramRegistryInfo::new(&program).unwrap();

        Self {
            gas: GasMetadata::new(
                &program,
                &program_info,
                Some(MetadataComputationConfig::default()),
            )
            .unwrap(),
            program,
            registry: Arc::new(registry),
            frames: Vec::new(),
            entry_points: None,
            builtin_costs: Default::default(),
        }
    }
}

impl VirtualMachine {
    pub fn new_starknet(
        program: Arc<Program>,
        entry_points: &ContractEntryPoints,
        sierra_version: VersionId,
    ) -> Self {
        let no_eq_solver = match sierra_version.major.cmp(&1) {
            Ordering::Less => false,
            Ordering::Equal => sierra_version.minor >= 4,
            Ordering::Greater => true,
        };

        let registry = ProgramRegistry::new(&program).unwrap();
        let program_info = ProgramRegistryInfo::new(&program).unwrap();

        Self {
            gas: GasMetadata::new(
                &program,
                &program_info,
                Some(MetadataComputationConfig {
                    function_set_costs: entry_points
                        .constructor
                        .iter()
                        .chain(entry_points.external.iter())
                        .chain(entry_points.l1_handler.iter())
                        .map(|x| {
                            (
                                FunctionId::new(x.function_idx as u64),
                                SmallOrderedMap::from_iter([(
                                    CostTokenType::Const,
                                    ENTRY_POINT_COST,
                                )]),
                            )
                        })
                        .collect(),
                    linear_gas_solver: no_eq_solver,
                    linear_ap_change_solver: no_eq_solver,
                    skip_non_linear_solver_comparisons: false,
                    compute_runtime_costs: false,
                }),
            )
            .unwrap(),
            program,
            registry: Arc::new(registry),
            frames: Vec::new(),
            entry_points: Some(entry_points.clone()),
            builtin_costs: Default::default(),
        }
    }

    pub fn registry(&self) -> &ProgramRegistry<CoreType, CoreLibfunc> {
        &self.registry
    }

    /// Utility to call a contract.
    pub fn call_contract<I>(
        &mut self,
        selector: Felt,
        initial_gas: u64,
        calldata: I,
        builtin_costs: Option<BuiltinCosts>,
    ) where
        I: IntoIterator<Item = Felt>,
        I::IntoIter: ExactSizeIterator,
    {
        self.builtin_costs = builtin_costs.unwrap_or_default();
        let args: Vec<_> = calldata.into_iter().map(Value::Felt).collect();
        let entry_points = self.entry_points.as_ref().expect("contract should have");
        let selector_uint = selector.to_biguint();
        let function_idx = entry_points
            .constructor
            .iter()
            .chain(entry_points.external.iter())
            .chain(entry_points.l1_handler.iter())
            .find(|x| x.selector == selector_uint)
            .map(|x| x.function_idx)
            .expect("function id not found");
        let function = &self.program.funcs[function_idx];

        self.push_frame(
            function.id.clone(),
            function
                .signature
                .param_types
                .iter()
                .map(|type_id| {
                    let type_info = self.registry().get_type(type_id).unwrap();
                    match type_info {
                        CoreTypeConcrete::GasBuiltin(_) => Value::U64(initial_gas),
                        // Add the calldata structure
                        CoreTypeConcrete::Struct(inner) => {
                            let member = self.registry().get_type(&inner.members[0]).unwrap();
                            match member {
                                CoreTypeConcrete::Snapshot(inner) => {
                                    let inner = self.registry().get_type(&inner.ty).unwrap();
                                    match inner {
                                        CoreTypeConcrete::Array(inner) => {
                                            let felt_ty = &inner.ty;
                                            Value::Struct(vec![Value::Array {
                                                ty: felt_ty.clone(),
                                                data: args.clone(),
                                            }])
                                        }
                                        _ => unreachable!(),
                                    }
                                }
                                _ => unreachable!(),
                            }
                        }
                        CoreTypeConcrete::BuiltinCosts(_) => {
                            Value::BuiltinCosts(builtin_costs.unwrap_or_default())
                        }
                        CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_)) => Value::Unit,
                        CoreTypeConcrete::RangeCheck(_)
                        | CoreTypeConcrete::RangeCheck96(_)
                        | CoreTypeConcrete::Circuit(
                            CircuitTypeConcrete::MulMod(_) | CircuitTypeConcrete::AddMod(_),
                        )
                        | CoreTypeConcrete::Pedersen(_)
                        | CoreTypeConcrete::Poseidon(_)
                        | CoreTypeConcrete::Bitwise(_)
                        | CoreTypeConcrete::EcOp(_)
                        | CoreTypeConcrete::SegmentArena(_) => Value::Unit,
                        x => {
                            todo!("{:?}", x.info())
                        }
                    }
                })
                .collect::<Vec<_>>(),
        );
    }

    /// Utility to call a contract.
    pub fn call_program<I>(
        &mut self,
        function: &GenFunction<StatementIdx>,
        initial_gas: u64,
        args: I,
    ) where
        I: IntoIterator<Item = Value>,
        I::IntoIter: ExactSizeIterator,
    {
        let required_gas = self.gas.initial_required_gas(&function.id).unwrap();
        let initial_gas = initial_gas.checked_sub(required_gas).unwrap();

        let mut iter = args.into_iter();
        self.push_frame(
            function.id.clone(),
            function
                .signature
                .param_types
                .iter()
                .map(|type_id| {
                    let type_info = self.registry().get_type(type_id).unwrap();
                    match type_info {
                        CoreTypeConcrete::GasBuiltin(_) => Value::U64(initial_gas),
                        CoreTypeConcrete::RangeCheck(_)
                        | CoreTypeConcrete::RangeCheck96(_)
                        | CoreTypeConcrete::Bitwise(_)
                        | CoreTypeConcrete::Pedersen(_)
                        | CoreTypeConcrete::Poseidon(_)
                        | CoreTypeConcrete::SegmentArena(_)
                        | CoreTypeConcrete::Circuit(
                            CircuitTypeConcrete::AddMod(_) | CircuitTypeConcrete::MulMod(_),
                        ) => Value::Unit,
                        CoreTypeConcrete::Starknet(inner) => match inner {
                            StarknetTypeConcrete::System(_) => Value::Unit,
                            _ => todo!(),
                        },
                        _ => iter.next().unwrap(),
                    }
                })
                .collect::<Vec<_>>(),
        );
    }

    /// Effectively a function call (for entry points).
    pub fn push_frame<I>(&mut self, function_id: FunctionId, args: I)
    where
        I: IntoIterator<Item = Value>,
        I::IntoIter: ExactSizeIterator,
    {
        let function = self.registry.get_function(&function_id).unwrap();

        let args = args.into_iter();
        assert_eq!(args.len(), function.params.len());
        self.frames.push(SierraFrame {
            _function_id: function_id,
            state: function
                .params
                .iter()
                .zip(args)
                .map(|(param, value)| {
                    assert!(value.is(&self.registry, &param.ty));
                    (param.id.clone(), value)
                })
                .collect(),

            pc: function.entry_point,
        })
    }

    /// Run a single statement and return the state before its execution.
    pub fn step(
        &mut self,
        syscall_handler: &mut impl StarknetSyscallHandler,
    ) -> Option<(StatementIdx, OrderedHashMap<VarId, Value>)> {
        let frame = self.frames.last_mut()?;

        let pc_snapshot = frame.pc;
        let state_snapshot = frame.state.clone();

        debug!(
            "Evaluating statement {} ({})",
            frame.pc.0, &self.program.statements[frame.pc.0],
        );
        trace!("values: \n{:#?}\n", state_snapshot);
        match &self.program.statements[frame.pc.0] {
            GenStatement::Invocation(invocation) => {
                let libfunc = self.registry.get_libfunc(&invocation.libfunc_id).unwrap();
                debug!(
                    "Executing invocation of libfunc: {}",
                    libfunc_to_name(libfunc)
                );
                let (state, values) = {
                    let mut state = std::mem::take(&mut frame.state);
                    let values = state.take_vars(invocation.args.iter()).unwrap();
                    (state, values)
                };

                match eval(
                    &self.registry,
                    &invocation.libfunc_id,
                    values,
                    syscall_handler,
                    &self.gas,
                    &frame.pc,
                    self.builtin_costs,
                ) {
                    EvalAction::NormalBranch(branch_idx, results) => {
                        assert_eq!(
                            results.len(),
                            invocation.branches[branch_idx].results.len(),
                            "invocation of {invocation} returned the wrong number of values"
                        );

                        assert!(
                            results
                                .iter()
                                .zip(
                                    &self
                                        .registry
                                        .get_libfunc(&invocation.libfunc_id)
                                        .unwrap()
                                        .branch_signatures()[branch_idx]
                                        .vars
                                )
                                .all(|(value, ret)| value.is(&self.registry, &ret.ty)),
                            "invocation of {} returned an invalid argument",
                            libfunc_to_name(
                                self.registry.get_libfunc(&invocation.libfunc_id).unwrap()
                            )
                        );

                        frame.pc = frame.pc.next(invocation.branches[branch_idx].target);
                        frame.state = state;
                        frame
                            .state
                            .put_vars(invocation.branches[branch_idx].results.iter().zip(results))
                            .unwrap();
                    }
                    EvalAction::FunctionCall(function_id, args) => {
                        let function = self.registry.get_function(&function_id).unwrap();
                        frame.state = state;
                        self.frames.push(SierraFrame {
                            _function_id: function_id,
                            state: function
                                .params
                                .iter()
                                .map(|param| param.id.clone())
                                .zip(args.iter().cloned())
                                .collect(),

                            pc: function.entry_point,
                        });
                    }
                }
            }
            GenStatement::Return(ids) => {
                let mut curr_frame = self.frames.pop().unwrap();
                if let Some(prev_frame) = self.frames.last_mut() {
                    let (state, values) = {
                        let mut state = std::mem::take(&mut curr_frame.state);
                        let values = state.take_vars(ids.iter()).unwrap();
                        (state, values)
                    };
                    assert!(state.is_empty());

                    let target_branch = match &self.program.statements[prev_frame.pc.0] {
                        GenStatement::Invocation(Invocation { branches, .. }) => {
                            assert_eq!(branches.len(), 1);
                            &branches[0]
                        }
                        _ => unreachable!(),
                    };

                    assert_eq!(target_branch.results.len(), values.len());
                    prev_frame.pc = prev_frame.pc.next(target_branch.target);
                    prev_frame
                        .state
                        .put_vars(target_branch.results.iter().zip(values))
                        .unwrap();
                }
            }
        }

        Some((pc_snapshot, state_snapshot))
    }

    /// Run all the statement and return the trace.
    pub fn run_with_trace(
        &mut self,
        syscall_handler: &mut impl StarknetSyscallHandler,
    ) -> ProgramTrace {
        let mut trace = ProgramTrace::new();

        while let Some((statement_idx, state)) = self.step(syscall_handler) {
            trace.push(StateDump::new(statement_idx, state));
        }

        trace
    }

    /// Run all the statement and return the trace.
    pub fn run(
        &mut self,
        syscall_handler: &mut impl StarknetSyscallHandler,
    ) -> Option<ContractExecutionResult> {
        let mut last = None;

        while let Some((statement_idx, state)) = self.step(syscall_handler) {
            last = Some(StateDump::new(statement_idx, state));
        }

        ContractExecutionResult::from_state(&last?)
    }

    /// One-shot convenience over [`VirtualMachine::new_starknet`],
    /// [`VirtualMachine::call_contract`] and [`VirtualMachine::run`]: build a
    /// Starknet VM for `program`, call the entry point identified by `selector`
    /// and run it to completion.
    ///
    /// Returns `None` when execution never produced a final state.
    #[allow(clippy::too_many_arguments)]
    pub fn run_contract<I>(
        program: Arc<Program>,
        entry_points: &ContractEntryPoints,
        sierra_version: VersionId,
        selector: Felt,
        initial_gas: u64,
        calldata: I,
        builtin_costs: Option<BuiltinCosts>,
        syscall_handler: &mut impl StarknetSyscallHandler,
    ) -> Option<ContractExecutionResult>
    where
        I: IntoIterator<Item = Felt>,
        I::IntoIter: ExactSizeIterator,
    {
        let mut vm = Self::new_starknet(program, entry_points, sierra_version);
        vm.call_contract(selector, initial_gas, calldata, builtin_costs);
        vm.run(syscall_handler)
    }
}

#[derive(Clone, Debug)]
struct SierraFrame {
    _function_id: FunctionId,

    state: OrderedHashMap<VarId, Value>,
    pc: StatementIdx,
}

enum EvalAction {
    NormalBranch(usize, SmallVec<[Value; 2]>),
    FunctionCall(FunctionId, SmallVec<[Value; 2]>),
}

fn eval<'a>(
    registry: &'a ProgramRegistry<CoreType, CoreLibfunc>,
    id: &'a ConcreteLibfuncId,
    args: Vec<Value>,
    syscall_handler: &mut impl StarknetSyscallHandler,
    gas: &GasMetadata,
    statement_idx: &StatementIdx,
    builtin_costs: BuiltinCosts,
) -> EvalAction {
    match registry.get_libfunc(id).unwrap() {
        CoreConcreteLibfunc::ApTracking(selector) => {
            self::ap_tracking::eval(registry, selector, args)
        }
        CoreConcreteLibfunc::Array(selector) => self::array::eval(registry, selector, args),
        CoreConcreteLibfunc::Bool(selector) => self::bool::eval(registry, selector, args),
        CoreConcreteLibfunc::BoundedInt(selector) => {
            self::bounded_int::eval(registry, selector, args)
        }
        CoreConcreteLibfunc::Box(selector) => self::r#box::eval(registry, selector, args),
        CoreConcreteLibfunc::BranchAlign(info) => self::branch_align::eval(registry, info, args),
        CoreConcreteLibfunc::Bytes31(selector) => self::bytes31::eval(registry, selector, args),
        CoreConcreteLibfunc::Cast(selector) => self::cast::eval(registry, selector, args),
        CoreConcreteLibfunc::Circuit(selector) => self::circuit::eval(registry, selector, args),
        CoreConcreteLibfunc::Const(selector) => self::r#const::eval(registry, selector, args),
        CoreConcreteLibfunc::Coupon(selector) => self::coupon::eval(registry, selector, args),
        CoreConcreteLibfunc::Debug(_) => todo!(),
        CoreConcreteLibfunc::Drop(info) => self::drop::eval(registry, info, args),
        CoreConcreteLibfunc::Dup(info) => self::dup::eval(registry, info, args),
        CoreConcreteLibfunc::Ec(selector) => self::ec::eval(registry, selector, args),
        CoreConcreteLibfunc::Enum(selector) => self::r#enum::eval(registry, selector, args),
        CoreConcreteLibfunc::Felt252(selector) => self::felt252::eval(registry, selector, args),
        CoreConcreteLibfunc::Felt252Dict(selector) => {
            self::felt252_dict::eval(registry, selector, args)
        }
        CoreConcreteLibfunc::Felt252DictEntry(selector) => {
            self::felt252_dict_entry::eval(registry, selector, args)
        }
        CoreConcreteLibfunc::FunctionCall(info) => {
            self::function_call::eval_function_call(registry, info, args)
        }
        CoreConcreteLibfunc::CouponCall(info) => {
            self::function_call::eval_coupon_call(registry, info, args)
        }
        CoreConcreteLibfunc::Gas(selector) => {
            self::gas::eval(registry, selector, args, gas, *statement_idx, builtin_costs)
        }
        CoreConcreteLibfunc::Mem(selector) => self::mem::eval(registry, selector, args),
        CoreConcreteLibfunc::Nullable(selector) => self::nullable::eval(registry, selector, args),
        CoreConcreteLibfunc::Pedersen(selector) => self::pedersen::eval(registry, selector, args),
        CoreConcreteLibfunc::Poseidon(selector) => self::poseidon::eval(registry, selector, args),
        CoreConcreteLibfunc::Sint8(selector) => self::int::eval_signed(registry, selector, args),
        CoreConcreteLibfunc::Sint16(selector) => self::int::eval_signed(registry, selector, args),
        CoreConcreteLibfunc::Sint32(selector) => self::int::eval_signed(registry, selector, args),
        CoreConcreteLibfunc::Sint64(selector) => self::int::eval_signed(registry, selector, args),
        CoreConcreteLibfunc::Sint128(selector) => self::int::eval_i128(registry, selector, args),
        CoreConcreteLibfunc::Uint8(selector) => self::int::eval_unsigned(registry, selector, args),
        CoreConcreteLibfunc::Uint16(selector) => self::int::eval_unsigned(registry, selector, args),
        CoreConcreteLibfunc::Uint32(selector) => self::int::eval_unsigned(registry, selector, args),
        CoreConcreteLibfunc::Uint64(selector) => self::int::eval_unsigned(registry, selector, args),
        CoreConcreteLibfunc::Uint128(selector) => self::int::eval_uint128(registry, selector, args),
        CoreConcreteLibfunc::Uint256(selector) => self::uint256::eval(registry, selector, args),
        CoreConcreteLibfunc::Uint512(selector) => self::uint512::eval(registry, selector, args),
        CoreConcreteLibfunc::Struct(selector) => self::r#struct::eval(registry, selector, args),
        CoreConcreteLibfunc::SnapshotTake(info) => self::snapshot_take::eval(registry, info, args),
        CoreConcreteLibfunc::Starknet(selector) => {
            self::starknet::eval(registry, selector, args, syscall_handler)
        }
        CoreConcreteLibfunc::UnconditionalJump(info) => self::jump::eval(registry, info, args),
        CoreConcreteLibfunc::UnwrapNonZero(_info) => {
            let [value] = args.try_into().unwrap();

            EvalAction::NormalBranch(0, smallvec![value])
        }
        CoreConcreteLibfunc::IntRange(selector) => self::int_range::eval(registry, selector, args),
        CoreConcreteLibfunc::GasReserve(selector) => self::gas_reserve::eval(selector, args),
        CoreConcreteLibfunc::Blake(_) => todo!(),
        CoreConcreteLibfunc::QM31(_) => todo!(),
        CoreConcreteLibfunc::Felt252SquashedDict(_) => todo!(),
        CoreConcreteLibfunc::Trace(_) => todo!(),
        CoreConcreteLibfunc::UnsafePanic(_) => todo!(),
        CoreConcreteLibfunc::DummyFunctionCall(_) => todo!(),
    }
}
