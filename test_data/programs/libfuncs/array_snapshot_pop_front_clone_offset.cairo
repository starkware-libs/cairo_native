fn run_test() -> Span<felt252> {
    let data = array![7, 3, 4, 193827];
    let mut data = data.span();

    assert(*data.pop_front().unwrap() == 7, 0);
    let _data2 = data.clone();

    assert(*data.pop_front().unwrap() == 3, 1);

    data
}
