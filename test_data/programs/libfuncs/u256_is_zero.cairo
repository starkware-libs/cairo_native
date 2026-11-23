use zeroable::IsZeroResult;

#[allow(extern_outside_corelib)]
extern fn u256_is_zero(a: u256) -> IsZeroResult<u256> implicits() nopanic;

fn run_test(value: u256) -> bool {
    match u256_is_zero(value) {
        IsZeroResult::Zero(_) => true,
        IsZeroResult::NonZero(_) => false,
    }
}
