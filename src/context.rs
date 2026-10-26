use crate::{
    clone_option_mut,
    error::{panic::ToNativeAssertError, Error},
    ffi::{get_data_layout_rep, get_target_triple},
    metadata::{gas::GasMetadata, runtime_bindings::RuntimeBindingsMeta, MetadataStorage},
    module::NativeModule,
    native_assert,
    statistics::Statistics,
    utils::{run_pass_manager, walk_ir::walk_mlir_operations},
};
use cairo_lang_sierra::{
    extensions::core::{CoreLibfunc, CoreType},
    program::Program,
    program_registry::ProgramRegistry,
};
use cairo_lang_sierra_to_casm::metadata::MetadataComputationConfig;
use cairo_lang_sierra_type_size::ProgramRegistryInfo;
use melior::{
    dialect::DialectRegistry,
    ir::{
        attribute::StringAttribute,
        operation::{OperationBuilder, OperationPrintingFlags},
        Attribute, AttributeLike, Block, Identifier, Location, Module, Region,
    },
    utility::{register_all_dialects, register_all_llvm_translations, register_all_passes},
    Context,
};
use mlir_sys::{
    mlirDisctinctAttrCreate, mlirLLVMDICompileUnitAttrGet, mlirLLVMDIFileAttrGet,
    mlirLLVMDIModuleAttrGet, MlirLLVMDIEmissionKind_MlirLLVMDIEmissionKindFull,
    MlirLLVMDINameTableKind_MlirLLVMDINameTableKindDefault,
};
use std::time::Instant;

/// Context of IRs, dialects and passes for Cairo programs compilation.
#[derive(Debug, Eq, PartialEq)]
pub struct NativeContext {
    context: Context,
}

unsafe impl Send for NativeContext {}
unsafe impl Sync for NativeContext {}

impl Default for NativeContext {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeContext {
    pub fn new() -> Self {
        let context = initialize_mlir();
        Self { context }
    }

    pub const fn context(&self) -> &Context {
        &self.context
    }

    /// Compiles a sierra program into MLIR and then lowers to LLVM.
    /// Returns the corresponding NativeModule struct.
    ///
    /// If `ignore_debug_names` is true then debug names will not be added to function names.
    /// Mainly useful for the ContractExecutor.
    pub fn compile(
        &'_ self,
        program: &Program,
        ignore_debug_names: bool,
        gas_metadata_config: Option<MetadataComputationConfig>,
        stats: Option<&mut Statistics>,
    ) -> Result<NativeModule<'_>, Error> {
        crate::ffi::initialize_llvm_targets();
        let target_triple = get_target_triple();

        let module_region = Region::new();
        module_region.append_block(Block::new(&[]));

        let data_layout_ret = &get_data_layout_rep()?;

        let di_unit_id = unsafe {
            let id = StringAttribute::new(&self.context, "compile_unit_id").to_raw();
            mlirDisctinctAttrCreate(id)
        };

        let op = OperationBuilder::new(
            "builtin.module",
            Location::fused(
                &self.context,
                &[Location::new(&self.context, "program.sierra", 0, 0)],
                {
                    let file_attr = unsafe {
                        Attribute::from_raw(mlirLLVMDIFileAttrGet(
                            self.context.to_raw(),
                            StringAttribute::new(&self.context, "program.sierra").to_raw(),
                            StringAttribute::new(&self.context, "").to_raw(),
                        ))
                    };
                    unsafe {
                        let di_unit = mlirLLVMDICompileUnitAttrGet(
                            self.context.to_raw(),
                            di_unit_id,
                            0x1c, // rust
                            file_attr.to_raw(),
                            StringAttribute::new(&self.context, "cairo-native").to_raw(),
                            false,
                            MlirLLVMDIEmissionKind_MlirLLVMDIEmissionKindFull,
                            MlirLLVMDINameTableKind_MlirLLVMDINameTableKindDefault,
                        );

                        let context = &self.context;

                        let di_module = mlirLLVMDIModuleAttrGet(
                            context.to_raw(),
                            file_attr.to_raw(),
                            di_unit,
                            StringAttribute::new(context, "LLVMDialectModule").to_raw(),
                            StringAttribute::new(context, "").to_raw(),
                            StringAttribute::new(context, "").to_raw(),
                            StringAttribute::new(context, "").to_raw(),
                            0,
                            false,
                        );

                        Attribute::from_raw(di_module)
                    }
                },
            ),
        )
        .add_attributes(&[
            (
                Identifier::new(&self.context, "llvm.target_triple"),
                StringAttribute::new(&self.context, &target_triple).into(),
            ),
            (
                Identifier::new(&self.context, "llvm.data_layout"),
                StringAttribute::new(&self.context, data_layout_ret).into(),
            ),
        ])
        .add_regions([module_region])
        .build()?;

