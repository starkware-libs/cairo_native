#[feature("bounded-int-utils")]
use core::internal::bounded_int::{self, SubHelper, BoundedInt};

impl U8BISub of SubHelper<u8, u8> {
    type Result = BoundedInt<-255, 255>;
}

#[allow(extern_outside_corelib)]
extern fn bounded_int_sub<Lhs, Rhs, impl H: SubHelper<Lhs, Rhs>>(
    lhs: Lhs, rhs: Rhs,
) -> H::Result nopanic;

fn main() -> BoundedInt<-255, 255> {
    bounded_int_sub(0_u8, 255_u8)
}
