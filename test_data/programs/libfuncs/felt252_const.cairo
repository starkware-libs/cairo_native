#[allow(extern_outside_corelib)]
extern fn felt252_const<const value: felt252>() -> felt252 nopanic;

fn run_test() -> (felt252, felt252, felt252, felt252) {
    (
        felt252_const::<0>(),
        felt252_const::<1>(),
        felt252_const::<-2>(),
        felt252_const::<-1>()
    )
}
