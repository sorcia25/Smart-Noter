use crate::DbError;
use smart_noter_core::models::ai::Chunk;
use sqlx::SqlitePool;

/// Encode a `Vec<f32>` as little-endian bytes for BLOB storage.
fn encode_vector(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Decode little-endian bytes back to `Vec<f32>`.
/// Returns an empty vector if the byte slice length is not a multiple of 4.
fn decode_vector(bytes: &[u8]) -> Vec<f32> {
    if !bytes.len().is_multiple_of(4) {
        return Vec::new();
    }
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// Replace all stored embeddings for `meeting_id` with the new chunks.
///
/// Semantics: DELETE all existing rows for the meeting, then INSERT the new
/// chunks.  This ensures that re-running a summary never leaves stale chunks.
///
/// `chunks` is a slice of `(chunk_idx, text, vector)`.
pub async fn upsert(
    pool: &SqlitePool,
    meeting_id: &str,
    chunks: &[(i64, String, Vec<f32>)],
) -> Result<(), DbError> {
    // Delete existing embeddings for this meeting.
    sqlx::query("DELETE FROM transcript_embeddings WHERE meeting_id = ?")
        .bind(meeting_id)
        .execute(pool)
        .await?;

    // Insert the new chunks.
    for (idx, text, vector) in chunks {
        let blob = encode_vector(vector);
        sqlx::query(
            "INSERT INTO transcript_embeddings (meeting_id, chunk_idx, text, vector) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(meeting_id)
        .bind(idx)
        .bind(text)
        .bind(blob)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// Load all embeddings for a meeting, ordered by `chunk_idx`.
pub async fn load(pool: &SqlitePool, meeting_id: &str) -> Result<Vec<Chunk>, DbError> {
    let rows: Vec<(i64, String, Vec<u8>)> = sqlx::query_as(
        "SELECT chunk_idx, text, vector FROM transcript_embeddings \
         WHERE meeting_id = ? ORDER BY chunk_idx ASC",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(idx, text, blob)| Chunk {
            idx,
            text,
            vector: decode_vector(&blob),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    async fn setup() -> SqlitePool {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query(
            "INSERT INTO meetings (id, title_es, template_id, date, duration_sec) \
             VALUES ('m1', 'M1', 't', '2025-01-01', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[test]
    fn encode_decode_round_trip() {
        let original = vec![1.0f32, -0.5, std::f32::consts::PI, 0.0, f32::MIN_POSITIVE];
        let bytes = encode_vector(&original);
        let decoded = decode_vector(&bytes);
        assert_eq!(decoded.len(), original.len(), "length must match");
        for (a, b) in original.iter().zip(decoded.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "bit-exact round-trip required");
        }
    }

    #[test]
    fn decode_non_multiple_of_4_returns_empty() {
        let bad_bytes = vec![0u8, 1, 2]; // 3 bytes — not a multiple of 4
        let result = decode_vector(&bad_bytes);
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn upsert_and_load_round_trip() {
        let pool = setup().await;

        let v1 = vec![1.0f32, 2.0, 3.0];
        let v2 = vec![-1.0f32, 0.0, 0.5];
        let chunks = vec![
            (0i64, "chunk zero".to_string(), v1.clone()),
            (1i64, "chunk one".to_string(), v2.clone()),
        ];

        upsert(&pool, "m1", &chunks).await.unwrap();

        let loaded = load(&pool, "m1").await.unwrap();
        assert_eq!(loaded.len(), 2);

        assert_eq!(loaded[0].idx, 0);
        assert_eq!(loaded[0].text, "chunk zero");
        for (a, b) in v1.iter().zip(loaded[0].vector.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }

        assert_eq!(loaded[1].idx, 1);
        assert_eq!(loaded[1].text, "chunk one");
        for (a, b) in v2.iter().zip(loaded[1].vector.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[tokio::test]
    async fn upsert_replaces_existing_chunks() {
        let pool = setup().await;

        // First upsert: 3 chunks.
        let first = vec![
            (0i64, "old zero".to_string(), vec![0.0f32]),
            (1i64, "old one".to_string(), vec![1.0f32]),
            (2i64, "old two".to_string(), vec![2.0f32]),
        ];
        upsert(&pool, "m1", &first).await.unwrap();
        assert_eq!(load(&pool, "m1").await.unwrap().len(), 3);

        // Second upsert: 1 chunk — must replace, not accumulate.
        let second = vec![(0i64, "new zero".to_string(), vec![9.0f32])];
        upsert(&pool, "m1", &second).await.unwrap();

        let loaded = load(&pool, "m1").await.unwrap();
        assert_eq!(loaded.len(), 1, "old chunks must be gone after re-upsert");
        assert_eq!(loaded[0].text, "new zero");
    }
}
