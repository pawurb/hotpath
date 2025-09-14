/// Test case for verifying allocation tracking accuracy with nested function calls
/// This example creates a known allocation pattern and verifies the reported numbers are correct

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn allocate_inner(size: usize) -> Vec<u8> {
    // Allocate exactly the requested size
    vec![0u8; size]
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn allocate_outer(inner_size: usize, outer_size: usize) -> (Vec<u8>, Vec<u8>) {
    // First allocate in the outer function
    let outer_vec = vec![0u8; outer_size];

    // Then call inner function which does its own allocation
    let inner_vec = allocate_inner(inner_size);

    (outer_vec, inner_vec)
}

#[cfg_attr(feature = "hotpath", hotpath::main)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing nested allocation tracking...");

    // Test case:
    // - outer function allocates 1000 bytes
    // - inner function allocates 500 bytes
    // - Total actual allocation: 1500 bytes
    // - Expected report: outer=1000, inner=500, total=1500

    let (outer, inner) = allocate_outer(500, 1000);

    println!("Actual allocations:");
    println!("- Outer function: {} bytes", outer.len());
    println!("- Inner function: {} bytes", inner.len());
    println!("- Total actual: {} bytes", outer.len() + inner.len());

    // Keep the vectors alive so they're not optimized away
    std::hint::black_box((outer, inner));

    Ok(())
}
