use cairo_native::context::NativeContext;
use cairo_native::utils::testing::load_program_and_runner;
use std::error::Error;
use tempfile::NamedTempFile;

#[test]
pub fn compile_library() -> Result<(), Box<dyn Error>> {
    // Load the program.
    let context = NativeContext::new();

    let program = load_program_and_runner("programs/felt252_add");

    let module = context.compile(&program.1, false, Some(Default::default()), None)?;

    let object = cairo_native::module_to_object(module.module(), Default::default(), None)?;

    let file = NamedTempFile::new()?.into_temp_path();
    cairo_native::object_to_shared_lib(&object, &file, None)?;

    Ok(())
}
