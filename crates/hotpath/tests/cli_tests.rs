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
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--example",
                    "basic",
                    "--features",
                    "hotpath",
                    feature,
                ])
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
                "P99",
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
            let output = Command::new("cargo")
                .args([
                    "run",
                    "--example",
                    "early_returns",
                    "--features",
                    "hotpath",
                    feature,
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
}
