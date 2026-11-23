#[starknet::contract]
mod DivModTestContract {
    #[storage]
    struct Storage {}

    #[external(v0)]
    fn div_mod_test(self: @ContractState, x: u8, y: u8) -> u8 {
        let (res, _) = integer::u8_safe_divmod(x, y.try_into().unwrap());
        res
    }
}
