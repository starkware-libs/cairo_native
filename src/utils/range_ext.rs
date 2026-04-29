use cairo_lang_sierra::extensions::utils::Range;
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::One;

pub trait RangeExt {
    /// Width in bits when the offset is zero (aka. the natural representation).
    fn zero_based_bit_width(&self) -> u32;
    /// Width in bits when the offset is not necessarily zero (aka. the compact representation).
    fn offset_bit_width(&self) -> u32;
}

impl RangeExt for Range {
    fn zero_based_bit_width(&self) -> u32 {
        // Formula for unsigned integers:
        //     x.bits()
        //
        // Formula for signed values (n-bit two's complement holds
        // [-2^(n-1), 2^(n-1) - 1]):
        //   - Positive: x.magnitude().bits() + 1
        //   - Negative: (x.magnitude() - BigUint::one()).bits() + 1
        //   - Zero: 0

        let width = if self.lower.sign() == Sign::Minus {
            let lower_width = (self.lower.magnitude() - BigUint::one()).bits() + 1;
            let upper_width = {
                let upper = &self.upper - &BigInt::one();
                match upper.sign() {
                    Sign::Minus => (upper.magnitude() - BigUint::one()).bits() + 1,
                    Sign::NoSign => 0,
                    Sign::Plus => upper.magnitude().bits() + 1,
                }
            };

            lower_width.max(upper_width) as u32
        } else {
            (&self.upper - &BigInt::one()).bits() as u32
        };

        // FIXME: Workaround for segfault in canonicalization (including LLVM 19).
        width.max(1)
    }

    fn offset_bit_width(&self) -> u32 {
        // FIXME: Workaround for segfault in canonicalization (including LLVM 19).
        ((self.size() - BigInt::one()).bits() as u32).max(1)
    }
}
