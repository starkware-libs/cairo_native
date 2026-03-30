use core::ec::EcPoint;

mod value {
    extern type Const<T, const X: felt252, const Y: felt252>;
}
extern fn const_as_box<T, const SEGMENT_INDEX: felt252>() -> Box<EcPoint> nopanic;

fn run_test() -> Box<EcPoint> {
    const_as_box::<
        value::Const<
            EcPoint,
            0x1ef15c18599971b7beced415a40f0c7deacfd9b0d1819e03d723d8bc943cfca,
            0x5668060aa49730b7be4801df46ec62de53ecd11abe43a32873000c36e8dc1f,
        >,
        0,
    >()
}
