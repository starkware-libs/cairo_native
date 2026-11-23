use zeroable::IsZeroResult;

#[allow(extern_outside_corelib)]
extern fn u64_is_zero(a: u64) -> IsZeroResult<u64> implicits() nopanic;

use traits::TryInto;
use core::option::OptionTrait;

fn program(value: u64) -> bool {
    match u64_is_zero(value) {
        IsZeroResult::Zero(_) => true,
        IsZeroResult::NonZero(_) => false,
    }
}

fn run_test(value: felt252) -> bool {
    program(value.try_into().unwrap())
}
