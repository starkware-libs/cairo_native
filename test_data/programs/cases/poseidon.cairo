use debug::PrintTrait;
//use poseidon::hades_permutation;

// FIXME: Hack to make it compile.
#[allow(extern_outside_corelib)]
extern fn hades_permutation(
    s0: felt252, s1: felt252, s2: felt252
) -> (felt252, felt252, felt252) implicits(Poseidon) nopanic;

fn main() -> (
    (felt252, felt252, felt252),
    (felt252, felt252, felt252),
    (felt252, felt252, felt252),
) {
    (
        hades_permutation(1, 2, 3),
        hades_permutation(4, 5, 6),
        hades_permutation(7, 8, 9),
    )
}
