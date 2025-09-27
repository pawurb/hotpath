use std::time::Duration;

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn sync_function(sleep: u64) {
    let vec1 = vec![
        1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    ];
    std::hint::black_box(&vec1);
    drop(vec1);
    let vec2 = vec![
        1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
    ];
    std::hint::black_box(&vec2);
    std::thread::sleep(Duration::from_nanos(sleep));
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
async fn async_function(sleep: u64) {
    let vec1 = vec![1, 2, 3, 5, 6, 7, 8, 9, 10];
    std::hint::black_box(&vec1);
    drop(vec1);
    let vec = vec![1, 2, 3, 5, 6, 7, 8, 9, 10];
    std::hint::black_box(&vec);
    tokio::time::sleep(Duration::from_nanos(sleep)).await;
}

use hotpath::{FunctionStats, Reporter};
use std::collections::HashMap;

struct FileReporter;

impl Reporter for FileReporter {
    fn report(
        &self,
        stats: &HashMap<&'static str, FunctionStats>,
        total_elapsed: Duration,
        caller_name: &str,
        _percentiles: &[u8],
    ) {
        let mut output = String::new();
        output.push_str(&format!("HotPath Report for: {}\n", caller_name));
        output.push_str(&format!("Total Elapsed: {:?}\n", total_elapsed));
        output.push_str(&format!("Functions measured: {}\n", stats.len()));
        output.push_str("Statistics:\n");

        for (function_name, stats) in stats {
            output.push_str(&format!("  {}: {} calls\n", function_name, stats.count));
        }

        std::fs::write("hotpath_report.txt", output).unwrap();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut hotpath = hotpath::init("main".to_string(), &[50, 90, 95], hotpath::Format::Table);
    hotpath.set_reporter(Box::new(FileReporter));

    for i in 0..100 {
        sync_function(i);
        async_function(i * 2).await;

        #[cfg(feature = "hotpath")]
        hotpath::measure_block!("custom_block", {
            if i == 0 {
                println!("custom_block output");
            }
            std::thread::sleep(Duration::from_nanos(i * 3))
        });
    }

    Ok(())
}
