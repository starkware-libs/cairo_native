#[allow(extern_outside_corelib)]
pub extern fn i8_diff(lhs: i8, rhs: i8) -> Result<u8, u8> implicits(RangeCheck) nopanic;

fn main() -> Result<u8, u8> {
    i8_diff(-14, -2)
}
