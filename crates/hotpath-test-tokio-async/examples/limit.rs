use std::time::Duration;

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn function_one() {
    std::thread::sleep(Duration::from_nanos(500));
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn function_two() {
    std::thread::sleep(Duration::from_nanos(500));
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
fn function_three() {}

#[cfg_attr(feature = "hotpath", hotpath::main(limit = 3))]
fn main() {
    for _ in 0..10 {
        function_one();
        function_two();
        function_three();
    }
}