        native_assert!(op.verify(), "module operation should be valid");

        let mut module = Module::from_operation(op)
            .to_native_assert_error("value should be module operation")?;

        let mut metadata = MetadataStorage::new();
        // Make the runtime library available.
        metadata.insert(RuntimeBindingsMeta::default());
        // We assume that GasMetadata will be always present when the program uses the gas builtin.
        let gas_metadata = GasMetadata::new(
            program,
            &ProgramRegistryInfo::new(program)?,
            gas_metadata_config,
        )?;
        // Unwrapping here is not necessary since the insertion will only fail if there was
        // already some metadata of the same type.
        metadata.insert(gas_metadata);

        #[cfg(feature = "with-libfunc-profiling")]
        metadata.insert(crate::metadata::profiler::ProfilerMeta::new());

        // Create the Sierra program registry
        let registry = ProgramRegistry::<CoreType, CoreLibfunc>::new(program)?;

        let pre_sierra_to_mlir_instant = Instant::now();
        crate::compile(
            &self.context,
            &module,
            program,
            &registry,
            &mut metadata,
            unsafe { Attribute::from_raw(di_unit_id) },
            ignore_debug_names,
            clone_option_mut!(stats),
        )?;
        let sierra_to_mlir_time = pre_sierra_to_mlir_instant.elapsed().as_millis();
        if let Some(&mut ref mut stats) = stats {
            stats.compilation_sierra_to_mlir_time_ms = Some(sierra_to_mlir_time);
        }

        if let Ok(x) = std::env::var("NATIVE_DEBUG_DUMP") {
            if x == "1" || x == "true" {
                std::fs::write("dump-prepass.mlir", module.as_operation().to_string())?;
                std::fs::write(
                    "dump-prepass-debug-valid.mlir",
                    module.as_operation().to_string_with_flags(
                        OperationPrintingFlags::new().enable_debug_info(true, false),
                    )?,
                )?;
                std::fs::write(
                    "dump-prepass-debug-pretty.mlir",
                    module.as_operation().to_string_with_flags(
                        OperationPrintingFlags::new().enable_debug_info(true, false),
                    )?,
                )?;
            }
        }

        if let Some(&mut ref mut stats) = stats {
            let mut operations = 0;
            walk_mlir_operations(module.as_operation(), &mut |_| operations += 1);
            stats.mlir_operation_count = Some(operations)
        }

        let pre_mlir_passes_instant = Instant::now();
        run_pass_manager(&self.context, &mut module)?;
        let mlir_passes_time = pre_mlir_passes_instant.elapsed().as_millis();
        if let Some(&mut ref mut stats) = stats {
            stats.compilation_mlir_passes_time_ms = Some(mlir_passes_time);
        }

        if let Ok(x) = std::env::var("NATIVE_DEBUG_DUMP") {
            if x == "1" || x == "true" {
                std::fs::write("dump.mlir", module.as_operation().to_string())?;
                std::fs::write(
                    "dump-debug-pretty.mlir",
                    module.as_operation().to_string_with_flags(
                        OperationPrintingFlags::new().enable_debug_info(true, false),
                    )?,
                )?;
                std::fs::write(
                    "dump-debug.mlir",
                    module.as_operation().to_string_with_flags(
                        OperationPrintingFlags::new().enable_debug_info(true, false),
                    )?,
                )?;
            }
        }

        Ok(NativeModule::new(module, registry, metadata))
    }
}

/// Initialize an MLIR context.
pub fn initialize_mlir() -> Context {
    let context = Context::new();
    context.append_dialect_registry(&{
        let registry = DialectRegistry::new();
        register_all_dialects(&registry);
        registry
    });
    context.load_all_available_dialects();
    register_all_passes();
    register_all_llvm_translations(&context);
    context
}
