use crate::DbError;
use sqlx::SqlitePool;

pub async fn seed_if_empty(
    _pool: &SqlitePool,
    _json_path: &std::path::Path,
) -> Result<(), DbError> {
    // Implementation lands in Task 1.6
    Ok(())
}
