//! All types use arena allocation, so dropping is a no-op. Memory is reclaimed
//! when the arena is destroyed at invocation end.

use super::LibfuncHelper;
use crate::{error::Result, metadata::MetadataStorage};
use cairo_lang_sierra::{
    extensions::{
        core::{CoreLibfunc, CoreType},
        lib_func::SignatureOnlyConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::ir::{Block, Location};
use melior::Context;

/// Generate MLIR operations for the `drop` libfunc.
pub fn build<'ctx, 'this>(
    _context: &'ctx Context,
    _registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    _metadata: &mut MetadataStorage,
    _info: &SignatureOnlyConcreteLibfunc,
) -> Result<()> {
    // Noop: arena owns all memory.
    helper.br(entry, 0, &[], location)
}
