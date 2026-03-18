use crate::{decode_instruction, Memory, StepId, Trace, ValueId};
use cairo_lang_casm::{
    hints::{CoreHint, CoreHintBase, DeprecatedHint, ExternalHint, Hint, StarknetHint},
    instructions::InstructionBody,
    operand::{CellRef, DerefOrImmediate, Register, ResOperand},
};
use crate::trace::RelocatedTraceEntry;
use std::{
    collections::{HashMap, HashSet},
    ops::Index,
};

#[derive(Debug)]
pub struct GraphMappings {
    step2value: HashMap<StepId, HashSet<ValueId>>,
    value2step: HashMap<ValueId, HashSet<StepId>>,
}

impl GraphMappings {
    pub fn new(memory: &Memory, trace: &Trace, hints: &HashMap<usize, Vec<Hint>>) -> Self {
        let mut step2value = HashMap::<StepId, HashSet<ValueId>>::new();
        let mut value2step = HashMap::<ValueId, HashSet<StepId>>::new();

        for (step, trace) in trace.iter().enumerate() {
            let mut add_mapping = |value| {
                step2value
                    .entry(StepId(step))
                    .or_default()
                    .insert(ValueId(value));
                value2step
                    .entry(ValueId(value))
                    .or_default()
                    .insert(StepId(step));
            };

            Self::iter_memory_references(memory, trace, &mut add_mapping);
            if let Some(hints) = hints.get(&step) {
                for hint in hints {
                    Self::iter_hint_references(memory, trace, hint, &mut add_mapping);
                }
            }
        }

        Self {
            step2value,
            value2step,
        }
    }

    pub fn step2value(&self) -> &HashMap<StepId, HashSet<ValueId>> {
        &self.step2value
    }

    pub fn value2step(&self) -> &HashMap<ValueId, HashSet<StepId>> {
        &self.value2step
    }

    fn iter_memory_references(
        memory: &Memory,
        trace: &RelocatedTraceEntry,
        mut callback: impl FnMut(usize),
    ) {
        let instr = decode_instruction(memory, trace.pc);

        let mut process_cell_ref = |x: CellRef| {
            let offset = match x.register {
                Register::AP => trace.ap.wrapping_add_signed(x.offset as isize),
                Register::FP => trace.fp.wrapping_add_signed(x.offset as isize),
            };
            callback(offset);
            offset
        };

        match instr.body {
            InstructionBody::AddAp(add_ap_instruction) => match add_ap_instruction.operand {
                ResOperand::Deref(_) => todo!(),
                ResOperand::DoubleDeref(_, _) => todo!(),
                ResOperand::Immediate(_) => {}
                ResOperand::BinOp(_) => todo!(),
            },
            InstructionBody::AssertEq(assert_eq_instruction) => {
                process_cell_ref(assert_eq_instruction.a);
                match assert_eq_instruction.b {
                    ResOperand::Deref(cell_ref) => {
                        process_cell_ref(cell_ref);
                    }
                    ResOperand::DoubleDeref(cell_ref, _) => {
                        let offset = process_cell_ref(cell_ref);
                        callback(memory[offset].unwrap().try_into().unwrap());
                    }
                    ResOperand::Immediate(_) => {}
                    ResOperand::BinOp(bin_op_operand) => {
                        process_cell_ref(bin_op_operand.a);
                        match bin_op_operand.b {
                            DerefOrImmediate::Deref(cell_ref) => {
                                process_cell_ref(cell_ref);
                            }
                            DerefOrImmediate::Immediate(_) => {}
                        }
                    }
                }
            }
            InstructionBody::Call(call_instruction) => match call_instruction.target {
                DerefOrImmediate::Deref(_) => todo!(),
                DerefOrImmediate::Immediate(_) => {}
            },
            InstructionBody::Jnz(jnz_instruction) => {
                process_cell_ref(jnz_instruction.condition);
                match jnz_instruction.jump_offset {
                    DerefOrImmediate::Deref(_) => todo!(),
                    DerefOrImmediate::Immediate(_) => {}
                }
            }
            InstructionBody::Jump(jump_instruction) => match jump_instruction.target {
                DerefOrImmediate::Deref(cell_ref) => {
                    process_cell_ref(cell_ref);
                }
                DerefOrImmediate::Immediate(_) => {}
            },
            InstructionBody::Ret(_) => {}
            InstructionBody::QM31AssertEq(_) => todo!(),
            InstructionBody::Blake2sCompress(_) => todo!(),
        }
    }

