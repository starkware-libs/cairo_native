//! # `Felt`-related libfuncs

use super::LibfuncHelper;
use crate::{
    error::{panic::ToNativeAssertError, Result},
    metadata::{runtime_bindings::RuntimeBindingsMeta, MetadataStorage},
    utils::{felt_to_unsigned, ProgramRegistryExt, PRIME},
};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        felt252::{
            Felt252BinaryOperationConcrete, Felt252BinaryOperator, Felt252Concrete,
            Felt252ConstConcreteLibfunc,
        },
        lib_func::SignatureOnlyConcreteLibfunc,
        ConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::arith::{self, CmpiPredicate},
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, Location, Value, ValueLike},
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
    selector: &Felt252Concrete,
) -> Result<()> {
    match selector {
        Felt252Concrete::BinaryOperation(info) => {
            build_binary_operation(context, registry, entry, location, helper, metadata, info)
        }
        Felt252Concrete::Const(info) => {
            build_const(context, registry, entry, location, helper, metadata, info)
        }
        Felt252Concrete::IsZero(info) => {
            build_is_zero(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the following libfuncs:
///   - `felt252_add` and `felt252_add_const`.
///   - `felt252_sub` and `felt252_sub_const`.
///   - `felt252_mul` and `felt252_mul_const`.
///   - `felt252_div` and `felt252_div_const`.
pub fn build_binary_operation<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &Felt252BinaryOperationConcrete,
) -> Result<()> {
    let felt252_ty = registry.build_type(
        context,
        helper,
        metadata,
        &info.branch_signatures()[0].vars[0].ty,
    )?;
    let i256 = IntegerType::new(context, 256).into();
    let i512 = IntegerType::new(context, 512).into();

    let (op, lhs, rhs) = match info {
        Felt252BinaryOperationConcrete::WithVar(operation) => {
            (operation.operator, entry.arg(0)?, entry.arg(1)?)
        }
        Felt252BinaryOperationConcrete::WithConst(operation) => {
            // TODO: Ensure that the constant is on the correct side of the operation.
            let rhs = entry.const_int_from_type(
                context,
                location,
                felt_to_unsigned(&operation.c),
                felt252_ty,
            )?;

            (operation.operator, entry.arg(0)?, rhs)
        }
    };

    let result = match op {
        Felt252BinaryOperator::Add => {
            let lhs = entry.extui(lhs, i256, location)?;
            let rhs = entry.extui(rhs, i256, location)?;
            let result = entry.addi(lhs, rhs, location)?;

            let prime = entry.const_int_from_type(context, location, PRIME.clone(), i256)?;
            let result_mod = entry.append_op_result(arith::subi(result, prime, location))?;
            let is_out_of_range =
                entry.cmpi(context, CmpiPredicate::Uge, result, prime, location)?;

            let result = entry.append_op_result(arith::select(
                is_out_of_range,
                result_mod,
                result,
                location,
            ))?;
            entry.trunci(result, felt252_ty, location)?
        }
        Felt252BinaryOperator::Sub => {
            let lhs = entry.extui(lhs, i256, location)?;
            let rhs = entry.extui(rhs, i256, location)?;
            let result = entry.append_op_result(arith::subi(lhs, rhs, location))?;

            let prime = entry.const_int_from_type(context, location, PRIME.clone(), i256)?;
            let result_mod = entry.addi(result, prime, location)?;
            let is_out_of_range = entry.cmpi(context, CmpiPredicate::Ult, lhs, rhs, location)?;

            let result = entry.append_op_result(arith::select(
                is_out_of_range,
                result_mod,
                result,
                location,
            ))?;
            entry.trunci(result, felt252_ty, location)?
        }
        Felt252BinaryOperator::Mul => {
            let lhs = entry.extui(lhs, i512, location)?;
            let rhs = entry.extui(rhs, i512, location)?;
            let result = entry.muli(lhs, rhs, location)?;

            let prime = entry.const_int_from_type(context, location, PRIME.clone(), i512)?;
            let result_mod = entry.append_op_result(arith::remui(result, prime, location))?;
            let is_out_of_range =
                entry.cmpi(context, CmpiPredicate::Uge, result, prime, location)?;

            let result = entry.append_op_result(arith::select(
                is_out_of_range,
                result_mod,
                result,
                location,
            ))?;
            entry.trunci(result, felt252_ty, location)?
        }
        Felt252BinaryOperator::Div => {
            let runtime_bindings_meta = metadata
                .get_mut::<RuntimeBindingsMeta>()
                .to_native_assert_error(
                    "Unable to get the RuntimeBindingsMeta from MetadataStorage",
                )?;

            let prime = entry.const_int_from_type(context, location, PRIME.clone(), felt252_ty)?;

            // Find 1 / rhs.
            let euclidean_result = runtime_bindings_meta.u252_extended_euclidean_algorithm(
                context,
                helper.module,
                entry,
                location,
                rhs,
                prime,
            )?;

            // Here we omit checking if inverse is actually the inverse,
            // satisfying gcd(a,b) == 1, because we are using a prime as the
            // modulus. This ensures that for any value of a, included in the
            // field, gcd(a,b) == 1.
            let prime = entry.const_int_from_type(context, location, PRIME.clone(), i512)?;
            let inverse = {
                let inverse =
                    entry.extract_value(context, location, euclidean_result, felt252_ty, 1)?;
                entry.extui(inverse, i512, location)?
            };

            // Peform lhs * (1 / rhs)
            let lhs = entry.extui(lhs, i512, location)?;
            let result = entry.muli(lhs, inverse, location)?;
            // Apply modulo and convert result to felt252
            let result_mod = entry.append_op_result(arith::remui(result, prime, location))?;
            let is_out_of_range =
                entry.cmpi(context, CmpiPredicate::Uge, result, prime, location)?;
            let result = entry.append_op_result(arith::select(
                is_out_of_range,
                result_mod,
                result,
                location,
            ))?;

            entry.trunci(result, felt252_ty, location)?
        }
    };

    helper.br(entry, 0, &[result], location)
}

/// Generate MLIR operations for the `felt252_const` libfunc.
pub fn build_const<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &Felt252ConstConcreteLibfunc,
) -> Result<()> {
    let felt252_ty = registry.build_type(
        context,
        helper,
        metadata,
        &info.branch_signatures()[0].vars[0].ty,
    )?;

    let value =
        entry.const_int_from_type(context, location, felt_to_unsigned(&info.c), felt252_ty)?;

    helper.br(entry, 0, &[value], location)
}

/// Generate MLIR operations for the `felt252_is_zero` libfunc.
pub fn build_is_zero<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let arg0: Value = entry.arg(0)?;

    let k0 = entry.const_int_from_type(context, location, 0, arg0.r#type())?;
    let condition = entry.cmpi(context, CmpiPredicate::Eq, arg0, k0, location)?;

    helper.cond_br(context, entry, condition, [0, 1], [&[], &[arg0]], location)
}

#[cfg(test)]
pub mod test {
    use crate::{
        jit_struct,
        utils::testing::{get_compiled_program, run_program},
        values::Value,
    };
    use cairo_lang_sierra::program::Program;
    use starknet_types_core::felt::Felt;

    fn f(val: &str) -> Felt {
        Felt::from_dec_str(val).unwrap()
    }

    #[test]
    fn felt252_add() {
        let program = &get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_add");
        fn r(lhs: Felt, rhs: Felt, program: &(String, Program)) -> Felt {
            match run_program(
                program,
                "run_test",
                &[Value::Felt252(lhs), Value::Felt252(rhs)],
            )
            .return_value
            {
                Value::Felt252(x) => x,
                _ => panic!("invalid return type"),
            }
        }

        assert_eq!(r(f("0"), f("0"), program), f("0"));
        assert_eq!(r(f("1"), f("2"), program), f("3"));

        assert_eq!(r(f("0"), f("1"), program), f("1"));
        assert_eq!(r(f("0"), f("-2"), program), f("-2"));
        assert_eq!(r(f("0"), f("-1"), program), f("-1"));

        assert_eq!(r(f("1"), f("0"), program), f("1"));
        assert_eq!(r(f("1"), f("1"), program), f("2"));
        assert_eq!(r(f("1"), f("-2"), program), f("-1"));
        assert_eq!(r(f("1"), f("-1"), program), f("0"));

        assert_eq!(r(f("-2"), f("0"), program), f("-2"));
        assert_eq!(r(f("-2"), f("1"), program), f("-1"));
        assert_eq!(r(f("-2"), f("-2"), program), f("-4"));
        assert_eq!(r(f("-2"), f("-1"), program), f("-3"));

        assert_eq!(r(f("-1"), f("0"), program), f("-1"));
        assert_eq!(r(f("-1"), f("1"), program), f("0"));
        assert_eq!(r(f("-1"), f("-2"), program), f("-3"));
        assert_eq!(r(f("-1"), f("-1"), program), f("-2"));
    }

    #[test]
    fn felt252_sub() {
        let program = &get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_sub");
        fn r(lhs: Felt, rhs: Felt, program: &(String, Program)) -> Felt {
            match run_program(
                program,
                "run_test",
                &[Value::Felt252(lhs), Value::Felt252(rhs)],
            )
            .return_value
            {
                Value::Felt252(x) => x,
                _ => panic!("invalid return type"),
            }
        }

        assert_eq!(r(f("0"), f("0"), program), f("0"));
        assert_eq!(r(f("0"), f("1"), program), f("-1"));
        assert_eq!(r(f("0"), f("-2"), program), f("2"));
        assert_eq!(r(f("0"), f("-1"), program), f("1"));

        assert_eq!(r(f("1"), f("0"), program), f("1"));
        assert_eq!(r(f("1"), f("1"), program), f("0"));
        assert_eq!(r(f("1"), f("-2"), program), f("3"));
        assert_eq!(r(f("1"), f("-1"), program), f("2"));

        assert_eq!(r(f("-2"), f("0"), program), f("-2"));
        assert_eq!(r(f("-2"), f("1"), program), f("-3"));
        assert_eq!(r(f("-2"), f("-2"), program), f("0"));
        assert_eq!(r(f("-2"), f("-1"), program), f("-1"));

        assert_eq!(r(f("-1"), f("0"), program), f("-1"));
        assert_eq!(r(f("-1"), f("1"), program), f("-2"));
        assert_eq!(r(f("-1"), f("-2"), program), f("1"));
        assert_eq!(r(f("-1"), f("-1"), program), f("0"));
    }

    #[test]
    fn felt252_mul() {
        let program = &get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_mul");
        fn r(lhs: Felt, rhs: Felt, program: &(String, Program)) -> Felt {
            match run_program(
                program,
                "run_test",
                &[Value::Felt252(lhs), Value::Felt252(rhs)],
            )
            .return_value
            {
                Value::Felt252(x) => x,
                _ => panic!("invalid return type"),
            }
        }

        assert_eq!(r(f("0"), f("0"), program), f("0"));
        assert_eq!(r(f("0"), f("1"), program), f("0"));
        assert_eq!(r(f("0"), f("-2"), program), f("0"));
        assert_eq!(r(f("0"), f("-1"), program), f("0"));

        assert_eq!(r(f("1"), f("0"), program), f("0"));
        assert_eq!(r(f("1"), f("1"), program), f("1"));
        assert_eq!(r(f("1"), f("-2"), program), f("-2"));
        assert_eq!(r(f("1"), f("-1"), program), f("-1"));

        assert_eq!(r(f("-2"), f("0"), program), f("0"));
        assert_eq!(r(f("-2"), f("1"), program), f("-2"));
        assert_eq!(r(f("-2"), f("-2"), program), f("4"));
        assert_eq!(r(f("-2"), f("-1"), program), f("2"));

        assert_eq!(r(f("-1"), f("0"), program), f("0"));
        assert_eq!(r(f("-1"), f("1"), program), f("-1"));
        assert_eq!(r(f("-1"), f("-2"), program), f("2"));
        assert_eq!(r(f("-1"), f("-1"), program), f("1"));
    }

    #[test]
    fn felt252_div() {
        let program = &get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_div");
        // Helper function to run the test and extract the return value.
        fn r(lhs: Felt, rhs: Felt, program: &(String, Program)) -> Option<Felt> {
            match run_program(
                program,
                "run_test",
                &[Value::Felt252(lhs), Value::Felt252(rhs)],
            )
            .return_value
            {
                Value::Enum { tag: 0, value, .. } => match *value {
                    Value::Struct { fields, .. } => {
                        assert_eq!(fields.len(), 1);
                        Some(match &fields[0] {
                            Value::Felt252(x) => *x,
                            _ => panic!("invalid return type payload"),
                        })
                    }
                    _ => panic!("invalid return type"),
                },
                Value::Enum { tag: 1, .. } => None,
                _ => panic!("invalid return type"),
            }
        }

        // Helper function to assert that a division panics.
        let assert_panics = |lhs, rhs, program| {
            assert!(
                r(lhs, rhs, program).is_none(),
                "division by 0 is expected to panic",
            )
        };

        // Division by zero is expected to panic.
        assert_panics(f("0"), f("0"), program);
        assert_panics(f("1"), f("0"), program);
        assert_panics(f("-2"), f("0"), program);

        // Test cases for valid division results.
        assert_eq!(r(f("0"), f("1"), program), Some(f("0")));
        assert_eq!(r(f("0"), f("-2"), program), Some(f("0")));
        assert_eq!(r(f("0"), f("-1"), program), Some(f("0")));
        assert_eq!(r(f("1"), f("1"), program), Some(f("1")));
        assert_eq!(
            r(f("1"), f("-2"), program),
            Some(f(
                "1809251394333065606848661391547535052811553607665798349986546028067936010240"
            ))
        );
        assert_eq!(r(f("1"), f("-1"), program), Some(f("-1")));
        assert_eq!(r(f("-2"), f("1"), program), Some(f("-2")));
        assert_eq!(r(f("-2"), f("-2"), program), Some(f("1")));
        assert_eq!(r(f("-2"), f("-1"), program), Some(f("2")));
        assert_eq!(r(f("-1"), f("1"), program), Some(f("-1")));
        assert_eq!(
            r(f("-1"), f("-2"), program),
            Some(f(
                "1809251394333065606848661391547535052811553607665798349986546028067936010241"
            ))
        );
        assert_eq!(r(f("-1"), f("-1"), program), Some(f("1")));
        assert_eq!(r(f("6"), f("2"), program), Some(f("3")));
        assert_eq!(r(f("1000"), f("2"), program), Some(f("500")));
    }

    #[test]
    fn felt252_const() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_const");
        assert_eq!(
            run_program(&program, "run_test", &[]).return_value,
            Value::Struct {
                fields: [f("0"), f("1"), f("-2"), f("-1")]
                    .map(Value::Felt252)
                    .to_vec(),
                debug_name: None
            }
        );
    }

    #[test]
    fn felt252_add_const() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_add_const");
        assert_eq!(
            run_program(&program, "run_test", &[]).return_value,
            jit_struct!(
                f("0").into(),
                f("1").into(),
                f("1").into(),
                f("2").into(),
                f("-1").into(),
                f("-1").into(),
                f("-2").into(),
                f("0").into(),
                f("0").into(),
            )
        );
    }

    #[test]
    fn felt252_sub_const() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_sub_const");
        assert_eq!(
            run_program(&program, "run_test", &[]).return_value,
            jit_struct!(
                f("0").into(),
                f("1").into(),
                f("-1").into(),
                f("0").into(),
                f("-1").into(),
                f("1").into(),
                f("0").into(),
                f("2").into(),
                f("-2").into(),
            )
        );
    }

    #[test]
    fn felt252_mul_const() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_mul_const");
        assert_eq!(
            run_program(&program, "run_test", &[]).return_value,
            jit_struct!(
                f("0").into(),
                f("0").into(),
                f("0").into(),
                f("1").into(),
                f("-2").into(),
                f("-4").into(),
                f("1").into(),
                f("-1").into(),
                f("-1").into(),
            )
        );
    }

    #[test]
    fn felt252_div_const() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_div_const");
        assert_eq!(
            run_program(&program, "run_test", &[]).return_value,
            jit_struct!(
                f("0").into(),
                f("1").into(),
                f("1809251394333065606848661391547535052811553607665798349986546028067936010240")
                    .into(),
                f("-1").into(),
                f("1").into(),
                f("-1").into(),
                f("-1").into(),
                f("2").into(),
                f("4").into(),
                f("-4").into(),
                f("-4").into(),
                f("4").into(),
                f("8").into(),
                f("-8").into(),
                f("-8").into(),
                f("8").into(),
            )
        );
    }

    #[test]
    fn felt252_is_zero() {
        let program =
            &get_compiled_program("test_data_artifacts/programs/libfuncs/felt252_is_zero");
        fn r(x: Felt, program: &(String, Program)) -> bool {
            match run_program(program, "run_test", &[Value::Felt252(x)]).return_value {
                Value::Enum { tag, .. } => tag != 0,
                _ => panic!("invalid return type"),
            }
        }

        assert!(r(f("0"), program));
        assert!(!r(f("1"), program));
        assert!(!r(f("-2"), program));
        assert!(!r(f("-1"), program));
    }
}
