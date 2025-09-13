use std::time::Duration;

#[cfg_attr(feature = "hotpath", hotpath::measure)]
async fn async_function(sleep: u64) {
    let vec1 = vec![1, 2, 3, 5, 6, 7, 8, 9, 10];
    drop(vec1);
    let _vec = vec![1, 2, 3, 5, 6, 7, 8, 9, 10];
    tokio::time::sleep(Duration::from_nanos(sleep)).await;
}

#[tokio::main(flavor = "multi_thread")]
#[cfg_attr(feature = "hotpath", hotpath::main(percentiles = [0,99,100]))]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    for i in 0..100 {
        async_function(i * 2).await;
    }

    Ok(())
}
