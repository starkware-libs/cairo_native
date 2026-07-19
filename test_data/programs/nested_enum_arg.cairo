// Both `Inner` and `Outer` are 2-variant enums, so both are memory-allocated.
// Passing an `Outer` as an argument forces `to_ptr` to serialize a nested
// memory-allocated enum. The return value depends on both discriminants and the
// payload, so any corruption of the nested value changes the result.

#[derive(Drop)]
enum Inner {
    A: felt252,
    B: felt252,
}

#[derive(Drop)]
enum Outer {
    X: Inner,
    Y: Inner,
}

fn run_test(x: Outer) -> felt252 {
    match x {
        Outer::X(inner) => match inner {
            Inner::A(v) => v,
            Inner::B(v) => v + 1,
        },
        Outer::Y(inner) => match inner {
            Inner::A(v) => v + 2,
            Inner::B(v) => v + 3,
        },
    }
}
