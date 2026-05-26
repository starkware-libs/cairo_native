use crate::error::{Error, Result};
use crate::{
    context::NativeContext, executor::AotNativeExecutor, module::NativeModule,
    utils::SHARED_LIBRARY_EXT, OptLevel,
};
use cairo_lang_sierra::program::Program;
use libloading::Library;
use std::{
    collections::HashMap,
    fmt::{self, Debug},
    hash::Hash,
    sync::Arc,
};

pub struct AotProgramCache<'a, K>
where
    K: PartialEq + Eq + Hash,
{
    context: &'a NativeContext,
    cache: HashMap<K, Arc<AotNativeExecutor>>,
}

impl<'a, K> AotProgramCache<'a, K>
where
    K: PartialEq + Eq + Hash,
{
    pub fn new(context: &'a NativeContext) -> Self {
        Self {
            context,
            cache: Default::default(),
        }
    }

    pub fn get(&self, key: &K) -> Option<Arc<AotNativeExecutor>> {
        self.cache.get(key).cloned()
    }

    pub fn compile_and_insert(
        &mut self,
        key: K,
        program: &Program,
        opt_level: OptLevel,
    ) -> Result<Arc<AotNativeExecutor>> {
        let NativeModule {
            module,
            registry,
            mut metadata,
        } = self
            .context
            .compile(program, false, Some(Default::default()), None)?;

        // Compile module into an object.
        let object_data = crate::ffi::module_to_object(&module, opt_level, None)?;

        // Compile object into a shared library.
        let shared_library_path = tempfile::Builder::new()
            .prefix("lib")
            .suffix(SHARED_LIBRARY_EXT)
            .tempfile()?
            .into_temp_path();
        crate::ffi::object_to_shared_lib(&object_data, &shared_library_path, None)?;

        let shared_library = unsafe { Library::new(shared_library_path)? };
        let executor = AotNativeExecutor::new(
            shared_library,
            registry,
            metadata.remove().ok_or(Error::MissingMetadata)?,
        );

        let executor = Arc::new(executor);
        self.cache.insert(key, executor.clone());

        Ok(executor)
    }
}

impl<K> Debug for AotProgramCache<'_, K>
where
    K: PartialEq + Eq + Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("AotProgramCache")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{utils::testing::load_program, values::Value};
    use starknet_types_core::felt::Felt;

    #[test]
    fn test_aot_compile_and_insert() {
        let native_context = NativeContext::new();
        let mut cache = AotProgramCache::new(&native_context);

        let program = load_program("programs/aot_program");

        let function_id = &program.funcs.first().expect("should have a function").id;
        let executor = cache.compile_and_insert((), &program, OptLevel::default());
        let res = executor
            .unwrap()
            .invoke_dynamic(function_id, &[], Some(u64::MAX))
            .expect("should run");

        // After compiling and inserting the program, we should be able to run it.
        assert_eq!(res.return_value, Value::Felt252(Felt::from(42)));
    }
}
