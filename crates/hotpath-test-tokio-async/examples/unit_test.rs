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

#[tokio::main(flavor = "current_thread")]
#[cfg_attr(feature = "hotpath", hotpath::main)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(feature = "hotpath", hotpath::main)]
    fn test_sync_function() {
        sync_function(100);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_async_function() {
        #[cfg(feature = "hotpath")]
        let _hotpath = hotpath::init(
            "test_async_function".to_string(),
            &[50, 90, 95],
            hotpath::Format::Table,
        );

        async_function(100).await;
    }
}
