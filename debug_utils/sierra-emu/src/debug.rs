use cairo_lang_sierra::{
    extensions::{
        array::ArrayConcreteLibfunc,
        boolean::BoolConcreteLibfunc,
        bounded_int::BoundedIntConcreteLibfunc,
        boxing::BoxConcreteLibfunc,
        bytes31::Bytes31ConcreteLibfunc,
        casts::CastConcreteLibfunc,
        circuit::{CircuitConcreteLibfunc, CircuitTypeConcrete},
        const_type::ConstConcreteLibfunc,
        core::{CoreConcreteLibfunc, CoreLibfunc, CoreType, CoreTypeConcrete},
        coupon::CouponConcreteLibfunc,
        debug::DebugConcreteLibfunc,
        ec::EcConcreteLibfunc,
        enm::EnumConcreteLibfunc,
        felt252::{Felt252BinaryOperationConcrete, Felt252BinaryOperator, Felt252Concrete},
        felt252_dict::{Felt252DictConcreteLibfunc, Felt252DictEntryConcreteLibfunc},
        gas::GasConcreteLibfunc,
        gas_reserve::GasReserveConcreteLibfunc,
        int::{
            signed::SintConcrete, signed128::Sint128Concrete, unsigned::UintConcrete,
            unsigned128::Uint128Concrete, unsigned256::Uint256Concrete,
            unsigned512::Uint512Concrete, IntOperator,
        },
        lib_func::{BranchSignature, ParamSignature},
        mem::MemConcreteLibfunc,
        nullable::NullableConcreteLibfunc,
        pedersen::PedersenConcreteLibfunc,
        poseidon::PoseidonConcreteLibfunc,
        range::IntRangeConcreteLibfunc,
        starknet::{
            secp256::{Secp256ConcreteLibfunc, Secp256OpConcreteLibfunc},
            testing::TestingConcreteLibfunc,
            StarknetConcreteLibfunc, StarknetTypeConcrete,
        },
        structure::StructConcreteLibfunc,
    },
    ids::ConcreteTypeId,
    program_registry::ProgramRegistry,
};

use crate::Value;

