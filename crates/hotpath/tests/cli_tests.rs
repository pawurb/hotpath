#[cfg(test)]
pub mod tests {
    use std::process::Command;

    #[test]
    fn test_basic_output() {
        let features = [
            "",
            "hotpath-alloc-bytes-total",
            "hotpath-alloc-bytes-max",
            "hotpath-alloc-count-total",
            "hotpath-alloc-count-max",
        ];

        for feature in features {
            let features_arg = if feature.is_empty() {
                "hotpath".to_string()
            } else {
                format!("hotpath,{}", feature)
            };

            let output = Command::new("cargo")
                .args([
                    "run",
                    "-p",
                    "hotpath-test-tokio-async",
                    "--example",
                    "basic",
                    "--features",
                    &features_arg,
                ])
                .output()
                .expect("Failed to execute command");

            assert!(
                output.status.success(),
                "Process did not exit successfully: {output:?}",
            );

            let all_expected = [
                "custom_block",
                "basic::sync_function",
                "basic::async_function",
                "p95",
                "total",
                "percent_total",
            ];

            let stdout = String::from_utf8_lossy(&output.stdout);
            for expected in all_expected {
                assert!(
                    stdout.contains(expected),
                    "Expected:\n{expected}\n\nGot:\n{stdout}",
                );
            }
        }
    }

    #[test]
    fn test_early_returns_output() {
        let features = [
            "hotpath",
            "hotpath-alloc-bytes-total",
            "hotpath-alloc-bytes-max",
            "hotpath-alloc-count-total",
            "hotpath-alloc-count-max",
        ];
        for feature in features {
            let features_arg = if feature == "hotpath" {
                "hotpath".to_string()
            } else {
                format!("hotpath,{}", feature)
            };

            let output = Command::new("cargo")
                .args([
                    "run",
                    "-p",
                    "hotpath-test-tokio-async",
                    "--example",
                    "early_returns",
                    "--features",
                    &features_arg,
                ])
                .output()
                .expect("Failed to execute command");

            assert!(
                output.status.success(),
                "Process did not exit successfully: {output:?}",
            );

            let all_expected = [
                "early_returns::early_return",
                "early_returns::propagates_error",
                "early_returns::normal_path",
            ];

            let stdout = String::from_utf8_lossy(&output.stdout);
            for expected in all_expected {
                assert!(
                    stdout.contains(expected),
                    "Expected:\n{expected}\n\nGot:\n{stdout}",
                );
            }
        }
    }

