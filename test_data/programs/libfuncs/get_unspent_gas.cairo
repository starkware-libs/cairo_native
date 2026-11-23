#[allow(extern_outside_corelib)]
extern fn get_unspent_gas() -> u128 implicits(GasBuiltin) nopanic;

#[inline(never)]
fn identity<T>(t: T) -> T {
    t
}

fn run_test() -> u128 {
    let one = identity(1);
    let two = identity(2);
    let prev = get_unspent_gas();
    let three = identity(one + two);
    let four = identity(one + three);
    let five = identity(two + three);
    let _ten = identity(one + five + four);
    let after = get_unspent_gas();
    return prev - after;
}
