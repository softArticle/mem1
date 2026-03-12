//! SurrealDB storage layer.

mod db;
mod memory;

pub use db::{connect, ensure_schema, store};
pub use memory::{MemoryStore, SurrealMemoryStore};
