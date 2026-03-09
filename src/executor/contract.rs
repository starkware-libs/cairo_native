//! A specialized executor for Starknet contracts, avoiding the overhead of storing the sierra program registry and
//! enabling efficient serialization of the program/data once compiled.
//!
//! This executor heavily relies on the stability of the contract entry point argument order.
//!
//! Right now this order is like the following:
//!
//! 1. One or more builtins (rangecheck, etc)
//! 2. Gas builtin
//! 3. System builtin (the syscall handler)
//! 4. A Array of felts (calldata)
//!
//! ## How it works:
//!
//! The only variable data we need to know at call time is the builtins order,
//! to save this, when first compiling the sierra program (with [`ContractExecutor::new`]) it iterates through all the user
//! defined functions (this includes the contract wrappers, which are the ones that matter)
//! and saves the builtin arguments in a `Vec`.
//!
//! The API provides two more methods: [`ContractExecutor::save`] and [`ContractExecutor::load`].
//!
//! Save can be used to save the compiled program into the given path, alongside it will be saved
//! a json file with the entry points and their builtins (as seen in the example)
//!
//! ```json
//! {"0":{"builtins":[]},"1":{"builtins":["RangeCheck","Gas","System"]}}
//! ```
//!
//! If the given path is "program.so", then at the same location, "program.json" will be saved.
//!
//! When loading, passing the "program.so" path will make it load the program and the "program.json" alongside it.
//!

use crate::{
    arch::AbiArgument,
    clone_option_mut,
    context::NativeContext,
    debug::libfunc_to_name,
    error::{panic::ToNativeAssertError, Error, Result},
    execution_result::{
        BuiltinStats, ContractExecutionResult, ADD_MOD_BUILTIN_SIZE, BITWISE_BUILTIN_SIZE,
        BLAKE_BUILTIN_SIZE, EC_OP_BUILTIN_SIZE, MUL_MOD_BUILTIN_SIZE, PEDERSEN_BUILTIN_SIZE,
        POSEIDON_BUILTIN_SIZE, RANGE_CHECK96_BUILTIN_SIZE, RANGE_CHECK_BUILTIN_SIZE,
        SEGMENT_ARENA_BUILTIN_SIZE,
    },
    executor::{invoke_trampoline, BuiltinCostsGuard},
    metadata::runtime_bindings::setup_runtime,
    module::NativeModule,
    native_assert, native_panic,
    runtime::BLAKE_CALL_COUNT,
    starknet::{handler::StarknetSyscallHandlerCallbacks, StarknetSyscallHandler},
    statistics::{SierraDeclaredTypeStats, SierraFuncStats, Statistics},
    types::{array::ArrayMetadata, TypeBuilder},
    utils::{
        decode_error_message, generate_function_name, get_integer_layout, get_types_total_size,
        libc_free, libc_malloc, BuiltinCosts,
    },
    OptLevel,
};
use bumpalo::Bump;
use cairo_lang_sierra::{
    extensions::{
        circuit::CircuitTypeConcrete,
        core::{CoreConcreteLibfunc, CoreLibfunc, CoreType, CoreTypeConcrete},
        gas::CostTokenType,
        starknet::StarknetTypeConcrete,
        ConcreteLibfunc,
    },
    ids::FunctionId,
    program::{GenFunction, GenStatement, Program, StatementIdx},
    program_registry::ProgramRegistry,
};
use cairo_lang_sierra_to_casm::metadata::MetadataComputationConfig;
use cairo_lang_starknet_classes::contract_class::ContractEntryPoints;
use cairo_lang_starknet_classes::{
    casm_contract_class::ENTRY_POINT_COST, compiler_version::VersionId,
};
use cairo_lang_utils::small_ordered_map::SmallOrderedMap;
use educe::Educe;
use itertools::{chain, Itertools};
use libloading::Library;
use serde::{Deserialize, Serialize};
use starknet_types_core::felt::Felt;
use std::{
    alloc::Layout,
    cmp::Ordering,
    collections::BTreeMap,
    ffi::c_void,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    ptr::{self, NonNull},
    sync::Arc,
    time::Instant,
};
use tempfile::NamedTempFile;