    #[test]
    fn test_unsupported_async_output() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "unsupported_async",
                "--features",
                "hotpath,hotpath-alloc-bytes-max",
            ])
            .output()
            .expect("Failed to execute command");
        let stdout = String::from_utf8_lossy(&output.stdout);

        let all_expected = ["N/A*", "only available for tokio current_thread"];

        for expected in all_expected {
            assert!(
                stdout.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_main_empty_params() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "main_empty",
                "--features",
                "hotpath",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("main_empty::example_function"));
        assert!(stdout.contains("P95"));
        assert!(stdout.contains("Function"));
    }

    #[test]
    fn test_main_percentiles_param() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "main_percentiles",
                "--features",
                "hotpath",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let all_expected = [
            "main_percentiles::example_function",
            "P50",
            "P90",
            "P99",
            "Function",
        ];

        let stdout = String::from_utf8_lossy(&output.stdout);
        for expected in all_expected {
            assert!(
                stdout.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_main_format_param() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "main_format",
                "--features",
                "hotpath",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let all_expected = [
            "main_format::example_function",
            "\"hotpath_profiling_mode\"",
            "\"calls\"",
        ];

        let stdout = String::from_utf8_lossy(&output.stdout);
        for expected in all_expected {
            assert!(
                stdout.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_main_percentiles_format_params() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "main_percentiles_format",
                "--features",
                "hotpath",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let all_expected = [
            "main_percentiles_format::example_function",
            "\"hotpath_profiling_mode\"",
            "\"p75\"",
            "\"p95\"",
        ];

        let stdout = String::from_utf8_lossy(&output.stdout);
        for expected in all_expected {
            assert!(
                stdout.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_unit_test_multiple_guards() {
        let output = Command::new("cargo")
            .args([
                "test",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "unit_test",
                "--features",
                "hotpath",
                "--",
                "--test-threads",
                "1",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let all_expected = ["unit_test::async_function", "unit_test::sync_function"];

        let stdout = String::from_utf8_lossy(&output.stdout);
        for expected in all_expected {
            assert!(
                stdout.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_async_smol_alloc_profiling_output() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-smol-async",
                "--example",
                "basic_smol",
                "--features",
                "hotpath,hotpath-alloc-bytes-max",
                "--",
                "--nocapture",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let all_expected = ["N/A*", "only available for tokio current_thread"];

        let stdout = String::from_utf8_lossy(&output.stdout);
        for expected in all_expected {
            assert!(
                stdout.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_all_features_output() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-all-features",
                "--example",
                "basic",
                "--all-features",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let all_expected = ["i ran"];

        let stdout = String::from_utf8_lossy(&output.stdout);

        for expected in all_expected {
            assert!(
                stdout.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_csv_file_reporter_output() {
        use std::fs;
        use std::path::Path;

        let report_path = "hotpath_report.csv";
        if Path::new(report_path).exists() {
            fs::remove_file(report_path).ok();
        }

        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "csv_file_reporter",
                "--features",
                "hotpath",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        assert!(
            Path::new(report_path).exists(),
            "Custom report file was not created"
        );

        let report_content = fs::read_to_string(report_path).expect("Failed to read report file");

        let expected_content = [
            "Function, Calls, Avg, P50, P90, P95, Total, % Total",
            "Functions measured: 3",
            "csv_file_reporter::async_function, 100",
            "csv_file_reporter::sync_function, 100",
            "custom_block, 100",
        ];

        for expected in expected_content {
            assert!(
                report_content.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{report_content}",
            );
        }

        fs::remove_file(report_path).ok();
    }

    #[test]
    fn test_tracing_reporter_output() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "tracing_reporter",
                "--features",
                "hotpath",
            ])
            .env("RUST_LOG", "info")
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let stdout = String::from_utf8_lossy(&output.stdout);

        let expected_content = [
            "HotPath Report for: main",
            "Headers: Function, Calls, Avg, P50, P90, P95, Total, % Total",
            "tracing_reporter::async_function, 100",
            "tracing_reporter::sync_function, 100",
            "custom_block, 100",
        ];

        for expected in expected_content {
            assert!(
                stdout.contains(expected),
                "Expected:\\n{expected}\\n\\nGot:\\n{stdout}",
            );
        }
    }

    #[test]
    fn test_json_file_reporter_output() {
        use std::fs;
        use std::path::Path;

        let report_path = "hotpath_report.json";
        if Path::new(report_path).exists() {
            fs::remove_file(report_path).ok();
        }

        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "json_file_reporter",
                "--features",
                "hotpath",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Report saved to hotpath_report.json"),
            "Expected success message not found in stdout: {stdout}"
        );

        assert!(
            Path::new(report_path).exists(),
            "JSON report file was not created"
        );

        let report_content = fs::read_to_string(report_path).expect("Failed to read report file");

        let expected_content = [
            "\"hotpath_profiling_mode\"",
            "\"timing\"",
            "\"total_elapsed\"",
            "\"caller_name\"",
            "\"main\"",
            "\"output\"",
            "\"json_file_reporter::async_function\"",
            "\"json_file_reporter::sync_function\"",
            "\"custom_block\"",
            "\"calls\"",
            "\"avg\"",
            "\"total\"",
            "\"percent_total\"",
        ];

        for expected in expected_content {
            assert!(
                report_content.contains(expected),
                "Expected:\n{expected}\n\nGot:\n{report_content}",
            );
        }

        serde_json::from_str::<serde_json::Value>(&report_content)
            .expect("Report content is not valid JSON");

        fs::remove_file(report_path).ok();
    }

    #[test]
    fn test_no_op_block_output() {
        let output = Command::new("cargo")
            .args([
                "run",
                "-p",
                "hotpath-test-tokio-async",
                "--example",
                "no_op_block",
            ])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("custom_block output"));
    }
}
