// Tests an enum with a large number of variants (> 16).

#[derive(Drop)]
enum LargeEnum {
    V0: u8, V1: u8, V2: u8, V3: u8, V4: u8, V5: u8, V6: u8, V7: u8,
    V8: u8, V9: u8, V10: u8, V11: u8, V12: u8, V13: u8, V14: u8, V15: u8,
    V16: u8, V17: u8, V18: u8, V19: u8, V20: u8, V21: u8, V22: u8, V23: u8,
    V24: u8, V25: u8, V26: u8, V27: u8, V28: u8, V29: u8, V30: u8, V31: u8,
    V32: u8, V33: u8, V34: u8, V35: u8, V36: u8, V37: u8, V38: u8, V39: u8,
    V40: u8, V41: u8, V42: u8, V43: u8, V44: u8, V45: u8, V46: u8, V47: u8,
    V48: u8, V49: u8, V50: u8, V51: u8, V52: u8, V53: u8, V54: u8, V55: u8,
    V56: u8, V57: u8, V58: u8, V59: u8, V60: u8, V61: u8, V62: u8, V63: u8,
    V64: u8,
}

fn main() -> (LargeEnum, u8) {
    (LargeEnum::V3(1), 2)
}
