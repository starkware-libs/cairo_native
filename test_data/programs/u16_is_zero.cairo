use zeroable::IsZeroResult;

#[allow(extern_outside_corelib)]
extern fn u16_is_zero(a: u16) -> IsZeroResult<u16> implicits() nopanic;

use traits::TryInto;
use core::option::OptionTrait;

fn program(value: u16) -> bool {
    match u16_is_zero(value) {
        IsZeroResult::Zero(_) => true,
        IsZeroResult::NonZero(_) => false,
    }
}

fn run_test(value: felt252) -> bool {
    program(value.try_into().unwrap())
}
