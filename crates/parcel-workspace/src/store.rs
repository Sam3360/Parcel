//! The workspace store: all persistence operations. Every mutation is transactional.
use crate::error::{Result, WsError};
use crate::models::*;
use crate::templates::*;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub struct Store {
    pub(crate) conn: Connection,
    /// Session-only undo history of removals (see `removal.rs`). Deliberately not persisted: no schema change.
    pub(crate) undo: std::sync::Mutex<std::collections::HashMap<String, Vec<crate::removal::UndoEntry>>>,
    pub(crate) undo_seq: std::sync::atomic::AtomicU64,
    /// Root of Parcel's own storage for imported files. File cleanup only ever acts inside it.
    pub(crate) managed_root: std::sync::Mutex<Option<std::path::PathBuf>>,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) const ENTRY_COLS: &str = "id,workspace_id,parent_id,kind,name,description,url,content,attached_to,sort_order,origin,source_path,size,sha256,mime,source_mtime_ms,tags,metadata,created_at,modified_at";

fn row_entry(r: &Row) -> rusqlite::Result<Entry> {
    let tags: String = r.get(16)?;
    let meta: String = r.get(17)?;
    Ok(Entry {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        parent_id: r.get(2)?,
        kind: r.get(3)?,
        name: r.get(4)?,
        description: r.get(5)?,
        url: r.get(6)?,
        content: r.get(7)?,
        attached_to: r.get(8)?,
        sort_order: r.get(9)?,
        origin: r.get(10)?,
        source_path: r.get(11)?,
        size: r.get(12)?,
        sha256: r.get(13)?,
        mime: r.get(14)?,
        source_mtime_ms: r.get(15)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        metadata: serde_json::from_str(&meta).unwrap_or_default(),
        created_at: r.get(18)?,
        modified_at: r.get(19)?,
    })
}

const WS_COLS: &str = "id,name,description,notes,metadata,package_id,revision,export_profile_id,template_id,last_export_path,created_at,modified_at,opened_at,deleted_at";

fn row_ws(r: &Row) -> rusqlite::Result<Workspace> {
    let meta: String = r.get(4)?;
    Ok(Workspace {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        notes: r.get(3)?,
        metadata: serde_json::from_str(&meta).unwrap_or_default(),
        package_id: r.get(5)?,
        revision: r.get(6)?,
        export_profile_id: r.get(7)?,
        template_id: r.get(8)?,
        last_export_path: r.get(9)?,
        created_at: r.get(10)?,
        modified_at: r.get(11)?,
        opened_at: r.get(12)?,
        deleted_at: r.get(13)?,
    })
}

