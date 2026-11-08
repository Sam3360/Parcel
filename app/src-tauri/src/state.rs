use parcel_core::cancel::{CancelToken, Progress};
use parcel_core::limits::ResourceLimits;
use parcel_workspace::Store;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub jobs: Mutex<HashMap<String, CancelToken>>,
    pub data_dir: PathBuf,
}

#[derive(Serialize, Clone)]
pub struct ProgressEvent {
    pub job_id: String,
    #[serde(flatten)]
    pub p: Progress,
}

impl AppState {
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
