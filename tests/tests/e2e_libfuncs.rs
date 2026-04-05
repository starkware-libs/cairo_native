use cairo_lang_sierra::ProgramParser;
use cairo_native::context::NativeContext;
use std::fs;
use std::path::Path;

/// Collects all `.sierra` files under the e2e_sierra directory and compiles each
/// through cairo-native (Sierra → LLVM) to verify they compile without errors.
#[test]
fn compile_e2e_libfunc_sierra() {
    let sierra_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/e2e_sierra");
    assert!(
        sierra_dir.exists(),
        "{} not found. Run 'git submodule update --init && make test_data/e2e_sierra' first.",
        sierra_dir.display()
    );

    let context = NativeContext::new();
    let parser = ProgramParser::new();

    let mut total = 0;
    let mut failures = Vec::new();

    for entry in walkdir::WalkDir::new(&sierra_dir) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "sierra") {
            total += 1;
            let source = fs::read_to_string(path).unwrap();
            let rel_path = path.strip_prefix(&sierra_dir).unwrap();

            let program = match parser.parse(&source) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(format!("{}: parse error: {e}", rel_path.display()));
                    continue;
                }
            };

            eprintln!("compiling {}", rel_path.display());
            if let Err(e) = context.compile(&program, false, Some(Default::default()), None) {
                failures.push(format!("{}: compile error: {e}", rel_path.display()));
            }
        }
    }

    assert!(
        total > 0,
        "No .sierra files found in {}",
        sierra_dir.display()
    );

    if !failures.is_empty() {
        panic!(
            "{}/{} e2e libfunc sierra files failed to compile:\n{}",
            failures.len(),
            total,
            failures.join("\n")
        );
    }
}
