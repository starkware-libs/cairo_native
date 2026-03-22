//! # Implementations of `AbiArgument` for the `aarch64` architecture.
//!
//! The aarch64 architecture uses 8 64-bit registers for arguments. This means that the first 64
//! bytes of the buffer will go into registers while the rest will be on the stack.
//!
//! The values that span multiple registers may be split or moved into the stack completely in some
//! cases, having to pad a register. In those cases the amount of usable register space is reduced
//! to only 56 bytes.

#![cfg(target_arch = "aarch64")]

use super::AbiArgument;
use crate::{error::Error, starknet::U256, utils::get_integer_layout};
use num_traits::ToBytes;
use starknet_types_core::felt::Felt;

fn align_to(buffer: &mut Vec<u8>, align: usize) {
    buffer.resize(buffer.len().next_multiple_of(align), 0);
}

impl AbiArgument for bool {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() < 64 {
            buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        } else {
            align_to(buffer, get_integer_layout(1).align());
            buffer.push((*self) as u8);
        }
        Ok(())
    }
}

impl AbiArgument for u8 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() < 64 {
            buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        } else {
            align_to(buffer, get_integer_layout(8).align());
            buffer.push(*self);
        }
        Ok(())
    }
}

impl AbiArgument for i8 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() < 64 {
            buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        } else {
            align_to(buffer, get_integer_layout(8).align());
            buffer.extend_from_slice(&self.to_ne_bytes());
        }
        Ok(())
    }
}

impl AbiArgument for u16 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() < 64 {
            buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        } else {
            align_to(buffer, get_integer_layout(16).align());
            buffer.extend_from_slice(&self.to_ne_bytes());
        }
        Ok(())
    }
}

impl AbiArgument for i16 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() < 64 {
            buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        } else {
            align_to(buffer, get_integer_layout(16).align());
            buffer.extend_from_slice(&self.to_ne_bytes());
        }
        Ok(())
    }
}

impl AbiArgument for u32 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() < 64 {
            buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        } else {
            align_to(buffer, get_integer_layout(32).align());
            buffer.extend_from_slice(&self.to_ne_bytes());
        }
        Ok(())
    }
}

impl AbiArgument for i32 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() < 64 {
            buffer.extend_from_slice(&(*self as u64).to_ne_bytes());
        } else {
            align_to(buffer, get_integer_layout(32).align());
            buffer.extend_from_slice(&self.to_ne_bytes());
        }
        Ok(())
    }
}

