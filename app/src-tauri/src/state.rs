use parcel_core::cancel::{CancelToken, Progress};
use parcel_core::limits::ResourceLimits;
use parcel_workspace::Store;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub jobs: Mutex<HashMap<String, CancelToken>>,
    pub data_dir: PathBuf,
    /// Package paths waiting for the UI to pick them up (launch arguments / second instance).
    pub pending_open: Mutex<Vec<String>>,
    /// Workspace ids of imports currently being staged; their folders must never be cleaned up.
    pub staging: Mutex<HashSet<String>>,
    /// Paths this app produced or opened; only these may be revealed in the file manager from the UI.
    pub known_paths: Mutex<Vec<PathBuf>>,
}

#[derive(Serialize, Clone)]
pub struct ProgressEvent {
    pub job_id: String,
    #[serde(flatten)]
    pub p: Progress,
}

impl AppState {
    pub fn push_open(&self, paths: Vec<PathBuf>) {
        let mut q = self.pending_open.lock().unwrap();
        for p in paths {
            let s = p.to_string_lossy().into_owned();
            if !q.contains(&s) {
                q.push(s);
            }
        }
    }
    pub fn remember_path(&self, p: PathBuf) {
        let mut k = self.known_paths.lock().unwrap();
        k.retain(|x| x != &p);
        k.push(p);
        let excess = k.len().saturating_sub(200);
        k.drain(..excess);
    }
    pub fn is_known_path(&self, p: &std::path::Path) -> bool {
        self.known_paths.lock().unwrap().iter().any(|x| x == p)
    }
    pub fn protected_import_ids(&self) -> HashSet<String> {
        self.staging.lock().unwrap().clone()
    }

    pub fn register_job(&self, id: &str) -> CancelToken {
        let t = CancelToken::new();
        self.jobs.lock().unwrap().insert(id.to_string(), t.clone());
        t
    }
    pub fn finish_job(&self, id: &str) {
        self.jobs.lock().unwrap().remove(id);
    }
    pub fn managed_dir(&self) -> PathBuf {
        self.data_dir.join("imported")
    }
    pub fn limits(&self) -> ResourceLimits {
        let guard = self.store.lock().unwrap();
        guard.get_pref("limits").ok().flatten().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
}

/// Throttled progress emitter so large operations do not flood the UI.
pub fn emitter(app: AppHandle, job_id: String) -> impl Fn(&Progress) + Send + Sync + 'static {
    let last = Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(1));
    move |p: &Progress| {
        let mut l = last.lock().unwrap();
        let done = p.items_total > 0 && p.items_done >= p.items_total;
        if done || l.elapsed() >= std::time::Duration::from_millis(60) {
            *l = std::time::Instant::now();
            let _ = app.emit("parcel://progress", ProgressEvent { job_id: job_id.clone(), p: p.clone() });
        }
    }
}
