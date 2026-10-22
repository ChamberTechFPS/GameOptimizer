//! What's installed and where: Steam, Epic, the Windows uninstall list, known folders,
//! the primary monitor, and running processes. Read-only. Other platforms get an empty PC
//! so the engine tests run on macOS.

use crate::catalog::{Game, Vars};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// One snapshot of the PC, taken per scan or action.
#[derive(Default)]
pub struct Pc {
    /// Path tokens that exist on this PC, e.g. "documents" -> C:\Users\me\Documents.
    pub roots: HashMap<&'static str, PathBuf>,
    pub vars: Vars,
    /// Lowercase exe names of running processes.
    pub running: HashSet<String>,
    pub steam_libraries: Vec<PathBuf>,
    /// (AppName, DisplayName, InstallLocation) from Epic's launcher manifests.
    pub epic: Vec<(String, String, PathBuf)>,
    /// (DisplayName, InstallLocation) from the Windows uninstall list.
    pub uninstall: Vec<(String, Option<PathBuf>)>,
}

impl Pc {
    /// Where a game is installed and which launcher says so (None = the game's own launcher).
    /// Config-folder checks live in the engine.
    pub fn install_location(&self, game: &Game) -> Option<(PathBuf, Option<&'static str>)> {
        if let Some(id) = game.find.steam {
            for lib in &self.steam_libraries {
                let manifest = lib.join("steamapps").join(format!("appmanifest_{id}.acf"));
                if let Ok(text) = std::fs::read_to_string(&manifest) {
                    let dir = vdf_value(&text, "installdir").unwrap_or_default();
                    return Some((lib.join("steamapps").join("common").join(dir), Some("steam")));
                }
            }
        }
        if let Some(name) = &game.find.epic {
            let hit = self.epic.iter().find(|(app, display, _)| app.eq_ignore_ascii_case(name) || display.eq_ignore_ascii_case(name));
            if let Some((_, _, location)) = hit {
                return Some((location.clone(), Some("epic")));
            }
        }
        if let Some(name) = &game.find.uninstall {
            if let Some((_, location)) = self.uninstall.iter().find(|(display, _)| display.eq_ignore_ascii_case(name)) {
                // Some installers leave InstallLocation blank; the game is still there.
                return Some((location.clone().unwrap_or_default(), None));
            }
        }
        None
    }

    pub fn is_running(&self, game: &Game) -> bool {
        game.process.iter().any(|p| self.running.contains(&p.to_ascii_lowercase()))
    }
}

/// The quoted values on a VDF line, e.g. `"path"  "D:\\SteamLibrary"` -> ["path", "D:\SteamLibrary"].
fn vdf_tokens(line: &str) -> Vec<String> {
    line.split('"').skip(1).step_by(2).map(|t| t.replace("\\\\", "\\")).collect()
}

fn vdf_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| match vdf_tokens(l).as_slice() {
        [k, v] if k.eq_ignore_ascii_case(key) => Some(v.clone()),
        _ => None,
    })
}

/// Every Steam library folder, from libraryfolders.vdf (covers games on other drives).
#[cfg_attr(not(windows), allow(dead_code))] // used by the Windows probe and the tests
pub fn steam_libraries(steam: &Path) -> Vec<PathBuf> {
    let mut libs = vec![steam.to_path_buf()];
    if let Ok(text) = std::fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
        for line in text.lines() {
            if let [k, v] = vdf_tokens(line).as_slice() {
                if k.eq_ignore_ascii_case("path") && !libs.iter().any(|l| l.as_os_str().eq_ignore_ascii_case(v)) {
                    libs.push(PathBuf::from(v));
                }
            }
        }
    }
    libs
}

