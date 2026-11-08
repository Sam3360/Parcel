//! SQLite connection, migrations and seed data.
use crate::error::{Result, WsError};
use rusqlite::{params, Connection};
use std::path::Path;

/// Ordered, append-only migrations. Never edit an existing one; add a new entry.
const MIGRATIONS: &[&str] = &[
    // 1: core schema
    r#"
CREATE TABLE workspaces(
  id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '', metadata TEXT NOT NULL DEFAULT '{}',
  package_id TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 0,
  export_profile_id TEXT, template_id TEXT, last_export_path TEXT,
  created_at TEXT NOT NULL, modified_at TEXT NOT NULL, opened_at TEXT, deleted_at TEXT);
CREATE TABLE entries(
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  parent_id TEXT REFERENCES entries(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(kind IN ('file','folder','note','link')),
  name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', url TEXT, content TEXT,
  attached_to TEXT REFERENCES entries(id) ON DELETE SET NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  origin TEXT NOT NULL DEFAULT 'source',
  source_path TEXT, size INTEGER NOT NULL DEFAULT 0, sha256 TEXT, mime TEXT, source_mtime_ms INTEGER,
  tags TEXT NOT NULL DEFAULT '[]', metadata TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL, modified_at TEXT NOT NULL);
CREATE INDEX idx_entries_ws ON entries(workspace_id, parent_id, sort_order);
CREATE INDEX idx_entries_hash ON entries(workspace_id, sha256);
CREATE TABLE refs(
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  title TEXT NOT NULL, url TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
  category TEXT NOT NULL DEFAULT '', entry_id TEXT REFERENCES entries(id) ON DELETE SET NULL,
  sort_order INTEGER NOT NULL DEFAULT 0);
CREATE TABLE dependencies(
  id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  from_entry TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(kind IN ('entry','external')), target TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '', required INTEGER NOT NULL DEFAULT 1);
CREATE TABLE activity(
  id INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL, workspace_id TEXT,
  kind TEXT NOT NULL, message TEXT NOT NULL, detail TEXT NOT NULL DEFAULT '{}');
CREATE INDEX idx_activity_ts ON activity(ts DESC);
CREATE TABLE preferences(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE templates(
  id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
  builtin INTEGER NOT NULL DEFAULT 0, spec TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE TABLE export_profiles(
  id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
  builtin INTEGER NOT NULL DEFAULT 0, spec TEXT NOT NULL);
CREATE TABLE recent_packages(
  path TEXT PRIMARY KEY, name TEXT NOT NULL, opened_at TEXT NOT NULL, size INTEGER NOT NULL DEFAULT 0);
CREATE TABLE revisions(
  id INTEGER PRIMARY KEY AUTOINCREMENT, workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  revision INTEGER NOT NULL, package_id TEXT NOT NULL, path TEXT NOT NULL,
  exported_at TEXT NOT NULL, entry_count INTEGER NOT NULL, package_size INTEGER NOT NULL);
"#,
];

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let existed = path.exists();
    let mut conn = Connection::open(path)?;
    if existed {
        // Check integrity before changing any pragma: a non-database file must be reported, not touched.
        let res: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0)).map_err(|e| WsError::Corrupt(e.to_string()))?;
        if res != "ok" {
            return Err(WsError::Corrupt(res));
        }
        let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if (current as usize) < MIGRATIONS.len() && current > 0 {
            // Keep a copy before changing an existing database's schema.
            drop(conn);
            let bak = path.with_extension(format!("bak-v{current}"));
            std::fs::copy(path, &bak)?;
            conn = Connection::open(path)?;
        }
    }
    configure(&conn)?;
    migrate(&mut conn)?;
    seed(&conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    configure(&conn)?;
    migrate(&mut conn)?;
    seed(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current as usize > MIGRATIONS.len() {
        return Err(WsError::Invalid(format!("database schema v{current} is newer than this version of Parcel supports")));
    }
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn schema_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

fn seed(conn: &Connection) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    for t in crate::templates::builtin_templates() {
        conn.execute(
            "INSERT OR IGNORE INTO templates(id,name,description,builtin,spec,created_at) VALUES(?1,?2,?3,1,?4,?5)",
            params![t.id, t.name, t.description, serde_json::to_string(&t.spec)?, now],
        )?;
    }
    for p in crate::templates::builtin_profiles() {
        conn.execute(
            "INSERT OR IGNORE INTO export_profiles(id,name,description,builtin,spec) VALUES(?1,?2,?3,1,?4)",
            params![p.id, p.name, p.description, serde_json::to_string(&p.spec)?],
        )?;
    }
    Ok(())
}
