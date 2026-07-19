use starknet::ClassHash;
use starknet::{
    call_contract_syscall, ContractAddress,
    deploy_syscall, deploy_v2_syscall, emit_event_syscall, ExecutionInfo, get_block_hash_syscall,
    keccak_syscall,
    library_call_syscall, replace_class_syscall, send_message_to_l1_syscall,
    storage_address_try_from_felt252, storage_read_syscall, storage_write_syscall, SyscallResult,
    testing::cheatcode,
};
use starknet::syscalls::get_execution_info_syscall;
use starknet::syscalls::get_execution_info_v2_syscall;

fn get_block_hash() -> SyscallResult<felt252> {
    get_block_hash_syscall(0)
}

fn get_execution_info() -> SyscallResult<Box<starknet::info::ExecutionInfo>> {
     get_execution_info_syscall()
}

fn get_execution_info_v2() -> SyscallResult<Box<starknet::info::v2::ExecutionInfo>> {
    get_execution_info_v2_syscall()
}

const ZERO_CLASS_HASH: ClassHash = 0x0.try_into().unwrap();
const ZERO_CONTRACT_ADDRESS: ContractAddress = 0x0.try_into().unwrap();

fn deploy() -> SyscallResult<(ContractAddress, Span<felt252>)> {
    deploy_syscall(ZERO_CLASS_HASH, 0, array![].span(), false)
}

fn deploy_v2() -> SyscallResult<(ContractAddress, Span<felt252>)> {
    deploy_v2_syscall(ZERO_CLASS_HASH, 0, array![].span(), false)
}

fn replace_class() -> SyscallResult<()> {
    replace_class_syscall(ZERO_CLASS_HASH)
}

fn library_call() -> SyscallResult<Span<felt252>> {
    library_call_syscall(ZERO_CLASS_HASH, 0, array![].span())
}

fn call_contract() -> SyscallResult<Span<felt252>> {
    call_contract_syscall(ZERO_CONTRACT_ADDRESS, 0, array![].span())
}

fn storage_read() -> felt252 {
    storage_read_syscall(0, storage_address_try_from_felt252(0).unwrap()).unwrap()
}

fn storage_write() {
    storage_write_syscall(0, storage_address_try_from_felt252(0).unwrap(), 0).unwrap()
}

fn emit_event() -> SyscallResult<()> {
    emit_event_syscall(array![].span(), array![].span())
}

fn send_message_to_l1() -> SyscallResult<()> {
    send_message_to_l1_syscall(3, array![2].span())
}

fn keccak() -> SyscallResult<u256> {
    keccak_syscall(array![].span())
}

fn set_sequencer_address(address: felt252) -> Span<felt252> {
    return cheatcode::<'set_sequencer_address'>(array![address].span());
}

fn set_account_contract_address(address: felt252) -> Span<felt252> {
    return cheatcode::<'set_account_contract_address'>(array![address].span());
}

fn set_block_number(number: felt252) -> Span<felt252> {
    return cheatcode::<'set_block_number'>(array![number].span());
}

fn set_block_timestamp(timestamp: felt252) -> Span<felt252> {
    return cheatcode::<'set_block_timestamp'>(array![timestamp].span());
}

fn set_caller_address(address: felt252) -> Span<felt252> {
    return cheatcode::<'set_caller_address'>(array![address].span());
}

fn set_chain_id(id: felt252) -> Span<felt252> {
    return cheatcode::<'set_chain_id'>(array![id].span());
}

fn set_contract_address(address: felt252) -> Span<felt252> {
    return cheatcode::<'set_contract_address'>(array![address].span());
}

fn set_max_fee(fee: felt252) -> Span<felt252> {
    return cheatcode::<'set_max_fee'>(array![fee].span());
}

fn set_nonce(nonce: felt252) -> Span<felt252> {
    return cheatcode::<'set_nonce'>(array![nonce].span());
}

fn set_signature(signature: Array<felt252>) -> Span<felt252> {
    return cheatcode::<'set_signature'>(signature.span());
}

fn set_transaction_hash(hash: felt252) -> Span<felt252> {
    return cheatcode::<'set_transaction_hash'>(array![hash].span());
}

fn set_version(version: felt252) -> Span<felt252> {
    return cheatcode::<'set_version'>(array![version].span());
}

fn pop_log(log: felt252) -> Span<felt252> {
    return cheatcode::<'pop_log'>(array![log].span());
}

fn pop_l2_to_l1_message(message: felt252) -> Span<felt252> {
    return cheatcode::<'pop_l2_to_l1_message'>(array![message].span());
}
