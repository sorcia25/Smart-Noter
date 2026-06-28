use crate::DbError;
use sqlx::SqlitePool;

/// Insert or replace the ciphertext for `provider`.
pub async fn upsert(pool: &SqlitePool, provider: &str, ciphertext: &[u8]) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO provider_secrets (provider, ciphertext, updated_at) \
         VALUES (?, ?, datetime('now')) \
         ON CONFLICT(provider) DO UPDATE SET ciphertext = excluded.ciphertext, updated_at = datetime('now')",
    )
    .bind(provider)
    .bind(ciphertext)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get the stored ciphertext for `provider`, if any.
pub async fn get(pool: &SqlitePool, provider: &str) -> Result<Option<Vec<u8>>, DbError> {
    let row: Option<(Vec<u8>,)> =
        sqlx::query_as("SELECT ciphertext FROM provider_secrets WHERE provider = ?")
            .bind(provider)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(c,)| c))
}

/// Remove the stored key for `provider` (no error if absent).
pub async fn delete(pool: &SqlitePool, provider: &str) -> Result<(), DbError> {
    sqlx::query("DELETE FROM provider_secrets WHERE provider = ?")
        .bind(provider)
        .execute(pool)
        .await?;
    Ok(())
}

/// List provider ids that currently have a stored key.
pub async fn list_providers(pool: &SqlitePool) -> Result<Vec<String>, DbError> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT provider FROM provider_secrets ORDER BY provider")
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|(p,)| p).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    #[tokio::test]
    async fn upsert_get_delete_round_trip() {
        let pool = init_pool_in_memory().await.unwrap();
        assert!(get(&pool, "openai").await.unwrap().is_none());

        upsert(&pool, "openai", &[1, 2, 3]).await.unwrap();
        assert_eq!(get(&pool, "openai").await.unwrap().unwrap(), vec![1, 2, 3]);

        // upsert replaces, not duplicates
        upsert(&pool, "openai", &[9, 9]).await.unwrap();
        assert_eq!(get(&pool, "openai").await.unwrap().unwrap(), vec![9, 9]);

        upsert(&pool, "anthropic", &[7]).await.unwrap();
        assert_eq!(
            list_providers(&pool).await.unwrap(),
            vec!["anthropic", "openai"]
        );

        delete(&pool, "openai").await.unwrap();
        assert!(get(&pool, "openai").await.unwrap().is_none());
        assert_eq!(list_providers(&pool).await.unwrap(), vec!["anthropic"]);
    }
}
