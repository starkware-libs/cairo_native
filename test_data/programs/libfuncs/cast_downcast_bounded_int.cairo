#[feature("bounded-int-utils")]
use core::internal::bounded_int::BoundedInt;

#[allow(extern_outside_corelib)]
extern const fn downcast<FromType, ToType>( x: FromType, ) -> Option<ToType> implicits(RangeCheck) nopanic;

fn test_x_y<
    X,
    Y,
    +TryInto<felt252, X>,
    +Into<Y, felt252>
>(v: felt252) -> felt252 {
    let v: X = v.try_into().unwrap();
    let v: Y = downcast(v).unwrap();
    v.into()
}

fn b0x30_b0x30(v: felt252) -> felt252 { test_x_y::<BoundedInt<0,30>, BoundedInt<0,30>>(v) }
fn bm31x30_b31x30(v: felt252) -> felt252 { test_x_y::<BoundedInt<-31,30>, BoundedInt<-31,30>>(v) }
fn bm31x30_bm5x30(v: felt252) -> felt252 { test_x_y::<BoundedInt<-31,30>, BoundedInt<-5,30>>(v) }
fn bm31x30_b5x30(v: felt252) -> felt252 { test_x_y::<BoundedInt<-31,30>, BoundedInt<5,30>>(v) }
fn b5x30_b31x31(v: felt252) -> felt252 { test_x_y::<BoundedInt<5,31>, BoundedInt<31,31>>(v) }
fn bm100x100_bm100xm1(v: felt252) -> felt252 { test_x_y::<BoundedInt<-100,100>, BoundedInt<-100,-1>>(v) }
fn bm31xm31_bm31xm31(v: felt252) -> felt252 { test_x_y::<BoundedInt<-31,-31>, BoundedInt<-31,-31>>(v) }
// Check if the target type is wider than the source type
fn b0x30_b5x40(v: felt252) -> felt252 { test_x_y::<BoundedInt<0,30>, BoundedInt<5,40>>(v) }
// Check if the source's lower and upper bound are included in the
// target type.
fn b0x30_bm40x40(v: felt252) -> felt252 { test_x_y::<BoundedInt<0,30>, BoundedInt<-40,40>>(v) }