pub fn libfunc_to_name(value: &CoreConcreteLibfunc) -> &'static str {
    match value {
        CoreConcreteLibfunc::ApTracking(value) => match value {
            cairo_lang_sierra::extensions::ap_tracking::ApTrackingConcreteLibfunc::Revoke(_) => {
                "revoke_ap_tracking"
            }
            cairo_lang_sierra::extensions::ap_tracking::ApTrackingConcreteLibfunc::Enable(_) => {
                "enable_ap_tracking"
            }
            cairo_lang_sierra::extensions::ap_tracking::ApTrackingConcreteLibfunc::Disable(_) => {
                "disable_ap_tracking"
            }
        },
        CoreConcreteLibfunc::Array(value) => match value {
            ArrayConcreteLibfunc::New(_) => "array_new",
            ArrayConcreteLibfunc::SpanFromTuple(_) => "span_from_tuple",
            ArrayConcreteLibfunc::Append(_) => "array_append",
            ArrayConcreteLibfunc::PopFront(_) => "array_pop_front",
            ArrayConcreteLibfunc::PopFrontConsume(_) => "array_pop_front_consume",
            ArrayConcreteLibfunc::Get(_) => "array_get",
            ArrayConcreteLibfunc::Slice(_) => "array_slice",
            ArrayConcreteLibfunc::Len(_) => "array_len",
            ArrayConcreteLibfunc::SnapshotPopFront(_) => "array_snapshot_pop_front",
            ArrayConcreteLibfunc::SnapshotPopBack(_) => "array_snapshot_pop_back",
            ArrayConcreteLibfunc::TupleFromSpan(_) => "array_tuple_from_span",
            ArrayConcreteLibfunc::SnapshotMultiPopFront(_) => "array_snapshot_multi_pop_front",
            ArrayConcreteLibfunc::SnapshotMultiPopBack(_) => "array_snapshot_multi_pop_back",
        },
        CoreConcreteLibfunc::BranchAlign(_) => "branch_align",
        CoreConcreteLibfunc::Bool(value) => match value {
            BoolConcreteLibfunc::And(_) => "bool_and",
            BoolConcreteLibfunc::Not(_) => "bool_not",
            BoolConcreteLibfunc::Xor(_) => "bool_xor",
            BoolConcreteLibfunc::Or(_) => "bool_or",
            BoolConcreteLibfunc::ToFelt252(_) => "bool_to_felt252",
        },
        CoreConcreteLibfunc::Box(value) => match value {
            BoxConcreteLibfunc::Into(_) => "box_into",
            BoxConcreteLibfunc::Unbox(_) => "box_unbox",
            BoxConcreteLibfunc::ForwardSnapshot(_) => "box_forward_snapshot",
            BoxConcreteLibfunc::LocalInto(_) => "box_local_into",
        },
        CoreConcreteLibfunc::Cast(value) => match value {
            CastConcreteLibfunc::Downcast(_) => "downcast",
            CastConcreteLibfunc::Upcast(_) => "upcast",
        },
        CoreConcreteLibfunc::Coupon(value) => match value {
            CouponConcreteLibfunc::Buy(_) => "coupon_buy",
            CouponConcreteLibfunc::Refund(_) => "coupon_refund",
        },
        CoreConcreteLibfunc::CouponCall(_) => "coupon_call",
        CoreConcreteLibfunc::Drop(_) => "drop",
        CoreConcreteLibfunc::Dup(_) => "dup",
        CoreConcreteLibfunc::Ec(value) => match value {
            EcConcreteLibfunc::IsZero(_) => "ec_is_zero",
            EcConcreteLibfunc::Neg(_) => "ec_neg",
            EcConcreteLibfunc::StateAdd(_) => "ec_state_add",
            EcConcreteLibfunc::TryNew(_) => "ec_try_new",
            EcConcreteLibfunc::StateFinalize(_) => "ec_state_finalize",
            EcConcreteLibfunc::StateInit(_) => "ec_state_init",
            EcConcreteLibfunc::StateAddMul(_) => "ec_state_add_mul",
            EcConcreteLibfunc::PointFromX(_) => "ec_point_from_x",
            EcConcreteLibfunc::UnwrapPoint(_) => "ec_unwrap_point",
            EcConcreteLibfunc::Zero(_) => "ec_zero",
            EcConcreteLibfunc::NegNz(_) => "ec_neg_nz",
        },
        CoreConcreteLibfunc::Felt252(value) => match value {
            Felt252Concrete::BinaryOperation(op) => match op {
                Felt252BinaryOperationConcrete::WithVar(op) => match &op.operator {
                    Felt252BinaryOperator::Add => "felt252_add",
                    Felt252BinaryOperator::Sub => "felt252_sub",
                    Felt252BinaryOperator::Mul => "felt252_mul",
                    Felt252BinaryOperator::Div => "felt252_div",
                },
                Felt252BinaryOperationConcrete::WithConst(op) => match &op.operator {
                    Felt252BinaryOperator::Add => "felt252_const_add",
                    Felt252BinaryOperator::Sub => "felt252_const_sub",
                    Felt252BinaryOperator::Mul => "felt252_const_mul",
                    Felt252BinaryOperator::Div => "felt252_const_div",
                },
            },
            Felt252Concrete::Const(_) => "felt252_const",
            Felt252Concrete::IsZero(_) => "felt252_is_zero",
        },
        CoreConcreteLibfunc::Const(value) => match value {
            ConstConcreteLibfunc::AsBox(_) => "const_as_box",
            ConstConcreteLibfunc::AsImmediate(_) => "const_as_immediate",
        },
        CoreConcreteLibfunc::FunctionCall(_) => "function_call",
        CoreConcreteLibfunc::Gas(value) => match value {
            GasConcreteLibfunc::WithdrawGas(_) => "withdraw_gas",
            GasConcreteLibfunc::RedepositGas(_) => "redeposit_gas",
            GasConcreteLibfunc::GetAvailableGas(_) => "get_available_gas",
            GasConcreteLibfunc::BuiltinWithdrawGas(_) => "builtin_withdraw_gas",
            GasConcreteLibfunc::GetBuiltinCosts(_) => "get_builtin_costs",
            GasConcreteLibfunc::GetUnspentGas(_) => todo!(),
        },
        CoreConcreteLibfunc::Uint8(value) => match value {
            UintConcrete::Const(_) => "u8_const",
            UintConcrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "u8_overflowing_add",
                IntOperator::OverflowingSub => "u8_overflowing_sub",
            },
            UintConcrete::SquareRoot(_) => "u8_sqrt",
            UintConcrete::Equal(_) => "u8_eq",
            UintConcrete::ToFelt252(_) => "u8_to_felt252",
            UintConcrete::FromFelt252(_) => "u8_from_felt252",
            UintConcrete::IsZero(_) => "u8_is_zero",
            UintConcrete::Divmod(_) => "u8_divmod",
            UintConcrete::WideMul(_) => "u8_wide_mul",
            UintConcrete::Bitwise(_) => "u8_bitwise",
        },
        CoreConcreteLibfunc::Uint16(value) => match value {
            UintConcrete::Const(_) => "u16_const",
            UintConcrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "u16_overflowing_add",
                IntOperator::OverflowingSub => "u16_overflowing_sub",
            },
            UintConcrete::SquareRoot(_) => "u16_sqrt",
            UintConcrete::Equal(_) => "u16_eq",
            UintConcrete::ToFelt252(_) => "u16_to_felt252",
            UintConcrete::FromFelt252(_) => "u16_from_felt252",
            UintConcrete::IsZero(_) => "u16_is_zero",
            UintConcrete::Divmod(_) => "u16_divmod",
            UintConcrete::WideMul(_) => "u16_wide_mul",
            UintConcrete::Bitwise(_) => "u16_bitwise",
        },
        CoreConcreteLibfunc::Uint32(value) => match value {
            UintConcrete::Const(_) => "u32_const",
            UintConcrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "u32_overflowing_add",
                IntOperator::OverflowingSub => "u32_overflowing_sub",
            },
            UintConcrete::SquareRoot(_) => "u32_sqrt",
            UintConcrete::Equal(_) => "u32_eq",
            UintConcrete::ToFelt252(_) => "u32_to_felt252",
            UintConcrete::FromFelt252(_) => "u32_from_felt252",
            UintConcrete::IsZero(_) => "u32_is_zero",
            UintConcrete::Divmod(_) => "u32_divmod",
            UintConcrete::WideMul(_) => "u32_wide_mul",
            UintConcrete::Bitwise(_) => "u32_bitwise",
        },
        CoreConcreteLibfunc::Uint64(value) => match value {
            UintConcrete::Const(_) => "u64_const",
            UintConcrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "u64_overflowing_add",
                IntOperator::OverflowingSub => "u64_overflowing_sub",
            },
            UintConcrete::SquareRoot(_) => "u64_sqrt",
            UintConcrete::Equal(_) => "u64_eq",
            UintConcrete::ToFelt252(_) => "u64_to_felt252",
            UintConcrete::FromFelt252(_) => "u64_from_felt252",
            UintConcrete::IsZero(_) => "u64_is_zero",
            UintConcrete::Divmod(_) => "u64_divmod",
            UintConcrete::WideMul(_) => "u64_wide_mul",
            UintConcrete::Bitwise(_) => "u64_bitwise",
        },
        CoreConcreteLibfunc::Uint128(value) => match value {
            Uint128Concrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "u128_overflowing_add",
                IntOperator::OverflowingSub => "u128_overflowing_sub",
            },
            Uint128Concrete::Divmod(_) => "u128_divmod",
            Uint128Concrete::GuaranteeMul(_) => "u128_guarantee_mul",
            Uint128Concrete::MulGuaranteeVerify(_) => "u128_mul_guarantee_verify",
            Uint128Concrete::Equal(_) => "u128_equal",
            Uint128Concrete::SquareRoot(_) => "u128_sqrt",
            Uint128Concrete::Const(_) => "u128_const",
            Uint128Concrete::FromFelt252(_) => "u128_from_felt",
            Uint128Concrete::ToFelt252(_) => "u128_to_felt252",
            Uint128Concrete::IsZero(_) => "u128_is_zero",
            Uint128Concrete::Bitwise(_) => "u128_bitwise",
            Uint128Concrete::ByteReverse(_) => "u128_byte_reverse",
        },
        CoreConcreteLibfunc::Uint256(value) => match value {
            Uint256Concrete::IsZero(_) => "u256_is_zero",
            Uint256Concrete::Divmod(_) => "u256_divmod",
            Uint256Concrete::SquareRoot(_) => "u256_sqrt",
            Uint256Concrete::InvModN(_) => "u256_inv_mod_n",
        },
        CoreConcreteLibfunc::Uint512(value) => match value {
            Uint512Concrete::DivModU256(_) => "u512_divmod_u256",
        },
        CoreConcreteLibfunc::Sint8(value) => match value {
            SintConcrete::Const(_) => "i8_const",
            SintConcrete::Equal(_) => "i8_eq",
            SintConcrete::ToFelt252(_) => "i8_to_felt252",
            SintConcrete::FromFelt252(_) => "i8_from_felt252",
            SintConcrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "i8_overflowing_add",
                IntOperator::OverflowingSub => "i8_overflowing_sub",
            },
            SintConcrete::Diff(_) => "i8_diff",
            SintConcrete::WideMul(_) => "i8_wide_mul",
        },
        CoreConcreteLibfunc::Sint16(value) => match value {
            SintConcrete::Const(_) => "i16_const",
            SintConcrete::Equal(_) => "i16_eq",
            SintConcrete::ToFelt252(_) => "i16_to_felt252",
            SintConcrete::FromFelt252(_) => "i16_from_felt252",
            SintConcrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "i16_overflowing_add",
                IntOperator::OverflowingSub => "i16_overflowing_sub",
            },
            SintConcrete::Diff(_) => "i16_diff",
            SintConcrete::WideMul(_) => "i16_wide_mul",
        },
        CoreConcreteLibfunc::Sint32(value) => match value {
            SintConcrete::Const(_) => "i32_const",
            SintConcrete::Equal(_) => "i32_eq",
            SintConcrete::ToFelt252(_) => "i32_to_felt252",
            SintConcrete::FromFelt252(_) => "i32_from_felt252",
            SintConcrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "i32_overflowing_add",
                IntOperator::OverflowingSub => "i32_overflowing_sub",
            },
            SintConcrete::Diff(_) => "i32_diff",
            SintConcrete::WideMul(_) => "i32_wide_mul",
        },
        CoreConcreteLibfunc::Sint64(value) => match value {
            SintConcrete::Const(_) => "i64_const",
            SintConcrete::Equal(_) => "i64_eq",
            SintConcrete::ToFelt252(_) => "i64_to_felt252",
            SintConcrete::FromFelt252(_) => "i64_from_felt252",
            SintConcrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "i64_overflowing_add",
                IntOperator::OverflowingSub => "i64_overflowing_sub",
            },
            SintConcrete::Diff(_) => "i64_diff",
            SintConcrete::WideMul(_) => "i64_wide_mul",
        },
        CoreConcreteLibfunc::Sint128(value) => match value {
            Sint128Concrete::Const(_) => "i128_const",
            Sint128Concrete::Equal(_) => "i128_eq",
            Sint128Concrete::ToFelt252(_) => "i128_to_felt252",
            Sint128Concrete::FromFelt252(_) => "i128_from_felt252",
            Sint128Concrete::Operation(op) => match &op.operator {
                IntOperator::OverflowingAdd => "i128_overflowing_add",
                IntOperator::OverflowingSub => "i128_overflowing_sub",
            },
            Sint128Concrete::Diff(_) => "i128_diff",
        },
        CoreConcreteLibfunc::Mem(value) => match value {
            MemConcreteLibfunc::StoreTemp(_) => "store_temp",
            MemConcreteLibfunc::StoreLocal(_) => "store_local",
            MemConcreteLibfunc::FinalizeLocals(_) => "finalize_locals",
            MemConcreteLibfunc::AllocLocal(_) => "alloc_local",
            MemConcreteLibfunc::Rename(_) => "rename",
        },
        CoreConcreteLibfunc::Nullable(value) => match value {
            NullableConcreteLibfunc::Null(_) => "nullable_null",
            NullableConcreteLibfunc::NullableFromBox(_) => "nullable_from_box",
            NullableConcreteLibfunc::MatchNullable(_) => "match_nullable",
            NullableConcreteLibfunc::ForwardSnapshot(_) => "nullable_forward_snapshot",
        },
        CoreConcreteLibfunc::UnwrapNonZero(_) => "unwrap_non_zero",
        CoreConcreteLibfunc::UnconditionalJump(_) => "jump",
        CoreConcreteLibfunc::Enum(value) => match value {
            EnumConcreteLibfunc::Init(_) => "enum_init",
            EnumConcreteLibfunc::FromBoundedInt(_) => "enum_from_bounded_int",
            EnumConcreteLibfunc::Match(_) => "enum_match",
            EnumConcreteLibfunc::SnapshotMatch(_) => "enum_snapshot_match",
            EnumConcreteLibfunc::BoxedMatch(_) => "enum_boxed_match",
        },
        CoreConcreteLibfunc::Struct(value) => match value {
            StructConcreteLibfunc::Construct(_) => "struct_construct",
            StructConcreteLibfunc::Deconstruct(_) => "struct_deconstruct",
            StructConcreteLibfunc::SnapshotDeconstruct(_) => "struct_snapshot_deconstruct",
            StructConcreteLibfunc::BoxedDeconstruct(_) => "struct_boxed_deconstruct",
        },
        CoreConcreteLibfunc::Felt252Dict(value) => match value {
            Felt252DictConcreteLibfunc::New(_) => "felt252dict_new",
            Felt252DictConcreteLibfunc::Squash(_) => "felt252dict_squash",
        },
        CoreConcreteLibfunc::Felt252DictEntry(value) => match value {
            Felt252DictEntryConcreteLibfunc::Get(_) => "felt252dict_get",
            Felt252DictEntryConcreteLibfunc::Finalize(_) => "felt252dict_finalize",
        },
        CoreConcreteLibfunc::Pedersen(value) => match value {
            PedersenConcreteLibfunc::PedersenHash(_) => "pedersen_hash",
        },
        CoreConcreteLibfunc::Poseidon(value) => match value {
            PoseidonConcreteLibfunc::HadesPermutation(_) => "hades_permutation",
        },
        CoreConcreteLibfunc::Starknet(value) => match value {
            StarknetConcreteLibfunc::CallContract(_) => "call_contract",
            StarknetConcreteLibfunc::ClassHashConst(_) => "class_hash_const",
            StarknetConcreteLibfunc::ClassHashTryFromFelt252(_) => "class_hash_try_from_felt252",
            StarknetConcreteLibfunc::ClassHashToFelt252(_) => "class_hash_to_felt252",
            StarknetConcreteLibfunc::ContractAddressConst(_) => "contract_address_const",
            StarknetConcreteLibfunc::ContractAddressTryFromFelt252(_) => {
                "contract_address_try_from_felt252"
            }
            StarknetConcreteLibfunc::ContractAddressToFelt252(_) => "contract_address_to_felt252",
            StarknetConcreteLibfunc::StorageRead(_) => "storage_read",
            StarknetConcreteLibfunc::StorageWrite(_) => "storage_write",
            StarknetConcreteLibfunc::StorageBaseAddressConst(_) => "storage_base_address_const",
            StarknetConcreteLibfunc::StorageBaseAddressFromFelt252(_) => {
                "storage_base_address_from_felt252"
            }
            StarknetConcreteLibfunc::StorageAddressFromBase(_) => "storage_address_from_base",
            StarknetConcreteLibfunc::StorageAddressFromBaseAndOffset(_) => {
                "storage_address_from_base_and_offset"
            }
            StarknetConcreteLibfunc::StorageAddressToFelt252(_) => "storage_address_to_felt252",
            StarknetConcreteLibfunc::StorageAddressTryFromFelt252(_) => {
                "storage_address_try_from_felt252"
            }
            StarknetConcreteLibfunc::EmitEvent(_) => "emit_event",
            StarknetConcreteLibfunc::GetBlockHash(_) => "get_block_hash",
            StarknetConcreteLibfunc::GetExecutionInfo(_) => "get_exec_info_v1",
            StarknetConcreteLibfunc::GetExecutionInfoV2(_) => "get_exec_info_v2",
            StarknetConcreteLibfunc::Deploy(_) => "deploy",
            StarknetConcreteLibfunc::DeployV2(_) => "deploy_v2",
            StarknetConcreteLibfunc::Keccak(_) => "keccak",
            StarknetConcreteLibfunc::LibraryCall(_) => "library_call",
            StarknetConcreteLibfunc::ReplaceClass(_) => "replace_class",
            StarknetConcreteLibfunc::SendMessageToL1(_) => "send_message_to_l1",
            StarknetConcreteLibfunc::Testing(value) => match value {
                TestingConcreteLibfunc::Cheatcode(_) => "cheatcode",
            },
            StarknetConcreteLibfunc::Secp256(value) => match value {
                Secp256ConcreteLibfunc::K1(value) => match value {
                    Secp256OpConcreteLibfunc::New(_) => "secp256k1_new",
                    Secp256OpConcreteLibfunc::Add(_) => "secp256k1_add",
                    Secp256OpConcreteLibfunc::Mul(_) => "secp256k1_mul",
                    Secp256OpConcreteLibfunc::GetPointFromX(_) => "secp256k1_get_point_from_x",
                    Secp256OpConcreteLibfunc::GetXy(_) => "secp256k1_get_xy",
                },
                Secp256ConcreteLibfunc::R1(value) => match value {
                    Secp256OpConcreteLibfunc::New(_) => "secp256r1_new",
                    Secp256OpConcreteLibfunc::Add(_) => "secp256r1_add",
                    Secp256OpConcreteLibfunc::Mul(_) => "secp256r1_mul",
                    Secp256OpConcreteLibfunc::GetPointFromX(_) => "secp256r1_get_point_from_x",
                    Secp256OpConcreteLibfunc::GetXy(_) => "secp256r1_get_xy",
                },
            },
            StarknetConcreteLibfunc::Sha256ProcessBlock(_) => "sha256_process_block",
            StarknetConcreteLibfunc::Sha256StateHandleInit(_) => "sha256_state_handle_init",
            StarknetConcreteLibfunc::Sha256StateHandleDigest(_) => "sha256_state_handle_digest",
            StarknetConcreteLibfunc::Sha512ProcessBlock(_) => "sha512_process_block",
            StarknetConcreteLibfunc::Sha512StateHandleInit(_) => "sha512_state_handle_init",
            StarknetConcreteLibfunc::Sha512StateHandleDigest(_) => "sha512_state_handle_digest",
            StarknetConcreteLibfunc::GetClassHashAt(_) => "get_class_hash_at",
            StarknetConcreteLibfunc::MetaTxV0(_) => todo!(),
            StarknetConcreteLibfunc::GetExecutionInfoV3(_) => todo!(),
        },
        CoreConcreteLibfunc::Debug(value) => match value {
            DebugConcreteLibfunc::Print(_) => "debug_print",
        },
        CoreConcreteLibfunc::SnapshotTake(_) => "snapshot_take",
        CoreConcreteLibfunc::Bytes31(value) => match value {
            Bytes31ConcreteLibfunc::Const(_) => "bytes31_const",
            Bytes31ConcreteLibfunc::ToFelt252(_) => "bytes31_to_felt252",
            Bytes31ConcreteLibfunc::TryFromFelt252(_) => "bytes31_try_from_felt252",
        },
        CoreConcreteLibfunc::Circuit(selector) => match selector {
            CircuitConcreteLibfunc::AddInput(_) => "circuit_add_input",
            CircuitConcreteLibfunc::Eval(_) => "circuit_eval",
            CircuitConcreteLibfunc::GetDescriptor(_) => "circuit_get_descriptor",
            CircuitConcreteLibfunc::InitCircuitData(_) => "circuit_init_circuit_data",
            CircuitConcreteLibfunc::GetOutput(_) => "circuit_get_output",
            CircuitConcreteLibfunc::TryIntoCircuitModulus(_) => "circuit_try_into_circuit_modulus",
            CircuitConcreteLibfunc::FailureGuaranteeVerify(_) => "circuit_failure_guarantee_verify",
            CircuitConcreteLibfunc::IntoU96Guarantee(_) => "circuit_into_u96_guarantee",
            CircuitConcreteLibfunc::U96GuaranteeVerify(_) => "circuit_u96_guarantee_verify",
            CircuitConcreteLibfunc::U96LimbsLessThanGuaranteeVerify(_) => {
                "circuit_u96_limbs_less_than_guarantee_verify"
            }
            CircuitConcreteLibfunc::U96SingleLimbLessThanGuaranteeVerify(_) => {
                "circuit_u96_single_limb_less_than_guarantee_verify"
            }
        },
        CoreConcreteLibfunc::BoundedInt(selector) => match selector {
            BoundedIntConcreteLibfunc::Add(_) => "bounded_int_add",
            BoundedIntConcreteLibfunc::Sub(_) => "bounded_int_sub",
            BoundedIntConcreteLibfunc::Mul(_) => "bounded_int_mul",
            BoundedIntConcreteLibfunc::DivRem(_) => "bounded_int_div_rem",
            BoundedIntConcreteLibfunc::Constrain(_) => "bounded_int_constrain",
            BoundedIntConcreteLibfunc::IsZero(_) => "bounded_int_is_zero",
            BoundedIntConcreteLibfunc::WrapNonZero(_) => "bounded_int_wrap_non_zero",
            BoundedIntConcreteLibfunc::TrimMin(_) => "bounded_int_trim_min",
            BoundedIntConcreteLibfunc::TrimMax(_) => "bounded_int_trim_max",
            BoundedIntConcreteLibfunc::GuaranteeVerify(_) => "bounded_int_guarantee_verify",
            BoundedIntConcreteLibfunc::U128ToU32Guarantees(_) => "u128_to_u32_guarantees",
        },
        CoreConcreteLibfunc::IntRange(selector) => match selector {
            IntRangeConcreteLibfunc::TryNew(_) => "int_range_try_new",
            IntRangeConcreteLibfunc::PopFront(_) => "int_range_pop_front",
        },
        CoreConcreteLibfunc::Blake(_) => todo!(),
        CoreConcreteLibfunc::Felt252SquashedDict(_) => todo!(),
        CoreConcreteLibfunc::Trace(_) => todo!(),
        CoreConcreteLibfunc::QM31(_) => todo!(),
        CoreConcreteLibfunc::UnsafePanic(_) => todo!(),
        CoreConcreteLibfunc::DummyFunctionCall(_) => "dummy_function_call",
        CoreConcreteLibfunc::GasReserve(selector) => match selector {
            GasReserveConcreteLibfunc::Create(_) => "gas_reserve_create",
            GasReserveConcreteLibfunc::Utilize(_) => "gas_reserve_utilize",
        },
    }
}

