use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening SQLite database {}", path.display()))?;
        Self::configure(&conn)?;
        Ok(Self { conn })
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("opening in-memory SQLite")?;
        Self::configure(&conn)?;
        Ok(Self { conn })
    }

    fn configure(conn: &Connection) -> Result<()> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let _ = conn.execute_batch("PRAGMA journal_mode = WAL;");
        let _ = conn.execute_batch("PRAGMA synchronous = NORMAL;");
        Self::migrate(conn)?;
        Ok(())
    }

    fn migrate(conn: &Connection) -> Result<()> {
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 1 {
            let sql = include_str!("../migrations/0001_init.sql");
            conn.execute_batch(sql).context("applying migration 0001")?;
            conn.execute_batch("PRAGMA user_version = 1;")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn opens_and_migrates() {
        let db = Db::open_memory().unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT count(*) FROM tag", [], |r| r.get(0))
            .unwrap();
        assert!(count >= 10);
        db.conn
            .execute(
                "INSERT INTO test_case (id,title,created_at,updated_at) VALUES (?1,?2,?3,?3)",
                params!["TC-1", "one", "now"],
            )
            .unwrap();
    }
}
