use starknet::ContractAddress;
use starknet::SyscallResultTrait;
use starknet::syscalls::call_contract_syscall;

#[inline(never)]
fn make_box(v: felt252) -> Box<felt252> {
    BoxTrait::new(v)
}

#[starknet::contract]
mod caller {
    use super::make_box;
    use starknet::ContractAddress;
    use starknet::SyscallResultTrait;
    use starknet::syscalls::call_contract_syscall;

    #[storage]
    struct Storage {}

    // Deliberately holds a box LIVE across the nested `call_contract_syscall`.
    //
    // Under the current flat-reset design, the callee's `reset_box_arena()`
    // fires while we're still executing, freeing `boxed_x`. We then allocate
    // `clobber` (using a value that comes from the callee's result, so the
    // compiler can't const-fold the read), which lands on `boxed_x`'s freed
    // slot. Reading `boxed_x.unbox()` after that is a use-after-free.
    //
    // Expected under correct scoping: boxed_x.unbox() == x
    // Expected under current UAF:     boxed_x.unbox() == *result[0]  (= x+1)
    #[external(v0)]
    fn proxy_add_one(
        ref self: ContractState,
        target: ContractAddress,
        selector: felt252,
        x: felt252,
    ) -> (felt252, felt252) {
        // Alloc #1 — lives across the syscall.
        let boxed_x: Box<felt252> = make_box(x);

        // Pass raw `x` to the callee so the syscall itself doesn't consume boxed_x.
        let result = call_contract_syscall(target, selector, array![x].span())
            .unwrap_syscall();

        // After the callee's reset, allocate a fresh box holding the callee's
        // return value. Because the arena was reset, this lands at offset 0 —
        // exactly where boxed_x was.
        let clobber: Box<felt252> = make_box(*result[0]);

        // Now read boxed_x. Correct scoping → x; under UAF → *result[0].
        let recovered = boxed_x.unbox();
        let clobber_val = clobber.unbox();

        (recovered, clobber_val)
    }
}
