use crate::common::{compare_outputs, load_cairo, run_native_program, run_vm_program};
use cairo_native::starknet::DummySyscallHandler;
use cairo_native::utils::testing::load_program_and_runner;

#[test]
fn enum_init() {
    let program = load_program_and_runner("programs/enum_init");

    let result_vm = run_vm_program(&program, "run_test", vec![], None).unwrap();
    let result_native = run_native_program(
        &program,
        "run_test",
        &[],
        None,
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("run_test").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn enum_match() {
    let program = load_program_and_runner("programs/enum_match");

    let result_vm = run_vm_program(&program, "match_a", vec![], None).unwrap();
    let result_native = run_native_program(
        &program,
        "match_a",
        &[],
        None,
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("match_a").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();

    let result_vm = run_vm_program(&program, "match_b", vec![], None).unwrap();
    let result_native = run_native_program(
        &program,
        "match_b",
        &[],
        None,
        Option::<DummySyscallHandler>::None,
    );

    compare_outputs(
        &program.1,
        &program.2.find_function("match_b").unwrap().id,
        &result_vm,
        &result_native,
    )
    .unwrap();
}

#[test]
fn const_as_box() {
    let program = load_cairo! {
        use core::box::BoxTrait;

        #[derive(Drop, Copy)]
        struct Padded {
            a: u8,
            b: u64,
            c: u16,
            d: i128,
        }

        #[derive(Drop, Copy)]
        enum Shape {
            Small: u8,
            Wide: u128,
            Tagged: Padded,
        }

        #[derive(Drop, Copy)]
        struct Outer {
            x: felt252,
            inner: Padded,
            shape: Shape,
            opt: Option<u32>,
            nz: NonZero<u64>,
        }

        // Keeps the boxes from being folded into `const_as_immediate`.
        #[inline(never)]
        fn unbox<T>(value: Box<T>) -> T {
            value.unbox()
        }

        fn nested() -> Outer {
            unbox(BoxTrait::new(Outer {
                x: -1,
                inner: Padded { a: 7, b: 0xDEADBEEF_CAFEBABE, c: 513, d: -1234567890123456789012345 },
                shape: Shape::Wide(0xFFFFFFFF_FFFFFFFF_FFFFFFFF_FFFFFFFE),
                opt: Option::Some(42),
                nz: 1000000,
            }))
        }

        fn enum_small() -> Box<Shape> {
            BoxTrait::new(Shape::Small(200))
        }

        fn felt() -> Box<felt252> {
            BoxTrait::new(-1)
        }

        fn enum_struct() -> Shape {
            unbox(BoxTrait::new(Shape::Tagged(Padded { a: 1, b: 2, c: 3, d: -4 })))
        }

        fn same_const_twice() -> (Padded, Box<Padded>) {
            let a = BoxTrait::new(Padded { a: 1, b: 2, c: 3, d: -4 });
            let b = BoxTrait::new(Padded { a: 1, b: 2, c: 3, d: -4 });
            (unbox(a), b)
        }
    };

    // Make sure the constants are actually lowered through `const_as_box`.
    assert!(program
        .1
        .libfunc_declarations
        .iter()
        .any(|decl| decl.long_id.generic_id.0 == "const_as_box"));

    for entry_point in [
        "nested",
        "enum_small",
        "felt",
        "enum_struct",
        "same_const_twice",
    ] {
        let result_vm = run_vm_program(&program, entry_point, vec![], None).unwrap();
        let result_native = run_native_program(
            &program,
            entry_point,
            &[],
            None,
            Option::<DummySyscallHandler>::None,
        );

        compare_outputs(
            &program.1,
            &program.2.find_function(entry_point).unwrap().id,
            &result_vm,
            &result_native,
        )
        .unwrap();
    }
}
