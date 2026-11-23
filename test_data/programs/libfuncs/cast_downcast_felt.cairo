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

fn felt252_i8(v: felt252) -> felt252 { test_x_y::<felt252, i8>(v) }
fn felt252_i16(v: felt252) -> felt252 { test_x_y::<felt252, i16>(v) }
fn felt252_i32(v: felt252) -> felt252 { test_x_y::<felt252, i32>(v) }
fn felt252_i64(v: felt252) -> felt252 { test_x_y::<felt252, i64>(v) }
