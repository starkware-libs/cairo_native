use super::U256;
use crate::Value;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Secp256k1Point {
    pub x: U256,
    pub y: U256,
    pub is_infinity: bool,
}

impl Secp256k1Point {
    #[allow(unused)]
    pub fn into_value(self) -> Value {
        // Sierra has no slot for `is_infinity`; encode the identity element as the
        // canonical (0, 0) sentinel so `from_value` can recover the flag losslessly.
        // (0, 0) is not on the curve, so this aliasing is unambiguous.
        let (x, y) = if self.is_infinity {
            (U256 { lo: 0, hi: 0 }, U256 { lo: 0, hi: 0 })
        } else {
            (self.x, self.y)
        };
        Value::Struct(vec![
            Value::Struct(vec![Value::U128(x.lo), Value::U128(x.hi)]),
            Value::Struct(vec![Value::U128(y.lo), Value::U128(y.hi)]),
        ])
    }

    pub fn from_value(v: Value) -> Self {
        let Value::Struct(mut v) = v else { panic!() };

        let y = U256::from_value(v.remove(1));
        let x = U256::from_value(v.remove(0));

        // Recover the flag from the (0, 0) sentinel — see `into_value`.
        let is_infinity = x.lo == 0 && x.hi == 0 && y.lo == 0 && y.hi == 0;

        Self { x, y, is_infinity }
    }
}
