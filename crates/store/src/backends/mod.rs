//! Storage backend implementations

pub mod sled;

// TODO: Add SQLite backend
// pub mod sqlite;

pub mod memory;

pub use memory::MemoryStore;
