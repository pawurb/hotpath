use std::time::Duration;

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn sync_function() {
    std::thread::sleep(Duration::from_millis(100));
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
async fn async_function() {
    tokio::time::sleep(Duration::from_millis(150)).await;
}

#[tokio::main]
#[cfg_attr(feature = "hotpath", hotpath::main)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    sync_function();
    async_function().await;

    // Measure code blocks with static labels
    #[cfg(feature = "hotpath")]
    hotpath::measure_block!("sync_block", {
        std::thread::sleep(Duration::from_millis(100))
    });

    #[cfg(feature = "hotpath")]
    hotpath::measure_block!("async_block", {
        tokio::time::sleep(Duration::from_millis(150)).await;
    });

    Ok(())
}
