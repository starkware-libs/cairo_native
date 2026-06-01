#[feature("bounded-int-utils")]
use core::internal::bounded_int::{BoundedInt,upcast,downcast};

// A non-negative source `[0,P-1]` downcast to `i8`.
fn run_test(v: felt252) -> Option<i8> {
    let x: BoundedInt<0, 0x800000000000011000000000000000000000000000000000000000000000000> = upcast(v);
    downcast::<_, i8>(x)
}
