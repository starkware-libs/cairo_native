extern fn coupon_buy<T>() -> T nopanic;
fn run_test(mut x: felt252) {

    let mut y = x + 4;
    let mut c = coupon_buy();
    bar(ref x, ref c, ref y);
    foo(__coupon__: c);
}
#[inline(never)]
fn foo() {

}
#[inline(never)]
fn bar(ref x: felt252, ref c: foo::Coupon, ref y: felt252) {
}
