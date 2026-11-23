#[allow(extern_outside_corelib)]
extern fn felt252_sub_const<const rhs: felt252>(lhs: felt252) -> felt252 nopanic;

fn run_test() -> (felt252, felt252, felt252, felt252, felt252, felt252, felt252, felt252, felt252) {
    (
        felt252_sub_const::<0>(0),
        felt252_sub_const::<0>(1),
        felt252_sub_const::<1>(0),
        felt252_sub_const::<1>(1),
        felt252_sub_const::<0>(-1),
        felt252_sub_const::<-1>(0),
        felt252_sub_const::<-1>(-1),
        felt252_sub_const::<-1>(1),
        felt252_sub_const::<1>(-1),
    )
}
