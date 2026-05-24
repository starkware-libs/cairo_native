use std::{path::Path, sync::Arc};

use cairo_lang_compiler::{
    db::RootDatabase,
    diagnostics::DiagnosticsReporter,
    project::{check_compiler_path, setup_project},
};
use cairo_lang_filesystem::{
    cfg::{Cfg, CfgSet},
    ids::CrateInput,
};
use cairo_lang_runner::{casm_run::format_for_panic, RunResultValue};
use cairo_lang_test_plugin::{
    compile_test_prepared_db,
    test_config::{PanicExpectation, TestExpectation},
    test_plugin_suite, TestCompilation, TestsCompilationConfig,
};
use common::value_to_felt;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use sierra_emu::{run_program, EntryPoint, ProgramTrace, Value};

mod common;

enum TestStatus {
    Passed,
    Failed(String),
    Ignored,
}

#[test]
fn test_corelib() {
    let compiler_path = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("corelib");

    check_compiler_path(false, &compiler_path)
        .expect("Couldn't find the corelib in the given path");

    let db = &mut {
        let mut b = RootDatabase::builder();
        b.detect_corelib();
        b.with_cfg(CfgSet::from_iter([Cfg::name("test")]));
        b.with_default_plugin_suite(test_plugin_suite());
        b.build().unwrap()
    };

    let main_crate_inputs = setup_project(db, &compiler_path).unwrap();

    let db = db.snapshot();
    let test_crate_ids = main_crate_inputs.clone();
    let test_config = TestsCompilationConfig {
        starknet: false,
        add_statements_functions: false,
        add_statements_code_locations: false,
        contract_declarations: None,
        contract_crate_ids: None,
        executable_crate_ids: None,
        add_functions_debug_info: false,
        add_type_names: false,
        replace_ids: true,
    };

    let diag_reporter = DiagnosticsReporter::stderr().with_crates(&main_crate_inputs);

    let filtered_tests = vec![
        "core::test::dict_test::test_array_from_squash_dict",
        "core::test::hash_test::test_blake2s",
        "core::test::testing_test::test_get_unspent_gas",
        "core::test::qm31_test::",
    ];

    let compiled = compile_tests(
        &db,
        test_config,
        test_crate_ids,
        diag_reporter,
        Some(&filtered_tests),
    );

    let results = run_tests(compiled);

    display_results(&results);

    assert!(results
        .iter()
        .all(|s| matches!(s, TestStatus::Passed | TestStatus::Ignored)),);
}

/// Runs the tests and process the results for a summary.
fn run_tests(compiled: TestCompilation) -> Vec<TestStatus> {
    let program = Arc::new(compiled.sierra_program.program);

    compiled
        .metadata
        .named_tests
        .into_par_iter()
        .map(|(name, test)| {
            if test.ignored {
                println!("test {} ... Ignored", name);
                return TestStatus::Ignored;
            }

            // catch any panic during the test run
            let res = std::panic::catch_unwind(|| {
                run_program(
                    program.clone(),
                    EntryPoint::String(name.clone()),
                    vec![],
                    test.available_gas.map(|g| g as u64).unwrap_or_default(),
                )
            });

            let status = match res {
                Ok(trace) => {
                    let run_result = trace_to_run_result(trace);

                    assert_test_expectation(test.expectation, run_result)
                }
                Err(panic) => {
                    TestStatus::Failed(format!("PANIC: {:?}", panic.downcast_ref::<String>()))
                }
            };

            match &status {
                TestStatus::Passed => println!("test {} ... OK", name),
                TestStatus::Failed(err) => println!("test {} ... FAILED: {}", name, err),
                TestStatus::Ignored => {} // already handled before
            };

            status
        })
        .collect::<Vec<TestStatus>>()
}

fn trace_to_run_result(trace: ProgramTrace) -> RunResultValue {
    let return_value = trace.return_value();
    let mut felts = Vec::new();

    let is_success = match &return_value {
        outer_value @ Value::Enum {
            self_ty,
            index,
            payload,
            ..
        } => {
            let debug_name = self_ty.debug_name.as_ref().expect("missing debug name");
            if debug_name.starts_with("core::panics::PanicResult::")
                || debug_name.starts_with("Enum<ut@core::panics::PanicResult::")
            {
                let is_success = *index == 0;

                if !is_success {
                    match &**payload {
                        Value::Struct(fields) => {
                            for field in fields {
                                let felt = value_to_felt(field);
                                felts.extend(felt);
                            }
                        }
                        _ => panic!("unsuported return value in cairo-native"),
                    }
                } else {
                    felts.extend(value_to_felt(payload));
                }

                is_success
            } else {
                felts.extend(value_to_felt(outer_value));
                true
            }
        }
        x => {
            felts.extend(value_to_felt(x));
            true
        }
    };

    let return_values = felts.into_iter().map(|x| x.to_bigint().into()).collect();

    match is_success {
        true => RunResultValue::Success(return_values),
        false => RunResultValue::Panic(return_values),
    }
}

fn assert_test_expectation(expectation: TestExpectation, result: RunResultValue) -> TestStatus {
    match result {
        RunResultValue::Success(r) => {
            if let TestExpectation::Panics(_) = expectation {
                let err_msg = format_for_panic(r.into_iter());
                return TestStatus::Failed(err_msg);
            }
            TestStatus::Passed
        }
        RunResultValue::Panic(e) => match expectation {
            TestExpectation::Success => {
                let err_msg = format_for_panic(e.into_iter());
                TestStatus::Failed(err_msg)
            }
            TestExpectation::Panics(panic_expect) => match panic_expect {
                PanicExpectation::Exact(expected) => {
                    if expected != e {
                        let err_msg = format_for_panic(e.into_iter());
                        return TestStatus::Failed(err_msg);
                    }
                    TestStatus::Passed
                }
                PanicExpectation::Any => TestStatus::Passed,
            },
        },
    }
}

fn display_results(results: &[TestStatus]) {
    let mut passed = 0;
    let mut failed = 0;
    let mut ignored = 0;

    for status in results {
        match &status {
            TestStatus::Passed => passed += 1,
            TestStatus::Failed(_) => failed += 1,
            TestStatus::Ignored => ignored += 1,
        }
    }

    if failed > 0 {
        println!(
            "\n\ntest result: FAILED. {} PASSED; {} FAILED; {} FILTERED OUT;",
            passed, failed, ignored
        );
    } else {
        println!(
            "\n\ntest result: OK. {} PASSED; {} FAILED; {} FILTERED OUT;",
            passed, failed, ignored
        );
    }
}

fn compile_tests<'a>(
    db: &'a RootDatabase,
    test_config: TestsCompilationConfig<'a>,
    test_crate_inputs: Vec<CrateInput>,
    diag_reporter: DiagnosticsReporter<'_>,
    with_filtered_tests: Option<&[&str]>,
) -> TestCompilation<'a> {
    let mut compiled =
        compile_test_prepared_db(db, test_config, test_crate_inputs, diag_reporter).unwrap();
    // Note: No need to call replace_sierra_ids_in_program — compile_test_prepared_db with
    // replace_ids: true already replaces IDs (new in cairo 2.16).

    if let Some(compilation_filter) = with_filtered_tests {
        let should_skip_test = |name: &str| -> bool {
            compilation_filter
                .iter()
                .any(|filter| name.contains(filter))
        };

        // Ignore matching test cases.
        compiled
            .metadata
            .named_tests
            .iter_mut()
            .for_each(|(test, case)| {
                if should_skip_test(test) {
                    case.ignored = true
                }
            });
    }

    compiled
}
