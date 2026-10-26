//! # Const globals metadata
//!
//! Tracks which const types already have their value emitted as a constant global, so that the
//! const libfuncs emit each constant once per module and every call site just takes its address.

use cairo_lang_sierra::ids::ConcreteTypeId;
use std::collections::HashSet;

#[derive(Default)]
pub struct ConstGlobalsMeta {
    emitted: HashSet<u64>,
}

impl ConstGlobalsMeta {
    /// Marks the const type as emitted, returning `true` if it was not emitted before.
    pub fn mark_emitted(&mut self, const_type: &ConcreteTypeId) -> bool {
        self.emitted.insert(const_type.id)
    }
}
