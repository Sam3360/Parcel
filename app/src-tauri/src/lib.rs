mod commands;
mod error;
mod state;

use parcel_workspace::launch::package_paths_from_args;
use parcel_workspace::Store;
use state::AppState;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        // Must be registered first: a second launch (e.g. double-clicking another .parcel) is forwarded here
        // and then exits, so the existing window opens the package instead of a second app starting.
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            let paths = package_paths_from_args(&args, Some(std::path::Path::new(&cwd)), true);
            if let Some(state) = app.try_state::<AppState>() {
                state.push_open(paths);
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
            let _ = app.emit("parcel://open-package", ());
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = match Store::open(&dir.join("parcel.db")) {
                Ok(s) => s,
                Err(e) => {
                    // Never silently discard a damaged database: move it aside and start fresh.
                    eprintln!("database could not be opened ({e}); moving it aside");
                    let bad = dir.join(format!("parcel.db.damaged-{}", chrono_stamp()));
                    let _ = std::fs::rename(dir.join("parcel.db"), bad);
                    Store::open(&dir.join("parcel.db"))?
                }
            };
            let state = AppState {
                store: Arc::new(Mutex::new(store)),
                jobs: Mutex::new(Default::default()),
                data_dir: dir,
                pending_open: Mutex::new(Vec::new()),
                staging: Mutex::new(Default::default()),
                known_paths: Mutex::new(Vec::new()),
            };
            // A package passed on the command line (double-click / "Open with") opens once the UI is ready.
            state.push_open(package_paths_from_args(std::env::args(), std::env::current_dir().ok().as_deref(), true));
            // Reclaim space from imports that were interrupted or whose workspaces are gone.
            commands::cleanup_managed(&state);
            // Parcel's own storage for imported files; file cleanup never acts outside it.
            if let Ok(g) = state.store.lock() {
                g.set_managed_root(Some(state.managed_dir()));
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_workspaces,
            commands::recent_workspaces,
            commands::create_workspace,
            commands::get_workspace,
            commands::update_workspace,
            commands::duplicate_workspace,
            commands::trash_workspace,
            commands::restore_workspace,
            commands::purge_workspace,
            commands::list_entries,
            commands::create_folder,
            commands::create_note,
            commands::create_link,
            commands::rename_entry,
            commands::update_entry,
            commands::move_entries,
            commands::reorder_entries,
            commands::delete_entries,
            commands::batch_tag,
            commands::batch_set_metadata,
            commands::search_entries,
            commands::workspace_stats,
            commands::all_tags,
            commands::list_references,
            commands::add_reference,
            commands::update_reference,
            commands::delete_reference,
            commands::list_dependencies,
            commands::add_dependency,
            commands::delete_dependency,
            commands::open_external,
            commands::add_paths,
            commands::check_sources,
            commands::relink_source,
            commands::accept_source_changes,
            commands::find_duplicates,
            commands::workspace_diagnostics,
            commands::export_workspace,
            commands::import_package,
            commands::convert_zip,
            commands::open_package,
            commands::verify_package,
            commands::extract_package,
            commands::preview_package_entry,
            commands::preview_entry,
            commands::compare_packages,
            commands::list_templates,
            commands::save_template,
            commands::template_from_workspace,
            commands::delete_template,
            commands::list_profiles,
            commands::save_profile,
            commands::delete_profile,
            commands::list_activity,
            commands::clear_activity,
            commands::list_recent_packages,
            commands::remove_recent_package,
            commands::clear_recent_packages,
            commands::list_revisions,
            commands::take_launch_packages,
            commands::undo_remove,
            commands::list_removals,
            commands::finalize_removals,
            commands::storage_report,
            commands::remove_unused_storage,
            commands::reveal_source,
            commands::reveal_output,
            commands::get_prefs,
            commands::set_pref,
            commands::cancel_job,
            commands::app_info,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Parcel")
        .run(|app, event| {
            // On quit, pending removals become final so deleted items' stored copies are freed.
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    if let Ok(g) = state.store.lock() {
                        g.finalize_all_removals();
                    }
                }
            }
        });
}

fn chrono_stamp() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
