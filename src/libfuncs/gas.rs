//! # Gas management libfuncs

use super::LibfuncHelper;
use crate::{
    error::{panic::ToNativeAssertError, Error, Result},
    metadata::{gas::GasCost, runtime_bindings::RuntimeBindingsMeta, MetadataStorage},
    native_panic,
    utils::BuiltinCosts,
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        gas::GasConcreteLibfunc,
        lib_func::SignatureOnlyConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use cairo_lang_sierra_to_casm::environment::gas_wallet::GasWallet;
use itertools::Itertools;
use melior::{
    dialect::{arith::CmpiPredicate, ods},
    helpers::{ArithBlockExt, BuiltinBlockExt, GepIndex, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, Location, Value},
    Context,
};

/// Select and call the correct libfunc builder function from the selector.
pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &GasConcreteLibfunc,
) -> Result<()> {
    match selector {
        GasConcreteLibfunc::WithdrawGas(info) => {
            build_withdraw_gas(context, registry, entry, location, helper, metadata, info)
        }
        GasConcreteLibfunc::RedepositGas(info) => {
            build_redeposit_gas(context, registry, entry, location, helper, metadata, info)
        }
        GasConcreteLibfunc::GetAvailableGas(info) => {
            build_get_available_gas(context, registry, entry, location, helper, metadata, info)
        }
        GasConcreteLibfunc::BuiltinWithdrawGas(info) => {
            build_builtin_withdraw_gas(context, registry, entry, location, helper, metadata, info)
        }
        GasConcreteLibfunc::GetBuiltinCosts(info) => {
            build_get_builtin_costs(context, registry, entry, location, helper, metadata, info)
        }
        GasConcreteLibfunc::GetUnspentGas(info) => {
            build_get_unspent_gas(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `get_available_gas` libfunc.
pub fn build_get_available_gas<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let i128_ty = IntegerType::new(context, 128).into();

    let gas_u128 = entry.extui(entry.arg(0)?, i128_ty, location)?;

    // The gas is returned as u128 on the second arg.
    helper.br(entry, 0, &[entry.arg(0)?, gas_u128], location)
}

/// Generate MLIR operations for the `withdraw_gas` libfunc.
pub fn build_withdraw_gas<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let range_check = super::increment_builtin_counter(context, entry, location, entry.arg(0)?)?;
    let current_gas = entry.arg(1)?;
    let builtin_ptr = {
        let runtime = metadata
            .get_mut::<RuntimeBindingsMeta>()
            .ok_or(Error::MissingMetadata)?;
        runtime
            .get_costs_builtin(context, helper, entry, location)?
            .result(0)?
            .into()
    };

    let gas_cost = metadata
        .get::<GasCost>()
        .to_native_assert_error("withdraw_gas should always have a gas cost")?
        .clone();

    let total_gas_cost_value =
        build_calculate_gas_cost(context, entry, location, gas_cost, builtin_ptr)?;

    let is_enough = entry.cmpi(
        context,
        CmpiPredicate::Uge,
        current_gas,
        total_gas_cost_value,
        location,
    )?;

    let resulting_gas = entry.append_op_result(
        ods::llvm::intr_usub_sat(context, current_gas, total_gas_cost_value, location).into(),
    )?;

    helper.cond_br(
        context,
        entry,
        is_enough,
        [0, 1],
        [&[range_check, resulting_gas], &[range_check, current_gas]],
        location,
    )
}

/// Returns the unused gas to the remaining
///
/// ```cairo
/// extern fn redeposit_gas() implicits(GasBuiltin) nopanic;
/// ```
pub fn build_redeposit_gas<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let current_gas = entry.arg(0)?;

    let gas_cost = metadata
        .get::<GasCost>()
        .to_native_assert_error("redeposit_gas should always have a gas cost")?
        .clone();

    let builtin_ptr = {
        let runtime = metadata
            .get_mut::<RuntimeBindingsMeta>()
            .ok_or(Error::MissingMetadata)?;
        runtime
            .get_costs_builtin(context, helper, entry, location)?
            .result(0)?
            .into()
    };

    let total_gas_cost_value =
        build_calculate_gas_cost(context, entry, location, gas_cost, builtin_ptr)?;

    let resulting_gas = entry.append_op_result(
        ods::llvm::intr_uadd_sat(context, current_gas, total_gas_cost_value, location).into(),
    )?;

    helper.br(entry, 0, &[resulting_gas], location)
}

/// Generate MLIR operations for the `withdraw_gas_all` libfunc.
pub fn build_builtin_withdraw_gas<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let range_check = super::increment_builtin_counter(context, entry, location, entry.arg(0)?)?;
    let current_gas = entry.arg(1)?;
    let builtin_ptr = entry.arg(2)?;

    let gas_cost = metadata
        .get::<GasCost>()
        .to_native_assert_error("builtin_withdraw_gas should always have a gas cost")?
        .clone();

    let total_gas_cost_value =
        build_calculate_gas_cost(context, entry, location, gas_cost, builtin_ptr)?;

    let is_enough = entry.cmpi(
        context,
        CmpiPredicate::Uge,
        current_gas,
        total_gas_cost_value,
        location,
    )?;

    let resulting_gas = entry.append_op_result(
        ods::llvm::intr_usub_sat(context, current_gas, total_gas_cost_value, location).into(),
    )?;

    helper.cond_br(
        context,
        entry,
        is_enough,
        [0, 1],
        [&[range_check, resulting_gas], &[range_check, current_gas]],
        location,
    )
}

