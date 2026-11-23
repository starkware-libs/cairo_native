mod decons_3_fields {
    #[allow(extern_outside_corelib)]
    extern fn struct_boxed_deconstruct<T>(value: Box<T>) -> (Box<felt252>, Box<u8>, Box<u128>) nopanic;
}

mod decons_1_field {
    #[allow(extern_outside_corelib)]
    extern fn struct_boxed_deconstruct<T>(value: Box<T>) -> (Box<u8>,) nopanic;
}

mod decons_empty_struct {
    #[allow(extern_outside_corelib)]
    extern fn struct_boxed_deconstruct<T>(value: Box<T>) -> () nopanic;
}

mod decons_struct_snapshot {
    #[allow(extern_outside_corelib)]
    extern fn struct_boxed_deconstruct<T>(value: Box<T>) -> (Box<@felt252>, Box<@u8>, Box<@u128>) nopanic;
}

struct ThreeFields {
    x: felt252,
    y: u8,
    z: u128,
}

struct OneField {
    x: u8,
}

struct EmptyStruct { }

fn deconstruct_struct_3_fields() -> (Box<felt252>, Box<u8>, Box<u128>) {
    decons_3_fields::struct_boxed_deconstruct(BoxTrait::new(ThreeFields {x: 2, y: 2, z: 2}))
}

fn deconstruct_struct_1_field() -> (Box<u8>,) {
    decons_1_field::struct_boxed_deconstruct(BoxTrait::new(OneField {x: 2}))
}

fn deconstruct_empty_struct() -> () {
    decons_empty_struct::struct_boxed_deconstruct(BoxTrait::new(EmptyStruct { }))
}

fn deconstruct_struct_snapshot() -> (Box<@felt252>, Box<@u8>, Box<@u128>) {
    decons_struct_snapshot::struct_boxed_deconstruct(BoxTrait::new(ThreeFields {x: 2, y: 2, z: 2}))
}
