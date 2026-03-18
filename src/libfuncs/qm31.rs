use crate::{
    error::{panic::ToNativeAssertError, Error, Result},
    metadata::runtime_bindings::RuntimeBindingsMeta,
    utils::{get_integer_layout, ProgramRegistryExt},
};
use crate::{libfuncs::LibfuncHelper, metadata::MetadataStorage};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        lib_func::SignatureOnlyConcreteLibfunc,
        qm31::{QM31BinaryOpConcreteLibfunc, QM31Concrete, QM31ConstConcreteLibfunc},
        ConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    dialect::{
        arith::{self, CmpiPredicate},
        llvm,
    },
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{r#type::IntegerType, Block, Location},
    Context,
};

const M31_PRIME: u32 = 0x7fffffff;

pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &QM31Concrete,
) -> Result<()> {
    match selector {
        QM31Concrete::BinaryOperation(info) => {
            build_binary_op(context, registry, entry, location, helper, metadata, info)
        }
        QM31Concrete::Const(info) => {
            build_const(context, registry, entry, location, helper, metadata, info)
        }
        QM31Concrete::IsZero(info) => {
            build_is_zero(context, registry, entry, location, helper, metadata, info)
        }
        QM31Concrete::Pack(info) => {
            build_pack(context, registry, entry, location, helper, metadata, info)
        }
        QM31Concrete::Unpack(info) => {
            build_unpack(context, registry, entry, location, helper, metadata, info)
        }
        QM31Concrete::FromM31(info) => {
            build_from_m31(context, registry, entry, location, helper, metadata, info)
        }
    }
}

/// Generate MLIR operations for the `qm31_const` libfunc.
///
/// Receives 4 const m31 and returns a qm31.
///
/// # Constraints
///
/// m31 are always between 0 and 2**31 - 2 (inclusive)
///
/// # Cairo Signature
///
/// ```cairo
/// fn qm31_const<
///     const W0: m31, const W1: m31, const W2: m31, const W3: m31,
/// >() -> qm31 nopanic;
/// ```
pub fn build_const<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &QM31ConstConcreteLibfunc,
) -> Result<()> {
    let m31_ty = IntegerType::new(context, 31).into();
    let qm31_ty = registry.build_type(
        context,
        helper,
        metadata,
        &info.branch_signatures()[0].vars[0].ty,
    )?;

    let m31_0 = entry.const_int_from_type(context, location, info.w0, m31_ty)?;
    let m31_1 = entry.const_int_from_type(context, location, info.w1, m31_ty)?;
    let m31_2 = entry.const_int_from_type(context, location, info.w2, m31_ty)?;
    let m31_3 = entry.const_int_from_type(context, location, info.w3, m31_ty)?;

    let qm31 = entry.append_op_result(llvm::undef(qm31_ty, location))?;
    let qm31 = entry.insert_values(context, location, qm31, &[m31_0, m31_1, m31_2, m31_3])?;

    helper.br(entry, 0, &[qm31], location)
}

/// Generate MLIR operations for the `qm31_is_zero` libfunc.
///
/// Receives a qm31 and returns a Some(qm31) if the argument is not 0,
/// otherwise a None.
///
/// # Cairo Signature
///
/// ```cairo
/// fn qm31_is_zero(a: qm31) -> core::internal::OptionRev<NonZero<qm31>> nopanic;
/// ```
pub fn build_is_zero<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let qm31 = entry.arg(0)?;
    let m31_ty = IntegerType::new(context, 31).into();

    let m31_0 = entry.extract_value(context, location, qm31, m31_ty, 0)?;
    let m31_1 = entry.extract_value(context, location, qm31, m31_ty, 1)?;
    let m31_2 = entry.extract_value(context, location, qm31, m31_ty, 2)?;
    let m31_3 = entry.extract_value(context, location, qm31, m31_ty, 3)?;

    // Check that every limb is equal to 0:
    //      (m31_0 | m31_1 | m31_2 | m31_3) == 0
    let cond = entry.append_op_result(arith::ori(m31_0, m31_1, location))?;
    let cond = entry.append_op_result(arith::ori(cond, m31_2, location))?;
    let cond = entry.append_op_result(arith::ori(cond, m31_3, location))?;
    let k0 = entry.const_int_from_type(context, location, 0, m31_ty)?;
    let cond =
        entry.append_op_result(arith::cmpi(context, CmpiPredicate::Eq, k0, cond, location))?;

    helper.cond_br(
        context,
        entry,
        cond,
        [0, 1],
        [&[], &[entry.arg(0)?]],
        location,
    )
}

