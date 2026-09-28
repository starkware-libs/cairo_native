use cairo_lang_sierra::ProgramParser;
use cairo_native::context::NativeContext;
use std::fs;
use std::path::Path;

const SRC_DIR: &str = "vendor/cairo/tests/e2e_test_data/libfuncs";

/// Walks the raw e2e test data files, extracts sierra_code sections, and
/// compiles each through cairo-native (Sierra -> LLVM) to verify they compile
/// without errors.
#[test]
fn compile_e2e_libfunc_sierra() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SRC_DIR);
    assert!(
        src_dir.exists(),
        "{} not found. Run 'git submodule update --init' first.",
        src_dir.display()
    );

    let context = NativeContext::new();
    let parser = ProgramParser::new();

    let mut total = 0;
    let mut failures = Vec::new();

    for entry in walkdir::WalkDir::new(&src_dir).sort_by_file_name() {
        let entry = entry.unwrap();
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let content = fs::read_to_string(path).unwrap();
        let rel_path = path.strip_prefix(&src_dir).unwrap();
        let test_cases = extract_sierra_from_test_file(&content);

        for (test_name, sierra) in test_cases {
            total += 1;

            let label = format!("{}/{test_name}", rel_path.display());

            let program = match parser.parse(&sierra) {
                Ok(p) => p,
                Err(e) => {
                    failures.push(format!("{label}: parse error: {e}"));
                    continue;
                }
            };

            eprintln!("compiling {label}");
            if let Err(e) = context.compile(&program, false, Some(Default::default()), None) {
                failures.push(format!("{label}: compile error: {e}"));
            }
        }
    }

    assert!(
        total > 0,
        "No sierra test cases found in {}",
        src_dir.display()
    );

    if !failures.is_empty() {
        panic!(
            "{}/{} e2e libfunc sierra tests failed:\n{}",
            failures.len(),
            total,
            failures.join("\n")
        );
    }
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
