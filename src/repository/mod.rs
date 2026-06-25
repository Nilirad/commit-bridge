//! Implementation of the repository pattern.

pub mod branch;
pub mod error;
pub mod sqlite;
pub mod subscription;
pub mod trigger;

pub use error::RepositoryError;
pub use sqlite::SqliteRepository;