/// Generate MLIR operations for the `qm31_pack` libfunc.
///
/// Receives four m31 and packs them into a qm31.
///
/// # Constraints
///
/// m31 are always between 0 and 2**31 - 2 (inclusive)
///
/// # Cairo Signature
///
/// ```cairo
/// fn qm31_pack(w0: m31, w1: m31, w2: m31, w3: m31) -> qm31 nopanic;
/// ```
pub fn build_pack<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let m31_0 = entry.arg(0)?;
    let m31_1 = entry.arg(1)?;
    let m31_2 = entry.arg(2)?;
    let m31_3 = entry.arg(3)?;

    let qm31_ty = registry.build_type(
        context,
        helper,
        metadata,
        &info.branch_signatures()[0].vars[0].ty,
    )?;

    let qm31 = entry.append_op_result(llvm::undef(qm31_ty, location))?;
    let qm31 = entry.insert_values(context, location, qm31, &[m31_0, m31_1, m31_2, m31_3])?;

    helper.br(entry, 0, &[qm31], location)
}

/// Generate MLIR operations for the `qm31_unpack` libfunc.
///
/// Receives a qm31 and unpacks it, returning an array with the
/// four m31.
///
/// # Constraints
///
/// m31 are always between 0 and 2**31 - 2 (inclusive)
///
/// # Cairo Signature
///
/// ```cairo
/// fn qm31_unpack(a: qm31) -> (m31, m31, m31, m31) implicits(crate::RangeCheck) nopanic;
/// ```
pub fn build_unpack<'ctx, 'this>(
    context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let range_check =
        super::increment_builtin_counter_by(context, entry, location, entry.arg(0)?, 5)?;
    let m31_ty = IntegerType::new(context, 31);
    let qm31 = entry.arg(1)?;

    let m31_0 = entry.extract_value(context, location, qm31, m31_ty.into(), 0)?;
    let m31_1 = entry.extract_value(context, location, qm31, m31_ty.into(), 1)?;
    let m31_2 = entry.extract_value(context, location, qm31, m31_ty.into(), 2)?;
    let m31_3 = entry.extract_value(context, location, qm31, m31_ty.into(), 3)?;

    helper.br(
        entry,
        0,
        &[range_check, m31_0, m31_1, m31_2, m31_3],
        location,
    )
}

/// Generate MLIR operations for the `qm31_from_m31` libfunc.
///
/// Receives a m31 and returns a qm31 in which its first coeffiecient
/// has the value of the input and the other ones are 0.
///
/// # Constraints
///
/// m31 are always between 0 and 2**31 - 2 (inclusive)
///
/// # Cairo Signature
///
/// ```cairo
/// fn qm31_from_m31(a: m31) -> qm31 nopanic;
/// ```
pub fn build_from_m31<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let m31 = entry.arg(0)?;

    let m31_ty = IntegerType::new(context, 31).into();
    let qm31_ty = registry.build_type(
        context,
        helper,
        metadata,
        &info.branch_signatures()[0].vars[0].ty,
    )?;
    let k0 = entry.const_int_from_type(context, location, 0, m31_ty)?;

    let qm31 = entry.append_op_result(llvm::undef(qm31_ty, location))?;
    let qm31 = entry.insert_values(context, location, qm31, &[m31, k0, k0, k0])?;

    helper.br(entry, 0, &[qm31], location)
}

