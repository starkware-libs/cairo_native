//! # `BoundedInt` type
//!
//! A `BoundedInt` is a int with a lower and high bound.
//!
//! It's represented as the offseted range using the minimal number of bits. For example:
//!   - 10 as `BoundedInt<10, 20>` is represented as `0 : i4`.
//!   - 15 as `BoundedInt<10, 20>` is represented as `5 : i4`.
//!   - 1 as `BoundedInt<1, 1>` is represented as `0 : i0`.

use super::WithSelf;
use crate::{error::Result, metadata::MetadataStorage, utils::RangeExt};
use cairo_lang_sierra::{
    extensions::{
        bounded_int::BoundedIntConcreteType,
        core::{CoreLibfunc, CoreType},
    },
    program_registry::ProgramRegistry,
};
use melior::{
    ir::{r#type::IntegerType, Module, Type},
    Context,
};

/// Build the MLIR type.
///
/// Check out [the module](self) for more info.
pub fn build<'ctx>(
    context: &'ctx Context,
    _module: &Module<'ctx>,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    _metadata: &mut MetadataStorage,
    info: WithSelf<BoundedIntConcreteType>,
) -> Result<Type<'ctx>> {
    let n_bits = info.range.repr_bit_width();
    Ok(IntegerType::new(context, n_bits).into())
}
