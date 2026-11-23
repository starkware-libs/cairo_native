#[allow(extern_outside_corelib)]
extern fn felt252_div_const<const rhs: felt252>(lhs: felt252) -> felt252 nopanic;

fn run_test() -> (
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252,
    felt252
) {
    (
        felt252_div_const::<1>(0),
        felt252_div_const::<1>(1),
        felt252_div_const::<2>(-1),
        felt252_div_const::<-2>(2),
        felt252_div_const::<-1>(-1),
        felt252_div_const::<-1>(1),
        felt252_div_const::<1>(-1),
        felt252_div_const::<500>(1000),
        felt252_div_const::<256>(1024),
        felt252_div_const::<-256>(1024),
        felt252_div_const::<256>(-1024),
        felt252_div_const::<-256>(-1024),
        felt252_div_const::<8>(64),
        felt252_div_const::<8>(-64),
        felt252_div_const::<-8>(64),
        felt252_div_const::<-8>(-64),
    )
}
