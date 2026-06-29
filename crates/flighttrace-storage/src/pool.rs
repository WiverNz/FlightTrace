//! SQLite pool setup with migrations and the FlightTrace PRAGMAs.

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use std::str::FromStr;

use crate::error::{StorageError, StorageResult};

/// Embedded migration set, applied on connect.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Open (or create) a SQLite database and apply all migrations.
///
/// `database_url` follows the sqlx SQLite URL conventions, e.g.
/// `sqlite://./data/flighttrace.db` or `sqlite::memory:`.
pub async fn connect(database_url: &str) -> StorageResult<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new().connect_with(options).await?;
    MIGRATOR.run(&pool).await.map_err(StorageError::from)?;
    Ok(pool)
}

/// Open an in-memory SQLite database that lives for the lifetime of the pool.
///
/// Used by tests to get an isolated, schema-loaded pool without touching disk.
/// Foreign keys are enabled; the pool is constrained to a single connection so
/// the in-memory database is shared across queries.
pub async fn connect_in_memory() -> StorageResult<SqlitePool> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    MIGRATOR.run(&pool).await.map_err(StorageError::from)?;
    Ok(pool)
}
