use crate::DbError;
use smart_noter_core::models::AppSettings;
use sqlx::SqlitePool;

const KEY: &str = "app";

pub async fn get(pool: &SqlitePool) -> Result<AppSettings, DbError> {
    let row = sqlx::query!("SELECT value FROM settings WHERE key = ?", KEY)
        .fetch_optional(pool)
        .await?;
    match row {
        Some(r) => Ok(serde_json::from_str(&r.value).unwrap_or_default()),
        None => Ok(AppSettings::default()),
    }
}

pub async fn upsert(pool: &SqlitePool, settings: &AppSettings) -> Result<(), DbError> {
    let value = serde_json::to_string(settings).unwrap();
    sqlx::query!(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        KEY,
        value
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;
    use smart_noter_core::models::Theme;

    #[tokio::test]
    async fn get_returns_default_when_empty() {
        let pool = init_pool_in_memory().await.unwrap();
        let s = get(&pool).await.unwrap();
        assert_eq!(s.theme, Theme::Light);
    }

    #[tokio::test]
    async fn upsert_then_get_roundtrips() {
        let pool = init_pool_in_memory().await.unwrap();
        let s = AppSettings {
            theme: Theme::Dark,
            ..Default::default()
        };
        upsert(&pool, &s).await.unwrap();
        let loaded = get(&pool).await.unwrap();
        assert_eq!(loaded.theme, Theme::Dark);
    }
}
