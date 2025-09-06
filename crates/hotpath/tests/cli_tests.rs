#[cfg(test)]
pub mod tests {
    use std::process::Command;

    #[test]
    fn test_basic_output() {
        let output = Command::new("cargo")
            .args(["run", "--example", "basic", "--features", "hotpath"])
            .output()
            .expect("Failed to execute command");

        assert!(
            output.status.success(),
            "Process did not exit successfully: {output:?}",
        );

        let expected = [
            "async_block",
            "basic::sync_function",
            "sync_function",
            "basic::async_function",
        ];

        let stdout = String::from_utf8_lossy(&output.stdout);
        for expected in expected {
            assert!(
                stdout.contains(expected),
                "Output did not match expected.\nExpected:\n{expected}\n\nGot:\n{stdout}",
            );
        }
    }

    #[test]
    fn test_early_returns_output() {
        let output = Command::new("cargo")
            .args(["run", "--example", "early_returns", "--features", "hotpath"])
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
