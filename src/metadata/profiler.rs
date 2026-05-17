#![cfg(feature = "with-libfunc-profiling")]
//! The libfunc profiling feature is used to generate information about every libfunc executed in a sierra program.
//!
//! When this feature is used, the compiler will call the important methods:
//!
//! 1. `measure_timestamp`: called before every libfunc execution.
//!
//! 2. `push_frame`: called before every branching operation. This method will also call `measure_timestamp`. This,
//!    with the timestamp calculated before the execution, will allow to measure each statement's execution time.
//!    If for some reason, the statement delta time could not be gathered, we just record an unit value, recording that
//!    we executed the given statement.
//!
//! Once the program execution finished and the information was gathered, the `get_profile` method can be called.
//! It groups the samples by libfunc, and returns all data related to each libfunc.
//!
//! As well as with the trace-dump feature, in the context of Starknet contracts, we need to add support for building
//! profiles for multiple executions. To do so, we need two important elements, which must be set before every contract
//! execution:
//!
//! 1. A global static hashmap to map every profile ID to its respective profiler. See `LIBFUNC_PROFILE`.
//!
//! 2. A counter to track the ID of the current profiler, which gets updated every time we switch to another
//!    contract. Since a contract can call other contracts, we need a way of restoring the counter after every execution.
//!
//! See `cairo-native-run` for an example on how to do it.

use crate::error::{Error, Result};
use cairo_lang_sierra::{
    ids::ConcreteLibfuncId,
    program::{Program, Statement, StatementIdx},
};
use melior::{
    dialect::{
        arith::{self, CmpiPredicate},
        llvm::{self},
        memref, ods,
    },
    helpers::{ArithBlockExt, BuiltinBlockExt, LlvmBlockExt},
    ir::{
        attribute::{FlatSymbolRefAttribute, StringAttribute, TypeAttribute},
        operation::OperationBuilder,
        r#type::{IntegerType, MemRefType},
        Attribute, Block, BlockLike, Identifier, Location, Module, Region, Value,
    },
    Context,
};

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    ffi::c_void,
    ptr,
    sync::{LazyLock, Mutex},
};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ProfilerBinding {
    PushStmt,
    ProfileId,
}

impl ProfilerBinding {
    pub const fn symbol(self) -> &'static str {
        match self {
            ProfilerBinding::PushStmt => "cairo_native__profiler__push_stmt",
            ProfilerBinding::ProfileId => "cairo_native__profiler__profile_id",
        }
    }

    const fn function_ptr(self) -> *const () {
        match self {
            ProfilerBinding::PushStmt => ProfilerImpl::push_stmt as *const (),
            ProfilerBinding::ProfileId => ptr::null(),
        }
    }
}

#[derive(Clone, Default)]
pub struct ProfilerMeta {
    active_map: RefCell<HashSet<ProfilerBinding>>,
}

impl ProfilerMeta {
    pub fn new() -> Self {
        Self {
            active_map: RefCell::new(HashSet::new()),
        }
    }

