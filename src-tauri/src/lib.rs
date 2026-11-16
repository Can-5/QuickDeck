// QuickDeck — universal launcher. Global shortcut (Alt+Space) toggles a frameless
// spotlight-style window. App discovery is platform-native; websites/commands are
// user-configured; all state lives in one JSON file in the app config dir.

mod apps;
mod search;
mod store;

use apps::discover_apps;
use search::search_files;
use store::{JsonStore, Snapshot, Website, Command};
use tauri::{Manager, State};
use std::sync::Mutex;

struct AppState {
    store: Mutex<JsonStore>,
}

#[tauri::command]
fn discover_apps_cmd() -> Vec<apps::AppEntry> {
    discover_apps()
}

#[tauri::command]
fn search_files_cmd(query: String, limit: usize) -> Vec<search::FileEntry> {
    search_files(&query, limit)
}

#[tauri::command]
fn snapshot(state: State<AppState>) -> Snapshot {
    state.store.lock().unwrap().snapshot()
}

#[tauri::command]
fn toggle_favorite(state: State<AppState>, id: String) -> bool {
    state.store.lock().unwrap().toggle_favorite(&id)
}

#[tauri::command]
fn add_website(state: State<AppState>, name: String, url: String) -> Result<Website, String> {
    state.store.lock().unwrap().add_website(&name, &url)
}

#[tauri::command]
fn remove_website(state: State<AppState>, id: String) {
    state.store.lock().unwrap().remove_website(&id)
}

#[tauri::command]
fn add_command(state: State<AppState>, name: String, cmd: String) -> Result<Command, String> {
    state.store.lock().unwrap().add_command(&name, &cmd)
}

#[tauri::command]
fn remove_command(state: State<AppState>, id: String) {
    state.store.lock().unwrap().remove_command(&id)
}

#[tauri::command]
fn record_launch(state: State<AppState>, id: String, name: String, kind: String) {
    state.store.lock().unwrap().record_launch(&id, &name, &kind);
}

#[tauri::command]
fn clear_history(state: State<AppState>) {
    state.store.lock().unwrap().clear_history();
}

fn open_with_system(target: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(target);
        c
    };
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.arg("/c").arg("start").arg("").arg(target.replace('"', "\"\""));
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(target);
        c
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("failed to open: {e}"))
}

#[tauri::command]
fn launch_item(state: State<AppState>, id: String, _name: String) -> Result<(), String> {
    let (kind, rest) = id.split_once(':').ok_or("invalid item id")?;
    match kind {
        "app" => {
            #[cfg(target_os = "linux")]
            {
                // Prefer gtk-launch (handles desktop file semantics, D-Bus activation)
                let desktop_id = apps::desktop_id(&id).unwrap_or(rest).to_string();
                if std::process::Command::new("gtk-launch")
                    .arg(&desktop_id)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .is_ok()
                {
                    return Ok(());
                }
                // Fallback: strip field codes from Exec and spawn directly (no shell)
                let entries = discover_apps();
                let entry = entries.iter().find(|e| e.id == id)
                    .ok_or("application not found")?;
                let argv = apps::strip_field_codes(&entry.source);
                let (prog, args) = argv.split_first().ok_or("empty Exec line")?;
                std::process::Command::new(prog)
                    .args(args)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| format!("failed to launch {prog}: {e}"))
            }
            #[cfg(target_os = "windows")]
            {
                open_with_system(rest)
            }
            #[cfg(target_os = "macos")]
            {
                open_with_system(rest)
            }
        }
        "file" => {
            if !search::is_probably_safe_path(rest) {
                return Err("refusing to open a non-absolute or invalid path".into());
            }
            open_with_system(rest)
        }
        "web" => {
            if !(rest.starts_with("http://") || rest.starts_with("https://")) {
                return Err("only http(s) URLs can be opened".into());
            }
            open_with_system(rest)
        }
        "cmd" => {
            let store = state.store.lock().unwrap();
            let cmd = store.snapshot().commands.into_iter().find(|c| c.id == id)
                .map(|c| c.cmd)
                .ok_or("command not found")?;
            drop(store);
            #[cfg(not(target_os = "windows"))]
            let mut c = {
                let mut c = std::process::Command::new("sh");
                c.arg("-c").arg(&cmd);
                c
            };
            #[cfg(target_os = "windows")]
            let mut c = {
                let mut c = std::process::Command::new("cmd");
                c.arg("/c").arg(&cmd);
                c
            };
            c.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("failed to run command: {e}"))
        }
        "win" | "mac" => open_with_system(rest),
        _ => Err("unknown item kind".into()),
    }
}

#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let state = AppState { store: Mutex::new(JsonStore::load(config_dir)) };
            app.manage(state);

            // Hide on blur (spotlight behavior)
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = w.hide();
                    }
                });
            }

            // Global shortcut: Alt+Space toggles the launcher
            #[cfg(desktop)]
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                let handle = app.handle().clone();
                let result = app.global_shortcut().on_shortcut("Alt+Space", move |_app, _shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if let Some(window) = handle.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                });
                if let Err(e) = result {
                    eprintln!("QuickDeck: Alt+Space unavailable ({e}). The window stays available until closed.");
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover_apps_cmd, search_files_cmd, snapshot,
            toggle_favorite, add_website, remove_website, add_command, remove_command,
            record_launch, clear_history, launch_item, hide_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running QuickDeck");
}
