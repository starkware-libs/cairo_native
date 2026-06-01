#[feature("bounded-int-utils")]
use core::internal::bounded_int::{BoundedInt, downcast};

enum TestInput {
    MinusOneTwentyEight,
    MinusOne,
    Zero,
    One,
    OneTwentySeven,
    TwoFiftyFive,
}

// A non-negative source `[0,P-1]` downcast to `i8`.
fn run_test(v: TestInput) -> Option<i8> {
    // The source value is manually constructed for each variant: a literal coerced
    // directly into the wide non-negative `BoundedInt`. Negative literals are the
    // corresponding felt values (e.g. `-1` is `P-1`, the maximum of the range).
    let x: BoundedInt<0, 0x800000000000011000000000000000000000000000000000000000000000000> = match v {
        TestInput::MinusOneTwentyEight => 0x800000000000010ffffffffffffffffffffffffffffffffffffffffffffff81,
        TestInput::MinusOne => 0x800000000000011000000000000000000000000000000000000000000000000,
        TestInput::Zero => 0,
        TestInput::One => 1,
        TestInput::OneTwentySeven => 127,
        TestInput::TwoFiftyFive => 255,
    };
    downcast::<_, i8>(x)
}
