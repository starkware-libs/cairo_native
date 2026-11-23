#[allow(extern_outside_corelib)]
extern const fn downcast<FromType, ToType>( x: FromType, ) -> Option<ToType> implicits(RangeCheck) nopanic;

fn run_test(
    v8: u8, v16: u16, v32: u32, v64: u64, v128: u128
) -> (
    (Option<u8>, Option<u8>, Option<u8>, Option<u8>, Option<u8>),
    (Option<u16>, Option<u16>, Option<u16>, Option<u16>),
    (Option<u32>, Option<u32>, Option<u32>),
    (Option<u64>, Option<u64>),
    (Option<u128>,),
) {
    (
        (downcast(v128), downcast(v64), downcast(v32), downcast(v16), downcast(v8)),
        (downcast(v128), downcast(v64), downcast(v32), downcast(v16)),
        (downcast(v128), downcast(v64), downcast(v32)),
        (downcast(v128), downcast(v64)),
        (downcast(v128),),
    )
}
