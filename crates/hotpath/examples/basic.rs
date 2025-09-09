use std::time::Duration;

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn sync_function(sleep: u64) {
    std::thread::sleep(Duration::from_nanos(sleep));
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
async fn async_function(sleep: u64) {
    tokio::time::sleep(Duration::from_nanos(sleep)).await;
}

#[tokio::main]
#[cfg_attr(feature = "hotpath", hotpath::main(percentiles = [0,99,100]))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    for i in 0..100 {
        sync_function(i);
        async_function(i * 2).await;
        #[cfg(feature = "hotpath")]
        hotpath::measure_block!("custom_block", {
            std::thread::sleep(Duration::from_nanos(i * 3))
        });
    }

    Ok(())
}
