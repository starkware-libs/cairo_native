//! # Implementations of `AbiArgument` for the `x86_64` architecture.
//!
//! The x86_64 architecture uses 6 64-bit registers for arguments. This means that the first 48
//! bytes of the buffer will go into registers while the rest will be on the stack.
//!
//! The values that span multiple registers may be split or moved into the stack completely in some
//! cases, having to pad a register. In those cases the amount of usable register space is reduced
//! to only 40 bytes.

#![cfg(target_arch = "x86_64")]

use super::AbiArgument;
use crate::{error::Error, starknet::U256, utils::get_integer_layout};
use num_traits::ToBytes;
use starknet_types_core::felt::Felt;

fn align_to(buffer: &mut Vec<u8>, align: usize) {
    buffer.resize(buffer.len().next_multiple_of(align), 0);
}

impl AbiArgument for bool {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for u8 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for i8 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for u16 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for i16 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for u32 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for i32 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for u64 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&self.to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for i64 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        buffer.extend_from_slice(&self.to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for u128 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() >= 40 {
            align_to(buffer, get_integer_layout(128).align());
        }

        buffer.extend_from_slice(&self.to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for i128 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() >= 40 {
            align_to(buffer, get_integer_layout(128).align());
        }

        buffer.extend_from_slice(&self.to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for Felt {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() >= 40 {
            align_to(buffer, get_integer_layout(252).align());
        }

        buffer.extend_from_slice(&self.to_bytes_le());
        Ok(())
    }
}

impl AbiArgument for U256 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        self.lo.to_bytes(buffer)?;
        self.hi.to_bytes(buffer)
    }
}

impl AbiArgument for [u8; 31] {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        // The `bytes31` type is treated as a 248-bit integer, therefore it follows the same
        // splitting rules as them.

        if buffer.len() >= 40 {
            align_to(buffer, get_integer_layout(252).align());
        }

        buffer.extend_from_slice(self);
        buffer.push(0);
        Ok(())
    }
}

impl<T> AbiArgument for *const T {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        <u64 as AbiArgument>::to_bytes(&(*self as u64), buffer)
    }
}

impl<T> AbiArgument for *mut T {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        <u64 as AbiArgument>::to_bytes(&(*self as u64), buffer)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn u128_stack_split() {
        let mut buffer = vec![0; 40];
        u128::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 48].into_iter().chain([0xFF; 16]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn felt_stack_split() {
        // Only a single u64 spilled into the stack.
        let mut buffer = vec![0; 24];
        Felt::from_hex("0x00ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
            .unwrap()
            .to_bytes(&mut buffer)
            .unwrap();
        assert_eq!(
            buffer,
            [0; 24]
                .into_iter()
                .chain([0xFF; 31])
                .chain([0x00])
                .collect::<Vec<_>>()
        );

        // Half the felt spilled into the stack.
        let mut buffer = vec![0; 32];
        Felt::from_hex("0x00ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
            .unwrap()
            .to_bytes(&mut buffer)
            .unwrap();
        assert_eq!(
            buffer,
            [0; 32]
                .into_iter()
                .chain([0xFF; 31])
                .chain([0x00])
                .collect::<Vec<_>>()
        );

        // All the felt spilled into the stack (with padding).
        let mut buffer = vec![0; 40];
        Felt::from_hex("0x00ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
            .unwrap()
            .to_bytes(&mut buffer)
            .unwrap();
        assert_eq!(
            buffer,
            [0; 48]
                .into_iter()
                .chain([0xFF; 31])
                .chain([0x00])
                .collect::<Vec<_>>()
        );
    }
}