fn m31_add<'ctx, 'this>(
    context: &'ctx Context,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
) -> Result<()> {
    let lhs_value = entry.arg(0)?;
    let rhs_value = entry.arg(1)?;

    let lhs_value = entry.extui(lhs_value, IntegerType::new(context, 32).into(), location)?;
    let rhs_value = entry.extui(rhs_value, IntegerType::new(context, 32).into(), location)?;

    let res = entry.append_op_result(arith::addi(lhs_value, rhs_value, location))?;
    let prime = entry.const_int(context, location, M31_PRIME, 32)?;
    let res_mod = entry.append_op_result(arith::subi(res, prime, location))?;
    let is_out_of_range = entry.cmpi(context, CmpiPredicate::Uge, res, prime, location)?;

    let res = entry.append_op_result(arith::select(is_out_of_range, res_mod, res, location))?;

    let res = entry.trunci(res, IntegerType::new(context, 31).into(), location)?;

    helper.br(entry, 0, &[res], location)
}

fn m31_sub<'ctx, 'this>(
    context: &'ctx Context,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
) -> Result<()> {
    let lhs_value = entry.arg(0)?;
    let rhs_value = entry.arg(1)?;

    let res = entry.append_op_result(arith::subi(lhs_value, rhs_value, location))?;
    let prime = entry.const_int(context, location, M31_PRIME, 31)?;
    let res_mod = entry.append_op_result(arith::addi(res, prime, location))?;
    let is_out_of_range =
        entry.cmpi(context, CmpiPredicate::Ult, lhs_value, rhs_value, location)?;

    let res = entry.append_op_result(arith::select(is_out_of_range, res_mod, res, location))?;
    helper.br(entry, 0, &[res], location)
}

fn m31_mul<'ctx, 'this>(
    context: &'ctx Context,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
) -> Result<()> {
    let lhs_value = entry.arg(0)?;
    let rhs_value = entry.arg(1)?;

    let lhs_value = entry.extui(lhs_value, IntegerType::new(context, 64).into(), location)?;
    let rhs_value = entry.extui(rhs_value, IntegerType::new(context, 64).into(), location)?;
    let res = entry.muli(lhs_value, rhs_value, location)?;

    let prime = entry.const_int(context, location, M31_PRIME, 64)?;
    let res_mod = entry.append_op_result(arith::remui(res, prime, location))?;
    let is_out_of_range = entry.cmpi(context, CmpiPredicate::Uge, res, prime, location)?;

    let res = entry.append_op_result(arith::select(is_out_of_range, res_mod, res, location))?;
    let res = entry.trunci(res, IntegerType::new(context, 31).into(), location)?;

    helper.br(entry, 0, &[res], location)
}

fn m31_div<'ctx, 'this>(
    context: &'ctx Context,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
) -> Result<()> {
    let i31 = IntegerType::new(context, 31).into();
    let i64 = IntegerType::new(context, 64).into();
    let rhs_value = entry.arg(1)?;
    let prime = entry.const_int_from_type(context, location, M31_PRIME, i31)?;

    let runtime_bindings_meta = metadata
        .get_mut::<RuntimeBindingsMeta>()
        .to_native_assert_error("Unable to get the RuntimeBindingsMeta from MetadataStorage")?;
    let euclidean_result = runtime_bindings_meta.u31_extended_euclidean_algorithm(
        context,
        helper.module,
        entry,
        location,
        rhs_value,
        prime,
    )?;

    // Here we omit checking if inverse is actually the inverse,
    // satisfying gcd(a,b) == 1, because we are using a prime as the
    // modulus. This ensures that for any value of a, included in the
    // field, gcd(a,b) == 1.
    let prime = entry.const_int_from_type(context, location, M31_PRIME, i64)?;
    let inverse = {
        let inverse = entry.extract_value(context, location, euclidean_result, i31, 1)?;
        entry.extui(inverse, i64, location)?
    };

    // Perform lhs * (1 / rhs)
    let lhs = entry.extui(entry.arg(0)?, i64, location)?;
    let result = entry.muli(lhs, inverse, location)?;
    // Apply modulo and convert result to m31
    let result_mod = entry.append_op_result(arith::remui(result, prime, location))?;
    let is_out_of_range = entry.cmpi(context, CmpiPredicate::Uge, result, prime, location)?;

    let result =
        entry.append_op_result(arith::select(is_out_of_range, result_mod, result, location))?;
    let result = entry.trunci(result, i31, location)?;

    helper.br(entry, 0, &[result], location)
}

