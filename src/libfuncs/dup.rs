//! # State value duplication libfunc
//!
//! All types use arena allocation, so duplication is a simple bitwise copy.
//! No deep cloning is needed because the arena owns all memory.

use super::LibfuncHelper;
use crate::{error::Result, metadata::MetadataStorage};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        lib_func::SignatureOnlyConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    helpers::BuiltinBlockExt,
    ir::{Block, Location},
    Context,
};

/// Generate MLIR operations for the `dup` libfunc.
///
/// Since all types use arena allocation, dup is just a bitwise copy — return
/// the same value twice.
pub fn build<'ctx, 'this>(
    _context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    let value = entry.arg(0)?;
    helper.br(entry, 0, &[value, value], location)
}
