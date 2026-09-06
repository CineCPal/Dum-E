use crate::types::{Citation, ChatMessage, IndexStats, RetrievedChunk, DEFAULT_CHAT_ID, EMBEDDING_DIMENSIONS};
use rusqlite::{ffi::sqlite3_auto_extension, params, Connection};
use std::path::Path;
use std::sync::Once;

static VEC_EXTENSION_INIT: Once = Once::new();

/// Registers the sqlite-vec extension process-wide. Must run before the
/// first `Connection::open` call in the process — sqlite-vec compiles as a
/// statically-linked "auto extension", not a loadable `.dylib`, so this is
/// the one-time hook that makes `vec0` virtual tables available.
fn register_vec_extension() {
    VEC_EXTENSION_INIT.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

pub fn open(db_path: &Path) -> anyhow::Result<Connection> {
    register_vec_extension();
    let conn = Connection::open(db_path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    // The ingestion script (a separate process) writes to this same file
    // when re-indexing; wait rather than erroring out on a momentary lock.
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS manuals (
            id INTEGER PRIMARY KEY,
            filename TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            product TEXT NOT NULL DEFAULT '',
            file_hash TEXT NOT NULL,
            page_count INTEGER NOT NULL,
            file_size_bytes INTEGER NOT NULL,
            ingested_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY,
            manual_id INTEGER NOT NULL REFERENCES manuals(id) ON DELETE CASCADE,
            page_start INTEGER NOT NULL,
            page_end INTEGER NOT NULL,
            chunk_index INTEGER NOT NULL,
            text TEXT NOT NULL,
            token_count INTEGER NOT NULL,
            content_hash TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS chats (
            id INTEGER PRIMARY KEY,
            created_at TEXT NOT NULL,
            title TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY,
            chat_id INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            used_fallback INTEGER NOT NULL DEFAULT 0,
            citations_json TEXT NOT NULL DEFAULT '[]'
        );",
    )?;

    // `product` was added after the initial release; add it for databases
    // created before this column existed (no-op once it's already there).
    match conn.execute("ALTER TABLE manuals ADD COLUMN product TEXT NOT NULL DEFAULT ''", []) {
        Ok(_) => {}
        Err(rusqlite::Error::SqliteFailure(_, Some(msg))) if msg.contains("duplicate column") => {}
        Err(e) => return Err(e.into()),
    }

    conn.execute_batch(&format!(
        "CREATE VIRTUAL TABLE IF NOT EXISTS vec_chunks USING vec0(
            embedding float[{EMBEDDING_DIMENSIONS}] distance_metric=cosine
        );"
    ))?;

    conn.execute(
        "INSERT OR IGNORE INTO chats (id, created_at, title) VALUES (?1, datetime('now'), 'Dum-E Chat')",
        params![DEFAULT_CHAT_ID],
    )?;

    Ok(())
}

pub fn index_stats(conn: &Connection) -> rusqlite::Result<IndexStats> {
    let manual_count: i64 = conn.query_row("SELECT COUNT(*) FROM manuals", [], |r| r.get(0))?;
    let chunk_count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
    let last_indexed_at: Option<String> = conn.query_row(
        "SELECT MAX(ingested_at) FROM manuals",
        [],
        |r| r.get(0),
    )?;
    Ok(IndexStats {
        manual_count,
        chunk_count,
        last_indexed_at,
    })
}

/// Cosine-distance KNN search over the manual chunks. Returns hits ordered
/// nearest-first, each with `similarity = 1 - cosine_distance` (sqlite-vec
/// reports raw cosine *distance*, so 0.0 = identical, higher = less alike).
pub fn top_k_chunks(
    conn: &Connection,
    query_embedding: &[f32],
    k: usize,
) -> rusqlite::Result<Vec<RetrievedChunk>> {
    let query_json = serde_json::to_string(query_embedding).unwrap_or_else(|_| "[]".to_string());

    let mut stmt = conn.prepare(
        "SELECT c.id, m.title, m.product, c.page_start, c.page_end, c.text, v.distance
         FROM vec_chunks v
         JOIN chunks c ON c.id = v.rowid
         JOIN manuals m ON m.id = c.manual_id
         WHERE v.embedding MATCH ?1 AND k = ?2
         ORDER BY v.distance",
    )?;

    let rows = stmt.query_map(params![query_json, k as i64], |row| {
        let distance: f64 = row.get(6)?;
        Ok(RetrievedChunk {
            chunk_id: row.get(0)?,
            manual_title: row.get(1)?,
            product: row.get(2)?,
            page_start: row.get(3)?,
            page_end: row.get(4)?,
            text: row.get(5)?,
            similarity: (1.0 - distance) as f32,
        })
    })?;

    rows.collect()
}

pub fn insert_message(
    conn: &Connection,
    chat_id: i64,
    role: &str,
    content: &str,
    used_fallback: bool,
    citations: &[Citation],
) -> rusqlite::Result<ChatMessage> {
    let citations_json = serde_json::to_string(citations).unwrap_or_else(|_| "[]".to_string());
    conn.execute(
        "INSERT INTO messages (chat_id, role, content, created_at, used_fallback, citations_json)
         VALUES (?1, ?2, ?3, datetime('now'), ?4, ?5)",
        params![chat_id, role, content, used_fallback, citations_json],
    )?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id, chat_id, role, content, used_fallback, citations_json, created_at
         FROM messages WHERE id = ?1",
        params![id],
        row_to_message,
    )
}

pub fn list_messages(conn: &Connection, chat_id: i64) -> rusqlite::Result<Vec<ChatMessage>> {
    let mut stmt = conn.prepare(
        "SELECT id, chat_id, role, content, used_fallback, citations_json, created_at
         FROM messages WHERE chat_id = ?1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![chat_id], row_to_message)?;
    rows.collect()
}

pub fn clear_messages(conn: &Connection, chat_id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM messages WHERE chat_id = ?1", params![chat_id])?;
    Ok(())
}

fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<ChatMessage> {
    let citations_json: String = row.get(5)?;
    let citations: Vec<Citation> = serde_json::from_str(&citations_json).unwrap_or_default();
    Ok(ChatMessage {
        id: row.get(0)?,
        chat_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        used_fallback: row.get(4)?,
        citations,
        created_at: row.get(6)?,
    })
}
