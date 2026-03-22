use core::dict::{Felt252Dict, Felt252DictTrait};

fn run_test() -> felt252 {
    let mut dict1: Felt252Dict<felt252> = Default::default();
    dict1.insert(1, 100);
    let squashed1 = dict1.squash();

    let mut dict2: Felt252Dict<felt252> = Default::default();
    dict2.insert(2, 200);
    let squashed2 = dict2.squash();

    let mut arr: Array<SquashedFelt252Dict<felt252>> = ArrayTrait::new();
    arr.append(squashed1);
    arr.append(squashed2);

    let snap = @arr;

    let len_a = get_len(snap);
    let len_b = get_len(snap);
    let len_c = get_len(snap);

    len_a + len_b + len_c
}

#[inline(never)]
fn get_len(snap: @Array<SquashedFelt252Dict<felt252>>) -> felt252 {
    snap.len().into()
}
