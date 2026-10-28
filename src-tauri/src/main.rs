#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod detect;
mod engine;
mod patch;

use detect::Pc;
use engine::Engine;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

/// One engine action at a time: concurrent applies would race on state.json.
struct App(Mutex<Engine>);

type Reply = Result<Value, String>;

fn with<T>(app: &State<'_, App>, f: impl FnOnce(&Engine) -> Result<T, String>) -> Result<T, String> {
    let engine = app.0.lock().map_err(|_| "Chamber hit an internal error. Restart the app.".to_string())?;
    f(&engine)
}

#[tauri::command]
async fn scan(app: State<'_, App>) -> Reply {
    with(&app, |e| {
        let games = e.scan(&e.catalog()?, &Pc::probe())?;
        let state = e.load_state()?;
        Ok(json!({ "games": games, "terms": state.terms, "libraries": state.libraries }))
    })
}

#[tauri::command]
async fn accept_terms(app: State<'_, App>, version: u32) -> Reply {
    with(&app, |e| e.accept_terms(version).map(|_| json!({})))
}

#[tauri::command]
async fn add_library(app: State<'_, App>, path: PathBuf) -> Reply {
    with(&app, |e| e.add_library(&path).map(|_| json!({})))
}

#[tauri::command]
async fn remove_library(app: State<'_, App>, path: PathBuf) -> Reply {
    with(&app, |e| e.remove_library(&path).map(|_| json!({})))
}

/// Declining the terms closes the app.
#[tauri::command]
fn quit(handle: tauri::AppHandle) {
    handle.exit(0);
}

#[tauri::command]
async fn apply(app: State<'_, App>, slug: String) -> Reply {
    with(&app, |e| Ok(json!({ "changed": e.apply(&e.catalog()?, &Pc::probe(), &slug)? })))
}

#[tauri::command]
async fn revert(app: State<'_, App>, slug: String) -> Reply {
    with(&app, |e| e.revert(&e.catalog()?, &Pc::probe(), &slug).map(|_| json!({})))
}

#[tauri::command]
async fn set_folder(app: State<'_, App>, slug: String, path: PathBuf) -> Reply {
    with(&app, |e| e.set_folder(&e.catalog()?, &slug, &path).map(|_| json!({})))
}

#[tauri::command]
async fn clear_folder(app: State<'_, App>, slug: String) -> Reply {
    with(&app, |e| e.clear_folder(&e.catalog()?, &slug).map(|_| json!({})))
}

#[tauri::command]
async fn set_hidden(app: State<'_, App>, slug: String, hidden: bool) -> Reply {
    with(&app, |e| e.set_hidden(&e.catalog()?, &slug, hidden).map(|_| json!({})))
}

#[tauri::command]
async fn open_folder(handle: tauri::AppHandle, app: State<'_, App>, slug: String, which: String) -> Reply {
    let dir = with(&app, |e| e.folder_for(&e.catalog()?, &Pc::probe(), &slug, &which))?;
    handle.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())?;
    Ok(json!({}))
}

/// The UI downloads the catalog; nothing is used until the signature checks out here.
#[tauri::command]
async fn install_catalog(app: State<'_, App>, json: Vec<u8>, sig: Vec<u8>) -> Reply {
    with(&app, |e| e.install_catalog(&json, &sig).map(|status| json!({ "status": status })))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data = app.path().local_data_dir()?.join("ChamberTech").join("GameOptimizer");
            let bundled = app.path().resource_dir()?.join("catalog");
            app.manage(App(Mutex::new(Engine { data, bundled })));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan,
            apply,
            revert,
            set_folder,
            clear_folder,
            set_hidden,
            open_folder,
            install_catalog,
            accept_terms,
            add_library,
            remove_library,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running Chamber Game Optimizer");
}