/// Generate MLIR operations for the `get_builtin_costs` libfunc.
pub fn build_get_builtin_costs<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    // Get the ptr to the global, holding a ptr to the list.
    let builtin_ptr = {
        let runtime = metadata
            .get_mut::<RuntimeBindingsMeta>()
            .ok_or(Error::MissingMetadata)?;
        runtime
            .get_costs_builtin(context, helper, entry, location)?
            .result(0)?
            .into()
    };

    helper.br(entry, 0, &[builtin_ptr], location)
}

/// Generate MLIR operations for the `get_unspent_gas` libfunc.
///
/// Returns the amount of gas available in the `GasBuiltin`, as well as the
/// amount of gas unused in the local wallet. Useful for asserting that a
/// certain amount of gas was used.
pub fn build_get_unspent_gas<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let current_gas = entry.arg(0)?;

    let gas_wallet = metadata
        .get::<GasWallet>()
        .to_native_assert_error("get_unspent_gas should always have a gas wallet")?
        .clone();

    let unspent_gas = match gas_wallet {
        GasWallet::Value(gas_wallet) => {
            let builtin_ptr = {
                let runtime = metadata
                    .get_mut::<RuntimeBindingsMeta>()
                    .ok_or(Error::MissingMetadata)?;
                runtime
                    .get_costs_builtin(context, helper, entry, location)?
                    .result(0)?
                    .into()
            };
            let gas_cost = GasCost(
                gas_wallet
                    .into_iter()
                    .map(|(token_type, value)| {
                        let Ok(value) = TryInto::<u64>::try_into(value) else {
                            native_panic!("could not cast gas cost from i64 to u64");
                        };
                        Ok((value, token_type))
                    })
                    .try_collect()?,
            );
            let gas_wallet_value =
                build_calculate_gas_cost(context, entry, location, gas_cost, builtin_ptr)?;

            entry.addi(current_gas, gas_wallet_value, location)?
        }
        GasWallet::Disabled => current_gas,
    };

    let unspent_gas = entry.extui(unspent_gas, IntegerType::new(context, 128).into(), location)?;

    helper.br(entry, 0, &[current_gas, unspent_gas], location)
}

/// Calculate the current gas cost, given the constant `GasCost` configuration,
/// and the current `BuiltinCosts` pointer.
pub fn build_calculate_gas_cost<'c, 'b>(
    context: &'c Context,
    block: &'b Block<'c>,
    location: Location<'c>,
    gas_cost: GasCost,
    builtin_ptr: Value<'c, 'b>,
) -> Result<Value<'c, 'b>> {
    let u64_type: melior::ir::Type = IntegerType::new(context, 64).into();

    let mut total_gas_cost = block.const_int_from_type(context, location, 0, u64_type)?;

    // For each gas cost entry
    for (token_count, token_type) in &gas_cost.0 {
        if *token_count == 0 {
            continue;
        }

        let token_count = block.const_int_from_type(context, location, *token_count, u64_type)?;

        // Calculate the index of the token type in the builtin costs array
        let token_costs_index = block.const_int_from_type(
            context,
            location,
            BuiltinCosts::index_for_token_type(token_type)?,
            u64_type,
        )?;

        // Index the builtin costs array
        let token_cost_ptr = block.gep(
            context,
            location,
            builtin_ptr,
            &[GepIndex::Value(token_costs_index)],
            u64_type,
        )?;
        let token_cost = block.load(context, location, token_cost_ptr, u64_type)?;

        // Multiply the number of tokens by the cost of each token
        let gas_cost = block.muli(token_count, token_cost, location)?;

        total_gas_cost = block.addi(total_gas_cost, gas_cost, location)?;
    }

    Ok(total_gas_cost)
}

#[cfg(test)]
mod test {
    use crate::{
        utils::testing::{get_compiled_program, run_program},
        Value,
    };

    #[test]
    fn run_withdraw_gas() {
        let program = get_compiled_program("programs/libfuncs/gas_withdraw");

        let result = run_program(&program, "run_test", &[]);
        assert_eq!(result.remaining_gas, Some(18446744073709533495));
    }

    #[test]
    fn run_get_unspent_gas() {
        #[rustfmt::skip]
        let program = get_compiled_program("programs/libfuncs/get_unspent_gas");

        let result = run_program(&program, "run_test", &[]);
        assert_eq!(
            result.return_value,
            Value::Enum {
                tag: 0,
                value: Box::new(Value::Struct {
                    fields: vec![Value::Uint128(3600)],
                    debug_name: None
                }),
                debug_name: None,
            }
        );
    }
}
