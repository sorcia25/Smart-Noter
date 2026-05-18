use crate::DbError;
use smart_noter_core::{models::Template, Bilingual};
use sqlx::SqlitePool;

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Template>, DbError> {
    let rows = sqlx::query!(
        r#"SELECT id, color_class, icon, name_es, name_en, desc_es, desc_en, sections_json, is_default
           FROM templates ORDER BY id"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Template {
            id: r.id,
            color_class: r.color_class,
            icon: r.icon,
            name: Bilingual::with_en(r.name_es, r.name_en),
            desc: Bilingual::with_en(r.desc_es, r.desc_en),
            sections: serde_json::from_str(&r.sections_json).unwrap_or_default(),
            is_default: r.is_default != 0,
        })
        .collect())
}

pub async fn set_default(pool: &SqlitePool, id: &str) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query!("UPDATE templates SET is_default = 0")
        .execute(&mut *tx)
        .await?;
    sqlx::query!("UPDATE templates SET is_default = 1 WHERE id = ?", id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    #[tokio::test]
    async fn list_all_returns_empty_on_fresh_db() {
        let pool = init_pool_in_memory().await.unwrap();
        let templates = list_all(&pool).await.unwrap();
        assert!(templates.is_empty());
    }

    #[tokio::test]
    async fn set_default_flips_flag() {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query!(
            "INSERT INTO templates VALUES ('a','c','i','na','ne','da','de','[]',1),
                                          ('b','c','i','na','ne','da','de','[]',0)"
        )
        .execute(&pool)
        .await
        .unwrap();
        set_default(&pool, "b").await.unwrap();
        let templates = list_all(&pool).await.unwrap();
        assert!(!templates.iter().find(|t| t.id == "a").unwrap().is_default);
        assert!(templates.iter().find(|t| t.id == "b").unwrap().is_default);
    }
}