pub fn validate_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(WsError::Invalid("name cannot be empty".into()));
    }
    if n.len() > 255 {
        return Err(WsError::Invalid("name is too long (max 255)".into()));
    }
    if n.contains(['/', '\\']) || n.chars().any(|c| c.is_control()) {
        return Err(WsError::Invalid("name cannot contain slashes or control characters".into()));
    }
    if n == "." || n == ".." {
        return Err(WsError::Invalid("invalid name".into()));
    }
    Ok(n.to_string())
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self::from_conn(crate::db::open(path)?))
    }
    pub fn open_in_memory() -> Result<Self> {
        Ok(Self::from_conn(crate::db::open_in_memory()?))
    }
    fn from_conn(conn: Connection) -> Self {
        Self { conn, undo: Default::default(), undo_seq: Default::default(), managed_root: Default::default() }
    }
    pub fn schema_version(&self) -> Result<i64> {
        crate::db::schema_version(&self.conn)
    }

    // ---------- activity / preferences ----------
    pub fn log(&self, ws: Option<&str>, kind: &str, message: &str, detail: serde_json::Value) {
        let _ = self.conn.execute(
            "INSERT INTO activity(ts,workspace_id,kind,message,detail) VALUES(?1,?2,?3,?4,?5)",
            params![now(), ws, kind, message, detail.to_string()],
        );
    }

    pub fn list_activity(&self, workspace: Option<&str>, limit: i64) -> Result<Vec<ActivityRow>> {
        let mut st = self.conn.prepare(
            "SELECT id,ts,workspace_id,kind,message,detail FROM activity WHERE (?1 IS NULL OR workspace_id=?1) ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = st.query_map(params![workspace, limit], |r| {
            let d: String = r.get(5)?;
            Ok(ActivityRow {
                id: r.get(0)?,
                ts: r.get(1)?,
                workspace_id: r.get(2)?,
                kind: r.get(3)?,
                message: r.get(4)?,
                detail: serde_json::from_str(&d).unwrap_or(serde_json::Value::Null),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn clear_activity(&self) -> Result<()> {
        self.conn.execute("DELETE FROM activity", [])?;
        Ok(())
    }

    pub fn get_pref(&self, key: &str) -> Result<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM preferences WHERE key=?1", [key], |r| r.get(0)).optional()?)
    }
    pub fn set_pref(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO preferences(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
    pub fn all_prefs(&self) -> Result<BTreeMap<String, String>> {
        let mut st = self.conn.prepare("SELECT key,value FROM preferences")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ---------- workspaces ----------
    pub fn create_workspace(&mut self, name: &str, description: &str, template_id: Option<&str>) -> Result<Workspace> {
        let name = name.trim();
        if name.is_empty() || name.len() > 200 {
            return Err(WsError::Invalid("workspace name must be 1-200 characters".into()));
        }
        let tpl = match template_id {
            Some(t) => Some(self.get_template(t)?),
            None => None,
        };
        let id = new_id();
        let ts = now();
        let mut meta = BTreeMap::new();
        if let Some(t) = &tpl {
            for f in &t.spec.metadata_fields {
                meta.insert(f.key.clone(), f.default.clone());
            }
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO workspaces(id,name,description,metadata,package_id,revision,template_id,export_profile_id,created_at,modified_at) VALUES(?1,?2,?3,?4,?5,0,?6,?7,?8,?8)",
            params![id, name, description, serde_json::to_string(&meta)?, new_id(), template_id, tpl.as_ref().and_then(|t| t.spec.default_profile.clone()), ts],
        )?;
        tx.commit()?;
        if let Some(t) = tpl {
            for f in &t.spec.folders {
                self.ensure_folder_path(&id, f)?;
            }
            for n in &t.spec.notes {
                let (dir, file) = match n.path.rsplit_once('/') {
                    Some((d, f)) => (Some(self.ensure_folder_path(&id, d)?), f.to_string()),
                    None => (None, n.path.clone()),
                };
                self.create_note(&id, dir.as_deref(), &file, &n.content, None)?;
            }
        }
        self.log(Some(&id), "workspace.created", &format!("Created workspace “{name}”"), serde_json::json!({"template": template_id}));
        self.get_workspace(&id)
    }

    pub fn get_workspace(&self, id: &str) -> Result<Workspace> {
        self.conn
            .query_row(&format!("SELECT {WS_COLS} FROM workspaces WHERE id=?1"), [id], row_ws)
            .optional()?
            .ok_or_else(|| WsError::NotFound(format!("workspace {id}")))
    }

    pub fn list_workspaces(&self, trashed: bool) -> Result<Vec<WorkspaceSummary>> {
        let sql = format!(
            "SELECT w.id,w.name,w.description,
               (SELECT COUNT(*) FROM entries e WHERE e.workspace_id=w.id),
               (SELECT COALESCE(SUM(size),0) FROM entries e WHERE e.workspace_id=w.id),
               w.revision,w.modified_at,w.opened_at,w.deleted_at
             FROM workspaces w WHERE w.deleted_at IS {} NULL ORDER BY w.modified_at DESC",
            if trashed { "NOT" } else { "" }
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map([], |r| {
            Ok(WorkspaceSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                entry_count: r.get(3)?,
                total_size: r.get(4)?,
                revision: r.get(5)?,
                modified_at: r.get(6)?,
                opened_at: r.get(7)?,
                deleted_at: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn recent_workspaces(&self, limit: i64) -> Result<Vec<WorkspaceSummary>> {
        let mut v: Vec<_> = self.list_workspaces(false)?.into_iter().filter(|w| w.opened_at.is_some()).collect();
        v.sort_by(|a, b| b.opened_at.cmp(&a.opened_at));
        v.truncate(limit as usize);
        Ok(v)
    }

    pub fn touch_opened(&self, id: &str) -> Result<()> {
        self.conn.execute("UPDATE workspaces SET opened_at=?2 WHERE id=?1", params![id, now()])?;
        Ok(())
    }

    fn touch(&self, ws: &str) -> Result<()> {
        self.conn.execute("UPDATE workspaces SET modified_at=?2 WHERE id=?1", params![ws, now()])?;
        Ok(())
    }

    pub fn update_workspace(&mut self, id: &str, u: WorkspaceUpdate) -> Result<Workspace> {
        let w = self.get_workspace(id)?;
        let name = match &u.name {
            Some(n) => {
                let n = n.trim();
                if n.is_empty() || n.len() > 200 {
                    return Err(WsError::Invalid("workspace name must be 1-200 characters".into()));
                }
                n.to_string()
            }
            None => w.name.clone(),
        };
        let profile = match u.export_profile_id {
            Some(p) => p,
            None => w.export_profile_id.clone(),
        };
        self.conn.execute(
            "UPDATE workspaces SET name=?2,description=?3,notes=?4,metadata=?5,export_profile_id=?6,modified_at=?7 WHERE id=?1",
            params![
                id,
                name,
                u.description.unwrap_or(w.description),
                u.notes.unwrap_or(w.notes),
                serde_json::to_string(&u.metadata.unwrap_or(w.metadata))?,
                profile,
                now()
            ],
        )?;
        if name != w.name {
            self.log(Some(id), "workspace.renamed", &format!("Renamed “{}” to “{name}”", w.name), serde_json::Value::Null);
        }
        self.get_workspace(id)
    }

    pub fn duplicate_workspace(&mut self, id: &str, new_name: &str) -> Result<Workspace> {
        let src = self.get_workspace(id)?;
        let dst = self.create_workspace(new_name, &src.description, None)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE workspaces SET notes=?2,metadata=?3,export_profile_id=?4,template_id=?5 WHERE id=?1",
            params![dst.id, src.notes, serde_json::to_string(&src.metadata)?, src.export_profile_id, src.template_id],
        )?;
        let mut idmap: HashMap<String, String> = HashMap::new();
        let ids: Vec<String> = {
            let mut st = tx.prepare("SELECT id FROM entries WHERE workspace_id=?1")?;
            let v = st.query_map([id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
            v
        };
        for i in &ids {
            idmap.insert(i.clone(), new_id());
        }
        let entries: Vec<Entry> = {
            let mut st = tx.prepare(&format!("SELECT {ENTRY_COLS} FROM entries WHERE workspace_id=?1"))?;
            let v = st.query_map([id], row_entry)?.collect::<std::result::Result<_, _>>()?;
            v
        };
        // Insert parents before children is unnecessary when FK checks are deferred.
        tx.execute_batch("PRAGMA defer_foreign_keys = ON;")?;
        for e in entries {
            tx.execute(
                &format!(
                    "INSERT INTO entries({ENTRY_COLS}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)"
                ),
                params![
                    idmap[&e.id],
                    dst.id,
                    e.parent_id.as_ref().map(|p| idmap[p].clone()),
                    e.kind,
                    e.name,
                    e.description,
                    e.url,
                    e.content,
                    e.attached_to.as_ref().and_then(|p| idmap.get(p).cloned()),
                    e.sort_order,
                    e.origin,
                    e.source_path,
                    e.size,
                    e.sha256,
                    e.mime,
                    e.source_mtime_ms,
                    serde_json::to_string(&e.tags)?,
                    serde_json::to_string(&e.metadata)?,
                    e.created_at,
                    e.modified_at
                ],
            )?;
        }
        let refs = {
            let mut st = tx.prepare("SELECT title,url,description,category,entry_id,sort_order FROM refs WHERE workspace_id=?1")?;
            let v: Vec<(String, String, String, String, Option<String>, i64)> = st
                .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
                .collect::<std::result::Result<_, _>>()?;
            v
        };
        for (t, u, d, c, e, s) in refs {
            tx.execute(
                "INSERT INTO refs(id,workspace_id,title,url,description,category,entry_id,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![new_id(), dst.id, t, u, d, c, e.and_then(|x| idmap.get(&x).cloned()), s],
            )?;
        }
        let deps = {
            let mut st = tx.prepare("SELECT from_entry,kind,target,description,required FROM dependencies WHERE workspace_id=?1")?;
            let v: Vec<(String, String, String, String, bool)> = st
                .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
                .collect::<std::result::Result<_, _>>()?;
            v
        };
        for (f, k, t, d, req) in deps {
            let target = if k == "entry" { idmap.get(&t).cloned().unwrap_or(t) } else { t };
            tx.execute(
                "INSERT INTO dependencies(id,workspace_id,from_entry,kind,target,description,required) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![new_id(), dst.id, idmap[&f], k, target, d, req],
            )?;
        }
        tx.commit()?;
        self.log(Some(&dst.id), "workspace.duplicated", &format!("Duplicated “{}” as “{}”", src.name, dst.name), serde_json::Value::Null);
        self.get_workspace(&dst.id)
    }

    /// Moves a workspace to the trash. Nothing on disk is touched; the workspace can be restored.
    pub fn trash_workspace(&self, id: &str) -> Result<()> {
        let w = self.get_workspace(id)?;
        self.conn.execute("UPDATE workspaces SET deleted_at=?2 WHERE id=?1", params![id, now()])?;
        self.log(Some(id), "workspace.trashed", &format!("Moved “{}” to trash", w.name), serde_json::Value::Null);
        Ok(())
    }
    pub fn restore_workspace(&self, id: &str) -> Result<()> {
        self.conn.execute("UPDATE workspaces SET deleted_at=NULL WHERE id=?1", [id])?;
        self.log(Some(id), "workspace.restored", "Restored workspace from trash", serde_json::Value::Null);
        Ok(())
    }
    /// Permanently removes a *trashed* workspace record. Source files are never deleted.
    pub fn purge_workspace(&self, id: &str) -> Result<()> {
        let w = self.get_workspace(id)?;
        if w.deleted_at.is_none() {
            return Err(WsError::Invalid("only workspaces in the trash can be permanently deleted".into()));
        }
        self.conn.execute("DELETE FROM workspaces WHERE id=?1", [id])?;
        self.undo.lock().unwrap().remove(id); // its pending undo history goes with it
        self.log(None, "workspace.purged", &format!("Permanently deleted “{}”", w.name), serde_json::Value::Null);
        Ok(())
    }

    // ---------- entries ----------
    pub fn get_entry(&self, id: &str) -> Result<Entry> {
        self.conn
            .query_row(&format!("SELECT {ENTRY_COLS} FROM entries WHERE id=?1"), [id], row_entry)
            .optional()?
            .ok_or_else(|| WsError::NotFound(format!("entry {id}")))
    }

    pub fn list_entries(&self, ws: &str) -> Result<Vec<Entry>> {
        let mut st = self
            .conn
            .prepare(&format!("SELECT {ENTRY_COLS} FROM entries WHERE workspace_id=?1 ORDER BY sort_order, name COLLATE NOCASE"))?;
        let v = st.query_map([ws], row_entry)?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    pub fn list_children(&self, ws: &str, parent: Option<&str>) -> Result<Vec<Entry>> {
        let mut st = self.conn.prepare(&format!(
            "SELECT {ENTRY_COLS} FROM entries WHERE workspace_id=?1 AND parent_id IS ?2 ORDER BY (kind<>'folder'), sort_order, name COLLATE NOCASE"
        ))?;
        let v = st.query_map(params![ws, parent], row_entry)?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    pub fn entry_paths(&self, ws: &str) -> Result<HashMap<String, String>> {
        let all = self.list_entries(ws)?;
        let by_id: HashMap<&str, &Entry> = all.iter().map(|e| (e.id.as_str(), e)).collect();
        let mut out = HashMap::new();
        for e in &all {
            let mut parts = vec![e.name.clone()];
            let mut cur = e.parent_id.as_deref();
            let mut guard = 0;
            while let Some(p) = cur {
                let pe = by_id.get(p).ok_or_else(|| WsError::Corrupt(format!("dangling parent {p}")))?;
                parts.push(pe.name.clone());
                cur = pe.parent_id.as_deref();
                guard += 1;
                if guard > 256 {
                    return Err(WsError::Corrupt("folder cycle".into()));
                }
            }
            parts.reverse();
            out.insert(e.id.clone(), parts.join("/"));
        }
        Ok(out)
    }

    fn sibling_names(&self, ws: &str, parent: Option<&str>) -> Result<Vec<(String, String)>> {
        let mut st = self.conn.prepare("SELECT id,name FROM entries WHERE workspace_id=?1 AND parent_id IS ?2")?;
        let v = st.query_map(params![ws, parent], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    pub fn unique_name(&self, ws: &str, parent: Option<&str>, desired: &str, exclude: Option<&str>) -> Result<String> {
        let taken: Vec<String> = self
            .sibling_names(ws, parent)?
            .into_iter()
            .filter(|(i, _)| Some(i.as_str()) != exclude)
            .map(|(_, n)| n.to_lowercase())
            .collect();
        if !taken.contains(&desired.to_lowercase()) {
            return Ok(desired.to_string());
        }
        let (stem, ext) = match desired.rfind('.') {
            Some(i) if i > 0 => (&desired[..i], &desired[i..]),
            _ => (desired, ""),
        };
        for n in 1..100_000 {
            let cand = format!("{stem} ({n}){ext}");
            if !taken.contains(&cand.to_lowercase()) {
                return Ok(cand);
            }
        }
        Err(WsError::Invalid("cannot find a free name".into()))
    }

    fn check_parent(&self, ws: &str, parent: Option<&str>) -> Result<()> {
        if let Some(p) = parent {
            let e = self.get_entry(p)?;
            if e.workspace_id != ws || e.kind != "folder" {
                return Err(WsError::Invalid("parent must be a folder in the same workspace".into()));
            }
        }
        Ok(())
    }

    fn next_order(&self, ws: &str, parent: Option<&str>) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(sort_order),0)+1 FROM entries WHERE workspace_id=?1 AND parent_id IS ?2",
            params![ws, parent],
            |r| r.get(0),
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_entry(&self, ws: &str, parent: Option<&str>, kind: &str, name: &str, f: impl FnOnce(&mut Entry)) -> Result<Entry> {
        self.check_parent(ws, parent)?;
        let ts = now();
        let mut e = Entry {
            id: new_id(),
            workspace_id: ws.into(),
            parent_id: parent.map(String::from),
            kind: kind.into(),
            name: name.into(),
            description: String::new(),
            url: None,
            content: None,
            attached_to: None,
            sort_order: self.next_order(ws, parent)?,
            origin: "source".into(),
            source_path: None,
            size: 0,
            sha256: None,
            mime: None,
            source_mtime_ms: None,
            tags: vec![],
            metadata: BTreeMap::new(),
            created_at: ts.clone(),
            modified_at: ts,
        };
        f(&mut e);
        self.conn.execute(
            &format!("INSERT INTO entries({ENTRY_COLS}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)"),
            params![
                e.id,
                e.workspace_id,
                e.parent_id,
                e.kind,
                e.name,
                e.description,
                e.url,
                e.content,
                e.attached_to,
                e.sort_order,
                e.origin,
                e.source_path,
                e.size,
                e.sha256,
                e.mime,
                e.source_mtime_ms,
                serde_json::to_string(&e.tags)?,
                serde_json::to_string(&e.metadata)?,
                e.created_at,
                e.modified_at
            ],
        )?;
        self.touch(ws)?;
        Ok(e)
    }

    pub fn create_folder(&self, ws: &str, parent: Option<&str>, name: &str) -> Result<Entry> {
        let name = validate_name(name)?;
        if self.sibling_names(ws, parent)?.iter().any(|(_, n)| n.eq_ignore_ascii_case(&name)) {
            return Err(WsError::NameConflict(name));
        }
        let e = self.insert_entry(ws, parent, "folder", &name, |_| {})?;
        self.log(Some(ws), "entry.folder_created", &format!("Created folder “{name}”"), serde_json::Value::Null);
        Ok(e)
    }

    /// Create (or find) a nested folder path like "Sources/Papers". Returns the leaf id.
    pub fn ensure_folder_path(&self, ws: &str, path: &str) -> Result<String> {
        let mut parent: Option<String> = None;
        for part in path.split('/').filter(|p| !p.is_empty()) {
            let part = validate_name(part)?;
            let existing = self.sibling_names(ws, parent.as_deref())?.into_iter().find(|(_, n)| n.eq_ignore_ascii_case(&part));
            let id = match existing {
                Some((id, _)) => {
                    if self.get_entry(&id)?.kind != "folder" {
                        return Err(WsError::NameConflict(part));
                    }
                    id
                }
                None => self.insert_entry(ws, parent.as_deref(), "folder", &part, |_| {})?.id,
            };
            parent = Some(id);
        }
        parent.ok_or_else(|| WsError::Invalid("empty folder path".into()))
    }

    pub fn create_note(&self, ws: &str, parent: Option<&str>, name: &str, content: &str, attached_to: Option<&str>) -> Result<Entry> {
        let mut name = validate_name(name)?;
        if !name.to_lowercase().ends_with(".md") {
            name.push_str(".md");
        }
        let name = self.unique_name(ws, parent, &name, None)?;
        let content = content.to_string();
        let att = attached_to.map(String::from);
        let e = self.insert_entry(ws, parent, "note", &name, |e| {
            e.size = content.len() as i64;
            e.sha256 = Some(parcel_core::hash::sha256_bytes(content.as_bytes()));
            e.mime = Some("text/markdown".into());
            e.content = Some(content);
            e.attached_to = att;
        })?;
        self.log(Some(ws), "entry.note_created", &format!("Created note “{name}”"), serde_json::Value::Null);
        Ok(e)
    }

    pub fn create_link(&self, ws: &str, parent: Option<&str>, name: &str, url: &str, description: &str) -> Result<Entry> {
        if !parcel_core::manifest::is_allowed_url(url) {
            return Err(WsError::Invalid("only http, https, ftp and mailto links are supported".into()));
        }
        let name = self.unique_name(ws, parent, &validate_name(name)?, None)?;
        let (u, d) = (url.trim().to_string(), description.to_string());
        let e = self.insert_entry(ws, parent, "link", &name, |e| {
            e.url = Some(u);
            e.description = d;
        })?;
        self.log(Some(ws), "entry.link_created", &format!("Added link “{name}”"), serde_json::Value::Null);
        Ok(e)
    }

    pub fn rename_entry(&self, id: &str, new_name: &str) -> Result<Entry> {
        let e = self.get_entry(id)?;
        let name = validate_name(new_name)?;
        if self.sibling_names(&e.workspace_id, e.parent_id.as_deref())?.iter().any(|(i, n)| i != id && n.eq_ignore_ascii_case(&name)) {
            return Err(WsError::NameConflict(name));
        }
        self.conn.execute("UPDATE entries SET name=?2,modified_at=?3 WHERE id=?1", params![id, name, now()])?;
        self.touch(&e.workspace_id)?;
        self.log(Some(&e.workspace_id), "entry.renamed", &format!("Renamed “{}” to “{name}”", e.name), serde_json::Value::Null);
        self.get_entry(id)
    }

    pub fn update_entry(&self, id: &str, u: EntryUpdate) -> Result<Entry> {
        let e = self.get_entry(id)?;
        if let Some(url) = &u.url {
            if e.kind != "link" || !parcel_core::manifest::is_allowed_url(url) {
                return Err(WsError::Invalid("invalid link URL".into()));
            }
        }
        let tags: Vec<String> =
            u.tags.unwrap_or(e.tags.clone()).into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        let mut meta = u.metadata.unwrap_or(e.metadata.clone());
        meta.retain(|k, _| !k.trim().is_empty());
        let (content, size, sha) = if e.kind == "note" {
            let c = u.content.unwrap_or_else(|| e.content.clone().unwrap_or_default());
            let sha = parcel_core::hash::sha256_bytes(c.as_bytes());
            let size = c.len() as i64;
            (Some(c), size, Some(sha))
        } else {
            (e.content.clone(), e.size, e.sha256.clone())
        };
        let attached = match u.attached_to {
            Some(a) => {
                if let Some(a) = &a {
                    if self.get_entry(a)?.workspace_id != e.workspace_id {
                        return Err(WsError::Invalid("cannot attach to an entry in another workspace".into()));
                    }
                }
                a
            }
            None => e.attached_to.clone(),
        };
        self.conn.execute(
            "UPDATE entries SET description=?2,tags=?3,metadata=?4,content=?5,size=?6,sha256=?7,url=?8,attached_to=?9,modified_at=?10 WHERE id=?1",
            params![
                id,
                u.description.unwrap_or(e.description),
                serde_json::to_string(&tags)?,
                serde_json::to_string(&meta)?,
                content,
                size,
                sha,
                u.url.or(e.url),
                attached,
                now()
            ],
        )?;
        self.touch(&e.workspace_id)?;
        self.get_entry(id)
    }

    /// Move entries into `new_parent` (None = workspace root). Rejects cycles and name collisions.
    pub fn move_entries(&mut self, ids: &[String], new_parent: Option<&str>) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let first = self.get_entry(&ids[0])?;
        let ws = first.workspace_id.clone();
        self.check_parent(&ws, new_parent)?;
        let all = self.list_entries(&ws)?;
        let by_id: HashMap<&str, &Entry> = all.iter().map(|e| (e.id.as_str(), e)).collect();
        for id in ids {
            let e = by_id.get(id.as_str()).ok_or_else(|| WsError::NotFound(id.clone()))?;
            let mut cur = new_parent;
            while let Some(c) = cur {
                if c == id {
                    return Err(WsError::Invalid(format!("cannot move folder “{}” into itself", e.name)));
                }
                cur = by_id.get(c).and_then(|p| p.parent_id.as_deref());
            }
        }
        let mut names: Vec<String> =
            self.sibling_names(&ws, new_parent)?.into_iter().filter(|(i, _)| !ids.contains(i)).map(|(_, n)| n.to_lowercase()).collect();
        for id in ids {
            let e = by_id[id.as_str()];
            if names.contains(&e.name.to_lowercase()) {
                return Err(WsError::NameConflict(e.name.clone()));
            }
            names.push(e.name.to_lowercase());
        }
        let first_order = self.next_order(&ws, new_parent)?;
        let tx = self.conn.transaction()?;
        for (id, order) in ids.iter().zip(first_order..) {
            tx.execute("UPDATE entries SET parent_id=?2,sort_order=?3,modified_at=?4 WHERE id=?1", params![id, new_parent, order, now()])?;
        }
        tx.commit()?;
        self.touch(&ws)?;
        self.log(Some(&ws), "entry.moved", &format!("Moved {} item(s)", ids.len()), serde_json::Value::Null);
        Ok(())
    }

    pub fn reorder(&mut self, ws: &str, parent: Option<&str>, ordered_ids: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for (i, id) in ordered_ids.iter().enumerate() {
            tx.execute(
                "UPDATE entries SET sort_order=?3 WHERE id=?1 AND workspace_id=?2 AND parent_id IS ?4",
                params![id, ws, i as i64, parent],
            )?;
        }
        tx.commit()?;
        self.touch(ws)
    }

    /// Remove entries (and descendants) from the workspace. Original files are never touched.
    pub fn delete_entries(&mut self, ids: &[String]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }
        let ws = self.get_entry(&ids[0])?.workspace_id;
        let tx = self.conn.transaction()?;
        let mut n = 0;
        for id in ids {
            n += tx.execute("DELETE FROM entries WHERE id=?1 AND workspace_id=?2", params![id, ws])?;
        }
        tx.commit()?;
        self.touch(&ws)?;
        self.log(
            Some(&ws),
            "entry.removed",
            &format!("Removed {} item(s) from the workspace (originals untouched)", ids.len()),
            serde_json::Value::Null,
        );
        Ok(n)
    }

    pub fn batch_tag(&mut self, ids: &[String], add: &[String], remove: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for id in ids {
            let tags: String = tx.query_row("SELECT tags FROM entries WHERE id=?1", [id], |r| r.get(0))?;
            let mut tags: Vec<String> = serde_json::from_str(&tags).unwrap_or_default();
            tags.retain(|t| !remove.iter().any(|r| r.eq_ignore_ascii_case(t)));
            for a in add {
                let a = a.trim();
                if !a.is_empty() && !tags.iter().any(|t| t.eq_ignore_ascii_case(a)) {
                    tags.push(a.to_string());
                }
            }
            tx.execute("UPDATE entries SET tags=?2,modified_at=?3 WHERE id=?1", params![id, serde_json::to_string(&tags)?, now()])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn batch_set_metadata(&mut self, ids: &[String], key: &str, value: Option<&str>) -> Result<()> {
        if key.trim().is_empty() {
            return Err(WsError::Invalid("metadata key cannot be empty".into()));
        }
        let tx = self.conn.transaction()?;
        for id in ids {
            let m: String = tx.query_row("SELECT metadata FROM entries WHERE id=?1", [id], |r| r.get(0))?;
            let mut m: BTreeMap<String, String> = serde_json::from_str(&m).unwrap_or_default();
            match value {
                Some(v) => m.insert(key.trim().to_string(), v.to_string()),
                None => m.remove(key.trim()),
            };
            tx.execute("UPDATE entries SET metadata=?2,modified_at=?3 WHERE id=?1", params![id, serde_json::to_string(&m)?, now()])?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---------- references & dependencies ----------
    pub fn add_reference(&self, ws: &str, r: RefInput) -> Result<RefRow> {
        if !parcel_core::manifest::is_allowed_url(&r.url) {
            return Err(WsError::Invalid("only http, https, ftp and mailto URLs are supported".into()));
        }
        if r.title.trim().is_empty() {
            return Err(WsError::Invalid("a reference needs a title".into()));
        }
        let id = new_id();
        let order: i64 = self.conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM refs WHERE workspace_id=?1", [ws], |r| r.get(0))?;
        self.conn.execute(
            "INSERT INTO refs(id,workspace_id,title,url,description,category,entry_id,sort_order) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![id, ws, r.title.trim(), r.url.trim(), r.description, r.category.trim(), r.entry_id, order],
        )?;
        self.touch(ws)?;
        self.log(Some(ws), "reference.added", &format!("Added reference “{}”", r.title.trim()), serde_json::Value::Null);
        self.get_reference(&id)
    }

    pub fn get_reference(&self, id: &str) -> Result<RefRow> {
        self.conn
            .query_row("SELECT id,workspace_id,title,url,description,category,entry_id,sort_order FROM refs WHERE id=?1", [id], |r| {
                Ok(RefRow {
                    id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    title: r.get(2)?,
                    url: r.get(3)?,
                    description: r.get(4)?,
                    category: r.get(5)?,
                    entry_id: r.get(6)?,
                    sort_order: r.get(7)?,
                })
            })
            .optional()?
            .ok_or_else(|| WsError::NotFound(format!("reference {id}")))
    }

    pub fn list_references(&self, ws: &str) -> Result<Vec<RefRow>> {
        let mut st = self.conn.prepare("SELECT id,workspace_id,title,url,description,category,entry_id,sort_order FROM refs WHERE workspace_id=?1 ORDER BY category COLLATE NOCASE, sort_order")?;
        let v = st
            .query_map([ws], |r| {
                Ok(RefRow {
                    id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    title: r.get(2)?,
                    url: r.get(3)?,
                    description: r.get(4)?,
                    category: r.get(5)?,
                    entry_id: r.get(6)?,
                    sort_order: r.get(7)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    pub fn update_reference(&self, id: &str, r: RefInput) -> Result<RefRow> {
        let old = self.get_reference(id)?;
        if !parcel_core::manifest::is_allowed_url(&r.url) || r.title.trim().is_empty() {
            return Err(WsError::Invalid("a reference needs a title and a valid http/https/ftp/mailto URL".into()));
        }
        self.conn.execute(
            "UPDATE refs SET title=?2,url=?3,description=?4,category=?5,entry_id=?6 WHERE id=?1",
            params![id, r.title.trim(), r.url.trim(), r.description, r.category.trim(), r.entry_id],
        )?;
        self.touch(&old.workspace_id)?;
        self.get_reference(id)
    }

    pub fn delete_reference(&self, id: &str) -> Result<()> {
        let r = self.get_reference(id)?;
        self.conn.execute("DELETE FROM refs WHERE id=?1", [id])?;
        self.touch(&r.workspace_id)
    }

    pub fn add_dependency(
        &self,
        ws: &str,
        from_entry: &str,
        kind: &str,
        target: &str,
        description: &str,
        required: bool,
    ) -> Result<DepRow> {
        if kind != "entry" && kind != "external" {
            return Err(WsError::Invalid("dependency kind must be entry or external".into()));
        }
        if self.get_entry(from_entry)?.workspace_id != ws {
            return Err(WsError::Invalid("entry belongs to another workspace".into()));
        }
        if kind == "entry" && self.get_entry(target)?.workspace_id != ws {
            return Err(WsError::Invalid("target belongs to another workspace".into()));
        }
        if target.trim().is_empty() {
            return Err(WsError::Invalid("dependency target cannot be empty".into()));
        }
        let id = new_id();
        self.conn.execute(
            "INSERT INTO dependencies(id,workspace_id,from_entry,kind,target,description,required) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![id, ws, from_entry, kind, target.trim(), description, required],
        )?;
        self.touch(ws)?;
        Ok(DepRow {
            id,
            workspace_id: ws.into(),
            from_entry: from_entry.into(),
            kind: kind.into(),
            target: target.trim().into(),
            description: description.into(),
            required,
        })
    }

    pub fn list_dependencies(&self, ws: &str) -> Result<Vec<DepRow>> {
        let mut st = self
            .conn
            .prepare("SELECT id,workspace_id,from_entry,kind,target,description,required FROM dependencies WHERE workspace_id=?1")?;
        let v = st
            .query_map([ws], |r| {
                Ok(DepRow {
                    id: r.get(0)?,
                    workspace_id: r.get(1)?,
                    from_entry: r.get(2)?,
                    kind: r.get(3)?,
                    target: r.get(4)?,
                    description: r.get(5)?,
                    required: r.get(6)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    pub fn delete_dependency(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM dependencies WHERE id=?1", [id])?;
        Ok(())
    }

    // ---------- templates & profiles ----------
    pub fn list_templates(&self) -> Result<Vec<Template>> {
        let mut st =
            self.conn.prepare("SELECT id,name,description,builtin,spec FROM templates ORDER BY builtin DESC, name COLLATE NOCASE")?;
        let rows = st.query_map([], |r| {
            let spec: String = r.get(4)?;
            Ok(Template {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                builtin: r.get(3)?,
                spec: serde_json::from_str(&spec).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_template(&self, id: &str) -> Result<Template> {
        self.list_templates()?.into_iter().find(|t| t.id == id).ok_or_else(|| WsError::NotFound(format!("template {id}")))
    }

    pub fn save_template(&self, id: Option<&str>, name: &str, description: &str, spec: &TemplateSpec) -> Result<Template> {
        if name.trim().is_empty() {
            return Err(WsError::Invalid("template name cannot be empty".into()));
        }
        for f in &spec.folders {
            for p in f.split('/') {
                validate_name(p)?;
            }
        }
        let id = id.map(String::from).unwrap_or_else(|| format!("tpl-{}", new_id()));
        if let Ok(existing) = self.get_template(&id) {
            if existing.builtin {
                return Err(WsError::Invalid("built-in templates cannot be modified; save a copy instead".into()));
            }
        }
        self.conn.execute(
            "INSERT INTO templates(id,name,description,builtin,spec,created_at) VALUES(?1,?2,?3,0,?4,?5)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,spec=excluded.spec",
            params![id, name.trim(), description, serde_json::to_string(spec)?, now()],
        )?;
        self.log(None, "template.saved", &format!("Saved template “{}”", name.trim()), serde_json::Value::Null);
        self.get_template(&id)
    }

    /// Capture the folder structure, notes and metadata keys of a workspace as a reusable template.
    pub fn template_from_workspace(&self, ws: &str, name: &str, description: &str, include_notes: bool) -> Result<Template> {
        let w = self.get_workspace(ws)?;
        let paths = self.entry_paths(ws)?;
        let entries = self.list_entries(ws)?;
        let mut spec = TemplateSpec::default();
        for e in &entries {
            match e.kind.as_str() {
                "folder" => spec.folders.push(paths[&e.id].clone()),
                "note" if include_notes => {
                    spec.notes.push(TemplateNote { path: paths[&e.id].clone(), content: e.content.clone().unwrap_or_default() })
                }
                _ => {}
            }
        }
        spec.folders.sort();
        spec.metadata_fields =
            w.metadata.iter().map(|(k, v)| MetadataField { key: k.clone(), label: k.clone(), default: v.clone() }).collect();
        spec.default_profile = w.export_profile_id;
        self.save_template(None, name, description, &spec)
    }

    pub fn delete_template(&self, id: &str) -> Result<()> {
        if self.get_template(id)?.builtin {
            return Err(WsError::Invalid("built-in templates cannot be deleted".into()));
        }
        self.conn.execute("DELETE FROM templates WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<ExportProfile>> {
        let mut st =
            self.conn.prepare("SELECT id,name,description,builtin,spec FROM export_profiles ORDER BY builtin DESC, name COLLATE NOCASE")?;
        let rows = st.query_map([], |r| {
            let spec: String = r.get(4)?;
            Ok(ExportProfile {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                builtin: r.get(3)?,
                spec: serde_json::from_str(&spec).unwrap_or_default(),
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_profile(&self, id: Option<&str>) -> Result<ExportProfile> {
        let id = id.unwrap_or("prof-standard");
        self.list_profiles()?.into_iter().find(|p| p.id == id).ok_or_else(|| WsError::NotFound(format!("export profile {id}")))
    }

    pub fn save_profile(&self, id: Option<&str>, name: &str, description: &str, spec: &ProfileSpec) -> Result<ExportProfile> {
        if name.trim().is_empty() {
            return Err(WsError::Invalid("profile name cannot be empty".into()));
        }
        let id = id.map(String::from).unwrap_or_else(|| format!("prof-{}", new_id()));
        if let Ok(p) = self.get_profile(Some(&id)) {
            if p.builtin {
                return Err(WsError::Invalid("built-in profiles cannot be modified; save a copy instead".into()));
            }
        }
        self.conn.execute(
            "INSERT INTO export_profiles(id,name,description,builtin,spec) VALUES(?1,?2,?3,0,?4)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,spec=excluded.spec",
            params![id, name.trim(), description, serde_json::to_string(spec)?],
        )?;
        self.get_profile(Some(&id))
    }

    pub fn delete_profile(&self, id: &str) -> Result<()> {
        if self.get_profile(Some(id))?.builtin {
            return Err(WsError::Invalid("built-in profiles cannot be deleted".into()));
        }
        self.conn.execute("DELETE FROM export_profiles WHERE id=?1", [id])?;
        self.conn.execute("UPDATE workspaces SET export_profile_id=NULL WHERE export_profile_id=?1", [id])?;
        Ok(())
    }

    // ---------- recent packages & revisions ----------
    pub fn add_recent_package(&self, path: &str, name: &str, size: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO recent_packages(path,name,opened_at,size) VALUES(?1,?2,?3,?4)
             ON CONFLICT(path) DO UPDATE SET name=excluded.name,opened_at=excluded.opened_at,size=excluded.size",
            params![path, name, now(), size],
        )?;
        Ok(())
    }
    pub fn list_recent_packages(&self, limit: i64) -> Result<Vec<RecentPackage>> {
        let mut st = self.conn.prepare("SELECT path,name,opened_at,size FROM recent_packages ORDER BY opened_at DESC LIMIT ?1")?;
        let v = st
            .query_map([limit], |r| Ok(RecentPackage { path: r.get(0)?, name: r.get(1)?, opened_at: r.get(2)?, size: r.get(3)? }))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }
    pub fn remove_recent_package(&self, path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM recent_packages WHERE path=?1", [path])?;
        Ok(())
    }
    pub fn clear_recent_packages(&self) -> Result<()> {
        self.conn.execute("DELETE FROM recent_packages", [])?;
        Ok(())
    }
    pub fn list_revisions(&self, ws: &str) -> Result<Vec<RevisionRow>> {
        let mut st = self.conn.prepare("SELECT revision,package_id,path,exported_at,entry_count,package_size FROM revisions WHERE workspace_id=?1 ORDER BY revision DESC")?;
        let v = st
            .query_map([ws], |r| {
                Ok(RevisionRow {
                    revision: r.get(0)?,
                    package_id: r.get(1)?,
                    path: r.get(2)?,
                    exported_at: r.get(3)?,
                    entry_count: r.get(4)?,
                    package_size: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(v)
    }

    // ---------- tags, stats, search ----------
    pub fn all_tags(&self, ws: &str) -> Result<Vec<(String, i64)>> {
        let mut counts: BTreeMap<String, i64> = BTreeMap::new();
        for e in self.list_entries(ws)? {
            for t in e.tags {
                *counts.entry(t).or_default() += 1;
            }
        }
        Ok(counts.into_iter().collect())
    }

    pub fn workspace_stats(&self, ws: &str) -> Result<WorkspaceStats> {
        let entries = self.list_entries(ws)?;
        let mut s = WorkspaceStats::default();
        let mut ext: BTreeMap<String, i64> = BTreeMap::new();
        for e in &entries {
            match e.kind.as_str() {
                "file" => {
                    s.files += 1;
                    s.total_size += e.size;
                    let x = e.name.rsplit_once('.').map(|(_, x)| x.to_lowercase()).unwrap_or_else(|| "(none)".into());
                    *ext.entry(x).or_default() += 1;
                }
                "folder" => s.folders += 1,
                "note" => {
                    s.notes += 1;
                    s.total_size += e.size;
                }
                _ => s.links += 1,
            }
            if !e.tags.is_empty() {
                s.tagged_entries += 1;
            }
        }
        let mut big: Vec<(String, i64)> = entries.iter().filter(|e| e.kind == "file").map(|e| (e.name.clone(), e.size)).collect();
        big.sort_by_key(|b| std::cmp::Reverse(b.1));
        big.truncate(5);
        s.largest = big;
        let mut ex: Vec<_> = ext.into_iter().collect();
        ex.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        ex.truncate(8);
        s.by_extension = ex;
        s.references = self.list_references(ws)?.len() as i64;
        Ok(s)
    }

    /// Search with optional structured filters. Text terms are ANDed and match name, description,
    /// tags, URL, metadata values and note content. In-query operators: kind: tag: ext: in:
    pub fn search(&self, ws: &str, q: &SearchQuery) -> Result<Vec<Entry>> {
        let mut q = q.clone();
        let mut terms: Vec<String> = vec![];
        for tok in q.text.split_whitespace() {
            if let Some((k, v)) = tok.split_once(':') {
                if !v.is_empty() {
                    match k.to_lowercase().as_str() {
                        "kind" => {
                            q.kind = Some(v.to_lowercase());
                            continue;
                        }
                        "tag" => {
                            q.tag = Some(v.to_string());
                            continue;
                        }
                        "ext" => {
                            q.ext = Some(v.trim_start_matches('.').to_lowercase());
                            continue;
                        }
                        _ => {}
                    }
                }
            }
            terms.push(tok.to_lowercase());
        }
        let paths = self.entry_paths(ws)?;
        let scope_prefix = q.parent_id.as_ref().map(|p| format!("{}/", paths.get(p).cloned().unwrap_or_default().to_lowercase()));
        let mut out: Vec<Entry> = self
            .list_entries(ws)?
            .into_iter()
            .filter(|e| q.kind.as_ref().is_none_or(|k| &e.kind == k))
            .filter(|e| q.tag.as_ref().is_none_or(|t| e.tags.iter().any(|x| x.eq_ignore_ascii_case(t))))
            .filter(|e| q.ext.as_ref().is_none_or(|x| e.name.to_lowercase().ends_with(&format!(".{x}"))))
            .filter(|e| scope_prefix.as_ref().is_none_or(|p| paths[&e.id].to_lowercase().starts_with(p)))
            .filter(|e| {
                if terms.is_empty() {
                    return true;
                }
                let hay = format!(
                    "{}\n{}\n{}\n{}\n{}\n{}",
                    paths[&e.id],
                    e.description,
                    e.tags.join(" "),
                    e.url.clone().unwrap_or_default(),
                    e.metadata.values().cloned().collect::<Vec<_>>().join(" "),
                    e.content.clone().unwrap_or_default()
                )
                .to_lowercase();
                terms.iter().all(|t| hay.contains(t))
            })
            .collect();
        let key = q.sort.as_deref().unwrap_or("name");
        out.sort_by(|a, b| {
            let o = match key {
                "size" => a.size.cmp(&b.size),
                "modified" => a.modified_at.cmp(&b.modified_at),
                "kind" => a.kind.cmp(&b.kind).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())),
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            };
            if q.descending {
                o.reverse()
            } else {
                o
            }
        });
        if let Some(l) = q.limit {
            out.truncate(l as usize);
        }
        Ok(out)
    }
}

/// Insert one full entry row (all columns, original ids and timestamps). Used when restoring removed entries.
pub(crate) fn insert_entry_row(conn: &Connection, e: &Entry) -> Result<()> {
    conn.execute(
        &format!("INSERT INTO entries({ENTRY_COLS}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)"),
        params![
            e.id,
            e.workspace_id,
            e.parent_id,
            e.kind,
            e.name,
            e.description,
            e.url,
            e.content,
            e.attached_to,
            e.sort_order,
            e.origin,
            e.source_path,
            e.size,
            e.sha256,
            e.mime,
            e.source_mtime_ms,
            serde_json::to_string(&e.tags)?,
            serde_json::to_string(&e.metadata)?,
            e.created_at,
            e.modified_at
        ],
    )?;
    Ok(())
}
