//! Windows tweaks, grouped into profiles (Esports Pro, Casual Gamer, Security). The list is
//! `tweaks.json` at the repo root, compiled into the app: it is not downloaded, so changing a
//! Windows tweak takes an app release.
//!
//! Every tweak is one or more registry values, each with the value to `set` and the Windows
//! `default`. Apply writes `set`; reset writes `default`. Reset needs no record of what the app
//! did, so it also undoes the same values on a PC the playbook tuned.
//!
//! HKCU values are written by the app itself. HKLM values need administrator rights: the app
//! starts a second copy of itself through the Windows permission prompt, and that copy takes
//! only an action and a profile id, then reads the same compiled-in list. Nothing the
//! unelevated app or a file on disk says can make it write any other value.

use crate::catalog::{is_slug, is_text};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const EMBEDDED: &str = include_str!("../../tweaks.json");

/// Registry keys a tweak may touch. A key must be one of these or sit below one. Adding a root
/// is a code change on purpose: review what else lives under it first.
const HKLM_ROOTS: [&str; 16] = [
    MMCSS_ROOT,
    "SOFTWARE\\Policies\\Microsoft\\Windows",
    "SOFTWARE\\Policies\\Microsoft\\Windows Defender",
    "SOFTWARE\\Policies\\Microsoft\\Edge",
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Policies",
    "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\GameDVR",
    "SOFTWARE\\Microsoft\\Windows\\Dwm",
    "SYSTEM\\CurrentControlSet\\Control\\DeviceGuard",
    "SYSTEM\\CurrentControlSet\\Control\\PriorityControl",
    "SYSTEM\\CurrentControlSet\\Control\\Power",
    "SYSTEM\\CurrentControlSet\\Control\\GraphicsDrivers",
    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\kernel",
    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Memory Management",
    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Power",
    "SYSTEM\\CurrentControlSet\\Control\\FileSystem",
    SERVICES_ROOT,
];
const HKCU_ROOTS: [&str; 14] = [
    "System\\GameConfigStore",
    "Control Panel\\Mouse",
    "Control Panel\\Desktop",
    "Control Panel\\Accessibility",
    "Software\\Microsoft\\GameBar",
    "Software\\Microsoft\\InputPersonalization",
    "Software\\Microsoft\\Personalization",
    "Software\\Microsoft\\Windows\\DWM",
    "Software\\Microsoft\\Windows\\CurrentVersion\\GameDVR",
    "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced",
    "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
    "Software\\Microsoft\\Windows\\CurrentVersion\\BackgroundAccessApplications",
    "Software\\Microsoft\\Windows\\CurrentVersion\\ContentDeliveryManager",
    "Software\\Policies\\Microsoft\\Windows",
];
/// The only HKLM root where a text value is allowed (`Scheduling Category` = `High`).
const MMCSS_ROOT: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile";
const SERVICES_ROOT: &str = "SYSTEM\\CurrentControlSet\\Services";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TweakFile {
    pub profiles: Vec<Profile>,
    pub tweaks: Vec<Tweak>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub tweaks: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tweak {
    pub id: String,
    pub name: String,
    pub why: String,
    /// Takes effect after Windows restarts.
    #[serde(default)]
    pub restart: bool,
    pub values: Vec<RegValue>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegValue {
    pub hive: Hive,
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: Kind,
    /// What the tweak writes. `null` removes the value.
    pub set: Option<String>,
    /// What stock Windows has. `null` means stock Windows has no such value.
    pub default: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Hive {
    /// The signed-in player's own settings. No administrator rights needed.
    #[serde(rename = "HKCU")]
    User,
    /// Settings for the whole PC. Needs the Windows permission prompt.
    #[serde(rename = "HKLM")]
    Machine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A whole number from 0 to 4294967295, written in decimal.
    Dword,
    /// Short text: letters, digits, spaces, `-` and `,`. Never a path or a file name.
    Sz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Apply,
    Reset,
}

impl Action {
    pub fn parse(s: &str) -> Option<Action> {
        match s {
            "apply" => Some(Action::Apply),
            "reset" => Some(Action::Reset),
            _ => None,
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn word(self) -> &'static str {
        match self {
            Action::Apply => "apply",
            Action::Reset => "reset",
        }
    }
}

/// Reset can cover every tweak in the file: that is the "undo the playbook" button.
pub const ALL: &str = "all";

/// The compiled-in tweak list, checked. A bad list is a build mistake the tests catch.
pub fn load() -> Result<TweakFile, String> {
    let file: TweakFile = serde_json::from_str(EMBEDDED).map_err(|e| format!("tweaks.json is malformed: {e}"))?;
    file.validate()?;
    Ok(file)
}

impl TweakFile {
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        for t in &self.tweaks {
            let fail = |why: String| Err(format!("Tweak {:?} rejected: {why}", t.id));
            if !is_slug(&t.id) || !ids.insert(t.id.as_str()) {
                return fail("bad or duplicate id".into());
            }
            if !is_text(&t.name) || !is_text(&t.why) {
                return fail("needs a name and a why".into());
            }
            if t.values.is_empty() {
                return fail("lists no registry values".into());
            }
            for v in &t.values {
                if let Err(e) = v.check() {
                    return fail(e);
                }
            }
        }
        let mut profiles = HashSet::new();
        for p in &self.profiles {
            let fail = |why: String| Err(format!("Profile {:?} rejected: {why}", p.id));
            if !is_slug(&p.id) || p.id == ALL || !profiles.insert(p.id.as_str()) {
                return fail("bad or duplicate id".into());
            }
            if !is_text(&p.name) || !is_text(&p.summary) {
                return fail("needs a name and a summary".into());
            }
            if p.tweaks.is_empty() {
                return fail("lists no tweaks".into());
            }
            if let Some(missing) = p.tweaks.iter().find(|id| !ids.contains(id.as_str())) {
                return fail(format!("unknown tweak {missing:?}"));
            }
        }
        Ok(())
    }

    /// The tweaks a profile lists, or every tweak for `all`.
    fn scope(&self, scope: &str) -> Result<Vec<&Tweak>, String> {
        if scope == ALL {
            return Ok(self.tweaks.iter().collect());
        }
        let p = self.profiles.iter().find(|p| p.id == scope).ok_or_else(|| format!("Unknown profile: {scope}"))?;
        Ok(self.tweaks.iter().filter(|t| p.tweaks.contains(&t.id)).collect())
    }

    /// The values `action` would change right now in one hive, with what each becomes.
    fn plan<'a>(&'a self, reg: &dyn Registry, scope: &str, action: Action, hive: Hive) -> Result<Vec<(&'a RegValue, Option<&'a str>)>, String> {
        if action == Action::Apply && scope == ALL {
            return Err("Pick a profile to apply.".into());
        }
        Ok(self
            .scope(scope)?
            .into_iter()
            .flat_map(|t| &t.values)
            .filter(|v| v.hive == hive)
            .map(|v| (v, v.target(action)))
            .filter(|(v, want)| !v.is(reg, *want))
            .collect())
    }

    /// How many values in one hive `action` would change. Zero HKLM values means no prompt.
    pub fn pending(&self, reg: &dyn Registry, scope: &str, action: Action, hive: Hive) -> Result<usize, String> {
        Ok(self.plan(reg, scope, action, hive)?.len())
    }

    /// Write one hive's share of an action. Returns how many values changed.
    pub fn write(&self, reg: &dyn Registry, scope: &str, action: Action, hive: Hive) -> Result<usize, String> {
        let plan = self.plan(reg, scope, action, hive)?;
        for (v, want) in &plan {
            reg.set(v, *want).map_err(|e| format!("Couldn't change {}: {e}", v.name))?;
        }
        Ok(plan.len())
    }

    pub fn view(&self, reg: &dyn Registry) -> Vec<ProfileView> {
        self.profiles
            .iter()
            .map(|p| {
                let tweaks: Vec<TweakView> = self.tweaks.iter().filter(|t| p.tweaks.contains(&t.id)).map(|t| t.view(reg)).collect();
                ProfileView {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    summary: p.summary.clone(),
                    total: tweaks.len(),
                    applied: tweaks.iter().filter(|t| t.state == "applied").count(),
                    admin: tweaks.iter().any(|t| t.admin),
                    tweaks,
                }
            })
            .collect()
    }
}

impl Tweak {
    fn view(&self, reg: &dyn Registry) -> TweakView {
        let all = |action: Action| self.values.iter().all(|v| v.is(reg, v.target(action)));
        TweakView {
            id: self.id.clone(),
            name: self.name.clone(),
            why: self.why.clone(),
            restart: self.restart,
            admin: self.values.iter().any(|v| v.hive == Hive::Machine),
            // A tweak whose value equals the Windows default reads as applied.
            state: if all(Action::Apply) { "applied" } else if all(Action::Reset) { "default" } else { "other" },
            values: self
                .values
                .iter()
                .map(|v| ValueView {
                    name: v.name.clone(),
                    now: show(reg.get(v).as_deref()),
                    set: show(v.set.as_deref()),
                    default: show(v.default.as_deref()),
                })
                .collect(),
        }
    }
}

fn show(value: Option<&str>) -> String {
    value.unwrap_or("Not set").to_string()
}

impl RegValue {
    fn target(&self, action: Action) -> Option<&str> {
        match action {
            Action::Apply => self.set.as_deref(),
            Action::Reset => self.default.as_deref(),
        }
    }

    fn is(&self, reg: &dyn Registry, want: Option<&str>) -> bool {
        reg.get(self).as_deref() == want
    }

    fn check(&self) -> Result<(), String> {
        let bad = |why: &str| Err(format!("{}\\{}: {why}", self.key, self.name));
        let roots: &[&str] = match self.hive {
            Hive::Machine => &HKLM_ROOTS,
            Hive::User => &HKCU_ROOTS,
        };
        let clean = |seg: &str| seg.starts_with(|c: char| c.is_ascii_alphanumeric()) && seg.trim() == seg && seg.chars().all(|c| c.is_ascii_alphanumeric() || " ._-".contains(c));
        if !self.key.split('\\').all(clean) {
            return bad("the key has an empty or unsafe part");
        }
        if !roots.iter().any(|root| under(&self.key, root)) {
            return bad("the key is not under an allowed root (see HKLM_ROOTS and HKCU_ROOTS in tweaks.rs)");
        }
        if !clean(&self.name) || self.name.len() > 64 {
            return bad("bad value name");
        }
        if self.set.is_none() && self.default.is_none() {
            return bad("needs a set value, a default, or both");
        }
        for data in [&self.set, &self.default].into_iter().flatten() {
            let ok = match self.kind {
                // Canonical decimal, so a value read back compares equal to the one written.
                Kind::Dword => data.parse::<u32>().is_ok_and(|n| n.to_string() == *data),
                Kind::Sz => !data.is_empty() && data.len() <= 64 && data.trim() == data && data.chars().all(|c| c.is_ascii_alphanumeric() || " -,".contains(c)),
            };
            if !ok {
                return bad("bad data for its type");
            }
        }
        if self.hive == Hive::Machine && self.kind == Kind::Sz && !under(&self.key, MMCSS_ROOT) {
            return bad("text values for the whole PC are only allowed under Multimedia\\SystemProfile");
        }
        if self.hive == Hive::Machine && under(&self.key, SERVICES_ROOT) {
            // A service's start type (automatic, manual, disabled) or a number in its Parameters
            // key. Never ImagePath, and never the boot or system start types a PC needs to start.
            let depth = self.key.split('\\').count() - SERVICES_ROOT.split('\\').count();
            let start = depth == 1 && self.name == "Start" && [&self.set, &self.default].into_iter().flatten().all(|d| matches!(d.as_str(), "2" | "3" | "4"));
            let parameter = depth == 2 && self.key.to_ascii_lowercase().ends_with("\\parameters") && self.kind == Kind::Dword;
            if !start && !parameter {
                return bad("under Services only Start (2, 3 or 4) or a number in a Parameters key");
            }
        }
        Ok(())
    }
}

/// True when `key` is `root` or below it, compared like the registry does: ignoring case.
fn under(key: &str, root: &str) -> bool {
    let (key, root) = (key.to_ascii_lowercase(), root.to_ascii_lowercase());
    key == root || key.strip_prefix(&root).is_some_and(|rest| rest.starts_with('\\'))
}

/// One profile as the UI shows it.
#[derive(Debug, Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub total: usize,
    pub applied: usize,
    /// At least one tweak needs the Windows permission prompt.
    pub admin: bool,
    pub tweaks: Vec<TweakView>,
}

#[derive(Debug, Serialize)]
pub struct TweakView {
    pub id: String,
    pub name: String,
    pub why: String,
    pub restart: bool,
    pub admin: bool,
    /// "applied", "default", or "other" (changed by something else, or only partly applied).
    pub state: &'static str,
    pub values: Vec<ValueView>,
}

#[derive(Debug, Serialize)]
pub struct ValueView {
    pub name: String,
    pub now: String,
    pub set: String,
    pub default: String,
}

/// The registry, behind a trait so the tests run without one.
pub trait Registry {
    /// The value as the tweak file would write it, `None` when it isn't there. A value of
    /// another type comes back as text no tweak can match.
    fn get(&self, v: &RegValue) -> Option<String>;
    /// Write the data, or remove the value for `None`.
    fn set(&self, v: &RegValue, data: Option<&str>) -> Result<(), String>;
}

/// Apply or reset from the app: the player's own values here, the whole-PC ones through the
/// Windows permission prompt. Returns how many values changed.
pub fn run(action: Action, scope: &str) -> Result<usize, String> {
    let file = load()?;
    let reg = system();
    let machine = file.pending(&*reg, scope, action, Hive::Machine)?;
    let user = file.write(&*reg, scope, action, Hive::User)?;
    if machine > 0 {
        elevate(action, scope)?;
        let left = file.pending(&*reg, scope, action, Hive::Machine)?;
        if left > 0 {
            return Err(format!("Windows refused {left} of {machine} changes. Something else may be managing those settings."));
        }
    }
    Ok(user + machine)
}

/// What the app shows: every profile with each tweak's state right now.
pub fn scan() -> Result<Vec<ProfileView>, String> {
    Ok(load()?.view(&*system()))
}

/// The elevated copy's whole job: `--tweaks <apply|reset> <profile|all>`. Returns the exit code
/// when the arguments name that job, `None` for a normal start. It reads nothing else from the
/// command line, and nothing from disk.
pub fn elevated_entry(args: &[String]) -> Option<i32> {
    let [_, flag, action, scope] = args else { return None };
    if flag != "--tweaks" {
        return None;
    }
    let done = Action::parse(action)
        .ok_or_else(|| "bad action".to_string())
        .and_then(|action| load()?.write(&*system(), scope, action, Hive::Machine));
    Some(if done.is_ok() { 0 } else { 1 })
}

#[cfg(windows)]
fn system() -> Box<dyn Registry> {
    Box::new(windows::WinRegistry)
}

#[cfg(windows)]
use windows::elevate;

#[cfg(windows)]
mod windows {
    use super::*;
    use winreg::enums::{RegType, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE};
    use winreg::RegKey;

    pub struct WinRegistry;

    fn hive(v: &RegValue) -> RegKey {
        RegKey::predef(match v.hive {
            Hive::Machine => HKEY_LOCAL_MACHINE,
            Hive::User => HKEY_CURRENT_USER,
        })
    }

    impl Registry for WinRegistry {
        fn get(&self, v: &RegValue) -> Option<String> {
            let key = hive(v).open_subkey_with_flags(&v.key, KEY_READ).ok()?;
            let raw = key.get_raw_value(&v.name).ok()?;
            let other = || Some("(another type)".to_string());
            match (v.kind, &raw.vtype) {
                (Kind::Dword, RegType::REG_DWORD) => key.get_value::<u32, _>(&v.name).ok().map(|n| n.to_string()).or_else(other),
                (Kind::Sz, RegType::REG_SZ) => key.get_value::<String, _>(&v.name).ok().or_else(other),
                _ => other(),
            }
        }

        fn set(&self, v: &RegValue, data: Option<&str>) -> Result<(), String> {
            let io = |e: std::io::Error| e.to_string();
            let Some(data) = data else {
                // Already gone, key and all, is the result we wanted.
                let Ok(key) = hive(v).open_subkey_with_flags(&v.key, KEY_SET_VALUE) else { return Ok(()) };
                return match key.delete_value(&v.name) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(e)),
                    _ => Ok(()),
                };
            };
            let (key, _) = hive(v).create_subkey(&v.key).map_err(io)?;
            match v.kind {
                Kind::Dword => key.set_value(&v.name, &data.parse::<u32>().map_err(|e| e.to_string())?).map_err(io),
                Kind::Sz => key.set_value(&v.name, &data.to_string()).map_err(io),
            }
        }
    }

    /// Start this same exe through the Windows permission prompt to write the whole-PC values,
    /// and wait for it. Only the action and the profile id cross over.
    pub fn elevate(action: Action, scope: &str) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_CANCELLED};
        use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
        use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};

        let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
        let exe = wide(std::env::current_exe().map_err(|e| e.to_string())?.as_os_str());
        // Both words are checked ids (lowercase letters, digits, hyphens), so no quoting is needed.
        let params = wide(format!("--tweaks {} {scope}", action.word()).as_ref());
        let verb = wide("runas".as_ref());
        // SAFETY: a zeroed SHELLEXECUTEINFOW with cbSize set is the documented call; the three
        // strings are NUL-terminated and outlive it, and the process handle is closed below.
        unsafe {
            let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
            info.fMask = SEE_MASK_NOCLOSEPROCESS;
            info.lpVerb = verb.as_ptr();
            info.lpFile = exe.as_ptr();
            info.lpParameters = params.as_ptr();
            if ShellExecuteExW(&mut info) == 0 || info.hProcess.is_null() {
                return Err(if GetLastError() == ERROR_CANCELLED {
                    "You said no at the Windows permission prompt, so the whole-PC tweaks weren't changed.".into()
                } else {
                    "Windows couldn't start the step that needs administrator rights.".into()
                });
            }
            WaitForSingleObject(info.hProcess, INFINITE);
            let mut code = 1u32;
            GetExitCodeProcess(info.hProcess, &mut code);
            CloseHandle(info.hProcess);
            if code != 0 {
                return Err("Windows refused some of the changes that need administrator rights.".into());
            }
        }
        Ok(())
    }
}

