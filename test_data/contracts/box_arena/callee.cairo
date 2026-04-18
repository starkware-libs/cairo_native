#[inline(never)]
fn make_box(v: felt252) -> Box<felt252> {
    BoxTrait::new(v)
}

#[starknet::contract]
mod callee {
    use super::make_box;

    #[storage]
    struct Storage {}

    #[external(v0)]
    fn add_one(ref self: ContractState, x: felt252) -> felt252 {
        let boxed: Box<felt252> = make_box(x + 1);
        boxed.unbox()
    }
}
