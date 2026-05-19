use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

pub async fn init_pool(db_path: &Path) -> Result<SqlitePool, DbError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let url = format!("sqlite://{}", db_path.display());
    let options = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .pragma("journal_mode", "WAL");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn in_memory_pool() -> Result<SqlitePool, DbError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    // Ensure FK enforcement (matches init_pool behavior for consistency)
    sqlx::query("PRAGMA foreign_keys = ON;").execute(&pool).await?;
    Ok(pool)
}

pub async fn ensure_schema(pool: &SqlitePool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub async fn init_pool_in_memory() -> Result<SqlitePool, DbError> {
    let pool = in_memory_pool().await?;
    ensure_schema(&pool).await?;
    Ok(pool)
}
