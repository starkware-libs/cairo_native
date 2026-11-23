mod felt252_span_from_tuple {
    #[allow(extern_outside_corelib)]
    pub extern fn span_from_tuple<T>(struct_like: Box<@T>) -> @Array<felt252> nopanic;
}

fn run_test() -> Array<felt252> {
    let span = felt252_span_from_tuple::span_from_tuple(BoxTrait::new(@(10, 20, 30)));
    span.clone()
}