/// userdata folder of the most recently signed-in Steam account (not just the first one).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn steam_userdata(steam: &Path) -> Option<PathBuf> {
    let userdata = steam.join("userdata");
    if let Ok(text) = std::fs::read_to_string(steam.join("config").join("loginusers.vdf")) {
        let mut current: Option<u64> = None;
        for line in text.lines() {
            match vdf_tokens(line).as_slice() {
                [id] if id.len() == 17 && id.starts_with("7656119") => current = id.parse().ok(),
                [k, v] if k.eq_ignore_ascii_case("MostRecent") && v == "1" => {
                    // userdata folders use the 32-bit account id: SteamID64 minus the base.
                    let dir = current.and_then(|id| id.checked_sub(76_561_197_960_265_728)).map(|a| userdata.join(a.to_string()));
                    if let Some(dir) = dir.filter(|d| d.is_dir()) {
                        return Some(dir);
                    }
                }
                _ => {}
            }
        }
    }
    std::fs::read_dir(&userdata)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "0" && n.to_string_lossy().chars().all(|c| c.is_ascii_digit())))
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::core::GUID;
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    impl Pc {
        pub fn probe() -> Pc {
            let mut pc = Pc { vars: display_vars(), running: running_processes(), ..Pc::default() };
            use windows_sys::Win32::UI::Shell::{FOLDERID_Documents, FOLDERID_LocalAppData, FOLDERID_RoamingAppData, FOLDERID_SavedGames};
            for (token, id) in [
                ("localappdata", &FOLDERID_LocalAppData),
                ("appdata", &FOLDERID_RoamingAppData),
                ("documents", &FOLDERID_Documents), // follows OneDrive redirection
                ("savedgames", &FOLDERID_SavedGames),
            ] {
                if let Some(p) = known_folder(id) {
                    pc.roots.insert(token, p);
                }
            }
            if let Some(steam) = steam_root() {
                pc.steam_libraries = steam_libraries(&steam);
                if let Some(u) = steam_userdata(&steam) {
                    pc.roots.insert("steamuserdata", u);
                }
            }
            pc.epic = epic_installs();
            pc.uninstall = uninstall_list();
            pc
        }
    }

    fn known_folder(id: &GUID) -> Option<PathBuf> {
        use windows_sys::Win32::System::Com::CoTaskMemFree;
        use windows_sys::Win32::UI::Shell::SHGetKnownFolderPath;
        let mut raw = std::ptr::null_mut();
        // SAFETY: documented out-pointer API; the buffer is freed with CoTaskMemFree either way.
        unsafe {
            let ok = SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut raw) == 0 && !raw.is_null();
            let path = ok.then(|| {
                let len = (0..).take_while(|&i| *raw.add(i) != 0).count();
                PathBuf::from(OsString::from_wide(std::slice::from_raw_parts(raw, len)))
            });
            CoTaskMemFree(raw as _);
            path
        }
    }

    fn steam_root() -> Option<PathBuf> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Software\\Valve\\Steam").ok();
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam").ok();
        let path: String = hkcu
            .and_then(|k| k.get_value("SteamPath").ok())
            .or_else(|| hklm.and_then(|k| k.get_value("InstallPath").ok()))?;
        Some(PathBuf::from(path.replace('/', "\\"))).filter(|p| p.is_dir())
    }

    fn epic_installs() -> Vec<(String, String, PathBuf)> {
        let data = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| "C:\\ProgramData".into());
        let dir = data.join("Epic\\EpicGamesLauncher\\Data\\Manifests");
        let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
        entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")))
            .filter_map(|e| {
                let item: serde_json::Value = serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok()?;
                let field = |k: &str| item.get(k)?.as_str().map(String::from);
                Some((field("AppName")?, field("DisplayName").unwrap_or_default(), PathBuf::from(field("InstallLocation")?)))
            })
            .collect()
    }

    fn uninstall_list() -> Vec<(String, Option<PathBuf>)> {
        let mut out = Vec::new();
        for (hive, path) in [
            (HKEY_LOCAL_MACHINE, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall"),
            (HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall"),
            (HKEY_CURRENT_USER, "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall"),
        ] {
            let Ok(root) = RegKey::predef(hive).open_subkey(path) else { continue };
            for name in root.enum_keys().flatten() {
                let Ok(app) = root.open_subkey(&name) else { continue };
                let Ok(display) = app.get_value::<String, _>("DisplayName") else { continue };
                let location = app
                    .get_value::<String, _>("InstallLocation")
                    .ok()
                    .map(|s| PathBuf::from(s.trim().trim_matches('"')))
                    .filter(|p| !p.as_os_str().is_empty());
                out.push((display, location));
            }
        }
        out
    }

    fn display_vars() -> Vars {
        use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
        // SAFETY: zeroed DEVMODEW with dmSize set is the documented way to call this.
        unsafe {
            let mut mode: DEVMODEW = std::mem::zeroed();
            mode.dmSize = std::mem::size_of::<DEVMODEW>() as u16;
            if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut mode) != 0 {
                // 0 or 1 means "hardware default"; treat anything that low as 60 Hz.
                let refresh = if mode.dmDisplayFrequency < 30 { 60 } else { mode.dmDisplayFrequency };
                return Vars { refresh, width: mode.dmPelsWidth, height: mode.dmPelsHeight };
            }
        }
        Vars::default()
    }

    fn running_processes() -> HashSet<String> {
        use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows_sys::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
        };
        let mut out = HashSet::new();
        // SAFETY: standard ToolHelp snapshot walk; the handle is closed before returning.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return out;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snap, &mut entry) != 0;
            while more {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                out.insert(String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase());
                more = Process32NextW(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
        }
        out
    }
}

#[cfg(not(windows))]
impl Pc {
    /// Nothing to detect off Windows; the app only ships for Windows.
    pub fn probe() -> Pc {
        Pc::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("chamber-detect-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn steam_libraries_include_other_drives() {
        let steam = temp("libs");
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        std::fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"D:\\\\SteamLibrary\"\n\t}\n}\n",
        )
        .unwrap();
        let libs = steam_libraries(&steam);
        assert!(libs.contains(&PathBuf::from("D:\\SteamLibrary")));
        assert!(libs.contains(&PathBuf::from("C:\\Program Files (x86)\\Steam")));
        std::fs::remove_dir_all(steam).unwrap();
    }

    #[test]
    fn steam_userdata_follows_the_most_recent_login() {
        let steam = temp("users");
        for id in ["11111111", "22222222"] {
            std::fs::create_dir_all(steam.join("userdata").join(id)).unwrap();
        }
        std::fs::create_dir_all(steam.join("config")).unwrap();
        // 76561197982487950 - 76561197960265728 = 22222222 (MostRecent); the other is 11111111.
        std::fs::write(
            steam.join("config/loginusers.vdf"),
            "\"users\"\n{\n\t\"76561197971376839\"\n\t{\n\t\t\"MostRecent\"\t\t\"0\"\n\t}\n\t\"76561197982487950\"\n\t{\n\t\t\"MostRecent\"\t\t\"1\"\n\t}\n}\n",
        )
        .unwrap();
        assert_eq!(steam_userdata(&steam), Some(steam.join("userdata/22222222")));
        std::fs::remove_dir_all(steam).unwrap();
    }
}
