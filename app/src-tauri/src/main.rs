#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::process::Command;
use tauri::Manager;

fn engine_script(app: &tauri::AppHandle) -> PathBuf {
    // Production: engine/ is bundled next to the exe as a resource.
    if let Ok(res) = app.path().resource_dir() {
        let bundled = res.join("engine").join("cli.ps1");
        if bundled.exists() {
            return bundled;
        }
    }
    // Dev (`cargo tauri dev` runs from src-tauri/): use the repo copy.
    PathBuf::from("../../engine/cli.ps1")
}

#[tauri::command]
fn engine(
    app: tauri::AppHandle,
    action: String,
    game: Option<String>,
    path: Option<String>,
) -> Result<String, String> {
    let script = engine_script(&app);
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .args(["-Action", &action]);
    if let Some(g) = game {
        cmd.args(["-Game", &g]);
    }
    if let Some(p) = path {
        cmd.args(["-Path", &p]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https urls allowed".into());
    }
    Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![engine, open_url])
        .run(tauri::generate_context!())
        .expect("error while running Chamber Game Optimizer");
}
