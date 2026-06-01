extern const fn downcast<FromType, ToType>(
    x: FromType,
) -> Option<ToType> implicits(RangeCheck) nopanic;

// Downcasts a felt252 directly into an `i8`.
fn run_test(v: felt252) -> Option<i8> {
    downcast::<felt252, i8>(v)
}
