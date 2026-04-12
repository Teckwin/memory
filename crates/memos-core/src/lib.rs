//! Memory Core - Core types and traits for the memory management system

pub mod error;
pub mod traits;
pub mod types;

pub use error::MemoryError;
pub use traits::*;
pub use types::*;
