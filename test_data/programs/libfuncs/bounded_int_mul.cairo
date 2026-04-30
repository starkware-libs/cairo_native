#[feature("bounded-int-utils")]
use core::internal::bounded_int::{self, BoundedInt, MulHelper, mul, UnitInt};

impl MulHelperBI_m128x127_BI_m128x127 of MulHelper<BoundedInt<-128, 127>, BoundedInt<-128, 127>> {
    type Result = BoundedInt<-16256, 16384>;
}

impl MulHelperBI_0x128_BI_0x128 of MulHelper<BoundedInt<0, 128>, BoundedInt<0, 128>> {
    type Result = BoundedInt<0, 16384>;
}

impl MulHelperBI_1x31_BI_1x1 of MulHelper<BoundedInt<1, 31>, BoundedInt<1, 1>> {
    type Result = BoundedInt<1, 31>;
}

impl MulHelperBI_m1x31_BI_m1xm1 of MulHelper<BoundedInt<-1, 31>, BoundedInt<-1, -1>> {
    type Result = BoundedInt<-31, 1>;
}

impl MulHelperBI_31x31_BI_1x1 of MulHelper<BoundedInt<31, 31>, BoundedInt<1, 1>> {
    type Result = BoundedInt<31, 31>;
}

impl MulHelperBI_m10x0_BI_0x100 of MulHelper<BoundedInt<-100, 0>, BoundedInt<0, 100>> {
    type Result = BoundedInt<-10000, 0>;
}

impl MulHelperBI_1x1_BI_1x1 of MulHelper<BoundedInt<1, 1>, BoundedInt<1, 1>> {
    type Result = BoundedInt<1, 1>;
}

impl MulHelperBI_m5x5_UI_2 of MulHelper<BoundedInt<-5, 5>, UnitInt<2>> {
    type Result = BoundedInt<-10, 10>;
}

fn bi_m128x127_times_bi_m128x127(a: felt252, b: felt252) -> BoundedInt<-16256, 16384> {
    let a: BoundedInt<-128, 127> = a.try_into().unwrap();
    let b: BoundedInt<-128, 127> = b.try_into().unwrap();

    mul(a,b)
}

fn bi_0x128_times_bi_0x128(a: felt252, b: felt252) -> BoundedInt<0, 16384> {
    let a: BoundedInt<0, 128> = a.try_into().unwrap();
    let b: BoundedInt<0, 128> = b.try_into().unwrap();

    mul(a,b)
}

fn bi_1x31_times_bi_1x1(a: felt252, b: felt252) -> BoundedInt<1, 31> {
    let a: BoundedInt<1, 31> = a.try_into().unwrap();
    let b: BoundedInt<1, 1> = b.try_into().unwrap();

    mul(a,b)
}

fn bi_m1x31_times_bi_m1xm1(a: felt252, b: felt252) -> BoundedInt<-31, 1> {
    let a: BoundedInt<-1, 31> = a.try_into().unwrap();
    let b: BoundedInt<-1, -1> = b.try_into().unwrap();

    mul(a,b)
}

fn bi_31x31_times_bi_1x1(a: felt252, b: felt252) -> BoundedInt<31, 31> {
    let a: BoundedInt<31, 31> = a.try_into().unwrap();
    let b: BoundedInt<1, 1> = b.try_into().unwrap();

    mul(a,b)
}

fn bi_m100x0_times_bi_0x100(a: felt252, b: felt252) -> BoundedInt<-10000, 0> {
    let a: BoundedInt<-100, 0> = a.try_into().unwrap();
    let b: BoundedInt<0, 100> = b.try_into().unwrap();

    mul(a,b)
}

fn bi_1x1_times_bi_1x1(a: felt252, b: felt252) -> BoundedInt<1, 1> {
    let a: BoundedInt<1, 1> = a.try_into().unwrap();
    let b: BoundedInt<1, 1> = b.try_into().unwrap();

    mul(a,b)
}

fn bi_m5x5_times_ui_2(a: felt252, b: felt252) -> BoundedInt<-10, 10> {
    let a: BoundedInt<-5, 5> = a.try_into().unwrap();
    let b: UnitInt<2> = b.try_into().unwrap();

    mul(a,b)
}

impl MulHelperBI_m3x5_BI_m3x5 of MulHelper<BoundedInt<-3, 5>, BoundedInt<-3, 5>> {
    type Result = BoundedInt<-15, 25>;
}

fn bi_m3x5_times_bi_m3x5(a: felt252, b: felt252) -> BoundedInt<-15, 25> {
    let a: BoundedInt<-3, 5> = a.try_into().unwrap();
    let b: BoundedInt<-3, 5> = b.try_into().unwrap();

    mul(a,b)
}
