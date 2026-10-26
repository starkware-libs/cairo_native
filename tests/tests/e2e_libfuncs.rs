use cairo_lang_sierra::{program::Program, ProgramParser};
use cairo_native::{context::NativeContext, utils::testing::find_type_layout_mismatches};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const SRC_DIR: &str = "vendor/cairo/tests/e2e_test_data/libfuncs";

/// Walks the raw e2e test data files, extracts sierra_code sections, and
/// compiles each through cairo-native (Sierra -> LLVM) to verify they compile
/// without errors.
/// Walks the raw e2e test data files, extracts the `sierra_code` sections and
/// parses them. Returns `(label, program)` pairs along with any parse failures.
fn load_e2e_libfunc_programs() -> (Vec<(String, Program)>, Vec<String>) {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SRC_DIR);
    assert!(
        src_dir.exists(),
        "{} not found. Run 'git submodule update --init' first.",
        src_dir.display()
    );

    let parser = ProgramParser::new();
    let mut programs = Vec::new();
    let mut failures = Vec::new();

    for entry in walkdir::WalkDir::new(&src_dir).sort_by_file_name() {
        let entry = entry.unwrap();
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let content = fs::read_to_string(path).unwrap();
        let rel_path = path.strip_prefix(&src_dir).unwrap();

        for (test_name, sierra) in extract_sierra_from_test_file(&content) {
            let label = format!("{}/{test_name}", rel_path.display());
            match parser.parse(&sierra) {
                Ok(program) => programs.push((label, program)),
                Err(e) => failures.push(format!("{label}: parse error: {e}")),
            }
        }
    }

    assert!(
        !programs.is_empty() || !failures.is_empty(),
        "No sierra test cases found in {}",
        src_dir.display()
    );

    (programs, failures)
}

/// Compiles every e2e libfunc Sierra program through cairo-native
/// (Sierra -> LLVM) to verify they compile without errors.
#[test]
fn compile_e2e_libfunc_sierra() {
    let (programs, mut failures) = load_e2e_libfunc_programs();
    let total = programs.len() + failures.len();

    let context = NativeContext::new();
    for (label, program) in &programs {
        eprintln!("compiling {label}");
        if let Err(e) = context.compile(program, false, Some(Default::default()), None) {
            failures.push(format!("{label}: compile error: {e}"));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{}/{} e2e libfunc sierra tests failed:\n{}",
            failures.len(),
            total,
            failures.join("\n")
        );
    }
}

/// Checks that, for every type declared by any e2e libfunc Sierra program, the
/// Rust-side `layout()` matches the size and alignment LLVM assigns to the
/// emitted MLIR type.
///
/// Codegen bakes `layout()` into array strides, enum payload offsets and
/// argument marshalling, so a mismatch means generated code reads or writes
/// memory at the wrong offsets. Checking the types declared by the e2e corpus
/// covers every type any libfunc consumes or produces, without having to list
/// the types by hand.
#[test]
fn e2e_libfunc_type_layouts_match_llvm() {
    let (programs, failures) = load_e2e_libfunc_programs();
    assert!(
        failures.is_empty(),
        "e2e libfunc sierra parse failures:\n{}",
        failures.join("\n")
    );

    let context = NativeContext::new();
    let mut mismatches = BTreeSet::new();
    let mut errors = Vec::new();
    for (label, program) in &programs {
        eprintln!("checking type layouts of {label}");
        match find_type_layout_mismatches(&context, program) {
            Ok(found) => mismatches.extend(found.into_iter().map(|m| m.to_string())),
            Err(e) => errors.push(format!("{label}: {e}")),
        }
    }

    assert!(
        errors.is_empty(),
        "failed to check type layouts:\n{}",
        errors.join("\n")
    );
    assert!(
        mismatches.is_empty(),
        "{} type(s) whose layout() disagrees with LLVM:\n{}",
        mismatches.len(),
        mismatches.into_iter().collect::<Vec<_>>().join("\n")
    );
}

/// Parse a raw e2e test data file in `//! >` format and return a list of
/// `(test_name, sierra_code)` pairs.
fn extract_sierra_from_test_file(content: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();

    // Split on the separator lines: //! > ====...
    for case in content.split("\n//! > ==") {
        let mut test_name: Option<String> = None;
        let mut current_key: Option<&str> = None;
        let mut sierra: Option<String> = None;
        let mut lines: Vec<&str> = Vec::new();

        for line in case.lines() {
            if line.starts_with("==") {
                continue;
            }
            if let Some(header) = line.strip_prefix("//! > ") {
                let header = header.trim();
                if let Some(key) = current_key.take() {
                    if key == "sierra_code" {
                        sierra = Some(lines.join("\n"));
                    }
                    lines.clear();
                }
                if test_name.is_none() {
                    test_name = Some(header.to_string());
                }
                current_key = Some(header);
            } else {
                lines.push(line);
            }
        }
        if current_key == Some("sierra_code") {
            sierra = Some(lines.join("\n"));
        }

        if let (Some(name), Some(code)) = (test_name, sierra) {
            let code = code.trim().to_string();
            if !code.is_empty() {
                results.push((name, code));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sierra_from_test_file() {
        let input = r#"//! > my_test_name

//! > test_runner_name
SmallE2ETestRunner

//! > cairo_code
fn foo() {}

//! > sierra_code
type felt252 = felt252;
return();

//! > ==========================================================================

//! > another_test

//! > test_runner_name
SmallE2ETestRunner

//! > sierra_code
type u8 = u8;
return();
"#;

        let cases = extract_sierra_from_test_file(input);
        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].0, "my_test_name");
        assert!(cases[0].1.contains("type felt252"));
        assert_eq!(cases[1].0, "another_test");
        assert!(cases[1].1.contains("type u8"));
    }

    #[test]
    fn test_extract_skips_cases_without_sierra() {
        let input = r#"//! > no_sierra_test

//! > test_runner_name
SmallE2ETestRunner

//! > cairo_code
fn foo() {}
"#;

        let cases = extract_sierra_from_test_file(input);
        assert!(cases.is_empty());
    }
}
