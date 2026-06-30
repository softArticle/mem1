//! SurrealDB connection and bootstrap (embedded RocksDB).
//!
//! SurrealDB runs inside the mem1-server process; data is stored in a local
//! directory (e.g. `./mem1.db`) using the RocksDB backend.

use crate::error::Error;
use surrealdb::engine::local::RocksDb;
use surrealdb::Surreal;

use super::memory::SurrealMemoryStore;

/// Local embedded connection type (RocksDb is the storage engine; Connection is Db).
pub type Db = Surreal<surrealdb::engine::local::Db>;

pub async fn connect(path: &str) -> Result<Db, Error> {
    let db = Surreal::new::<RocksDb>(path)
        .await
        .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb: {e}")))?;
    db.use_ns("mem1")
        .use_db("mem1")
        .await
        .map_err(|e| Error::Storage(anyhow::anyhow!("surrealdb use_ns/use_db: {e}")))?;
    Ok(db)
}

pub fn store(db: Db) -> SurrealMemoryStore {
    SurrealMemoryStore(db)
}

pub async fn ensure_schema(db: &Db) -> Result<(), Error> {
    // SurrealDB 2.x references the 'simple' analyzer on table create; define it if missing.
    db.query("DEFINE ANALYZER IF NOT EXISTS simple TOKENIZERS blank, class FILTERS lowercase;")
        .await
        .map_err(|e| Error::Storage(anyhow::anyhow!("define analyzer: {e}")))?;
    db.query("DEFINE TABLE IF NOT EXISTS memories SCHEMALESS;")
        .await
        .map_err(|e| Error::Storage(anyhow::anyhow!("define table: {e}")))?;
    db.query("DEFINE TABLE IF NOT EXISTS memory_history SCHEMALESS;")
        .await
        .map_err(|e| Error::Storage(anyhow::anyhow!("define history table: {e}")))?;
    db.query("DEFINE TABLE IF NOT EXISTS graph_entities SCHEMALESS;")
        .await
        .map_err(|e| Error::Storage(anyhow::anyhow!("define graph entities table: {e}")))?;
    db.query("DEFINE TABLE IF NOT EXISTS memory_entities SCHEMALESS;")
        .await
        .map_err(|e| Error::Storage(anyhow::anyhow!("define memory entities table: {e}")))?;
    // Full-text search on content for hybrid (keyword + vector) retrieval.
    db.query(
        "DEFINE INDEX IF NOT EXISTS memories_content_ft ON TABLE memories COLUMNS content SEARCH ANALYZER simple;",
    )
    .await
    .map_err(|e| Error::Storage(anyhow::anyhow!("define search index: {e}")))?;
    // Vector index for KNN. Dimension must match the active embedder:
    // all-MiniLM-L6-v2 = 384 (default), Qwen3-Embedding-0.6B = 1024, etc.
    // Configurable via MEM1_EMBED_DIM. Parsed as usize so the value is never
    // attacker-controlled string interpolation.
    let dim: usize = std::env::var("MEM1_EMBED_DIM")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(384);
    db.query(format!(
        "DEFINE INDEX IF NOT EXISTS memories_embedding_hnsw ON TABLE memories COLUMNS embedding HNSW DIMENSION {dim} DIST COSINE;"
    ))
    .await
    .map_err(|e| Error::Storage(anyhow::anyhow!("define vector index: {e}")))?;
    Ok(())
}