/// Generate MLIR operations for the QM31 and M31 binary operations libfuncs.
///
/// Depending on the type of the parameters, it chooses which type of representation
/// it will manage (QM31 or M31). It either receives 2 qm31 or 2 m31 (which are represented
/// as bounded ints)
///
/// # Cairo Signature
/// ```cairo
/// // qm31
/// fn qm31_add(a: qm31, b: qm31) -> qm31 nopanic;
/// fn qm31_sub(a: qm31, b: qm31) -> qm31 nopanic;
/// fn qm31_mul(a: qm31, b: qm31) -> qm31 nopanic;
/// fn qm31_div(a: qm31, b: NonZero<qm31>) -> qm31 nopanic;
///
/// // m31
/// extern fn m31_add(a: m31, b: m31) -> m31 nopanic;
/// extern fn m31_sub(a: m31, b: m31) -> m31 nopanic;
/// extern fn m31_mul(a: m31, b: m31) -> m31 nopanic;
/// extern fn m31_div(a: m31, b: NonZero<m31>) -> m31 nopanic;
/// ```
pub fn build_binary_op<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &QM31BinaryOpConcreteLibfunc,
) -> Result<()> {
    // If the parameter is a bounded int, then we need to generate the operations
    // for the m31
    let type_concrete = registry.get_type(&info.param_signatures()[0].ty)?;
    if let CoreTypeConcrete::BoundedInt(_) = type_concrete {
        match info.operator {
            cairo_lang_sierra::extensions::qm31::QM31BinaryOperator::Add => {
                return m31_add(context, entry, location, helper);
            }
            cairo_lang_sierra::extensions::qm31::QM31BinaryOperator::Sub => {
                return m31_sub(context, entry, location, helper);
            }
            cairo_lang_sierra::extensions::qm31::QM31BinaryOperator::Mul => {
                return m31_mul(context, entry, location, helper);
            }
            cairo_lang_sierra::extensions::qm31::QM31BinaryOperator::Div => {
                return m31_div(context, entry, location, helper, metadata);
            }
        }
    }
    let qm31_ty = registry.build_type(context, helper, metadata, &info.param_signatures()[0].ty)?;

    let lhs = entry.arg(0)?;
    let rhs = entry.arg(1)?;

    let lhs_ptr = entry.alloca1(context, location, qm31_ty, get_integer_layout(31).align())?;
    let rhs_ptr = entry.alloca1(context, location, qm31_ty, get_integer_layout(31).align())?;

    entry.store(context, location, lhs_ptr, lhs)?;
    entry.store(context, location, rhs_ptr, rhs)?;

    let result = metadata
        .get_mut::<RuntimeBindingsMeta>()
        .ok_or(Error::MissingMetadata)?
        .libfunc_qm31_bin_op(
            context,
            helper,
            entry,
            lhs_ptr,
            rhs_ptr,
            info.operator,
            location,
        )?;

    helper.br(entry, 0, &[result], location)
}

#[cfg(test)]
mod test {
    use ark_ff::Zero;
    use cairo_lang_sierra::extensions::utils::Range;
    use num_bigint::BigInt;
    use starknet_types_core::felt::Felt as Felt252;

    use crate::{
        jit_enum, jit_struct,
        libfuncs::qm31::M31_PRIME,
        runtime::qm31_to_representative_coefficients,
        utils::testing::{get_compiled_program, run_program},
        Value,
    };

    impl From<&starknet_types_core::qm31::QM31> for Value {
        fn from(qm31: &starknet_types_core::qm31::QM31) -> Self {
            let coefficients = qm31_to_representative_coefficients(qm31.clone());
            Value::QM31(
                coefficients[0],
                coefficients[1],
                coefficients[2],
                coefficients[3],
            )
        }
    }

    #[test]
    fn run_unpack() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_unpack");

        let result = run_program(&program, "run_test_1", &[]).return_value;
        let m31_range = Range::closed(0, BigInt::from(2147483646));
        let Value::Struct { fields, .. } = result else {
            panic!("Expected a Value::Struct()");
        };
        assert_eq!(
            fields,
            vec![
                Value::BoundedInt {
                    value: Felt252::from(1),
                    range: m31_range.clone()
                },
                Value::BoundedInt {
                    value: Felt252::from(2),
                    range: m31_range.clone()
                },
                Value::BoundedInt {
                    value: Felt252::from(3),
                    range: m31_range.clone()
                },
                Value::BoundedInt {
                    value: Felt252::from(4),
                    range: m31_range.clone()
                },
            ]
        );

