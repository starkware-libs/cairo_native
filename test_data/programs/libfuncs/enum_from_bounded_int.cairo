#[feature("bounded-int-utils")]
use core::internal::bounded_int::BoundedInt;
mod b0x4 {
    #[feature("bounded-int-utils")]
    use core::internal::bounded_int::BoundedInt;
    #[allow(extern_outside_corelib)]
    pub extern fn enum_from_bounded_int<T>(index: BoundedInt<0, 4>) -> T nopanic;

    // This wrapper is required so that the compiler won't assume extern `enum_from_bounded_int` is a
    // branch function. Without it, the program does not compile.
    fn wrapper<T>(index: BoundedInt<0, 4>) -> T {
        enum_from_bounded_int(index)
    }
}

mod b0x0 {
    #[feature("bounded-int-utils")]
    use core::internal::bounded_int::BoundedInt;
    #[allow(extern_outside_corelib)]
    pub extern fn enum_from_bounded_int<T>(index: BoundedInt<0, 0>) -> T nopanic;

    // This wrapper is required so that the compiler won't assume extern `enum_from_bounded_int` is a
    // branch function. Without it, the program does not compile.
    fn wrapper<T>(index: BoundedInt<0, 0>) -> T {
        enum_from_bounded_int(index)
    }
}

enum Enum1 {
    Zero
}

enum Enum5 {
    Zero,
    One,
    Two,
    Three,
    Four
}

fn test_1_variants(input: felt252) -> Enum1 {
    let bi: BoundedInt<0, 0> = input.try_into().unwrap();
    b0x0::wrapper(bi)
}

fn test_5_variants(input: felt252) -> Enum5 {
    let bi: BoundedInt<0, 4> = input.try_into().unwrap();
    b0x4::wrapper(bi)
}
