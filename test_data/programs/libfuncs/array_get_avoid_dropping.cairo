use core::{array::{array_append, array_new}, box::into_box};

fn run_test() -> @Box<felt252> {
    let mut x: Array<Box<felt252>> = array_new();
    array_append(ref x, into_box(42));

    x.at(0)
}
