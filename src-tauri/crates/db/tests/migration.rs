use smart_noter_db::init_pool_in_memory;

#[tokio::test]
async fn migration_creates_expected_tables() {
    let pool = init_pool_in_memory().await.expect("pool");

    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlx_%' AND name NOT LIKE '_sqlx_%' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    )
    .fetch_all(&pool)
    .await
    .expect("query");

    let names: Vec<String> = tables.into_iter().map(|(n,)| n).collect();
    assert_eq!(
        names,
        vec![
            "actions",
            "blockers",
            "decisions",
            "meeting_assets",
            "meetings",
            "participants",
            "settings",
            "templates",
            "transcript_lines"
        ]
    );
}

#[tokio::test]
async fn foreign_keys_are_enabled() {
    let pool = init_pool_in_memory().await.expect("pool");
    let fk: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .expect("query");
    assert_eq!(fk.0, 1);
}
