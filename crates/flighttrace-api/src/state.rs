//! Shared application state passed into Axum handlers.

use sqlx::SqlitePool;
use std::sync::Arc;

/// State injected into every Axum handler.
#[derive(Debug, Clone)]
pub struct AppState {
    pool: Arc<SqlitePool>,
}

impl AppState {
    /// Build a new [`AppState`] from a connected pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }

    /// Borrow the SQLite pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
