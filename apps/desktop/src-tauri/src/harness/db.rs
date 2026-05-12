use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &PathBuf) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS capabilities (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT DEFAULT '',
                version TEXT DEFAULT '0.1.0',
                source TEXT DEFAULT 'builtin',
                kind TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                config_json TEXT DEFAULT '{}'
            );
            CREATE TABLE IF NOT EXISTS permission_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                approved INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now'))
            );
        ")?;
        Ok(())
    }

    pub fn upsert_capability(&self, id: &str, name: &str, kind: &str, source: &str, enabled: bool) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO capabilities (id, name, kind, source, enabled) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name=?2, kind=?3, source=?4, enabled=?5",
            rusqlite::params![id, name, kind, source, enabled as i32],
        )?;
        Ok(())
    }

    pub fn update_capability_description(&self, id: &str, description: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE capabilities SET description = ?1 WHERE id = ?2",
            rusqlite::params![description, id],
        )?;
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("UPDATE capabilities SET enabled = ?1 WHERE id = ?2",
            rusqlite::params![enabled as i32, id])?;
        Ok(())
    }

    pub fn get_capability_enabled(&self, id: &str) -> SqlResult<Option<bool>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT enabled FROM capabilities WHERE id = ?1")?;
        let mut rows = stmt.query(rusqlite::params![id])?;
        if let Some(row) = rows.next()? {
            let enabled: i32 = row.get(0)?;
            Ok(Some(enabled != 0))
        } else {
            Ok(None)
        }
    }

    pub fn delete_capability(&self, id: &str) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM capabilities WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn list_all(&self) -> SqlResult<Vec<CapRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, version, source, kind, enabled, config_json FROM capabilities"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CapRow {
                id: row.get(0)?, name: row.get(1)?, description: row.get(2)?,
                version: row.get(3)?, source: row.get(4)?, kind: row.get(5)?,
                enabled: row.get(6)?, config_json: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn upsert_permission(&self, tool_name: &str, approved: bool) -> SqlResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO permission_rules (tool_name, approved) VALUES (?1, ?2)",
            rusqlite::params![tool_name, approved as i32],
        )?;
        Ok(())
    }

    pub fn is_permission_approved(&self, tool_name: &str) -> SqlResult<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM permission_rules WHERE tool_name = ?1 AND approved = 1",
            rusqlite::params![tool_name], |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

#[derive(Debug, Clone)]
pub struct CapRow {
    pub id: String, pub name: String, pub description: String,
    pub version: String, pub source: String, pub kind: String,
    pub enabled: bool, pub config_json: String,
}
