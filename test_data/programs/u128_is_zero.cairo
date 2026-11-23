use zeroable::IsZeroResult;

#[allow(extern_outside_corelib)]
extern fn u128_is_zero(a: u128) -> IsZeroResult<u128> implicits() nopanic;

use traits::TryInto;
use core::option::OptionTrait;

fn program(value: u128) -> bool {
    match u128_is_zero(value) {
        IsZeroResult::Zero(_) => true,
        IsZeroResult::NonZero(_) => false,
    }
}

fn run_test(value: felt252) -> bool {
    program(value.try_into().unwrap())
}