/// Please look at the [module level docs](self).
#[derive(Educe, Clone)]
#[educe(Debug)]
pub struct AotContractExecutor {
    #[educe(Debug(ignore))]
    library: Arc<Library>,
    path: PathBuf,
    contract_info: NativeContractInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeContractInfo {
    pub version: ContractInfoVersion,
    pub entry_points: BTreeMap<Felt, EntryPointInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContractInfoVersion {
    V0,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntryPointInfo {
    pub function_id: u64,
    pub builtins: Vec<BuiltinType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum BuiltinType {
    Bitwise,
    EcOp,
    RangeCheck,
    SegmentArena,
    Poseidon,
    Pedersen,
    RangeCheck96,
    CircuitAdd,
    CircuitMul,
    Gas,
    System,
    BuiltinCosts,
    Blake,
}

impl BuiltinType {
    pub const fn size_in_bytes(&self) -> usize {
        size_of::<u64>()
    }
}

impl AotContractExecutor {
    /// Compile and load a program using a temporary shared library.
    ///
    /// When enabled, compilation stats will be saved to the `stats`. The
    /// initial statistics can be build using the default builder.
    pub fn new(
        program: &Program,
        entry_points: &ContractEntryPoints,
        sierra_version: VersionId,
        opt_level: OptLevel,
        stats: Option<&mut Statistics>,
    ) -> Result<Self> {
        let output_path = NamedTempFile::new()?
            .into_temp_path()
            .keep()
            .to_native_assert_error("can only fail on windows")?;

        let executor = Self::new_into(
            program,
            entry_points,
            sierra_version,
            output_path,
            opt_level,
            stats,
        )?
        .to_native_assert_error("temporary contract path collision")?;

        fs::remove_file(&executor.path)?;
        fs::remove_file(executor.path.with_extension("json"))?;
        Ok(executor)
    }

    /// Compile and load a program into a shared library.
    ///
    /// This function uses a lockfile to support cache sharing between multiple processes. An
    /// attempt to compile a program while the `output_path` is already locked will result in
    /// `Ok(None)` being returned. When this happens, the user should wait until the lock is
    /// released, at which point they can use `AotContractExecutor::from_path` to load it.
    ///
    /// When enabled, compilation stats will be saved to the `stats`. The
    /// initial statistics can be build using the default builder.
    pub fn new_into(
        program: &Program,
        entry_points: &ContractEntryPoints,
        sierra_version: VersionId,
        output_path: impl Into<PathBuf>,
        opt_level: OptLevel,
        stats: Option<&mut Statistics>,
    ) -> Result<Option<Self>> {
        let output_path = output_path.into();
        let lock_file = match LockFile::new(&output_path)? {
            Some(x) => x,
            None => return Ok(None),
        };

        let pre_compilation_instant = Instant::now();

        let context = NativeContext::new();

        let no_eq_solver = match sierra_version.major.cmp(&1) {
            Ordering::Less => false,
            Ordering::Equal => sierra_version.minor >= 4,
            Ordering::Greater => true,
        };

        if let Some(&mut ref mut stats) = stats {
            stats.sierra_type_count = Some(program.type_declarations.len());
            stats.sierra_libfunc_count = Some(program.libfunc_declarations.len());
            stats.sierra_statement_count = Some(program.statements.len());
            stats.sierra_func_count = Some(program.funcs.len());
        }

        // Compile the Sierra program.
        let NativeModule {
            module, registry, ..
        } = context.compile(
            program,
            true,
            Some(MetadataComputationConfig {
                function_set_costs: chain!(
                    entry_points.constructor.iter(),
                    entry_points.external.iter(),
                    entry_points.l1_handler.iter(),
                )
                .map(|x| {
                    (
                        FunctionId::new(x.function_idx as u64),
                        SmallOrderedMap::from_iter([(CostTokenType::Const, ENTRY_POINT_COST)]),
                    )
                })
                .collect(),
                linear_gas_solver: no_eq_solver,
                linear_ap_change_solver: no_eq_solver,
                skip_non_linear_solver_comparisons: false,
                compute_runtime_costs: false,
            }),
            clone_option_mut!(stats),
        )?;

        if let Some(&mut ref mut stats) = stats {
            for type_declaration in &program.type_declarations {
                if let Ok(type_concrete) = registry.get_type(&type_declaration.id) {
                    let type_id = type_declaration.id.id;
                    let type_size = type_concrete.layout(&registry)?.size();
                    stats.sierra_declared_types_stats.insert(
                        type_id,
                        SierraDeclaredTypeStats {
                            size: type_size,
                            as_param_count: 0,
                        },
                    );

                    if let CoreTypeConcrete::Circuit(CircuitTypeConcrete::Circuit(info)) =
                        type_concrete
                    {
                        stats.add_circuit_gates(&info.circuit_info)?;
                    }
                }
            }

            for statement in &program.statements {
                if let GenStatement::Invocation(invocation) = statement {
                    let libfunc = registry.get_libfunc(&invocation.libfunc_id)?;
                    let name = libfunc_to_name(libfunc).to_string();
                    *stats.sierra_libfunc_frequency.entry(name).or_insert(0) += 1;

                    for param in libfunc.param_signatures() {
                        let param_type_id = param.ty.id;
                        if let Some(type_stats) =
                            stats.sierra_declared_types_stats.get_mut(&param_type_id)
                        {
                            type_stats.as_param_count += 1;
                        }
                    }
                }
            }

            for func in &program.funcs {
                let func_id = func.id.id;
                // Params
                let params_total_size =
                    get_types_total_size(&func.signature.param_types, &registry)?;
                // Return types
                let return_types_total_size =
                    get_types_total_size(&func.signature.ret_types, &registry)?;

                stats.sierra_func_stats.insert(
                    func_id,
                    SierraFuncStats {
                        params_total_size,
                        return_types_total_size,
                        times_called: 0,
                    },
                );
            }

            for statement in &program.statements {
                match statement {
                    GenStatement::Invocation(gen_invocation) => {
                        let libfunc = registry.get_libfunc(&gen_invocation.libfunc_id)?;
                        if let CoreConcreteLibfunc::FunctionCall(function_call_libfunc) = libfunc {
                            let func_id = function_call_libfunc.function.id.id;
                            let func_entry = stats
                                .sierra_func_stats
                                .get_mut(&func_id)
                                .to_native_assert_error(&format!(
                                    "Function ID {func_id}, should be present in the stats"
                                ))?;
                            func_entry.times_called += 1;
                        }
                    }
                    GenStatement::Return(_) => continue,
                }
            }
        }

        // Generate mappings between the entry point's selectors and their function indexes.
        let entry_point_mappings = chain!(
            entry_points.constructor.iter(),
            entry_points.external.iter(),
            entry_points.l1_handler.iter(),
        )
        .map(|x| {
            let function_id = x.function_idx as u64;
            let function = registry
                .get_function(&FunctionId::new(function_id))
                .to_native_assert_error("unreachable")?;

            let builtins = find_entrypoint_builtins(function, &registry)?;

            Ok((
                Felt::from(&x.selector),
                EntryPointInfo {
                    function_id: x.function_idx as u64,
                    builtins,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;

        let object_data = crate::module_to_object(&module, opt_level, clone_option_mut!(stats))?;
        if let Some(&mut ref mut stats) = stats {
            stats.object_size_bytes = Some(object_data.len());
        }

        // Build the shared library into the lockfile, to avoid using a tmp file.
        crate::object_to_shared_lib(&object_data, &lock_file.0, clone_option_mut!(stats))?;

        let compilation_time = pre_compilation_instant.elapsed().as_millis();
        if let Some(&mut ref mut stats) = stats {
            stats.compilation_total_time_ms = Some(compilation_time);
        }

        // Write the contract info.
        fs::write(
            output_path.with_extension("json"),
            serde_json::to_string(&NativeContractInfo {
                version: ContractInfoVersion::V0,
                entry_points: entry_point_mappings,
            })?,
        )?;

        if let Some(&mut ref mut stats) = stats {
            native_assert!(stats.validate(), "some statistics are missing");
        }

        // Atomically move the built shared library to the correct path. This will avoid data races
        // when loading contracts.
        lock_file.rename(&output_path)?;

        Self::from_path(output_path)
    }

    /// Load a program from a shared library.
    ///
    /// This function will check for the existence of a lockfile. If found, it'll return `Ok(None)`.
    /// When this happens, the user should wait until the lock is released, then try loading it
    /// again.
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Option<Self>> {
        let path = path.into();

        // Note: Library should load first, otherwise there could theoretically be a race condition.
        //   See the `new_into` function's code for details.
        let library = Arc::new(unsafe { Library::new(&path)? });
        let contract_info =
            serde_json::from_str(&fs::read_to_string(path.with_extension("json"))?)?;

        let executor = Self {
            library,
            path,
            contract_info,
        };

        setup_runtime(|x| executor.find_symbol_ptr(x));

        #[cfg(feature = "with-debug-utils")]
        crate::metadata::debug_utils::setup_runtime(|name| executor.find_symbol_ptr(name));

        #[cfg(feature = "with-trace-dump")]
        crate::metadata::trace_dump::setup_runtime(|name| executor.find_symbol_ptr(name));

        #[cfg(feature = "with-libfunc-profiling")]
        crate::metadata::profiler::setup_runtime(|name| executor.find_symbol_ptr(name));

        Ok(Some(executor))
    }

    /// Runs the entry point by the given selector.
    ///
    /// - selector: The selector of the entry point to run.
    /// - args: The calldata.
    /// - gas: The gas for the execution.
    /// - builtin_costs: An optional argument to customize the costs of the builtins.
    /// - syscall_handler: The syscall handler implementation to use when executing the contract.
    ///
    /// The entry point gas cost is not deducted from the gas counter.
    pub fn run(
        &self,
        selector: Felt,
        args: &[Felt],
        gas: u64,
        builtin_costs: Option<BuiltinCosts>,
        mut syscall_handler: impl StarknetSyscallHandler,
    ) -> Result<ContractExecutionResult> {
        let arena = Bump::new();
        let mut invoke_data = Vec::<u8>::new();

        let function_id = FunctionId {
            id: self
                .contract_info
                .entry_points
                .get(&selector)
                .ok_or(Error::SelectorNotFound)?
                .function_id,
            debug_name: None,
        };
        let function_ptr = self.find_function_ptr(&function_id, true)?;

        // Initialize syscall handler and builtin costs.
        // We may be inside a recursive contract, save the possible saved builtin costs to restore it after our call.
        let mut syscall_handler = StarknetSyscallHandlerCallbacks::new(&mut syscall_handler);
        let builtin_costs = builtin_costs.unwrap_or_default();
        let builtin_costs_guard = BuiltinCostsGuard::install(builtin_costs);

        //  it can vary from contract to contract thats why we need to store/ load it.
        let builtins_size: usize = self.contract_info.entry_points[&selector]
            .builtins
            .iter()
            .map(|x| x.size_in_bytes())
            .sum();

        // There is always a return ptr because contracts always return more than 1 thing (builtin counters, syscall, enum)
        let return_ptr = arena.alloc_layout(unsafe {
            // 56 = size of enum
            Layout::from_size_align_unchecked(128 + builtins_size, 16)
        });

        return_ptr
            .as_ptr()
            .to_bytes(&mut invoke_data, |_| unreachable!())?;

        for b in &self.contract_info.entry_points[&selector].builtins {
            match b {
                BuiltinType::Gas => {
                    gas.to_bytes(&mut invoke_data, |_| unreachable!())?;
                }
                BuiltinType::BuiltinCosts => {
                    let ptr = crate::runtime::BUILTIN_COSTS.with(|x| x.as_ptr());
                    (ptr as *const ()).to_bytes(&mut invoke_data, |_| unreachable!())?;
                }
                BuiltinType::System => {
                    (&mut syscall_handler as *mut StarknetSyscallHandlerCallbacks<_>)
                        .to_bytes(&mut invoke_data, |_| unreachable!())?;
                }
                _ => {
                    0u64.to_bytes(&mut invoke_data, |_| unreachable!())?;
                }
            }
        }

        let felt_layout = get_integer_layout(252).pad_to_align();

        let len_u32: u32 = args
            .len()
            .try_into()
            .to_native_assert_error("number of arguments should fit into a u32")?;

        let data_ptr = match args.len() {
            0 => std::ptr::null_mut(),
            _ => unsafe { libc_malloc(felt_layout.size() * args.len()).cast::<u8>() },
        };

        for (idx, elem) in args.iter().enumerate() {
            let f = elem.to_bytes_le();
            unsafe {
                std::ptr::copy_nonoverlapping(
                    f.as_ptr().cast::<u8>(),
                    data_ptr.byte_add(idx * felt_layout.size()).cast::<u8>(),
                    felt_layout.size(),
                )
            };
        }

        // Allocate metadata struct: { refcount: u32, max_len: u32, data_ptr: *mut () }
        let metadata_ptr = if data_ptr.is_null() {
            ptr::null_mut()
        } else {
            unsafe {
                let metadata = libc_malloc(size_of::<ArrayMetadata>()).cast::<ArrayMetadata>();
                metadata.write(ArrayMetadata {
                    refcount: 1,
                    max_len: len_u32,
                    data_ptr,
                });
                metadata.cast::<()>()
            }
        };

        metadata_ptr.to_bytes(&mut invoke_data, |_| unreachable!())?;
        if cfg!(target_arch = "aarch64") {
            0u32.to_bytes(&mut invoke_data, |_| unreachable!())?; // start
            len_u32.to_bytes(&mut invoke_data, |_| unreachable!())?; // end
            len_u32.to_bytes(&mut invoke_data, |_| unreachable!())?; // cap
        } else if cfg!(target_arch = "x86_64") {
            (0u32 as u64).to_bytes(&mut invoke_data, |_| unreachable!())?; // start
            (len_u32 as u64).to_bytes(&mut invoke_data, |_| unreachable!())?; // end
            (len_u32 as u64).to_bytes(&mut invoke_data, |_| unreachable!())?; // cap
        } else {
            unreachable!("unsupported architecture");
        }

        // Pad invoke data to the 16 byte boundary avoid segfaults.
        #[cfg(target_arch = "aarch64")]
        const REGISTER_BYTES: usize = 64;
        #[cfg(target_arch = "x86_64")]
        const REGISTER_BYTES: usize = 48;
        if invoke_data.len() > REGISTER_BYTES {
            invoke_data.resize(
                REGISTER_BYTES + (invoke_data.len() - REGISTER_BYTES).next_multiple_of(16),
                0,
            );
        }

        // Invoke the trampoline.
        #[cfg(target_arch = "x86_64")]
        let mut ret_registers = [0; 2];
        #[cfg(target_arch = "aarch64")]
        let mut ret_registers = [0; 4];

        #[allow(unused_mut)]
        let mut run_trampoline = || unsafe {
            invoke_trampoline(
                function_ptr,
                invoke_data.as_ptr().cast(),
                invoke_data.len() >> 3,
                ret_registers.as_mut_ptr(),
            );
        };
        #[cfg(feature = "with-segfault-catcher")]
        crate::utils::safe_runner::run_safely(run_trampoline).map_err(Error::SafeRunner)?;
        #[cfg(not(feature = "with-segfault-catcher"))]
        run_trampoline();

        // Parse final gas.
        unsafe fn read_value<T>(ptr: &mut NonNull<()>) -> &T {
            let align_offset = ptr
                .cast::<u8>()
                .as_ptr()
                .align_offset(std::mem::align_of::<T>());
            let value_ptr = ptr.cast::<u8>().as_ptr().add(align_offset).cast::<T>();

            *ptr = NonNull::new_unchecked(value_ptr.add(1)).cast();
            &*value_ptr
        }

        let mut remaining_gas = 0;
        let mut builtin_stats = BuiltinStats::default();

        let return_ptr = &mut return_ptr.cast();

        for b in &self.contract_info.entry_points[&selector].builtins {
            match b {
                BuiltinType::Gas => {
                    remaining_gas = unsafe { *read_value::<u64>(return_ptr) };
                }
                BuiltinType::System => {
                    unsafe { read_value::<*mut ()>(return_ptr) };
                }
                BuiltinType::BuiltinCosts => {
                    unsafe { read_value::<*mut ()>(return_ptr) };
                    // ptr holds the builtin costs, but they dont change, so its of no use, but we read to advance the ptr.
                }
                x => {
                    let value = unsafe { *read_value::<u64>(return_ptr) } as usize;

                    match x {
                        BuiltinType::RangeCheck => {
                            builtin_stats.range_check = value / RANGE_CHECK_BUILTIN_SIZE
                        }
                        BuiltinType::Pedersen => {
                            builtin_stats.pedersen = value / PEDERSEN_BUILTIN_SIZE
                        }
                        BuiltinType::Bitwise => {
                            builtin_stats.bitwise = value / BITWISE_BUILTIN_SIZE
                        }
                        BuiltinType::EcOp => builtin_stats.ec_op = value / EC_OP_BUILTIN_SIZE,
                        BuiltinType::Poseidon => {
                            builtin_stats.poseidon = value / POSEIDON_BUILTIN_SIZE
                        }
                        BuiltinType::SegmentArena => {
                            builtin_stats.segment_arena = value / SEGMENT_ARENA_BUILTIN_SIZE
                        }
                        BuiltinType::RangeCheck96 => {
                            builtin_stats.range_check96 = value / RANGE_CHECK96_BUILTIN_SIZE
                        }
                        BuiltinType::CircuitAdd => {
                            builtin_stats.add_mod = value / ADD_MOD_BUILTIN_SIZE
                        }
                        BuiltinType::CircuitMul => {
                            builtin_stats.mul_mod = value / MUL_MOD_BUILTIN_SIZE
                        }
                        BuiltinType::Blake => builtin_stats.blake = value / BLAKE_BUILTIN_SIZE,
                        BuiltinType::Gas => {}
                        BuiltinType::System => {}
                        BuiltinType::BuiltinCosts => {}
                    }
                }
            }
        }

        // align the pointer
        // layout of the enum type.
        let layout = unsafe { Layout::from_size_align_unchecked(32, 8) };
        let align_offset = return_ptr
            .cast::<u8>()
            .as_ptr()
            .align_offset(layout.align());

        let tag_layout = Layout::from_size_align(1, 1)?;
        let enum_ptr = unsafe {
            NonNull::new(return_ptr.cast::<u8>().as_ptr().add(align_offset))
                .to_native_assert_error("return ptr should not be null")?
        };

        let tag = *unsafe { enum_ptr.cast::<u8>().as_ref() } as usize;
        let tag = tag & 0x01; // Filter out bits that are not part of the enum's tag.

        let value_layout = unsafe { Layout::from_size_align_unchecked(24, 8) };
        let mut value_ptr = unsafe { enum_ptr.byte_add(tag_layout.extend(value_layout)?.1).cast() };

        let metadata_ptr = unsafe { *read_value::<*mut NonNull<()>>(&mut value_ptr) };
        let array_start = unsafe { *read_value::<u32>(&mut value_ptr) };
        let array_end = unsafe { *read_value::<u32>(&mut value_ptr) };
        let _array_capacity = unsafe { *read_value::<u32>(&mut value_ptr) };

        let mut array_value = Vec::with_capacity((array_end - array_start) as usize);
        if !metadata_ptr.is_null() {
            let metadata = unsafe { metadata_ptr.cast::<ArrayMetadata>().read() };
            let data_ptr = metadata.data_ptr;

            let elem_stride = felt_layout.pad_to_align().size();
            for i in array_start..array_end {
                let cur_elem_ptr = unsafe { data_ptr.byte_add(elem_stride * i as usize) };

                let mut data = unsafe { cur_elem_ptr.cast::<[u8; 32]>().read() };
                data[31] &= 0x0F; // Filter out first 4 bits (they're outside an i252).

                array_value.push(Felt::from_bytes_le(&data));
            }

            unsafe {
                native_assert!(
                    metadata.refcount == 1,
                    "return array should have a reference count of 1"
                );
                libc_free(data_ptr.cast());
                libc_free(metadata_ptr.cast());
            }
        }

        let error_msg = match tag {
            0 => None,
            _ => {
                Some(decode_error_message(
                    &array_value
                        .iter()
                        .flat_map(|felt| felt.to_bytes_be().to_vec())
                        // remove null chars
                        .filter(|b| *b != 0)
                        .collect::<Vec<_>>(),
                ))
            }
        };

        // Restore the original builtin costs pointer.
        drop(builtin_costs_guard);

        // Get the blake call count from the global counter (blake doesn't have a buffer-based counter
        // like other builtins, so it's tracked globally via the blake libfuncs on each invocation)
        builtin_stats.blake = BLAKE_CALL_COUNT.with(|c| c.replace(0)) as usize;

        #[cfg(feature = "with-mem-tracing")]
        crate::utils::mem_tracing::report_stats();

        Ok(ContractExecutionResult {
            remaining_gas,
            failure_flag: tag != 0,
            return_values: array_value,
            error_msg,
            builtin_stats,
        })
    }

    pub fn find_function_ptr(
        &self,
        function_id: &FunctionId,
        is_for_contract_executor: bool,
    ) -> Result<*mut c_void> {
        let function_name = generate_function_name(function_id, is_for_contract_executor);
        let function_name = format!("_mlir_ciface_{function_name}");

        // Arguments and return values are hardcoded since they'll be handled by the trampoline.
        Ok(unsafe {
            self.library
                .get::<extern "C" fn()>(function_name.as_bytes())?
                .into_raw()
                .into_raw()
        })
    }

    pub fn find_symbol_ptr(&self, name: &str) -> Option<*mut c_void> {
        unsafe {
            self.library
                .get::<*mut ()>(name.as_bytes())
                .ok()
                .map(|x| x.into_raw().into_raw())
        }
    }
}

fn find_entrypoint_builtins(
    function: &GenFunction<StatementIdx>,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
) -> Result<Vec<BuiltinType>> {
    let param_type_infos = function
        .params
        .iter()
        .map(|x| -> Result<_> {
            let ty = registry.get_type(&x.ty)?;
            let is_zst = ty.is_zst(registry)?;
            Ok((ty, is_zst))
        })
        .try_collect::<_, Vec<_>, _>()?;

    param_type_infos
        .iter()
        .take_while(|(ty, _)| ty.is_builtin())
        .filter(|(_, is_zst)| !is_zst)
        .map(|(ty, _)| -> Result<_> {
            Ok(match ty {
                CoreTypeConcrete::Bitwise(_) => BuiltinType::Bitwise,
                CoreTypeConcrete::EcOp(_) => BuiltinType::EcOp,
                CoreTypeConcrete::RangeCheck(_) => BuiltinType::RangeCheck,
                CoreTypeConcrete::Pedersen(_) => BuiltinType::Pedersen,
                CoreTypeConcrete::Poseidon(_) => BuiltinType::Poseidon,
                CoreTypeConcrete::BuiltinCosts(_) => BuiltinType::BuiltinCosts,
                CoreTypeConcrete::SegmentArena(_) => BuiltinType::SegmentArena,
                CoreTypeConcrete::RangeCheck96(_) => BuiltinType::RangeCheck96,
                CoreTypeConcrete::Circuit(CircuitTypeConcrete::AddMod(_)) => {
                    BuiltinType::CircuitAdd
                }
                CoreTypeConcrete::Circuit(CircuitTypeConcrete::MulMod(_)) => {
                    BuiltinType::CircuitMul
                }
                CoreTypeConcrete::GasBuiltin(_) => BuiltinType::Gas,
                CoreTypeConcrete::Starknet(StarknetTypeConcrete::System(_)) => BuiltinType::System,
                CoreTypeConcrete::Blake(_) => BuiltinType::Blake,
                _ => native_panic!("unknown builtin type for function {}", function),
            })
        })
        .try_collect()
}

#[derive(Debug)]
struct LockFile(PathBuf);

impl LockFile {
    pub fn new(path: impl Into<PathBuf>) -> io::Result<Option<Self>> {
        let path: PathBuf = path.into();
        let path = path.with_extension("lock");

        match File::create_new(&path) {
            Ok(_) => Ok(Some(Self(path))),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn rename(self, path: impl AsRef<Path>) -> io::Result<()> {
        fs::rename(&self.0, path.as_ref())?;

        // don't remove lockfile, as we just renamed it
        std::mem::forget(self);

        Ok(())
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::include_contract;
    use crate::starknet_stub::StubSyscallHandler;
    use cairo_lang_starknet_classes::contract_class::ContractClass;
    use rayon::iter::ParallelBridge;
    use rstest::*;

    // todo add recursive contract test See: https://github.com/lambdaclass/cairo_native/issues/1220

    #[fixture]
    fn starknet_program() -> ContractClass {
        include_contract!("test_data_artifacts/contracts/simple_storage_dup.contract.json")
    }

    #[fixture]
    fn starknet_program_factorial() -> ContractClass {
        include_contract!("test_data_artifacts/contracts/simple_storage_factorial.contract.json")
    }

    #[fixture]
    fn starknet_program_empty() -> ContractClass {
        include_contract!("test_data_artifacts/contracts/simple_storage_empty.contract.json")
    }

    #[rstest]
    #[case(OptLevel::Default)]
    fn test_contract_executor_parallel(
        starknet_program: ContractClass,
        #[case] optlevel: OptLevel,
    ) {
        use rayon::iter::ParallelIterator;

        let extracted = starknet_program.extract_sierra_program(false).unwrap();
        let executor = Arc::new(
            AotContractExecutor::new(
                &extracted.program,
                &starknet_program.entry_points_by_type,
                extracted.sierra_version,
                optlevel,
                None,
            )
            .unwrap(),
        );

        // The last function in the program is the `get` wrapper function.
        let selector = starknet_program
            .entry_points_by_type
            .external
            .last()
            .unwrap()
            .selector
            .clone();

        (0..200).par_bridge().for_each(|n| {
            let result = executor
                .run(
                    Felt::from(&selector),
                    &[n.into()],
                    u64::MAX,
                    None,
                    &mut StubSyscallHandler::default(),
                )
                .unwrap();
            assert_eq!(result.return_values, vec![Felt::from(n), Felt::from(n * 2)]);
            assert_eq!(result.remaining_gas, 18446744073709551615);
        });
    }

    #[rstest]
    #[case(OptLevel::None)]
    #[case(OptLevel::Default)]
    fn test_contract_executor(starknet_program: ContractClass, #[case] optlevel: OptLevel) {
        let extracted = starknet_program.extract_sierra_program(false).unwrap();
        let executor = AotContractExecutor::new(
            &extracted.program,
            &starknet_program.entry_points_by_type,
            extracted.sierra_version,
            optlevel,
            None,
        )
        .unwrap();

        // The last function in the program is the `get` wrapper function.
        let selector = starknet_program
            .entry_points_by_type
            .external
            .last()
            .unwrap()
            .selector
            .clone();

        let result = executor
            .run(
                Felt::from(&selector),
                &[2.into()],
                u64::MAX,
                None,
                &mut StubSyscallHandler::default(),
            )
            .unwrap();

        assert_eq!(result.return_values, vec![Felt::from(2), Felt::from(4)]);
    }

    #[rstest]
    #[case(OptLevel::Aggressive)]
    fn test_contract_executor_factorial(
        starknet_program_factorial: ContractClass,
        #[case] optlevel: OptLevel,
    ) {
        let extracted = starknet_program_factorial
            .extract_sierra_program(false)
            .unwrap();
        let executor = AotContractExecutor::new(
            &extracted.program,
            &starknet_program_factorial.entry_points_by_type,
            extracted.sierra_version,
            optlevel,
            None,
        )
        .unwrap();

        // The last function in the program is the `get` wrapper function.
        let selector = starknet_program_factorial
            .entry_points_by_type
            .external
            .last()
            .unwrap()
            .selector
            .clone();

        let result = executor
            .run(
                Felt::from(&selector),
                &[10.into()],
                u64::MAX,
                None,
                &mut StubSyscallHandler::default(),
            )
            .unwrap();
        assert_eq!(result.return_values, vec![Felt::from(3628800)]);
        assert_eq!(result.remaining_gas, 18446744073709546675);
    }

    #[rstest]
    #[case(OptLevel::None)]
    #[case(OptLevel::Default)]
    fn test_contract_executor_empty(
        starknet_program_empty: ContractClass,
        #[case] optlevel: OptLevel,
    ) {
        let extracted = starknet_program_empty
            .extract_sierra_program(false)
            .unwrap();
        let executor = AotContractExecutor::new(
            &extracted.program,
            &starknet_program_empty.entry_points_by_type,
            extracted.sierra_version,
            optlevel,
            None,
        )
        .unwrap();

        // The last function in the program is the `get` wrapper function.
        let selector = starknet_program_empty
            .entry_points_by_type
            .external
            .last()
            .unwrap()
            .selector
            .clone();

        let result = executor
            .run(
                Felt::from(&selector),
                &[],
                u64::MAX,
                None,
                &mut StubSyscallHandler::default(),
            )
            .unwrap();

        assert_eq!(result.return_values, vec![]);
    }
}