/// Off Windows there is no registry: everything reads as not set, and nothing can be written.
#[cfg(not(windows))]
fn system() -> Box<dyn Registry> {
    struct None_;
    impl Registry for None_ {
        fn get(&self, _: &RegValue) -> Option<String> {
            None
        }
        fn set(&self, _: &RegValue, _: Option<&str>) -> Result<(), String> {
            Err("Windows tweaks only work on Windows.".into())
        }
    }
    Box::new(None_)
}

#[cfg(not(windows))]
fn elevate(_: Action, _: &str) -> Result<(), String> {
    Err("Windows tweaks only work on Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// A registry in memory. `locked` makes whole-PC writes fail, like an unelevated app.
    #[derive(Default)]
    struct Fake {
        values: RefCell<HashMap<String, String>>,
        locked: bool,
    }

    fn at(v: &RegValue) -> String {
        format!("{:?}\\{}\\{}", v.hive, v.key, v.name).to_ascii_lowercase()
    }

    impl Registry for Fake {
        fn get(&self, v: &RegValue) -> Option<String> {
            self.values.borrow().get(&at(v)).cloned()
        }
        fn set(&self, v: &RegValue, data: Option<&str>) -> Result<(), String> {
            if self.locked && v.hive == Hive::Machine {
                return Err("Access is denied.".into());
            }
            match data {
                Some(d) => self.values.borrow_mut().insert(at(v), d.to_string()),
                None => self.values.borrow_mut().remove(&at(v)),
            };
            Ok(())
        }
    }

    fn file(values: serde_json::Value) -> TweakFile {
        serde_json::from_value(serde_json::json!({
            "profiles": [{ "id": "p", "name": "P", "summary": "s", "tweaks": ["t"] }],
            "tweaks": [{ "id": "t", "name": "T", "why": "w", "values": values }]
        }))
        .unwrap()
    }

    fn one(hive: &str, key: &str, name: &str, kind: &str, set: &str) -> TweakFile {
        file(serde_json::json!([{ "hive": hive, "key": key, "name": name, "type": kind, "set": set, "default": null }]))
    }

    #[test]
    fn the_shipped_tweak_list_is_valid() {
        let f = load().unwrap();
        assert!(f.profiles.iter().any(|p| p.id == "esports"));
        assert!(f.profiles.iter().any(|p| p.id == "casual"));
        assert!(f.profiles.iter().any(|p| p.id == "security"));
        let used: HashSet<&String> = f.profiles.iter().flat_map(|p| &p.tweaks).collect();
        for t in &f.tweaks {
            assert!(used.contains(&t.id), "{} is in no profile", t.id);
        }
    }

    #[test]
    fn keys_outside_the_allowed_roots_are_rejected() {
        let ok = [
            ("HKLM", "SYSTEM\\CurrentControlSet\\Control\\PriorityControl", "Win32PrioritySeparation", "dword", "38"),
            ("HKLM", "system\\currentcontrolset\\control\\prioritycontrol", "Win32PrioritySeparation", "dword", "38"),
            ("HKLM", "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Multimedia\\SystemProfile\\Tasks\\Games", "Scheduling Category", "sz", "High"),
            ("HKLM", "SYSTEM\\CurrentControlSet\\Services\\SysMain", "Start", "dword", "4"),
            ("HKLM", "SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters", "TcpAckFrequency", "dword", "1"),
            ("HKCU", "Control Panel\\Mouse", "MouseSpeed", "sz", "0"),
        ];
        for (hive, key, name, kind, set) in ok {
            one(hive, key, name, kind, set).validate().unwrap_or_else(|e| panic!("{key}\\{name}: {e}"));
        }
        let bad = [
            // Places that start programs.
            ("HKLM", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", "x", "sz", "calc"),
            ("HKCU", "Software\\Microsoft\\Windows\\CurrentVersion\\Run", "x", "sz", "calc"),
            ("HKLM", "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Image File Execution Options\\cs2.exe", "Debugger", "sz", "calc"),
            ("HKLM", "SYSTEM\\CurrentControlSet\\Control\\Session Manager", "BootExecute", "sz", "x"),
            // A root's name as a prefix of another key is not that root.
            ("HKLM", "SYSTEM\\CurrentControlSet\\Control\\PriorityControlEvil", "x", "dword", "1"),
            ("HKLM", "SYSTEM\\CurrentControlSet\\Control\\PriorityControl\\..\\Lsa", "x", "dword", "1"),
            ("HKLM", "SYSTEM\\CurrentControlSet\\Control\\PriorityControl\\", "x", "dword", "1"),
            // A HKCU root is not allowed in HKLM.
            ("HKLM", "Control Panel\\Mouse", "MouseSpeed", "sz", "0"),
            // Whole-PC text only under MMCSS; anything path-like never.
            ("HKLM", "SYSTEM\\CurrentControlSet\\Control\\Power", "x", "sz", "High"),
            ("HKCU", "Control Panel\\Desktop", "SCRNSAVE.EXE", "sz", "C:\\evil.scr"),
            ("HKCU", "Control Panel\\Desktop", "SCRNSAVE.EXE", "sz", "evil.scr"),
            // Services: start type only, never a program path or a boot start type.
            ("HKLM", "SYSTEM\\CurrentControlSet\\Services\\SysMain", "ImagePath", "sz", "x"),
            ("HKLM", "SYSTEM\\CurrentControlSet\\Services\\SysMain", "Start", "dword", "0"),
            ("HKLM", "SYSTEM\\CurrentControlSet\\Services\\SysMain", "Type", "dword", "16"),
            ("HKLM", "SYSTEM\\CurrentControlSet\\Services\\SysMain\\Security", "x", "dword", "1"),
            // Numbers are plain decimal that fits 32 bits.
            ("HKCU", "System\\GameConfigStore", "GameDVR_Enabled", "dword", "4294967296"),
            ("HKCU", "System\\GameConfigStore", "GameDVR_Enabled", "dword", "0x1"),
            ("HKCU", "System\\GameConfigStore", "GameDVR_Enabled", "dword", "01"),
            ("HKCU", "System\\GameConfigStore", "GameDVR_Enabled", "dword", "-1"),
        ];
        for (hive, key, name, kind, set) in bad {
            assert!(one(hive, key, name, kind, set).validate().is_err(), "{hive}\\{key}\\{name} = {set} should be rejected");
        }
    }

    #[test]
    fn profiles_must_name_real_tweaks() {
        let mut f = one("HKCU", "Control Panel\\Mouse", "MouseSpeed", "sz", "0");
        f.validate().unwrap();
        f.profiles[0].tweaks.push("missing".into());
        assert!(f.validate().is_err());
        f.profiles[0].tweaks.pop();
        f.profiles[0].id = ALL.into();
        assert!(f.validate().is_err(), "`all` is the reset-everything scope, not a profile");
        let empty = file(serde_json::json!([{ "hive": "HKCU", "key": "Control Panel\\Mouse", "name": "MouseSpeed", "type": "sz", "set": null, "default": null }]));
        assert!(empty.validate().is_err(), "a value needs something to write");
    }

    fn mixed() -> TweakFile {
        let f = file(serde_json::json!([
            { "hive": "HKCU", "key": "Control Panel\\Mouse", "name": "MouseSpeed", "type": "sz", "set": "0", "default": "1" },
            { "hive": "HKCU", "key": "Software\\Microsoft\\GameBar", "name": "AutoGameModeEnabled", "type": "dword", "set": "1", "default": null },
            { "hive": "HKLM", "key": "SYSTEM\\CurrentControlSet\\Control\\PriorityControl", "name": "Win32PrioritySeparation", "type": "dword", "set": "38", "default": "2" }
        ]));
        f.validate().unwrap();
        f
    }

    #[test]
    fn apply_then_reset_round_trips_and_reports_state() {
        let f = mixed();
        let reg = Fake::default();
        reg.set(&f.tweaks[0].values[0], Some("1")).unwrap();
        reg.set(&f.tweaks[0].values[2], Some("2")).unwrap();
        assert_eq!(f.view(&reg)[0].tweaks[0].state, "default", "a stock PC");
        assert_eq!(f.view(&reg)[0].applied, 0);

        assert_eq!(f.write(&reg, "p", Action::Apply, Hive::User).unwrap(), 2);
        assert_eq!(f.view(&reg)[0].tweaks[0].state, "other", "only the player's half is done");
        assert_eq!(f.pending(&reg, "p", Action::Apply, Hive::Machine).unwrap(), 1);
        assert_eq!(f.write(&reg, "p", Action::Apply, Hive::Machine).unwrap(), 1);
        let view = &f.view(&reg)[0];
        assert_eq!((view.applied, view.total, view.admin), (1, 1, true));
        assert_eq!(view.tweaks[0].values[1].now, "1");
        assert_eq!(f.write(&reg, "p", Action::Apply, Hive::User).unwrap(), 0, "nothing left to change");

        // Reset writes the Windows defaults and removes values Windows doesn't have.
        assert_eq!(f.write(&reg, ALL, Action::Reset, Hive::User).unwrap(), 2);
        assert_eq!(f.write(&reg, ALL, Action::Reset, Hive::Machine).unwrap(), 1);
        assert_eq!(reg.get(&f.tweaks[0].values[0]).as_deref(), Some("1"));
        assert_eq!(reg.get(&f.tweaks[0].values[1]), None);
        assert_eq!(reg.get(&f.tweaks[0].values[2]).as_deref(), Some("2"));
        assert_eq!(f.view(&reg)[0].tweaks[0].state, "default");
    }

    #[test]
    fn reset_undoes_values_the_app_never_wrote() {
        // A PC the playbook tuned: the app has no record, and needs none.
        let f = mixed();
        let reg = Fake::default();
        for v in &f.tweaks[0].values {
            reg.set(v, v.set.as_deref()).unwrap();
        }
        assert_eq!(f.view(&reg)[0].tweaks[0].state, "applied");
        assert_eq!(f.write(&reg, ALL, Action::Reset, Hive::User).unwrap() + f.write(&reg, ALL, Action::Reset, Hive::Machine).unwrap(), 3);
        assert_eq!(f.view(&reg)[0].tweaks[0].state, "default");
    }

    #[test]
    fn scopes_and_failures() {
        let f = mixed();
        let reg = Fake { locked: true, ..Fake::default() };
        assert!(f.write(&reg, "nope", Action::Apply, Hive::User).unwrap_err().contains("Unknown profile"));
        assert!(f.write(&reg, ALL, Action::Apply, Hive::User).is_err(), "apply needs a profile");
        let err = f.write(&reg, "p", Action::Apply, Hive::Machine).unwrap_err();
        assert!(err.contains("Win32PrioritySeparation") && err.contains("denied"), "{err}");
        assert_eq!(reg.get(&f.tweaks[0].values[2]), None, "a refused write changes nothing");
    }

    #[test]
    fn only_the_tweaks_command_line_starts_the_elevated_job() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(elevated_entry(&args(&["app.exe"])), None, "a normal start");
        assert_eq!(elevated_entry(&args(&["app.exe", "--other", "apply", "esports"])), None);
        assert_eq!(elevated_entry(&args(&["app.exe", "--tweaks", "apply"])), None);
        assert_eq!(elevated_entry(&args(&["app.exe", "--tweaks", "delete", "esports"])), Some(1), "unknown action");
        assert_eq!(elevated_entry(&args(&["app.exe", "--tweaks", "apply", "not-a-profile"])), Some(1));
        assert_eq!(elevated_entry(&args(&["app.exe", "--tweaks", "apply", "all"])), Some(1), "apply needs a profile");
    }
}
