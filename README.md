# hotpath - find and profile bottlenecks in Rust
[![Latest Version](https://img.shields.io/crates/v/hotpath.svg)](https://crates.io/crates/hotpath) [![GH Actions](https://github.com/pawurb/hotpath/actions/workflows/ci.yml/badge.svg)](https://github.com/pawurb/hotpath/actions)

![Report](hotpath-report2.png)

A lightweight, easy-to-configure Rust profiler that shows exactly where your code spends time. Instrument any function or code block to quickly spot bottlenecks, and focus your optimizations where they matter most.

## Features

- **Zero-cost when disabled** — fully gated by a feature flag.
- **Low-overhead** profiling for both sync and async code.
- **Detailed stats**: min, max, avg, total time, call count, % of total runtime, and configurable percentiles (p95, p99, etc.).
- **Background processing** for minimal profiling impact.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
hotpath = { version = "0.2", optional = true }

[features]
hotpath = ["dep:hotpath"]
```

## Usage

```rust
use std::time::Duration;

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn sync_function(sleep: u64) {
    std::thread::sleep(Duration::from_nanos(sleep));
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
async fn async_function(sleep: u64) {
    tokio::time::sleep(Duration::from_nanos(sleep)).await;
}

// When using with tokio, place the #[tokio::main] first
#[tokio::main]
// You can configure any percentile between 1 and 99
#[cfg_attr(feature = "hotpath", hotpath::main(percentiles = [99]))]
async fn main() {
    for i in 0..100 {
        // Measured functions will automatically send metrics
        sync_function(i);
        async_function(i * 2).await;

        // Measure code blocks with static labels
        #[cfg(feature = "hotpath")]
        hotpath::measure_block!("custom_block", {
            std::thread::sleep(Duration::from_nanos(i * 3))
        });
    }
}
```

Run your program with a `hotpath` feature:

```
cargo run --features=hotpath
```

Output:
```
[hotpath] Performance Summary from basic::main (Total time: 126.86ms):
+-----------------------+-------+----------+---------+---------+---------+----------+---------+
| Function              | Calls | Min      | Max     | Avg     | P99     | Total    | % Total |
+-----------------------+-------+----------+---------+---------+---------+----------+---------+
| basic::async_function | 100   | 58.50µs  | 1.30ms  | 1.18ms  | 1.30ms  | 118.48ms | 93.40%  |
+-----------------------+-------+----------+---------+---------+---------+----------+---------+
| custom_block          | 100   | 125.00ns | 67.04µs | 21.28µs | 42.63µs | 2.13ms   | 1.68%   |
+-----------------------+-------+----------+---------+---------+---------+----------+---------+
| basic::sync_function  | 100   | 250.00ns | 44.54µs | 20.89µs | 37.67µs | 2.09ms   | 1.65%   |
+-----------------------+-------+----------+---------+---------+---------+----------+---------+
```

## How It Works

1. `#[cfg_attr(feature = "hotpath", hotpath::main)]` - Macro that initializes the background measurement processing
2. `#[cfg_attr(feature = "hotpath", hotpath::measure)]` - Macro that wraps functions with timing code
3. **Background thread** - Measurements are sent to a dedicated worker thread via bounded channel
4. **Statistics aggregation** - Worker thread maintains running statistics for each function/code block
5. **Automatic reporting** - Performance summary displayed when the program exits

## API

`#[cfg_attr(feature = "hotpath", hotpath::main]`

Attribute macro that initializes the background measurement processing when applied to your main function. Can only be used once per program. 

`#[cfg_attr(feature = "hotpath", hotpath::measure)]`

An opt-in attribute macro that instruments functions to send timing measurements to the background processor.

`hotpath::measure_block!(label, expr)`

Macro that measures the execution time of a code block with a static string label.

### Percentiles Support

By default, `hotpath` displays P95 percentile in the performance summary. You can customize which percentiles to display using the `percentiles` parameter:

```rust
#[tokio::main]
#[cfg_attr(feature = "hotpath", hotpath::main(percentiles = [50, 75, 90, 95, 99]))]
async fn main() {
    // Your code here
}
```

For multiple measurements of the same function or code block, percentiles help identify performance distribution patterns.

## Benchmarking

Measure overhead of profiling 1 million empty method calls with [hyperfine](https://github.com/sharkdp/hyperfine):

```
cargo build --example benchmark --features hotpath --release
hyperfine --warmup 3 './target/release/examples/benchmark'
```
