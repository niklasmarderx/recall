//! SQLite-backed memory store with full-text search (FTS5).

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// A single stored memory.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Memory {
    pub id: i64,
    pub content: String,
    pub tags: String,
    pub source: String,
    pub created_at: i64,
}

/// Handle to the on-disk (or in-memory) memory database.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (and, if needed, create) a database at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening database at {}", path.display()))?;
        let store = Store { conn };
        store.init()?;
        Ok(store)
    }

    /// Open an ephemeral in-memory database (used in tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Store { conn };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            CREATE TABLE IF NOT EXISTS memories (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                content    TEXT NOT NULL,
                tags       TEXT NOT NULL DEFAULT '',
                source     TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                content, tags, content='memories', content_rowid='id'
            );
            CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, content, tags)
                VALUES (new.id, new.content, new.tags);
            END;
            CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, tags)
                VALUES ('delete', old.id, old.content, old.tags);
            END;
            CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, content, tags)
                VALUES ('delete', old.id, old.content, old.tags);
                INSERT INTO memories_fts(rowid, content, tags)
                VALUES (new.id, new.content, new.tags);
            END;
            "#,
        )?;
        Ok(())
    }

    /// Store a new memory and return its id.
    pub fn add(&self, content: &str, tags: &str, source: &str) -> Result<i64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.conn.execute(
            "INSERT INTO memories (content, tags, source, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![content, tags, source, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Delete a memory by id. Returns `true` if a row was removed.
    pub fn delete(&self, id: i64) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// Return the most recent memories, newest first.
    pub fn list(&self, limit: usize) -> Result<Vec<Memory>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, tags, source, created_at \
             FROM memories ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], Self::row_to_memory)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Full-text search over memories, ranked by relevance (BM25).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>> {
        let fts = build_fts_query(query);
        if fts.is_empty() {
            return self.list(limit);
        }
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.content, m.tags, m.source, m.created_at \
             FROM memories_fts JOIN memories m ON m.id = memories_fts.rowid \
             WHERE memories_fts MATCH ?1 ORDER BY bm25(memories_fts) LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts, limit as i64], Self::row_to_memory)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Total number of stored memories.
    pub fn count(&self) -> Result<i64> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))?;
        Ok(n)
    }

    fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
        Ok(Memory {
            id: row.get(0)?,
            content: row.get(1)?,
            tags: row.get(2)?,
            source: row.get(3)?,
            created_at: row.get(4)?,
        })
    }
}

/// Turn a free-text query into a safe FTS5 prefix query (`foo* OR bar*`).
fn build_fts_query(query: &str) -> String {
    let tokens: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("{}*", t.to_lowercase()))
        .collect();
    tokens.join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_search_and_list() {
        let store = Store::open_in_memory().unwrap();
        store
            .add("Niklas prefers Rust for systems programming", "pref", "test")
            .unwrap();
        store
            .add("The deploy runs every Monday at 9am", "ops", "test")
            .unwrap();

        let hits = store.search("rust", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].content.contains("Rust"));

        assert_eq!(store.list(10).unwrap().len(), 2);
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn delete_works() {
        let store = Store::open_in_memory().unwrap();
        let id = store.add("temporary note", "", "").unwrap();
        assert!(store.delete(id).unwrap());
        assert!(!store.delete(id).unwrap());
    }

    #[test]
    fn empty_query_lists_recent() {
        let store = Store::open_in_memory().unwrap();
        store.add("one", "", "").unwrap();
        store.add("two", "", "").unwrap();
        assert_eq!(store.search("   ", 10).unwrap().len(), 2);
    }
}
