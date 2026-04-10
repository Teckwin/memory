//! Memory Core - Core types and traits for the memory management system

pub mod types;
pub mod error;
pub mod traits;

pub use types::*;
pub use error::MemoryError;
pub use traits::*;