    fn iter_hint_references(
        _memory: &Memory,
        trace: &RelocatedTraceEntry,
        hint: &Hint,
        mut callback: impl FnMut(usize),
    ) {
        let mut process_cell_ref = |x: CellRef| {
            let offset = match x.register {
                Register::AP => trace.ap.wrapping_add_signed(x.offset as isize),
                Register::FP => trace.fp.wrapping_add_signed(x.offset as isize),
            };
            callback(offset);
            offset
        };

        match hint {
            Hint::Core(core_hint_base) => match core_hint_base {
                CoreHintBase::Core(core_hint) => match core_hint {
                    CoreHint::AllocSegment { dst } => {
                        process_cell_ref(*dst);
                    }
                    CoreHint::TestLessThan { lhs, rhs, dst } => {
                        match lhs {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(bin_op_operand) => {
                                process_cell_ref(bin_op_operand.a);
                                match bin_op_operand.b {
                                    DerefOrImmediate::Deref(cell_ref) => {
                                        process_cell_ref(cell_ref);
                                    }
                                    DerefOrImmediate::Immediate(_) => {}
                                }
                            }
                        }
                        match rhs {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        process_cell_ref(*dst);
                    }
                    CoreHint::TestLessThanOrEqual { lhs, rhs, dst } => {
                        match lhs {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        match rhs {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        process_cell_ref(*dst);
                    }
                    CoreHint::TestLessThanOrEqualAddress { .. } => todo!(),
                    CoreHint::WideMul128 {
                        lhs,
                        rhs,
                        high,
                        low,
                    } => {
                        match lhs {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        match rhs {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        process_cell_ref(*high);
                        process_cell_ref(*low);
                    }
                    CoreHint::DivMod {
                        lhs,
                        rhs,
                        quotient,
                        remainder,
                    } => {
                        match lhs {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        match rhs {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        process_cell_ref(*quotient);
                        process_cell_ref(*remainder);
                    }
                    CoreHint::Uint256DivMod {
                        dividend0,
                        dividend1,
                        divisor0,
                        divisor1,
                        quotient0,
                        quotient1,
                        remainder0,
                        remainder1,
                    } => {
                        match dividend0 {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        match dividend1 {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        match divisor0 {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        match divisor1 {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        process_cell_ref(*quotient0);
                        process_cell_ref(*quotient1);
                        process_cell_ref(*remainder0);
                        process_cell_ref(*remainder1);
                    }
                    CoreHint::Uint512DivModByUint256 { .. } => todo!(),
                    CoreHint::SquareRoot { .. } => todo!(),
                    CoreHint::Uint256SquareRoot { .. } => todo!(),
                    CoreHint::LinearSplit {
                        value,
                        scalar,
                        max_x,
                        x,
                        y,
                    } => {
                        match value {
                            ResOperand::Deref(cell_ref) => {
                                process_cell_ref(*cell_ref);
                            }
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        match scalar {
                            ResOperand::Deref(_) => todo!(),
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        match max_x {
                            ResOperand::Deref(_) => todo!(),
                            ResOperand::DoubleDeref(_, _) => todo!(),
                            ResOperand::Immediate(_) => {}
                            ResOperand::BinOp(_) => todo!(),
                        }
                        process_cell_ref(*x);
                        process_cell_ref(*y);
                    }
                    CoreHint::AllocFelt252Dict { .. } => todo!(),
                    CoreHint::Felt252DictEntryInit { .. } => todo!(),
                    CoreHint::Felt252DictEntryUpdate { .. } => todo!(),
                    CoreHint::GetSegmentArenaIndex { .. } => todo!(),
                    CoreHint::InitSquashData { .. } => todo!(),
                    CoreHint::GetCurrentAccessIndex { .. } => todo!(),
                    CoreHint::ShouldSkipSquashLoop { .. } => todo!(),
                    CoreHint::GetCurrentAccessDelta { .. } => todo!(),
                    CoreHint::ShouldContinueSquashLoop { .. } => todo!(),
                    CoreHint::GetNextDictKey { .. } => todo!(),
                    CoreHint::AssertLeFindSmallArcs { .. } => todo!(),
                    CoreHint::AssertLeIsFirstArcExcluded { .. } => todo!(),
                    CoreHint::AssertLeIsSecondArcExcluded { .. } => todo!(),
                    CoreHint::RandomEcPoint { .. } => todo!(),
                    CoreHint::FieldSqrt { .. } => todo!(),
                    CoreHint::DebugPrint { .. } => todo!(),
                    CoreHint::AllocConstantSize { .. } => todo!(),
                    CoreHint::U256InvModN { .. } => todo!(),
                    CoreHint::EvalCircuit { .. } => todo!(),
                },
                CoreHintBase::Deprecated(deprecated_hint) => match deprecated_hint {
                    DeprecatedHint::AssertCurrentAccessIndicesIsEmpty => todo!(),
                    DeprecatedHint::AssertAllAccessesUsed { .. } => {
                        todo!()
                    }
                    DeprecatedHint::AssertAllKeysUsed => todo!(),
                    DeprecatedHint::AssertLeAssertThirdArcExcluded => todo!(),
                    DeprecatedHint::AssertLtAssertValidInput { .. } => todo!(),
                    DeprecatedHint::Felt252DictRead { .. } => todo!(),
                    DeprecatedHint::Felt252DictWrite { .. } => todo!(),
                },
            },
            Hint::Starknet(starknet_hint) => match starknet_hint {
                StarknetHint::SystemCall { system } => match system {
                    ResOperand::Deref(cell_ref) => {
                        process_cell_ref(*cell_ref);
                    }
                    ResOperand::DoubleDeref(_, _) => todo!(),
                    ResOperand::Immediate(_) => {}
                    ResOperand::BinOp(bin_op_operand) => {
                        process_cell_ref(bin_op_operand.a);
                        match bin_op_operand.b {
                            DerefOrImmediate::Deref(_) => todo!(),
                            DerefOrImmediate::Immediate(_) => {}
                        }
                    }
                },
                StarknetHint::Cheatcode { .. } => todo!(),
            },
            Hint::External(external_hint) => match external_hint {
                ExternalHint::AddRelocationRule { .. } => todo!(),
                ExternalHint::WriteRunParam { .. } => todo!(),
                ExternalHint::AddMarker { .. } => todo!(),
                ExternalHint::AddTrace { .. } => todo!(),
            },
        }
    }
}

impl Index<StepId> for GraphMappings {
    type Output = HashSet<ValueId>;

    fn index(&self, index: StepId) -> &Self::Output {
        self.step2value.get(&index).unwrap()
    }
}

impl Index<ValueId> for GraphMappings {
    type Output = HashSet<StepId>;

    fn index(&self, index: ValueId) -> &Self::Output {
        self.value2step.get(&index).unwrap()
    }
}
