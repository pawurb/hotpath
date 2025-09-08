#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn noop_sync_function() {
    // This function does nothing - pure no-op for benchmarking
}

#[cfg_attr(feature = "hotpath", hotpath::main)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for _ in 0..100_0000 {
        noop_sync_function();
    }

    Ok(())
}
