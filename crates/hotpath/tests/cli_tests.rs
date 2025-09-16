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
                .args(["run", "--example", "basic", "--features", &features_arg])
                .output()
                .expect("Failed to execute command");

            assert!(
                output.status.success(),
                "Process did not exit successfully: {output:?}",
            );

            let expected = [
                "custom_block",
                "basic::sync_function",
                "basic::async_function",
                "P95",
                "Total",
                "% Total",
            ];

            let stdout = String::from_utf8_lossy(&output.stdout);
            for expected in expected {
                assert!(
                    stdout.contains(expected),
                    "Output did not match expected.\nExpected:\n{expected}\n\nGot:\n{stdout}",
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

            let expected = [
                "early_returns::early_return",
                "early_returns::propagates_error",
                "early_returns::normal_path",
            ];

            let stdout = String::from_utf8_lossy(&output.stdout);
            for expected in expected {
                assert!(
                    stdout.contains(expected),
                    "Output did not match expected.\nExpected:\n{expected}\n\nGot:\n{stdout}",
                );
            }
        }
    }

    #[test]
    fn test_unsupported_async_output() {
        let output = Command::new("cargo")
            .args([
                "run",
                "--example",
                "unsupported_async",
                "--features",
                "hotpath,hotpath-alloc-bytes-max",
            ])
            .output()
            .expect("Failed to execute command");
        let stdout = String::from_utf8_lossy(&output.stdout);

        let expected = ["N/A*"];

        for expected in expected {
            assert!(
                stdout.contains(expected),
                "Output did not match expected.\nExpected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_main_empty_params() {
        let output = Command::new("cargo")
            .args(["run", "--example", "main_empty", "--features", "hotpath"])
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

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("main_percentiles::example_function"));
        assert!(stdout.contains("P50"));
        assert!(stdout.contains("P90"));
        assert!(stdout.contains("P99"));
        assert!(stdout.contains("Function"));
    }

    #[test]
    fn test_main_format_param() {
        let output = Command::new("cargo")
            .args(["run", "--example", "main_format", "--features", "hotpath"])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("main_format::example_function"));
        assert!(stdout.contains("\"hotpath_profiling_mode\""));
        assert!(stdout.contains("\"calls\""));
    }

    #[test]
    fn test_main_percentiles_format_params() {
        let output = Command::new("cargo")
            .args([
                "run",
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

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("main_percentiles_format::example_function"));
        assert!(stdout.contains("\"hotpath_profiling_mode\""));
        assert!(stdout.contains("\"p75\""));
        assert!(stdout.contains("\"p95\"")); // Custom percentile in JSON
    }

    #[test]
    fn test_unit_test_multiple_guards() {
        let output = Command::new("cargo")
            .args([
                "test",
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

        let expected = ["unit_test::async_function", "unit_test::sync_function"];

        let stdout = String::from_utf8_lossy(&output.stdout);
        for expected in expected {
            assert!(
                stdout.contains(expected),
                "Output did not match expected.\nExpected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }
}
