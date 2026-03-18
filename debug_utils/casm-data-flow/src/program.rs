use crate::Memory;
use cairo_lang_casm::{
    instructions::{
        AddApInstruction, AssertEqInstruction, CallInstruction, Instruction, InstructionBody,
        JnzInstruction, JumpInstruction, RetInstruction,
    },
    operand::{BinOpOperand, CellRef, DerefOrImmediate, Operation, ResOperand},
};
use cairo_lang_utils::bigint::BigIntAsHex;
use starknet_types_core::felt::Felt;

// Local instruction representation types, replacing cairo-vm dependency.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Register {
    AP,
    FP,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op1Addr {
    Imm,
    AP,
    FP,
    Op0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Res {
    Op1,
    Add,
    Mul,
    Unconstrained,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PcUpdate {
    Regular,
    Jump,
    JumpRel,
    Jnz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApUpdate {
    Regular,
    Add,
    Add1,
    Add2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FpUpdate {
    Regular,
    APPlus2,
    Dst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opcode {
    NOp,
    AssertEq,
    Call,
    Ret,
}

#[derive(Debug)]
struct InstructionRepr {
    off0: isize,
    off1: isize,
    off2: isize,
    dst_register: Register,
    op0_register: Register,
    op1_addr: Op1Addr,
    res: Res,
    pc_update: PcUpdate,
    ap_update: ApUpdate,
    fp_update: FpUpdate,
    opcode: Opcode,
}

const OFF0_SHIFT: u128 = 0;
const OFF1_SHIFT: u128 = 16;
const OFF2_SHIFT: u128 = 32;
const DST_REG_SHIFT: u128 = 48;
const OP0_REG_SHIFT: u128 = 49;
const OP1_SRC_SHIFT: u128 = 50;
const RES_LOGIC_SHIFT: u128 = 53;
const PC_UPDATE_SHIFT: u128 = 55;
const AP_UPDATE_SHIFT: u128 = 57;
const OPCODE_SHIFT: u128 = 59;

const BIAS: i128 = 1 << 15;

fn decode_raw_instruction(encoded: u128) -> InstructionRepr {
    let off0 = (((encoded >> OFF0_SHIFT) & 0xFFFF) as i128 - BIAS) as isize;
    let off1 = (((encoded >> OFF1_SHIFT) & 0xFFFF) as i128 - BIAS) as isize;
    let off2 = (((encoded >> OFF2_SHIFT) & 0xFFFF) as i128 - BIAS) as isize;

    let dst_register = if (encoded >> DST_REG_SHIFT) & 1 == 0 {
        Register::AP
    } else {
        Register::FP
    };

    let op0_register = if (encoded >> OP0_REG_SHIFT) & 1 == 0 {
        Register::AP
    } else {
        Register::FP
    };

    let op1_src = (encoded >> OP1_SRC_SHIFT) & 0x7;
    let op1_addr = match op1_src {
        0 => Op1Addr::Op0,
        1 => Op1Addr::Imm,
        2 => Op1Addr::FP,
        4 => Op1Addr::AP,
        _ => panic!("Invalid op1_src: {op1_src}"),
    };

    let res_logic = (encoded >> RES_LOGIC_SHIFT) & 0x3;
    let res = match res_logic {
        0 => Res::Op1,
        1 => Res::Add,
        2 => Res::Mul,
        _ => panic!("Invalid res_logic: {res_logic}"),
    };

    let pc_update_bits = (encoded >> PC_UPDATE_SHIFT) & 0x3;
    let pc_update = match pc_update_bits {
        0 => PcUpdate::Regular,
        1 => PcUpdate::Jump,
        2 => PcUpdate::JumpRel,
        3 => PcUpdate::Jnz,
        _ => unreachable!(),
    };

    let ap_update_bits = (encoded >> AP_UPDATE_SHIFT) & 0x3;
    let ap_update = match ap_update_bits {
        0 => ApUpdate::Regular,
        1 => ApUpdate::Add,
        2 => ApUpdate::Add1,
        _ => panic!("Invalid ap_update: {ap_update_bits}"),
    };

    let opcode_bits = (encoded >> OPCODE_SHIFT) & 0x7;
    let opcode = match opcode_bits {
        0 => Opcode::NOp,
        1 => Opcode::Call,
        2 => Opcode::Ret,
        4 => Opcode::AssertEq,
        _ => panic!("Invalid opcode: {opcode_bits}"),
    };

    // Derive fp_update from opcode
    let fp_update = match opcode {
        Opcode::Call => FpUpdate::APPlus2,
        Opcode::Ret => FpUpdate::Dst,
        _ => FpUpdate::Regular,
    };

    // Fix ap_update for Call opcode
    let ap_update = match opcode {
        Opcode::Call => ApUpdate::Add2,
        _ => ap_update,
    };

    // Fix res for Jnz pc_update
    let res = match pc_update {
        PcUpdate::Jnz => Res::Unconstrained,
        _ => res,
    };

    InstructionRepr {
        off0,
        off1,
        off2,
        dst_register,
        op0_register,
        op1_addr,
        res,
        pc_update,
        ap_update,
        fp_update,
        opcode,
    }
}

/// Source: https://github.com/starkware-libs/cairo/blob/main/crates/cairo-lang-casm/src/assembler.rs
pub fn decode_instruction(memory: &Memory, offset: usize) -> Instruction {
    let encoded: u128 = memory[offset].unwrap().try_into().unwrap();
    let instr_repr = decode_raw_instruction(encoded);

    match instr_repr {
        InstructionRepr {
            off0: -1,
            off1,
            off2,
            dst_register: Register::FP,
            op0_register,
            op1_addr,
            res,
            pc_update: PcUpdate::Regular,
            ap_update: ApUpdate::Add,
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        } => Instruction {
            body: InstructionBody::AddAp(AddApInstruction {
                operand: decode_res_operand(ResDescription {
                    off1,
                    off2,
                    imm: memory.get(offset + 1).copied().flatten(),
                    op0_register,
                    op1_addr,
                    res,
                }),
            }),
            inc_ap: false,
            hints: Vec::new(),
        },
        InstructionRepr {
            off0,
            off1,
            off2,
            dst_register,
            op0_register,
            op1_addr,
            res,
            pc_update: PcUpdate::Regular,
            ap_update: ap_update @ (ApUpdate::Add1 | ApUpdate::Regular),
            fp_update: FpUpdate::Regular,
            opcode: Opcode::AssertEq,
        } => Instruction {
            body: InstructionBody::AssertEq(AssertEqInstruction {
                a: CellRef {
                    register: match dst_register {
                        Register::AP => cairo_lang_casm::operand::Register::AP,
                        Register::FP => cairo_lang_casm::operand::Register::FP,
                    },
                    offset: off0 as i16,
                },
                b: decode_res_operand(ResDescription {
                    off1,
                    off2,
                    imm: memory.get(offset + 1).copied().flatten(),
                    op0_register,
                    op1_addr,
                    res,
                }),
            }),
            inc_ap: match ap_update {
                ApUpdate::Regular => false,
                ApUpdate::Add1 => true,
                _ => unreachable!(),
            },
            hints: Vec::new(),
        },
        InstructionRepr {
            off0: 0,
            off1: 1,
            off2,
            dst_register: Register::AP,
            op0_register: Register::AP,
            op1_addr: op1_addr @ (Op1Addr::AP | Op1Addr::FP | Op1Addr::Imm),
            res: Res::Op1,
            pc_update: pc_update @ (PcUpdate::JumpRel | PcUpdate::Jump),
            ap_update: ApUpdate::Add2,
            fp_update: FpUpdate::APPlus2,
            opcode: Opcode::Call,
        } => Instruction {
            body: InstructionBody::Call(CallInstruction {
                target: match op1_addr {
                    Op1Addr::Imm => {
                        assert_eq!(off2, 1);
                        DerefOrImmediate::Immediate(BigIntAsHex {
                            value: memory[offset + 1].unwrap().to_bigint(),
                        })
                    }
                    Op1Addr::AP => DerefOrImmediate::Deref(CellRef {
                        register: cairo_lang_casm::operand::Register::AP,
                        offset: off2 as i16,
                    }),
                    Op1Addr::FP => DerefOrImmediate::Deref(CellRef {
                        register: cairo_lang_casm::operand::Register::FP,
                        offset: off2 as i16,
                    }),
                    _ => unreachable!(),
                },
                relative: match pc_update {
                    PcUpdate::Jump => false,
                    PcUpdate::JumpRel => true,
                    _ => unreachable!(),
                },
            }),
            inc_ap: false,
            hints: Vec::new(),
        },
        InstructionRepr {
            off0: -1,
            off1: -1,
            off2,
            dst_register: Register::FP,
            op0_register: Register::FP,
            op1_addr: op1_addr @ (Op1Addr::AP | Op1Addr::FP | Op1Addr::Imm),
            res: Res::Op1,
            pc_update: pc_update @ (PcUpdate::JumpRel | PcUpdate::Jump),
            ap_update: ap_update @ (ApUpdate::Add1 | ApUpdate::Regular),
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        } => Instruction {
            body: InstructionBody::Jump(JumpInstruction {
                target: match op1_addr {
                    Op1Addr::Imm => {
                        assert_eq!(off2, 1);
                        DerefOrImmediate::Immediate(BigIntAsHex {
                            value: memory[offset + 1].unwrap().to_bigint(),
                        })
                    }
                    Op1Addr::AP => DerefOrImmediate::Deref(CellRef {
                        register: cairo_lang_casm::operand::Register::AP,
                        offset: off2 as i16,
                    }),
                    Op1Addr::FP => DerefOrImmediate::Deref(CellRef {
                        register: cairo_lang_casm::operand::Register::FP,
                        offset: off2 as i16,
                    }),
                    _ => unreachable!(),
                },
                relative: match pc_update {
                    PcUpdate::Jump => false,
                    PcUpdate::JumpRel => true,
                    _ => unreachable!(),
                },
            }),
            inc_ap: match ap_update {
                ApUpdate::Regular => false,
                ApUpdate::Add1 => true,
                _ => unreachable!(),
            },
            hints: Vec::new(),
        },
        InstructionRepr {
            off0,
            off1: -1,
            off2,
            dst_register,
            op0_register: Register::FP,
            op1_addr: op1_addr @ (Op1Addr::AP | Op1Addr::FP | Op1Addr::Imm),
            res: Res::Unconstrained,
            pc_update: PcUpdate::Jnz,
            ap_update: ap_update @ (ApUpdate::Add1 | ApUpdate::Regular),
            fp_update: FpUpdate::Regular,
            opcode: Opcode::NOp,
        } => Instruction {
            body: InstructionBody::Jnz(JnzInstruction {
                jump_offset: match op1_addr {
                    Op1Addr::Imm => {
                        assert_eq!(off2, 1);
                        DerefOrImmediate::Immediate(BigIntAsHex {
                            value: memory[offset + 1].unwrap().to_bigint(),
                        })
                    }
                    Op1Addr::AP => DerefOrImmediate::Deref(CellRef {
                        register: cairo_lang_casm::operand::Register::AP,
                        offset: off2 as i16,
                    }),
                    Op1Addr::FP => DerefOrImmediate::Deref(CellRef {
                        register: cairo_lang_casm::operand::Register::FP,
                        offset: off2 as i16,
                    }),
                    _ => unreachable!(),
                },
                condition: CellRef {
                    register: match dst_register {
                        Register::AP => cairo_lang_casm::operand::Register::AP,
                        Register::FP => cairo_lang_casm::operand::Register::FP,
                    },
                    offset: off0 as i16,
                },
            }),
            inc_ap: match ap_update {
                ApUpdate::Regular => false,
                ApUpdate::Add1 => true,
                _ => unreachable!(),
            },
            hints: Vec::new(),
        },
        InstructionRepr {
            off0: -2,
            off1: -1,
            off2: -1,
            dst_register: Register::FP,
            op0_register: Register::FP,
            op1_addr: Op1Addr::FP,
            res: Res::Op1,
            pc_update: PcUpdate::Jump,
            ap_update: ApUpdate::Regular,
            fp_update: FpUpdate::Dst,
            opcode: Opcode::Ret,
        } => Instruction {
            body: InstructionBody::Ret(RetInstruction {}),
            inc_ap: false,
            hints: Vec::new(),
        },
        _ => panic!(),
    }
}

struct ResDescription {
    off1: isize,
    off2: isize,
    imm: Option<Felt>,
    op0_register: Register,
    op1_addr: Op1Addr,
    res: Res,
}

fn decode_res_operand(desc: ResDescription) -> ResOperand {
    match desc {
        ResDescription {
            off1: -1,
            off2,
            imm: _,
            op0_register: Register::FP,
            op1_addr: op1_addr @ (Op1Addr::AP | Op1Addr::FP),
            res: Res::Op1,
        } => ResOperand::Deref(CellRef {
            register: match op1_addr {
                Op1Addr::AP => cairo_lang_casm::operand::Register::AP,
                Op1Addr::FP => cairo_lang_casm::operand::Register::FP,
                _ => unreachable!(),
            },
            offset: off2 as i16,
        }),
        ResDescription {
            off1,
            off2,
            imm: _,
            op0_register,
            op1_addr: Op1Addr::Op0,
            res: Res::Op1,
        } => ResOperand::DoubleDeref(
            CellRef {
                register: match op0_register {
                    Register::AP => cairo_lang_casm::operand::Register::AP,
                    Register::FP => cairo_lang_casm::operand::Register::FP,
                },
                offset: off1 as i16,
            },
            off2 as i16,
        ),
        ResDescription {
            off1: -1,
            off2: 1,
            imm: Some(imm),
            op0_register: Register::FP,
            op1_addr: Op1Addr::Imm,
            res: Res::Op1,
        } => ResOperand::Immediate(BigIntAsHex {
            value: imm.to_bigint(),
        }),
        ResDescription {
            off1,
            off2,
            imm,
            op0_register,
            op1_addr: op1_addr @ (Op1Addr::AP | Op1Addr::FP | Op1Addr::Imm),
            res: res @ (Res::Add | Res::Mul),
        } => ResOperand::BinOp(BinOpOperand {
            op: match res {
                Res::Add => Operation::Add,
                Res::Mul => Operation::Mul,
                _ => unreachable!(),
            },
            a: CellRef {
                register: match op0_register {
                    Register::AP => cairo_lang_casm::operand::Register::AP,
                    Register::FP => cairo_lang_casm::operand::Register::FP,
                },
                offset: off1 as i16,
            },
            b: match op1_addr {
                Op1Addr::Imm => {
                    assert_eq!(off2, 1);
                    DerefOrImmediate::Immediate(BigIntAsHex {
                        value: imm.unwrap().to_bigint(),
                    })
                }
                Op1Addr::AP => DerefOrImmediate::Deref(CellRef {
                    register: cairo_lang_casm::operand::Register::AP,
                    offset: off2 as i16,
                }),
                Op1Addr::FP => DerefOrImmediate::Deref(CellRef {
                    register: cairo_lang_casm::operand::Register::FP,
                    offset: off2 as i16,
                }),
                _ => unreachable!(),
            },
        }),
        _ => panic!(),
    }
}
