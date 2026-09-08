//! SQLite implementation of the repository.
//!
//! This module hosts the [`SqliteRepository`] type and its connection
//! plumbing; each repository trait is implemented in its own submodule
//! (`branch`, `subscription`, `trigger`).

mod branch;
mod subscription;
mod trigger;

use std::str::FromStr;

use crate::config::DatabaseConfig;
use crate::error::FatalError;
use futures::future::BoxFuture;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug)]
/// Access point of the repository using a SQLite connection pool.
pub struct SqliteRepository {
    /// The SQLite connection pool to the database.
    pool: SqlitePool,
}

impl SqliteRepository {
    /// Connects to the database described by `config`.
    pub async fn connect(config: &DatabaseConfig) -> Result<Self, FatalError> {
        let options = SqliteConnectOptions::from_str(config.url.as_str())?
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .acquire_timeout(config.timeout)
            .connect_with(options)
            .await?;

        // Ensures database schema is up to date in all environments.
        sqlx::migrate!().run(&pool).await?;

        Ok(Self { pool })
    }

    /// Creates a new [`SqliteRepository`] from a [`SqlitePool`].
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Runs a closure within a transaction.
    #[tracing::instrument(skip_all, fields(otel.kind = "internal"))]
    pub async fn run_in_transaction<'a, F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'b> FnOnce(&'b mut SqliteConnection) -> BoxFuture<'b, Result<T, E>> + Send + 'a,
        E: From<sqlx::Error> + Send + 'a,
        T: Send + 'a,
    {
        let mut tx = self.pool.begin().await?;
        let result = f(&mut tx).await?;
        tx.commit().await?;
        Ok(result)
    }
}
