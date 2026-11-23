mod tuple_span_from_tuple {
    #[allow(extern_outside_corelib)]
    pub extern fn span_from_tuple<T>(
        struct_like: Box<@T>
    ) -> @Array<(felt252, felt252, felt252)> nopanic;
}

fn run_test() {
    let multi_tuple = ((10, 20, 30), (40, 50, 60), (70, 80, 90));
    let span = tuple_span_from_tuple::span_from_tuple(BoxTrait::new(@multi_tuple));
    assert!(*span[0] == (10, 20, 30));
    assert!(*span[1] == (40, 50, 60));
    assert!(*span[2] == (70, 80, 90));
}
