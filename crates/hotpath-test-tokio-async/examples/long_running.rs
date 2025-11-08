use rand::Rng;
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn fast_sync_allocator() -> Vec<u64> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(10..100);
    let data: Vec<u64> = (0..size).map(|_| rng.gen()).collect();
    std::hint::black_box(&data);
    std::thread::sleep(Duration::from_micros(rng.gen_range(10..50)));
    data
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn medium_sync_allocator() -> Vec<u64> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(100..1000);
    let data: Vec<u64> = (0..size).map(|_| rng.gen()).collect();
    std::hint::black_box(&data);
    std::thread::sleep(Duration::from_micros(rng.gen_range(50..150)));
    data
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn slow_sync_allocator() -> Vec<u64> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(1000..10000);
    let data: Vec<u64> = (0..size).map(|_| rng.gen()).collect();
    std::hint::black_box(&data);
    std::thread::sleep(Duration::from_micros(rng.gen_range(100..300)));
    data
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
async fn fast_async_allocator() -> Vec<u64> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(10..100);
    let data: Vec<u64> = (0..size).map(|_| rng.gen()).collect();
    std::hint::black_box(&data);
    sleep(Duration::from_micros(rng.gen_range(10..50))).await;
    data
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
async fn slow_async_allocator() -> Vec<u64> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(1000..5000);
    let data: Vec<u64> = (0..size).map(|_| rng.gen()).collect();
    std::hint::black_box(&data);
    sleep(Duration::from_micros(rng.gen_range(100..400))).await;
    data
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn process_data(data: Vec<u64>) -> u64 {
    let mut rng = rand::thread_rng();
    let sum: u64 = data
        .iter()
        .take(rng.gen_range(5..20))
        .fold(0u64, |acc, &x| acc.wrapping_add(x % 1000));
    std::hint::black_box(sum);
    sum
}

#[tokio::main(flavor = "current_thread")]
#[cfg_attr(feature = "hotpath", hotpath::main)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting 60-second profiling test...");
    println!("This will call 5 different sync/async methods with random allocations");
    println!("Press Ctrl+C to stop early\n");

    let start = Instant::now();
    let duration = Duration::from_secs(60);
    let mut iteration = 0;

    while start.elapsed() < duration {
        iteration += 1;
        let elapsed = start.elapsed().as_secs();

        if iteration % 10 == 0 {
            println!(
                "[{:>2}s] Iteration {}: Calling mixed sync/async functions...",
                elapsed, iteration
            );
        }

        let data1 = fast_sync_allocator();
        let data2 = medium_sync_allocator();

        if iteration % 3 == 0 {
            let data3 = slow_sync_allocator();
            process_data(data3);
        }

        let data4 = fast_async_allocator().await;
        process_data(data4);

        if iteration % 2 == 0 {
            let data5 = slow_async_allocator().await;
            process_data(data5);
        }

        process_data(data1);

        if iteration % 4 == 0 {
            process_data(data2);
        }

        let mut rng = rand::thread_rng();
        sleep(Duration::from_millis(rng.gen_range(10..50))).await;

        #[cfg(feature = "hotpath")]
        hotpath::measure_block!("iteration_block", {
            let temp: Vec<u32> = (0..rng.gen_range(50..200)).map(|_| rng.gen()).collect();
            std::hint::black_box(&temp);
        });
    }

    println!("\n60 seconds completed! Total iterations: {}", iteration);
    println!("Profiling report will be generated on exit...\n");

    Ok(())
}
