use cairo_lang_sierra::{
    extensions::{
        blake::BlakeConcreteLibfunc,
        core::{CoreLibfunc, CoreType, CoreTypeConcrete},
        lib_func::SignatureOnlyConcreteLibfunc,
    },
    program_registry::ProgramRegistry,
};
use melior::{
    helpers::{ArithBlockExt, BuiltinBlockExt},
    ir::{Block, Location},
    Context,
};

use crate::{
    error::{panic::ToNativeAssertError, Result},
    metadata::{runtime_bindings::RuntimeBindingsMeta, MetadataStorage},
    native_panic,
    types::TypeBuilder,
};

use super::LibfuncHelper;

pub fn build<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    selector: &BlakeConcreteLibfunc,
) -> Result<()> {
    match selector {
        BlakeConcreteLibfunc::Blake2sCompress(info) => build_blake_operation(
            context, registry, entry, location, helper, metadata, info, false,
        ),
        BlakeConcreteLibfunc::Blake2sFinalize(info) => build_blake_operation(
            context, registry, entry, location, helper, metadata, info, true,
        ),
    }
}

/// Performs a blake2s compression.
///
/// `bytes_count`: total amount of bytes hashed after hashing the message.
/// `finalize`: wether the libfunc call is a finalize or not.
/// ```cairo
/// pub extern fn blake2s_compress(
///     state: Blake2sState, byte_count: u32, msg: Blake2sInput,
/// ) -> Blake2sState nopanic;
/// ```
///
/// Similar to `blake2s_compress`, but it marks the end of the compression
/// ```cairo
/// pub extern fn blake2s_finalize(
///     state: Blake2sState, byte_count: u32, msg: Blake2sInput,
/// ) -> Blake2sState nopanic;
/// ```
#[allow(clippy::too_many_arguments)]
fn build_blake_operation<'ctx, 'this>(
    context: &'ctx Context,
    registry: &ProgramRegistry<CoreType, CoreLibfunc>,
    entry: &'this Block<'ctx>,
    location: Location<'ctx>,
    helper: &LibfuncHelper<'ctx, 'this>,
    metadata: &mut MetadataStorage,
    info: &SignatureOnlyConcreteLibfunc,
    finalize: bool,
) -> Result<()> {
    let state_ptr = entry.arg(0)?;
    let bytes_count = entry.arg(1)?;
    let message = entry.arg(2)?;
    let k_finalize = entry.const_int(context, location, finalize as u8, 1)?;

    // The state parameter is `Box<[u32; 8]>`; we need the layout of the inner
    // `[u32; 8]` to size the output slot, not the layout of the box pointer.
    let CoreTypeConcrete::Box(box_info) =
        registry.get_type(&info.signature.param_signatures[0].ty)?
    else {
        native_panic!("blake state parameter should be a Box");
    };
    let inner_layout = registry.get_type(&box_info.ty)?.layout(registry)?;
    let size = entry.const_int(context, location, inner_layout.size(), 64)?;
    let align = entry.const_int(context, location, inner_layout.align(), 64)?;

    let runtime_bindings = metadata
        .get_mut::<RuntimeBindingsMeta>()
        .to_native_assert_error("runtime library should be available")?;

    let out_state_ptr =
        runtime_bindings.box_alloc(context, helper, entry, location, size, align)?;

    runtime_bindings.libfunc_blake_compress(
        context,
        helper,
        entry,
        out_state_ptr,
        state_ptr,
        message,
        bytes_count,
        k_finalize,
        location,
    )?;

    helper.br(entry, 0, &[out_state_ptr], location)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        jit_struct,
        utils::testing::{get_compiled_program, run_program},
        Value,
    };

    // This test is taken from the Blake2s-256 implementeation RFC-7693, Appendix B.
    // https://www.rfc-editor.org/rfc/rfc7693#appendix-B.
    #[test]
    fn test_blake_3_bytes_compress() {
        let program =
            get_compiled_program("test_data_artifacts/programs/libfuncs/blake_3_bytes_compress");

        let result = run_program(&program, "run_test", &[]);

        assert_eq!(
            result.return_value,
            jit_struct!(
                Value::Uint32(0x8C5E8C50),
                Value::Uint32(0xE2147C32),
                Value::Uint32(0xA32BA7E1),
                Value::Uint32(0x2F45EB4E),
                Value::Uint32(0x208B4537),
                Value::Uint32(0x293AD69E),
                Value::Uint32(0x4C9B994D),
                Value::Uint32(0x82596786),
            )
        );

        // blake2s_finalize calls build_blake_operation once → blake count = 1
        assert_eq!(
            result.builtin_stats.blake, 1,
            "blake counter should be exactly 1 for a single blake2s_finalize call"
        );
    }
}
