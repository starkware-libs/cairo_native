use crate::libfuncs::LibfuncHelper;
use crate::{error::Result, metadata::MetadataStorage};
use cairo_lang_sierra::extensions::lib_func::SignatureOnlyConcreteLibfunc;
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        gas_reserve::GasReserveConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::dialect::arith::{self, CmpiPredicate};
use melior::helpers::{ArithBlockExt, BuiltinBlockExt};
use melior::ir::r#type::IntegerType;
use melior::{
    ir::{Block, Location},
    Context,
};

pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &GasReserveConcreteLibfunc,
) -> Result<()> {
    match selector {
        GasReserveConcreteLibfunc::Create(info) => {
            build_gas_reserve_create(context, registry, entry, location, helper, metadata, info)
        }
        GasReserveConcreteLibfunc::Utilize(info) => {
            build_gas_reserve_utilize(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `gas_reserve_create` libfunc.
///
/// # Cairo Signature
///
/// ```cairo
/// pub extern fn gas_reserve_create(
///     amount: u128,
/// ) -> Option<GasReserve> implicits(RangeCheck, GasBuiltin) nopanic;
/// ```
fn build_gas_reserve_create<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let range_check = super::increment_builtin_counter(context, entry, location, entry.arg(0)?)?;
    let current_gas = entry.arg(1)?; // u64
    let amount = entry.arg(2)?; // u128

    let amount_ty = IntegerType::new(context, 128).into();
    let current_gas_128 = entry.append_op_result(arith::extui(current_gas, amount_ty, location))?;
    let enough_gas = entry.cmpi(
        context,
        CmpiPredicate::Uge,
        current_gas_128,
        amount,
        location,
    )?;

    let gas_builtin_ty = IntegerType::new(context, 64).into();
    let spare_gas_128 = entry.append_op_result(arith::subi(current_gas_128, amount, location))?;
    let spare_gas =
        entry.append_op_result(arith::trunci(spare_gas_128, gas_builtin_ty, location))?;

    helper.cond_br(
        context,
        entry,
        enough_gas,
        [0, 1],
        [
            &[range_check, spare_gas, amount],
            &[range_check, current_gas],
        ],
        location,
    )
}

/// Generate MLIR operations for the `gas_reserve_utilize` libfunc.
///
/// # Cairo Signature
///
/// ```cairo
/// pub extern fn gas_reserve_utilize(reserve: GasReserve) implicits(GasBuiltin) nopanic;
/// ```
fn build_gas_reserve_utilize<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let current_gas = entry.arg(0)?; // u64
    let gas_reserve_128 = entry.arg(1)?; // u128

    let gas_reserve = entry.append_op_result(arith::trunci(
        gas_reserve_128,
        IntegerType::new(context, 64).into(),
        location,
    ))?;
    let updated_gas = entry.append_op_result(arith::addi(current_gas, gas_reserve, location))?;

    helper.br(entry, 0, &[updated_gas], location)
}

#[cfg(test)]
mod test {
    use crate::{
        utils::testing::{get_compiled_program, run_program},
        Value,
    };

    #[test]
    fn run_create() {
        let program = get_compiled_program("programs/libfuncs/gas_reserve_create");

        let result = run_program(&program, "run_test_1", &[]).return_value;
        if let Value::Enum { tag, value, .. } = result {
            assert_eq!(tag, 0);
            assert_eq!(value, Box::new(Value::Uint128(100)))
        }

        let gas_amount = 100;
        let result =
            run_program(&program, "run_test_2", &[Value::Uint128(gas_amount)]).return_value;
        if let Value::Enum { tag, value, .. } = result {
            if let Value::Struct { fields, .. } = *value {
                assert_eq!(tag, 0);
                assert_eq!(fields[0], Value::Uint128(gas_amount));
            }
        }

        let gas_amount = 700;
        let result =
            run_program(&program, "run_test_2", &[Value::Uint128(gas_amount)]).return_value;
        if let Value::Enum { tag, value, .. } = result {
            if let Value::Struct { fields, .. } = *value {
                assert_eq!(tag, 0);
                assert_eq!(fields[0], Value::Uint128(gas_amount));
            }
        }
    }

    #[test]
    fn run_utilize() {
        let program = get_compiled_program("programs/libfuncs/gas_reserve_utilize");

        let gas_amount = 10;
        let result = run_program(&program, "run_test", &[Value::Uint128(gas_amount)]).return_value;
        if let Value::Enum { tag, value, .. } = result {
            if let Value::Struct { fields, .. } = *value {
                assert_eq!(tag, 0);
                assert_eq!(fields[0], Value::Uint128(0));
            }
        }

        let gas_amount = 1000;
        let result = run_program(&program, "run_test", &[Value::Uint128(gas_amount)]).return_value;
        if let Value::Enum { tag, value, .. } = result {
            if let Value::Struct { fields, .. } = *value {
                assert_eq!(tag, 0);
                assert_eq!(fields[0], Value::Uint128(0));
            }
        }
    }
}
