use super::EvalAction;
use crate::Value;
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        ec::EcConcreteLibfunc,
        lib_func::SignatureOnlyConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use num_traits::identities::Zero;
use rand::Rng;
use smallvec::smallvec;
use starknet_crypto::Felt;
use starknet_curve::curve_params::BETA;
use starknet_types_core::curve::{AffinePoint, ProjectivePoint};
use std::ops::Mul;
use std::ops::Neg;

// todo: verify these are correct.

pub fn eval(
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    selector: &EcConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    match selector {
        EcConcreteLibfunc::IsZero(info) => eval_is_zero(registry, info, args),
        EcConcreteLibfunc::Neg(info) => eval_neg(registry, info, args),
        EcConcreteLibfunc::StateAdd(info) => eval_state_add(registry, info, args),
        EcConcreteLibfunc::TryNew(info) => eval_new(registry, info, args),
        EcConcreteLibfunc::StateFinalize(info) => eval_state_finalize(registry, info, args),
        EcConcreteLibfunc::StateInit(info) => eval_state_init(registry, info, args),
        EcConcreteLibfunc::StateAddMul(info) => eval_state_add_mul(registry, info, args),
        EcConcreteLibfunc::PointFromX(info) => eval_point_from_x(registry, info, args),
        EcConcreteLibfunc::UnwrapPoint(info) => eval_unwrap_point(registry, info, args),
        EcConcreteLibfunc::Zero(info) => eval_zero(registry, info, args),
        EcConcreteLibfunc::NegNz(info) => eval_neg(registry, info, args),
    }
}

fn eval_is_zero(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [value @ Value::EcPoint { x: _, y }]: [Value; 1] = args.try_into().unwrap() else {
        panic!()
    };
    // To check whether `(x, y) = (0, 0)` (the zero point), it is enough to check
    // whether `y = 0`, since there is no point on the curve with y = 0.
    if y.is_zero() {
        EvalAction::NormalBranch(0, smallvec![])
    } else {
        EvalAction::NormalBranch(1, smallvec![value])
    }
}

fn eval_unwrap_point(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::EcPoint { x, y }]: [Value; 1] = args.try_into().unwrap() else {
        panic!()
    };
    EvalAction::NormalBranch(0, smallvec![Value::Felt(x), Value::Felt(y)])
}

fn eval_neg(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::EcPoint { x, y }]: [Value; 1] = args.try_into().unwrap() else {
        panic!()
    };

    let point = AffinePoint::new(x, y).unwrap().neg();

    EvalAction::NormalBranch(
        0,
        smallvec![Value::EcPoint {
            x: point.x(),
            y: point.y(),
        }],
    )
}

fn eval_new(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::Felt(x), Value::Felt(y)]: [Value; 2] = args.try_into().unwrap() else {
        panic!()
    };

    match AffinePoint::new(x, y) {
        Ok(point) => EvalAction::NormalBranch(
            0,
            smallvec![Value::EcPoint {
                x: point.x(),
                y: point.y(),
            }],
        ),
        Err(_) => EvalAction::NormalBranch(1, smallvec![]),
    }
}

fn eval_state_init(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    _args: Vec<Value>,
) -> EvalAction {
    let state = random_ec_point();

    EvalAction::NormalBranch(
        0,
        smallvec![Value::EcState {
            x0: state.x(),
            y0: state.y(),
            x1: state.x(),
            y1: state.y(),
        }],
    )
}

fn eval_state_add(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::EcState { x0, y0, x1, y1 }, Value::EcPoint { x, y }]: [Value; 2] =
        args.try_into().unwrap()
    else {
        panic!()
    };

    let mut state = ProjectivePoint::from_affine(x0, y0).unwrap();
    let point = AffinePoint::new(x, y).unwrap();

    state += &point;
    let state = state.to_affine().unwrap();

    EvalAction::NormalBranch(
        0,
        smallvec![Value::EcState {
            x0: state.x(),
            y0: state.y(),
            x1,
            y1
        }],
    )
}

fn eval_state_add_mul(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [ec @ Value::Unit, Value::EcState { x0, y0, x1, y1 }, Value::Felt(scalar), Value::EcPoint { x, y }]: [Value; 4] =
        args.try_into().unwrap()
    else {
        panic!()
    };

    let mut state = ProjectivePoint::from_affine(x0, y0).unwrap();
    let point = ProjectivePoint::from_affine(x, y).unwrap();

    state += &point.mul(scalar);
    let state = state.to_affine().unwrap();

    EvalAction::NormalBranch(
        0,
        smallvec![
            ec,
            Value::EcState {
                x0: state.x(),
                y0: state.y(),
                x1,
                y1
            }
        ],
    )
}

fn eval_state_finalize(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [Value::EcState { x0, y0, x1, y1 }]: [Value; 1] = args.try_into().unwrap() else {
        panic!()
    };

    let state = ProjectivePoint::from_affine(x0, y0).unwrap();
    let random_point = ProjectivePoint::from_affine(x1, y1).unwrap();

    if state.x() == random_point.x() && state.y() == random_point.y() {
        EvalAction::NormalBranch(1, smallvec![])
    } else {
        let point = &state - &random_point;
        let point = point.to_affine().unwrap();
        EvalAction::NormalBranch(
            0,
            smallvec![Value::EcPoint {
                x: point.x(),
                y: point.y(),
            }],
        )
    }
}

fn eval_point_from_x(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    args: Vec<Value>,
) -> EvalAction {
    let [range_check @ Value::Unit, Value::Felt(x)]: [Value; 2] = args.try_into().unwrap() else {
        panic!()
    };

    // https://github.com/starkware-libs/cairo/blob/aaad921bba52e729dc24ece07fab2edf09ccfa15/crates/cairo-lang-sierra-to-casm/src/invocations/ec.rs#L63

    let x2 = x * x;
    let x3 = x2 * x;
    let alpha_x_plus_beta = x + BETA;
    let rhs = x3 + alpha_x_plus_beta;
    let y = rhs.sqrt().unwrap_or_else(|| Felt::from(3) * rhs);

    match AffinePoint::new(x, y) {
        Ok(point) => EvalAction::NormalBranch(
            0,
            smallvec![
                range_check,
                Value::EcPoint {
                    x: point.x(),
                    y: point.y(),
                }
            ],
        ),
        Err(_) => EvalAction::NormalBranch(1, smallvec![range_check]),
    }
}

fn random_ec_point() -> AffinePoint {
    // https://github.com/starkware-libs/cairo/blob/aaad921bba52e729dc24ece07fab2edf09ccfa15/crates/cairo-lang-runner/src/casm_run/mod.rs#L1802
    let mut rng = rand::rng();
    let (random_x, random_y) = loop {
        // Randominzing 31 bytes to make sure is in range.
        let x_bytes: [u8; 31] = rng.random();
        let random_x = Felt::from_bytes_be_slice(&x_bytes);
        let random_y_squared = random_x * random_x * random_x + random_x + BETA;
        if let Some(random_y) = random_y_squared.sqrt() {
            break (random_x, random_y);
        }
    };

    AffinePoint::new(random_x, random_y).unwrap()
}

fn eval_zero(
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _info: &SignatureOnlyConcreteLibfunc,
    _args: Vec<Value>,
) -> EvalAction {
    EvalAction::NormalBranch(
        0,
        smallvec![Value::EcPoint {
            x: 0.into(),
            y: 0.into()
        }],
    )
}
