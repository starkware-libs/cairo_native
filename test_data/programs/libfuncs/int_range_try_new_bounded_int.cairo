#[feature("bounded-int-utils")]
use core::internal::bounded_int::BoundedInt;

pub extern type IntRange<T>;
impl IntRangeDrop<T> of Drop<IntRange<T>>;

pub extern fn int_range_try_new<T>(
    x: T, y: T,
) -> Result<IntRange<T>, IntRange<T>> implicits(core::RangeCheck) nopanic;

fn run_test(lhs: felt252, rhs: felt252) -> bool {
    let lhs: BoundedInt<-10, 10> = lhs.try_into().unwrap();
    let rhs: BoundedInt<-10, 10> = rhs.try_into().unwrap();
    match int_range_try_new(lhs, rhs) {
        Result::Ok(_) => true,
        Result::Err(_) => false,
    }
}
