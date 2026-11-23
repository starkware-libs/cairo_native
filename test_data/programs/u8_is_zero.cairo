use zeroable::IsZeroResult;

#[allow(extern_outside_corelib)]
extern fn u8_is_zero(a: u8) -> IsZeroResult<u8> implicits() nopanic;

use traits::TryInto;
use core::option::OptionTrait;

fn program(value: u8) -> bool {
    match u8_is_zero(value) {
        IsZeroResult::Zero(_) => true,
        IsZeroResult::NonZero(_) => false,
    }
}

fn run_test(value: felt252) -> bool {
    program(value.try_into().unwrap())
}
