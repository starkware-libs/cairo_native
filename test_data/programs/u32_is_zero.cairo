use zeroable::IsZeroResult;

#[allow(extern_outside_corelib)]
extern fn u32_is_zero(a: u32) -> IsZeroResult<u32> implicits() nopanic;

use traits::TryInto;
use core::option::OptionTrait;

fn program(value: u32) -> bool {
    match u32_is_zero(value) {
        IsZeroResult::Zero(_) => true,
        IsZeroResult::NonZero(_) => false,
    }
}

fn run_test(value: felt252) -> bool {
    program(value.try_into().unwrap())
}