        let result = run_program(&program, "run_test_2", &[]).return_value;
        let Value::Struct { fields, .. } = result else {
            panic!("Expected a Value::Struct()");
        };
        assert_eq!(
            fields,
            vec![
                Value::BoundedInt {
                    value: Felt252::from(0x544b2fba),
                    range: m31_range.clone()
                },
                Value::BoundedInt {
                    value: Felt252::from(0x673cff77),
                    range: m31_range.clone()
                },
                Value::BoundedInt {
                    value: Felt252::from(0x60713d44),
                    range: m31_range.clone()
                },
                Value::BoundedInt {
                    value: Felt252::from(0x499602d2),
                    range: m31_range.clone()
                },
            ]
        );
    }

    #[test]
    fn run_pack() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_pack");
        // With small coefficients
        let result = run_program(&program, "run_test", &[]).return_value;
        assert_eq!(result, Value::QM31(1, 2, 3, 4));

        // With big coefficients
        let result = run_program(&program, "run_test_large_coefficients", &[]).return_value;
        let qm31_expected = starknet_types_core::qm31::QM31::from_coefficients(
            0x544b2fba, 0x673cff77, 0x60713d44, 0x499602d2,
        );
        let expected_coefficients = qm31_expected.to_coefficients();
        assert_eq!(
            result,
            Value::QM31(
                expected_coefficients.0,
                expected_coefficients.1,
                expected_coefficients.2,
                expected_coefficients.3
            )
        );
    }

    #[test]
    fn run_const() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_const");

        let result = run_program(&program, "run_test", &[]).return_value;
        assert_eq!(result, Value::QM31(1, 2, 3, 4));

        let result = run_program(&program, "run_test_large_coefficients", &[]).return_value;
        assert_eq!(
            result,
            Value::QM31(0x544b2fba, 0x673cff77, 0x60713d44, 0x499602d2)
        );
    }

    #[test]
    fn run_is_zero() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_is_zero");

        let result = run_program(&program, "run_test", &[Value::QM31(0, 0, 0, 0)]).return_value;
        assert_eq!(result, jit_enum!(0, jit_struct!()));

        let result = run_program(&program, "run_test", &[Value::QM31(0, 0, 1, 0)]).return_value;
        assert_eq!(result, jit_enum!(1, Value::QM31(0, 0, 1, 0)));

        let result = run_program(
            &program,
            "run_test",
            &[Value::QM31(0x544b2fba, 0x673cff77, 0x60713d44, 0x499602d2)],
        )
        .return_value;
        assert_eq!(
            result,
            jit_enum!(
                1,
                Value::QM31(0x544b2fba, 0x673cff77, 0x60713d44, 0x499602d2)
            )
        );

        let result = run_program(&program, "run_test_edge_case", &[]).return_value;
        assert_eq!(result, jit_enum!(0, jit_struct!()));
    }

    #[test]
    fn run_qm31_add() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_add");

        let a = starknet_types_core::qm31::QM31::from_coefficients(
            0x544b2fba, 0x673cff77, 0x60713d44, 0x499602d2,
        );
        let b = starknet_types_core::qm31::QM31::from_coefficients(
            0x499602d2, 0x544b2fba, 0x673cff77, 0x60713d44,
        );
        let c = starknet_types_core::qm31::QM31::from_coefficients(
            0x1de1328d, 0x3b882f32, 0x47ae3cbc, 0x2a074017,
        );
        let d = starknet_types_core::qm31::QM31::from_coefficients(
            0x7ffffffe, 0x7ffffffe, 0x7ffffffe, 0x7ffffffe,
        );
        let e = starknet_types_core::qm31::QM31::from_coefficients(1, 1, 1, 1);

        let result =
            run_program(&program, "run_test", &[Value::from(&a), Value::from(&b)]).return_value;
        let expected_qm31 = a.clone() + b.clone();
        assert_eq!(result, Value::from(&expected_qm31));

        let result =
            run_program(&program, "run_test", &[Value::from(&b), Value::from(&c)]).return_value;
        let expected_qm31 = b + c.clone();
        assert_eq!(result, Value::from(&expected_qm31));

        let result =
            run_program(&program, "run_test", &[Value::from(&a), Value::from(&c)]).return_value;
        let expected_qm31 = a + c;
        assert_eq!(result, Value::from(&expected_qm31));

        let result =
            run_program(&program, "run_test", &[Value::from(&d), Value::from(&e)]).return_value;
        assert_eq!(result, Value::QM31(0, 0, 0, 0));
    }

    #[test]
    fn run_qm31_sub() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_sub");

        let a = starknet_types_core::qm31::QM31::from_coefficients(
            0x544b2fba, 0x673cff77, 0x60713d44, 0x499602d2,
        );
        let b = starknet_types_core::qm31::QM31::from_coefficients(
            0x499602d2, 0x544b2fba, 0x673cff77, 0x60713d44,
        );
        let c = starknet_types_core::qm31::QM31::from_coefficients(
            0x1de1328d, 0x3b882f32, 0x47ae3cbc, 0x2a074017,
        );

        let result =
            run_program(&program, "run_test", &[Value::from(&c), Value::from(&a)]).return_value;
        let expected_qm31 = c.clone() - a.clone();
        assert_eq!(result, Value::from(&expected_qm31));

        let result =
            run_program(&program, "run_test", &[Value::from(&a), Value::from(&b)]).return_value;
        let expected_qm31 = a - b.clone();
        assert_eq!(result, Value::from(&expected_qm31));

        let result =
            run_program(&program, "run_test", &[Value::from(&b), Value::from(&c)]).return_value;
        let expected_qm31 = b - c;
        assert_eq!(result, Value::from(&expected_qm31));
    }

    #[test]
    fn run_qm31_mul() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_mul");

        let a = starknet_types_core::qm31::QM31::from_coefficients(
            0x544b2fba, 0x673cff77, 0x60713d44, 0x499602d2,
        );
        let b = starknet_types_core::qm31::QM31::from_coefficients(
            0x499602d2, 0x544b2fba, 0x673cff77, 0x60713d44,
        );
        let c = starknet_types_core::qm31::QM31::from_coefficients(
            0x1de1328d, 0x3b882f32, 0x47ae3cbc, 0x2a074017,
        );
        let d = starknet_types_core::qm31::QM31::zero();

        let result =
            run_program(&program, "run_test", &[Value::from(&a), Value::from(&b)]).return_value;
        let expected_qm31 = a.clone() * b.clone();
        assert_eq!(result, Value::from(&expected_qm31));

        let result =
            run_program(&program, "run_test", &[Value::from(&a), Value::from(&c)]).return_value;
        let expected_qm31 = a.clone() * c.clone();
        assert_eq!(result, Value::from(&expected_qm31));

        let result =
            run_program(&program, "run_test", &[Value::from(&b), Value::from(&c)]).return_value;
        let expected_qm31 = b.clone() * c;
        assert_eq!(result, Value::from(&expected_qm31));

        let result =
            run_program(&program, "run_test", &[Value::from(&d), Value::from(&b)]).return_value;
        let expected_qm31 = d * b;
        assert_eq!(result, Value::from(&expected_qm31));
    }

    #[test]
    fn run_qm31_div() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_div");

        let a = starknet_types_core::qm31::QM31::from_coefficients(
            0x544b2fba, 0x673cff77, 0x60713d44, 0x499602d2,
        );
        let b = starknet_types_core::qm31::QM31::from_coefficients(
            0x499602d2, 0x544b2fba, 0x673cff77, 0x60713d44,
        );
        let c = starknet_types_core::qm31::QM31::from_coefficients(
            0x1de1328d, 0x3b882f32, 0x47ae3cbc, 0x2a074017,
        );
        let d = starknet_types_core::qm31::QM31::from_coefficients(0, 0, 0, 0);

        let result =
            run_program(&program, "run_test", &[Value::from(&c), Value::from(&a)]).return_value;
        let expected_qm31 = (c.clone() / a.clone()).unwrap();
        assert_eq!(
            result,
            jit_enum!(0, jit_struct!(Value::from(&expected_qm31)))
        );

        let result =
            run_program(&program, "run_test", &[Value::from(&a), Value::from(&b)]).return_value;
        let expected_qm31 = (a.clone() / b.clone()).unwrap();
        assert_eq!(
            result,
            jit_enum!(0, jit_struct!(Value::from(&expected_qm31)))
        );

        let result =
            run_program(&program, "run_test", &[Value::from(&b), Value::from(&c)]).return_value;
        let expected_qm31 = (b.clone() / c).unwrap();
        assert_eq!(
            result,
            jit_enum!(0, jit_struct!(Value::from(&expected_qm31)))
        );

        let result =
            run_program(&program, "run_test", &[Value::from(&b), Value::from(&d)]).return_value;
        if let Value::Enum { tag, .. } = result {
            assert_eq!(tag, 1);
        } else {
            panic!("Expected a Value::Enum()");
        }
    }

    #[test]
    fn run_from_m31() {
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/qm31_from_m31");

        let result = run_program(&program, "run_test_with_0", &[]).return_value;
        assert_eq!(result, Value::QM31(0, 0, 0, 0));

        let result = run_program(&program, "run_test_with_1", &[]).return_value;
        assert_eq!(result, Value::QM31(1, 0, 0, 0));

        let result = run_program(&program, "run_test_with_big_coefficient", &[]).return_value;
        assert_eq!(result, Value::QM31(0x60713d44, 0, 0, 0));
    }

    #[test]
    fn run_m31_add() {
        // TODO: Refactor cairo functions to receive m31 as parameters so we don't need different ones
        // to test different cases and we can unify them into one. This can be done when issue #1217 gets closed.
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/m31_add");
        let expected_range = Range {
            lower: 0.into(),
            upper: M31_PRIME.into(),
        };
        let result = run_program(&program, "run_test_1", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(2),
                range: expected_range.clone()
            }
        );

        let result = run_program(&program, "run_test_2", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(0x367db914),
                range: expected_range.clone()
            }
        );

        let result = run_program(&program, "run_test_3", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(0),
                range: expected_range.clone()
            }
        );
    }

    #[test]
    fn run_m31_sub() {
        // TODO: Refactor cairo functions to receive m31 as parameters so we don't need different ones
        // to test different cases and we can unify them into one. This can be done when issue #1217 gets closed.
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/m31_sub");
        let expected_range = Range {
            lower: 0.into(),
            upper: M31_PRIME.into(),
        };
        let result = run_program(&program, "run_test_1", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(1),
                range: expected_range.clone()
            }
        );

        let result = run_program(&program, "run_test_2", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(0x7ffffff9),
                range: expected_range.clone()
            }
        );

        let result = run_program(&program, "run_test_3", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(0x7ffffffe),
                range: expected_range.clone()
            }
        );
    }

    #[test]
    fn run_m31_mul() {
        // TODO: Refactor cairo functions to receive m31 as parameters so we don't need different ones
        // to test different cases and we can unify them into one. This can be done when issue #1217 gets closed.
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/m31_mul");
        let expected_range = Range {
            lower: 0.into(),
            upper: M31_PRIME.into(),
        };
        let result = run_program(&program, "run_test_1", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(25),
                range: expected_range.clone()
            }
        );

        let result = run_program(&program, "run_test_2", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(0x69274523),
                range: expected_range.clone()
            }
        );

        let result = run_program(&program, "run_test_3", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(0x7ffffffd),
                range: expected_range.clone()
            }
        );
    }

    #[test]
    fn run_m31_div() {
        // TODO: Refactor cairo functions to receive m31 as parameters so we don't need different ones
        // to test different cases and we can unify them into one. This can be done when issue #1217 gets closed.
        let program = get_compiled_program("test_data_artifacts/programs/libfuncs/m31_div");
        let expected_range = Range {
            lower: 0.into(),
            upper: M31_PRIME.into(),
        };
        let result = run_program(&program, "run_test_1", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(5),
                range: expected_range.clone()
            }
        );

        let result = run_program(&program, "run_test_2", &[]).return_value;
        assert_eq!(
            result,
            Value::BoundedInt {
                value: Felt252::from(0x5138acb),
                range: expected_range.clone()
            }
        );
    }
}
