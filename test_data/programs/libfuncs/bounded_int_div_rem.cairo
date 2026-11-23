#[feature("bounded-int-utils")]
use core::internal::bounded_int::{self, BoundedInt, div_rem, DivRemHelper};
use core::internal::OptionRev;
#[allow(extern_outside_corelib)]
extern fn bounded_int_wrap_non_zero<T>(v: T) -> NonZero<T> nopanic;


impl Helper_u8_u8 of DivRemHelper<u8, u8> {
    type DivT = BoundedInt<0, 255>;
    type RemT = BoundedInt<0, 254>;
}
fn test_u8(a: felt252, b: felt252) -> (felt252, felt252) {
    let a_int: u8 = a.try_into().unwrap();
    let b_int: u8 = b.try_into().unwrap();
    let b_nz: NonZero<u8> = b_int.try_into().unwrap();
    let (q, r) = div_rem(a_int, b_nz);
    return (q.into(), r.into());
}

impl Helper_10_100_10_40 of DivRemHelper<BoundedInt<10, 100>, BoundedInt<10, 40>> {
    type DivT = BoundedInt<0, 10>;
    type RemT = BoundedInt<0, 39>;
}
fn test_10_100_10_40(a: felt252, b: felt252) -> (felt252, felt252) {
    let a_int: BoundedInt<10, 100> = a.try_into().unwrap();
    let b_int: BoundedInt<10, 40> = b.try_into().unwrap();
    let (q, r) = div_rem(a_int, bounded_int_wrap_non_zero(b_int));
    return (q.into(), r.into());
}

impl Helper_50_100_20_40 of DivRemHelper<BoundedInt<50, 100>, BoundedInt<20, 40>> {
    type DivT = BoundedInt<1, 5>;
    type RemT = BoundedInt<0, 39>;
}
fn test_50_100_20_40(a: felt252, b: felt252) -> (felt252, felt252) {
    let a_int: BoundedInt<50, 100> = a.try_into().unwrap();
    let b_int: BoundedInt<20, 40> = b.try_into().unwrap();
    let (q, r) = div_rem(a_int, bounded_int_wrap_non_zero(b_int));
    return (q.into(), r.into());
}
