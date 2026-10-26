use cairo_lang_sierra::{program::Program, ProgramParser};
use cairo_native::{context::NativeContext, utils::testing::find_type_layout_mismatches};
use itertools::Itertools;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

const SRC_DIR: &str = "vendor/cairo/tests/e2e_test_data/libfuncs";

/// Walks the raw e2e test data files, extracts the `sierra_code` sections and
/// parses them into `(label, program)` pairs. A parse failure means a broken
/// corpus, so it panics.
fn load_e2e_libfunc_programs() -> Vec<(String, Program)> {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SRC_DIR);
    assert!(
        src_dir.exists(),
        "{} not found. Run 'git submodule update --init' first.",
        src_dir.display()
    );

    let parser = ProgramParser::new();
    let mut programs = Vec::new();

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
            let program = parser
                .parse(&sierra)
                .unwrap_or_else(|e| panic!("{label}: parse error: {e}"));
            programs.push((label, program));
        }
    }

    assert!(
        !programs.is_empty(),
        "No sierra test cases found in {}",
        src_dir.display()
    );

    programs
}

/// Compiles every e2e libfunc Sierra program through cairo-native
/// (Sierra -> LLVM) to verify they compile without errors.
#[test]
fn compile_e2e_libfunc_sierra() {
    let programs = load_e2e_libfunc_programs();

    let context = NativeContext::new();
    let mut failures = Vec::new();
    for (label, program) in &programs {
        if let Err(e) = context.compile(program, false, Some(Default::default()), None) {
            failures.push(format!("{label}: compile error: {e}"));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{}/{} e2e libfunc sierra tests failed:\n{}",
            failures.len(),
            programs.len(),
            failures.join("\n")
        );
    }
}

/// Checks that `layout()` of every type declared by the e2e libfunc corpus
/// matches the size and alignment LLVM assigns to the emitted MLIR type.
#[test]
fn e2e_libfunc_type_layouts_match_llvm() {
    let programs = load_e2e_libfunc_programs();

    let context = NativeContext::new();
    let mut mismatches = BTreeSet::new();
    for (label, program) in &programs {
        mismatches.extend(
            find_type_layout_mismatches(&context, program)
                .unwrap_or_else(|e| panic!("{label}: {e}")),
        );
    }

    assert!(
        mismatches.is_empty(),
        "{} type(s) whose layout() disagrees with LLVM:\n{}",
        mismatches.len(),
        mismatches.iter().join("\n")
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
