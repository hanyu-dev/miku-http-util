//! Integration into other crates

#[cfg(feature = "feat-integrate-axum")]
pub mod integrate_axum;
#[cfg(feature = "feat-integrate-tower")]
pub mod integrate_tower;
#[cfg(any(feature = "feat-integrate-axum", feature = "feat-integrate-tower"))]
pub mod utils;

#[cfg(feature = "feat-integrate-axum")]
// re-export
pub use integrate_axum::*;
#[cfg(feature = "feat-integrate-tower")]
// re-export
pub use integrate_tower::*;
#[cfg(any(feature = "feat-integrate-axum", feature = "feat-integrate-tower"))]
// re-export
pub use utils::*;
