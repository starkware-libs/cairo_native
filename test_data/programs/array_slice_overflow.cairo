fn run_test(user_len: u32) {
    let arr: Array<u64> = array![10_u64, 20_u64];
    let slice = arr.span().slice(1, user_len);
}