    /// Register the global for the given binding, if not yet registered, and return
    /// a pointer to the stored value.
    ///
    /// For the function to be available, `setup_runtime` must be called before running the module
    fn build_function<'c, 'a>(
        &self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
        binding: ProfilerBinding,
    ) -> Result<Value<'c, 'a>> {
        if self.active_map.borrow_mut().insert(binding) {
            module.body().append_operation(
                ods::llvm::mlir_global(
                    context,
                    Region::new(),
                    TypeAttribute::new(llvm::r#type::pointer(context, 0)),
                    StringAttribute::new(context, binding.symbol()),
                    Attribute::parse(context, "#llvm.linkage<weak>")
                        .ok_or(Error::ParseAttributeError)?,
                    location,
                )
                .into(),
            );
        }

        let global_address = block.append_op_result(
            ods::llvm::mlir_addressof(
                context,
                llvm::r#type::pointer(context, 0),
                FlatSymbolRefAttribute::new(context, binding.symbol()),
                location,
            )
            .into(),
        )?;

        Ok(block.load(
            context,
            location,
            global_address,
            llvm::r#type::pointer(context, 0),
        )?)
    }

    pub fn build_profile_id<'c, 'a>(
        &self,
        context: &'c Context,
        module: &Module,
        block: &'a Block<'c>,
        location: Location<'c>,
    ) -> Result<Value<'c, 'a>> {
        if self
            .active_map
            .borrow_mut()
            .insert(ProfilerBinding::ProfileId)
        {
            module.body().append_operation(memref::global(
                context,
                ProfilerBinding::ProfileId.symbol(),
                None,
                MemRefType::new(IntegerType::new(context, 64).into(), &[], None, None),
                None,
                false,
                None,
                location,
            ));
        }

        let trace_profile_ptr = block
            .append_op_result(memref::get_global(
                context,
                ProfilerBinding::ProfileId.symbol(),
                MemRefType::new(IntegerType::new(context, 64).into(), &[], None, None),
                location,
            ))
            .unwrap();

        Ok(block.append_op_result(memref::load(trace_profile_ptr, &[], location))?)
    }

    /// Gets the current timestamp.
    ///
    /// The values returned are:
    /// 1. Timestamp: CPU cycles since its reset.
    /// 2. CPU's id core in which the execution is running (only for x86 arch).
    ///    In case of arm, 0 is always returned as there's no way to know in which
    ///    CPU core the execution was run.
    ///
    /// We use the last value to ensure that both the initial and the end timestamp of
    /// a libfunc's execution were calculated by the same core. This is to avoid gathering
    /// invalid data
    #[cfg(target_arch = "x86_64")]
    pub fn measure_timestamp<'c, 'a>(
        &self,
        context: &'c Context,
        block: &'a Block<'c>,
        location: Location<'c>,
    ) -> Result<(Value<'c, 'a>, Value<'c, 'a>)> {
        let i32_ty = IntegerType::new(context, 32).into();
        let i64_ty = IntegerType::new(context, 64).into();
        let k32 = block.const_int_from_type(context, location, 32, i64_ty)?;

        // edx:eax := TimeStampCounter   (clock value)
        // ecx     := IA32_TSC_AUX[31:0] (core ID)
        let value = block.append_op_result(
            OperationBuilder::new("llvm.inline_asm", location)
                .add_attributes(&[
                    (
                        Identifier::new(context, "asm_string"),
                        StringAttribute::new(context, "mfence\nrdtscp\nlfence").into(),
                    ),
                    (
                        Identifier::new(context, "has_side_effects"),
                        Attribute::unit(context),
                    ),
                    (
                        Identifier::new(context, "constraints"),
                        StringAttribute::new(context, "={edx},={eax},={ecx}").into(),
                    ),
                ])
                .add_results(&[llvm::r#type::r#struct(
                    context,
                    &[i32_ty, i32_ty, i32_ty],
                    false,
                )])
                .build()?,
        )?;
        let value_hi = block.extract_value(context, location, value, i32_ty, 0)?;
        let value_lo = block.extract_value(context, location, value, i32_ty, 1)?;
        let core_idx = block.extract_value(context, location, value, i32_ty, 2)?;

        let value_hi = block.extui(value_hi, i64_ty, location)?;
        let value_lo = block.extui(value_lo, i64_ty, location)?;
        let value = block.shli(value_hi, k32, location)?;
        let value = block.append_op_result(arith::ori(value, value_lo, location))?;

        Ok((value, core_idx))
    }

    /// Gets the current timestamp.
    ///
    /// The values returned are:
    /// 1. Timestamp: CPU cycles since its reset.
    /// 2. CPU's id core in which the program is running (only for x86 arch).
    ///    In case of arm, 0 is always returned as there's no way to know in which
    ///    CPU core the execution was run.
    ///
    /// We use the last value to ensure that both the initial and the end timestamp of
    /// a libfunc's execution were calculated by the same core. This is to avoid gathering
    /// invalid data
    #[cfg(target_arch = "aarch64")]
    pub fn measure_timestamp<'c, 'a>(
        &self,
        context: &'c Context,
        block: &'a Block<'c>,
        location: Location<'c>,
    ) -> Result<(Value<'c, 'a>, Value<'c, 'a>)> {
        use melior::helpers::ArithBlockExt;

        let i64_ty = IntegerType::new(context, 64).into();

        let value = block.append_op_result(
            OperationBuilder::new("llvm.inline_asm", location)
                .add_attributes(&[
                    (
                        Identifier::new(context, "asm_string"),
                        StringAttribute::new(context, "isb\nmrs $0, CNTVCT_EL0\nisb").into(),
                    ),
                    (
                        Identifier::new(context, "has_side_effects"),
                        Attribute::unit(context),
                    ),
                    (
                        Identifier::new(context, "constraints"),
                        StringAttribute::new(context, "=r").into(),
                    ),
                ])
                .add_results(&[i64_ty])
                .build()?,
        )?;
        let core_idx = block.const_int(context, location, 0, 64)?;

        Ok((value, core_idx))
    }

