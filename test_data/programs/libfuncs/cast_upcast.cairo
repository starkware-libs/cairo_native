#[feature("bounded-int-utils")]
use core::internal::bounded_int::{BoundedInt};
#[allow(extern_outside_corelib)]
extern const fn upcast<FromType, ToType>(x: FromType) -> ToType nopanic;

fn test_x_y<
    X,
    Y,
    +TryInto<felt252, X>,
    +Into<Y, felt252>
>(v: felt252) -> felt252 {
    let v: X = v.try_into().unwrap();
    let v: Y = upcast(v);
    v.into()
}

fn u8_u16(v: felt252) -> felt252 { test_x_y::<u8, u16>(v) }
fn u8_u32(v: felt252) -> felt252 { test_x_y::<u8, u32>(v) }
fn u8_u64(v: felt252) -> felt252 { test_x_y::<u8, u64>(v) }
fn u8_u128(v: felt252) -> felt252 { test_x_y::<u8, u128>(v) }
fn u8_felt252(v: felt252) -> felt252 { test_x_y::<u8, felt252>(v) }

fn u16_u32(v: felt252) -> felt252 { test_x_y::<u16, u32>(v) }
fn u16_u64(v: felt252) -> felt252 { test_x_y::<u16, u64>(v) }
fn u16_u128(v: felt252) -> felt252 { test_x_y::<u16, u128>(v) }
fn u16_felt252(v: felt252) -> felt252 { test_x_y::<u16, felt252>(v) }

fn u32_u64(v: felt252) -> felt252 { test_x_y::<u32, u64>(v) }
fn u32_u128(v: felt252) -> felt252 { test_x_y::<u32, u128>(v) }
fn u32_felt252(v: felt252) -> felt252 { test_x_y::<u32, felt252>(v) }

fn u64_u128(v: felt252) -> felt252 { test_x_y::<u64, u128>(v) }
fn u64_felt252(v: felt252) -> felt252 { test_x_y::<u64, felt252>(v) }

fn u128_felt252(v: felt252) -> felt252 { test_x_y::<u128, felt252>(v) }

fn i8_i16(v: felt252) -> felt252 { test_x_y::<i8, i16>(v) }
fn i8_i32(v: felt252) -> felt252 { test_x_y::<i8, i32>(v) }
fn i8_i64(v: felt252) -> felt252 { test_x_y::<i8, i64>(v) }
fn i8_i128(v: felt252) -> felt252 { test_x_y::<i8, i128>(v) }
fn i8_felt252(v: felt252) -> felt252 { test_x_y::<i8, felt252>(v) }

fn i16_i32(v: felt252) -> felt252 { test_x_y::<i16, i32>(v) }
fn i16_i64(v: felt252) -> felt252 { test_x_y::<i16, i64>(v) }
fn i16_i128(v: felt252) -> felt252 { test_x_y::<i16, i128>(v) }
fn i16_felt252(v: felt252) -> felt252 { test_x_y::<i16, felt252>(v) }

fn i32_i64(v: felt252) -> felt252 { test_x_y::<i32, i64>(v) }
fn i32_i128(v: felt252) -> felt252 { test_x_y::<i32, i128>(v) }
fn i32_felt252(v: felt252) -> felt252 { test_x_y::<i32, felt252>(v) }

fn i64_i128(v: felt252) -> felt252 { test_x_y::<i64, i128>(v) }
fn i64_felt252(v: felt252) -> felt252 { test_x_y::<i64, felt252>(v) }

fn i128_felt252(v: felt252) -> felt252 { test_x_y::<i128, felt252>(v) }

fn b0x5_b0x10(v: felt252) -> felt252 { test_x_y::<BoundedInt<0, 5>, BoundedInt<0, 10>>(v) }
fn b2x5_b2x10(v: felt252) -> felt252 { test_x_y::<BoundedInt<2, 5>, BoundedInt<2, 10>>(v) }
fn b2x5_b1x10(v: felt252) -> felt252 { test_x_y::<BoundedInt<2, 5>, BoundedInt<1, 10>>(v) }
fn b0x5_bm10x10(v: felt252) -> felt252 { test_x_y::<BoundedInt<0, 5>, BoundedInt<-10, 10>>(v) }
fn bm5x5_bm10x10(v: felt252) -> felt252 { test_x_y::<BoundedInt<-5, 5>, BoundedInt<-10, 10>>(v) }
fn i8_bm200x200(v: felt252) -> felt252 { test_x_y::<i8, BoundedInt<-200, 200>>(v) }
fn bm100x100_i8(v: felt252) -> felt252 { test_x_y::<BoundedInt<-100, 100>, i8>(v) }
