#[allow(extern_outside_corelib)]
extern fn local_into_box<T>(value: T) -> Box<T> nopanic;

#[inline(never)]
pub fn into_box<T>(value: T) -> Box<T> {
    local_into_box(value)
}

fn local_into_box_for_tuple() -> (u8, u8, u8) {
    into_box((4, 5, 6)).unbox()
}
