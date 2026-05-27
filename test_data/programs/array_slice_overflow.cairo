fn run_test(user_len: usize) -> usize {
    let arr: Array<u64> = array![10_u64, 20_u64];
    arr.span().slice(1, user_len).len()
}