impl AbiArgument for u64 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() >= 64 {
            align_to(buffer, get_integer_layout(64).align());
        }
        buffer.extend_from_slice(&self.to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for i64 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() >= 64 {
            align_to(buffer, get_integer_layout(64).align());
        }
        buffer.extend_from_slice(&self.to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for u128 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() >= 56 {
            align_to(buffer, get_integer_layout(128).align());
        }
        buffer.extend_from_slice(&self.to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for i128 {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() >= 56 {
            align_to(buffer, get_integer_layout(128).align());
        }
        buffer.extend_from_slice(&self.to_ne_bytes());
        Ok(())
    }
}

impl AbiArgument for Felt {
    fn to_bytes(&self, buffer: &mut Vec<u8>) -> Result<(), Error> {
        if buffer.len() >= 56 {
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
        if buffer.len() >= 56 {
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
    fn u8_to_bytes() {
        // Buffer initially empty
        let mut buffer = vec![];
        u8::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, [u8::MAX, 0, 0, 0, 0, 0, 0, 0]);

        // Buffer initially filled with 70 zeros (len > 64)
        let mut buffer = vec![0; 70];
        u8::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 70].into_iter().chain([u8::MAX]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn i8_to_bytes() {
        // Buffer initially empty
        let mut buffer = vec![];
        i8::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, [i8::MAX as u8, 0, 0, 0, 0, 0, 0, 0]);

        // Buffer initially empty with negative value
        let mut buffer = vec![];
        i8::MIN.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, [128, 255, 255, 255, 255, 255, 255, 255]);

        // Buffer initially filled with 70 zeros (len > 64)
        let mut buffer = vec![0; 70];
        i8::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 70]
                .into_iter()
                .chain([i8::MAX as u8])
                .collect::<Vec<_>>()
        );

        // Buffer initially filled with 70 zeros (len > 64) and negative value
        let mut buffer = vec![0; 70];
        i8::MIN.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, [0; 70].into_iter().chain([128]).collect::<Vec<_>>());
    }

    #[test]
    fn u16_to_bytes() {
        // Buffer initially empty
        let mut buffer = vec![];
        u16::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, vec![u8::MAX, u8::MAX, 0, 0, 0, 0, 0, 0]);

        // Buffer initially filled with 70 zeros (len > 64)
        let mut buffer = vec![0; 70];
        u16::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 70]
                .into_iter()
                .chain(vec![u8::MAX, u8::MAX])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn i16_to_bytes() {
        // Buffer initially empty
        let mut buffer = vec![];
        i16::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, vec![u8::MAX, i8::MAX as u8, 0, 0, 0, 0, 0, 0]);

        // Buffer initially empty with negative value
        let mut buffer = vec![];
        i16::MIN.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, [0, 128, 255, 255, 255, 255, 255, 255]);

        // Buffer initially filled with 70 zeros (len > 64)
        let mut buffer = vec![0; 70];
        i16::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 70]
                .into_iter()
                .chain(vec![u8::MAX, i8::MAX as u8])
                .collect::<Vec<_>>()
        );

        // Buffer initially filled with 70 zeros (len > 64) and negative value
        let mut buffer = vec![0; 70];
        i16::MIN.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 70].into_iter().chain([0, 128]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn u32_to_bytes() {
        // Buffer initially empty
        let mut buffer = vec![];
        u32::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            vec![u8::MAX; 4]
                .into_iter()
                .chain(vec![0; 4])
                .collect::<Vec<_>>()
        );

        // Buffer initially filled with 70 zeros (len > 64)
        let mut buffer = vec![0; 70];
        u32::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 72]
                .into_iter()
                .chain(vec![u8::MAX; 4])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn i32_to_bytes() {
        // Buffer initially empty
        let mut buffer = vec![];
        i32::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            vec![u8::MAX, u8::MAX, u8::MAX, i8::MAX as u8, 0, 0, 0, 0]
        );

        // Buffer initially empty with negative value
        let mut buffer = vec![];
        i32::MIN.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, [0, 0, 0, 128, 255, 255, 255, 255]);

        // Buffer initially filled with 70 zeros (len > 64)
        let mut buffer = vec![0; 70];
        i32::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 72]
                .into_iter()
                .chain(vec![u8::MAX, u8::MAX, u8::MAX, i8::MAX as u8])
                .collect::<Vec<_>>()
        );

        // Buffer initially filled with 70 zeros (len > 64) and negative value
        let mut buffer = vec![0; 70];
        i32::MIN.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 72]
                .into_iter()
                .chain([0, 0, 0, 128])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn u64_to_bytes() {
        // Buffer initially empty
        let mut buffer = vec![];
        u64::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, u64::MAX.to_ne_bytes().to_vec());

        // Buffer initially filled with 70 zeros (len > 64)
        let mut buffer = vec![0; 70];
        u64::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 72]
                .into_iter()
                .chain(u64::MAX.to_ne_bytes().to_vec())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn i64_to_bytes() {
        // Buffer initially empty
        let mut buffer = vec![];
        i64::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, i64::MAX.to_ne_bytes().to_vec());

        // Buffer initially empty with negative value
        let mut buffer = vec![];
        i64::MIN.to_bytes(&mut buffer).unwrap();
        assert_eq!(buffer, i64::MIN.to_ne_bytes().to_vec());

        // Buffer initially filled with 70 zeros (len > 64)
        let mut buffer = vec![0; 70];
        i64::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 72]
                .into_iter()
                .chain(i64::MAX.to_ne_bytes().to_vec())
                .collect::<Vec<_>>()
        );

        // Buffer initially filled with 70 zeros (len > 64) and negative value
        let mut buffer = vec![0; 70];
        i64::MIN.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 72]
                .into_iter()
                .chain(i64::MIN.to_ne_bytes().to_vec())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn u128_stack_split() {
        let mut buffer = vec![0; 56];
        u128::MAX.to_bytes(&mut buffer).unwrap();
        assert_eq!(
            buffer,
            [0; 64].into_iter().chain([0xFF; 16]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn felt_stack_split() {
        // Only a single u64 spilled into the stack.
        let mut buffer = vec![0; 40];
        Felt::from_hex("0x00ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
            .unwrap()
            .to_bytes(&mut buffer)
            .unwrap();
        assert_eq!(
            buffer,
            [0; 40]
                .into_iter()
                .chain([0xFF; 31])
                .chain([0x00])
                .collect::<Vec<_>>()
        );

        // Half the felt spilled into the stack.
        let mut buffer = vec![0; 48];
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

        // All the felt spilled into the stack (with padding).
        let mut buffer = vec![0; 56];
        Felt::from_hex("0x00ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")
            .unwrap()
            .to_bytes(&mut buffer)
            .unwrap();
        assert_eq!(
            buffer,
            [0; 64]
                .into_iter()
                .chain([0xFF; 31])
                .chain([0x00])
                .collect::<Vec<_>>()
        );
    }
}