pub fn type_to_name(
    ty_id: &ConcreteTypeId,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
) -> String {
    let ty = registry.get_type(ty_id).unwrap();
    match ty {
        CoreTypeConcrete::Array(info) => {
            format!("Array<{}>", type_to_name(&info.ty, registry))
        }
        CoreTypeConcrete::Box(info) => {
            format!("Box<{}>", type_to_name(&info.ty, registry))
        }
        CoreTypeConcrete::Uninitialized(info) => {
            format!("Uninitialized<{}>", type_to_name(&info.ty, registry))
        }
        CoreTypeConcrete::NonZero(info) => {
            format!("NonZero<{}>", type_to_name(&info.ty, registry))
        }
        CoreTypeConcrete::Nullable(info) => {
            format!("Nullable<{}>", type_to_name(&info.ty, registry))
        }
        CoreTypeConcrete::Span(info) => {
            format!("Span<{}>", type_to_name(&info.ty, registry))
        }
        CoreTypeConcrete::Snapshot(info) => {
            format!("Snapshot<{}>", type_to_name(&info.ty, registry))
        }
        CoreTypeConcrete::Struct(info) => {
            let fields = info
                .members
                .iter()
                .map(|ty_id| type_to_name(ty_id, registry))
                .collect::<Vec<_>>();
            let fields = fields.join(", ");

            format!("Struct<{}>", fields)
        }
        CoreTypeConcrete::Enum(info) => {
            let fields = info
                .variants
                .iter()
                .map(|ty_id| type_to_name(ty_id, registry))
                .collect::<Vec<_>>();
            let fields = fields.join(", ");

            format!("Enum<{}>", fields)
        }
        CoreTypeConcrete::Felt252Dict(_) => String::from("Felt252Dict"),
        CoreTypeConcrete::Felt252DictEntry(_) => String::from("Felt252DictEntry"),
        CoreTypeConcrete::SquashedFelt252Dict(_) => String::from("SquashedFelt252Dict"),
        CoreTypeConcrete::Starknet(selector) => match selector {
            StarknetTypeConcrete::ClassHash(_) => String::from("Starknet::ClassHash"),
            StarknetTypeConcrete::ContractAddress(_) => String::from("Starknet::ContractAddress"),
            StarknetTypeConcrete::StorageBaseAddress(_) => {
                String::from("Starknet::StorageBaseAddress")
            }
            StarknetTypeConcrete::StorageAddress(_) => String::from("Starknet::StorageAddress"),
            StarknetTypeConcrete::System(_) => String::from("Starknet::System"),
            StarknetTypeConcrete::Secp256Point(_) => String::from("Starknet::Secp256Point"),
            StarknetTypeConcrete::Sha256StateHandle(_) => {
                String::from("Starknet::Sha256StateHandle")
            }
            StarknetTypeConcrete::Sha512StateHandle(_) => {
                String::from("Starknet::Sha512StateHandle")
            }
        },
        CoreTypeConcrete::Bitwise(_) => String::from("Bitwise"),
        CoreTypeConcrete::Circuit(selector) => match selector {
            CircuitTypeConcrete::AddMod(_) => String::from("AddMod"),
            CircuitTypeConcrete::MulMod(_) => String::from("MulMod"),
            CircuitTypeConcrete::AddModGate(_) => String::from("AddModGate"),
            CircuitTypeConcrete::Circuit(_) => String::from("Circuit"),
            CircuitTypeConcrete::CircuitData(_) => String::from("CircuitData"),
            CircuitTypeConcrete::CircuitOutputs(_) => String::from("CircuitOutputs"),
            CircuitTypeConcrete::CircuitPartialOutputs(_) => String::from("CircuitPartialOutputs"),
            CircuitTypeConcrete::CircuitDescriptor(_) => String::from("CircuitDescriptor"),
            CircuitTypeConcrete::CircuitFailureGuarantee(_) => {
                String::from("CircuitFailureGuarantee")
            }
            CircuitTypeConcrete::CircuitInput(_) => String::from("CircuitInput"),
            CircuitTypeConcrete::CircuitInputAccumulator(_) => {
                String::from("CircuitInputAccumulator")
            }
            CircuitTypeConcrete::CircuitModulus(_) => String::from("CircuitModulus"),
            CircuitTypeConcrete::InverseGate(_) => String::from("InverseGate"),
            CircuitTypeConcrete::MulModGate(_) => String::from("MulModGate"),
            CircuitTypeConcrete::SubModGate(_) => String::from("SubModGate"),
            CircuitTypeConcrete::U96Guarantee(_) => String::from("U96Guarantee"),
            CircuitTypeConcrete::U96LimbsLessThanGuarantee(_) => {
                String::from("U96LimbsLessThanGuarantee")
            }
        },
        CoreTypeConcrete::Const(_) => String::from("Const"),
        CoreTypeConcrete::Coupon(_) => String::from("Coupon"),
        CoreTypeConcrete::EcOp(_) => String::from("EcOp"),
        CoreTypeConcrete::EcPoint(_) => String::from("EcPoint"),
        CoreTypeConcrete::EcState(_) => String::from("EcState"),
        CoreTypeConcrete::Felt252(_) => String::from("Felt252"),
        CoreTypeConcrete::GasBuiltin(_) => String::from("GasBuiltin"),
        CoreTypeConcrete::BuiltinCosts(_) => String::from("BuiltinCosts"),
        CoreTypeConcrete::Uint8(_) => String::from("Uint8"),
        CoreTypeConcrete::Uint16(_) => String::from("Uint16"),
        CoreTypeConcrete::Uint32(_) => String::from("Uint32"),
        CoreTypeConcrete::Uint64(_) => String::from("Uint64"),
        CoreTypeConcrete::Uint128(_) => String::from("Uint128"),
        CoreTypeConcrete::Uint128MulGuarantee(_) => String::from("Uint128MulGuarantee"),
        CoreTypeConcrete::Sint8(_) => String::from("Sint8"),
        CoreTypeConcrete::Sint16(_) => String::from("Sint16"),
        CoreTypeConcrete::Sint32(_) => String::from("Sint32"),
        CoreTypeConcrete::Sint64(_) => String::from("Sint64"),
        CoreTypeConcrete::Sint128(_) => String::from("Sint128"),
        CoreTypeConcrete::RangeCheck(_) => String::from("RangeCheck"),
        CoreTypeConcrete::RangeCheck96(_) => String::from("RangeCheck96"),
        CoreTypeConcrete::Pedersen(_) => String::from("Pedersen"),
        CoreTypeConcrete::Poseidon(_) => String::from("Poseidon"),
        CoreTypeConcrete::SegmentArena(_) => String::from("SegmentArena"),
        CoreTypeConcrete::Bytes31(_) => String::from("Bytes31"),
        CoreTypeConcrete::BoundedInt(_) => String::from("BoundedInt"),
        CoreTypeConcrete::BoundedIntGuarantee(_) => String::from("BoundedIntGuarantee"),
        CoreTypeConcrete::IntRange(info) => {
            format!("IntRange<{}>", type_to_name(&info.ty, registry))
        }
        CoreTypeConcrete::Blake(_) => todo!(),
        CoreTypeConcrete::QM31(_) => todo!(),
        CoreTypeConcrete::GasReserve(_) => String::from("GasReserve"),
    }
}

/// prints all the signature information, used while debugging to learn
/// how to implement a certain libfunc.
#[allow(dead_code)]
pub fn debug_signature(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    params: &[ParamSignature],
    branches: &[BranchSignature],
    args: &[Value],
) {
    println!(
        "Params: {:#?}",
        params
            .iter()
            .map(|p| type_to_name(&p.ty, registry))
            .collect::<Vec<_>>()
    );
    println!(
        "Branches: {:#?}",
        branches
            .iter()
            .map(|b| {
                b.vars
                    .iter()
                    .map(|vars| type_to_name(&vars.ty, registry))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    );
    println!("Args: {:#?}", args);
}