    #[allow(clippy::too_many_arguments)]
    /// Receives two timestamps, if they were originated in the same CPU core,
    /// the delta time between these two is calculated. If not, then the delta time is
    /// assigned to -1. Then it pushes the frame, which is made of the statement index
    /// the delta time.
    pub fn push_frame<'c>(
        &self,
        context: &'c Context,
        module: &Module,
        block: &Block<'c>,
        statement_idx: usize,
        // (timestamp, core_idx)
        t0: (Value<'c, '_>, Value<'c, '_>),
        t1: (Value<'c, '_>, Value<'c, '_>),
        location: Location<'c>,
    ) -> Result<()> {
        // If core idx matches:
        //   Calculate time delta.
        //   Write statement idx and time delta.
        // If core idx does not match:
        //   Write statement idx and -1.

        let trace_id = self.build_profile_id(context, module, block, location)?;

        let i64_ty = IntegerType::new(context, 64).into();

        let statement_idx = block.const_int_from_type(context, location, statement_idx, i64_ty)?;
        let is_same_core = block.cmpi(context, CmpiPredicate::Eq, t0.1, t1.1, location)?;

        let delta_value = block.append_op_result(arith::subi(t1.0, t0.0, location))?;
        let invalid_value = block.const_int_from_type(context, location, u64::MAX, i64_ty)?;
        let delta_value = block.append_op_result(arith::select(
            is_same_core,
            delta_value,
            invalid_value,
            location,
        ))?;

        let callback_ptr =
            self.build_function(context, module, block, location, ProfilerBinding::PushStmt)?;

        block.append_operation(
            ods::llvm::call(
                context,
                &[callback_ptr, trace_id, statement_idx, delta_value],
                location,
            )
            .into(),
        );

        Ok(())
    }
}

/// Represents the entire profile of the execution.
///
/// It maps the libfunc ID to a libfunc profile.
pub type Profile = HashMap<ConcreteLibfuncId, LibfuncProfileData>;

/// Represents the profile data for a particular libfunc.
#[derive(Default)]
pub struct LibfuncProfileData {
    /// A vector of execution times, for each time the libfunc was executed.
    /// It expreses the number of CPU cycles completed during the execution.
    pub deltas: Vec<u64>,
    /// If the time delta for a particular execution could not be gathered,
    /// we just increase `extra_counts` by 1.
    pub extra_counts: u64,
}

pub static LIBFUNC_PROFILE: LazyLock<Mutex<HashMap<u64, ProfilerImpl>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Default)]
pub struct ProfilerImpl {
    /// The samples recorded by the profiler. A value of `u64::MAX` implies
    /// that the delta time for a statement could not be gathered.
    pub samples: Vec<(StatementIdx, u64)>,
}

impl ProfilerImpl {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    // Push a profiler frame
    pub extern "C" fn push_stmt(profile_id: u64, statement_idx: u64, tick_delta: u64) {
        let mut profiler = LIBFUNC_PROFILE.lock().unwrap();

        let Some(profiler) = profiler.get_mut(&profile_id) else {
            eprintln!("Could not find libfunc profiler!");
            return;
        };

        profiler
            .samples
            .push((StatementIdx(statement_idx as usize), tick_delta));
    }

    /// Returns the execution profile, grouped by libfunc
    pub fn get_profile(&self, sierra_program: &Program) -> Profile {
        let mut profile = HashMap::<ConcreteLibfuncId, LibfuncProfileData>::new();

        for (statement_idx, tick_delta) in self.samples.iter() {
            if let Statement::Invocation(invocation) = &sierra_program.statements[statement_idx.0] {
                let LibfuncProfileData {
                    deltas,
                    extra_counts,
                } = profile.entry(invocation.libfunc_id.clone()).or_default();

                // A tick_delta equal to u64::MAX implies it is invalid, so we don't take it
                // into account
                if *tick_delta != u64::MAX {
                    deltas.push(*tick_delta);
                } else {
                    *extra_counts += 1;
                }
            }
        }

        profile
    }
}

pub fn setup_runtime(find_symbol_ptr: impl Fn(&str) -> Option<*mut c_void>) {
    let bindings = &[ProfilerBinding::PushStmt];

    for binding in bindings {
        if let Some(global) = find_symbol_ptr(binding.symbol()) {
            let global = global.cast::<*const ()>();
            unsafe { *global = binding.function_ptr() };
        }
    }
}
