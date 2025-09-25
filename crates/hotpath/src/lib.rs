#[cfg(not(feature = "hotpath-off"))]
pub mod lib_on;
#[cfg(not(feature = "hotpath-off"))]
pub use lib_on::*;

// When hotpath is disabled with hotpath-off feature we import methods from lib_off, which are all no-op
#[cfg(feature = "hotpath-off")]
pub mod lib_off;
#[cfg(feature = "hotpath-off")]
pub use lib_off::*;
