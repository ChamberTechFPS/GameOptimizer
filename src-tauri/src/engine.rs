//! Scan, apply, and revert. Every write the app makes to a game file happens here, after the
//! catalog passed its signature check, and only to the keys that game lists.

use crate::catalog::{self, Catalog, FileTarget, Format, Game, ManualSetting, Proof, Setting};
use crate::detect::Pc;
use crate::patch;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Engine {
    /// %LOCALAPPDATA%\ChamberTech\GameOptimizer: state.json, downloaded catalog, backups.
    pub data: PathBuf,
    /// The catalog that shipped inside the installer.
    pub bundled: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct State {
    /// Highest catalog version installed. Older downloads are refused (no rollbacks).
    pub catalog_version: u64,
    pub games: BTreeMap<String, GameState>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GameState {
    /// Folder the player picked; wins over auto-detection while it exists.
    pub folder: Option<PathBuf>,
    pub hidden: bool,
    pub applied: Option<Applied>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub rev: String,
    pub at: u64,
    pub files: Vec<AppliedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedFile {
    pub path: PathBuf,
    pub format: Format,
    /// Copy of the file from before the first apply.
    pub backup: PathBuf,
    /// SHA-256 of what Chamber wrote.
    pub written: String,
    /// True while backup + Chamber's edits == written, i.e. nothing else has touched the file
    /// since the first apply. Then revert can restore the backup byte for byte.
    #[serde(default)]
    pub exact: bool,
    pub was_read_only: bool,
    /// Each listed key's value before Chamber touched it. None means the key wasn't there.
    pub originals: Vec<Original>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Original {
    pub section: Option<String>,
    pub key: String,
    pub value: Option<String>,
}

/// One game as the UI shows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    pub slug: String,
    pub name: String,
    pub tile: String,
    pub launcher: String,
    pub found: bool,
    pub location: Option<String>,
    pub folder: Option<String>,
    pub hidden: bool,
    pub status: &'static str,
    pub running: bool,
    pub total: usize,
    pub differ: usize,
    pub applied_at: Option<u64>,
    pub files: Vec<String>,
    pub lock: bool,
    pub settings: Vec<Row>,
    pub manual: Vec<ManualSetting>,
    pub proof: Option<Proof>,
}

#[derive(Debug, Serialize)]
pub struct Row {
    pub label: String,
    pub now: String,
    pub want: String,
    #[serde(rename = "match")]
    pub matched: bool,
    pub why: String,
}

impl Engine {
    fn state_path(&self) -> PathBuf {
        self.data.join("state.json")
    }

    pub fn load_state(&self) -> Result<State, String> {
        match fs::read(self.state_path()) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                format!("Chamber's state file is damaged ({e}). Your backups are safe in {}", self.data.join("backups").display())
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(format!("Couldn't read Chamber's state file: {e}")),
        }
    }

    fn save_state(&self, state: &State) -> Result<(), String> {
        fs::create_dir_all(&self.data).map_err(io)?;
        let bytes = serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?;
        patch::write_atomic(&self.state_path(), &bytes).map_err(|e| format!("Couldn't save Chamber's state: {e}"))
    }

    /// The newest catalog that passes its signature check, downloaded or bundled.
    pub fn catalog(&self) -> Result<Catalog, String> {
        let load = |dir: &Path| -> Result<Catalog, String> {
            let json = fs::read(dir.join("catalog.json")).map_err(io)?;
            let sig = fs::read(dir.join("catalog.json.sig")).map_err(io)?;
            catalog::parse_verified(&json, &sig)
        };
        match (load(&self.data), load(&self.bundled)) {
            (Ok(a), Ok(b)) => Ok(if a.version >= b.version { a } else { b }),
            (Ok(c), Err(_)) | (Err(_), Ok(c)) => Ok(c),
            (Err(_), Err(e)) => Err(format!("No verified config catalog found ({e})")),
        }
    }

    /// Keep a catalog the UI downloaded, if it verifies and is newer than anything seen before.
    pub fn install_catalog(&self, json: &[u8], sig: &[u8]) -> Result<&'static str, String> {
        let new = catalog::parse_verified(json, sig)?;
        let mut state = self.load_state()?;
        let current = self.catalog().map(|c| c.version).unwrap_or(0).max(state.catalog_version);
        if new.version < current {
            return Err(format!("Refused an older config catalog ({} is older than {current})", new.version));
        }
        if new.version == current {
            return Ok("current");
        }
        fs::create_dir_all(&self.data).map_err(io)?;
        patch::write_atomic(&self.data.join("catalog.json.sig"), sig).map_err(io)?;
        patch::write_atomic(&self.data.join("catalog.json"), json).map_err(io)?;
        state.catalog_version = new.version;
        self.save_state(&state)?;
        Ok("updated")
    }

    /// Install folder and the launcher it was found through: the player's pick, then launcher
    /// records, then the game's own folders. An empty path means installed, location unknown.
    fn locate(&self, g: &Game, gs: Option<&GameState>, pc: &Pc) -> Option<(PathBuf, String)> {
        if let Some(folder) = gs.and_then(|s| s.folder.as_ref()).filter(|f| f.is_dir()) {
            return Some((folder.clone(), g.launcher.clone()));
        }
        if let Some((location, via)) = pc.install_location(g) {
            return Some((location, via.unwrap_or(&g.launcher).to_string()));
        }
        let roots = roots(pc, None);
        let found = g.find.paths.iter().filter_map(|p| catalog::expand(p, &roots).ok()).find(|p| p.exists());
        found.map(|p| (p, g.launcher.clone()))
    }

    pub fn scan(&self, cat: &Catalog, pc: &Pc) -> Result<Vec<GameView>, String> {
        let state = self.load_state()?;
        Ok(cat.games.iter().map(|g| self.view(g, state.games.get(&g.slug), pc)).collect())
    }

    fn view(&self, g: &Game, gs: Option<&GameState>, pc: &Pc) -> GameView {
        let (location, launcher) = match self.locate(g, gs, pc) {
            Some((path, via)) => (Some(path), via),
            None => (None, g.launcher.clone()),
        };
        let roots = roots(pc, location.as_deref());
        let applied = gs.and_then(|s| s.applied.as_ref());
        let mut rows = Vec::new();
        let mut missing_file = false;
        for f in &g.files {
            let texts: Vec<String> = catalog::expand_all(&f.path, &roots)
                .unwrap_or_default()
                .iter()
                .filter_map(|p| patch::decode(&fs::read(p).ok()?).ok())
                .map(|(_, t)| t)
                .collect();
            missing_file |= texts.is_empty();
            for s in &f.settings {
                let want = catalog::fill(&s.value, &pc.vars);
                // With a copy per account, show the first copy that still differs.
                let nows: Vec<Option<String>> = texts.iter().map(|t| get(f.format, t, s)).collect();
                let now = nows
                    .iter()
                    .find(|n| !n.as_deref().is_some_and(|v| patch::same_value(v, &want)))
                    .or(nows.first())
                    .cloned()
                    .flatten();
                rows.push(Row {
                    label: s.label.clone(),
                    matched: now.as_deref().is_some_and(|n| patch::same_value(n, &want)),
                    now: show(now.as_deref(), s),
                    want: show(Some(&want), s),
                    why: s.why.clone(),
                });
            }
        }
        let differ = rows.iter().filter(|r| !r.matched).count();
        let status = match (&location, applied) {
            (None, _) => "not-found",
            _ if !g.manual.is_empty() => "manual",
            _ if missing_file => "launch-once",
            (_, Some(_)) if differ > 0 => "changed",
            (_, Some(a)) if a.rev != g.rev => "update",
            (_, None) if differ > 0 => "not-optimized",
            _ => "optimized",
        };
        GameView {
            slug: g.slug.clone(),
            name: g.name.clone(),
            tile: g.tile.clone(),
            launcher,
            found: location.is_some(),
            location: location.filter(|l| !l.as_os_str().is_empty()).map(|l| l.display().to_string()),
            folder: gs.and_then(|s| s.folder.as_ref()).map(|f| f.display().to_string()),
            hidden: gs.is_some_and(|s| s.hidden),
            status,
            running: pc.is_running(g),
            total: rows.len(),
            differ,
            applied_at: applied.map(|a| a.at),
            files: g.files.iter().map(|f| f.path.rsplit('/').next().unwrap_or_default().to_string()).collect(),
            lock: g.files.iter().any(|f| f.lock),
            settings: rows,
            manual: g.manual.clone(),
            proof: g.proof.clone(),
        }
    }

    /// Apply a game's listed settings. Returns how many values actually changed.
    pub fn apply(&self, cat: &Catalog, pc: &Pc, slug: &str) -> Result<usize, String> {
        let g = cat.game(slug)?;
        if !g.manual.is_empty() {
            return Err(format!("{} keeps its settings on its servers. Use the checklist instead.", g.name));
        }
        if pc.is_running(g) {
            return Err(format!("Close {} first, then try again.", g.name));
        }
        let mut state = self.load_state()?;
        let gs = state.games.entry(slug.to_string()).or_default();
        let (location, _) = self
            .locate(g, Some(gs), pc)
            .ok_or_else(|| format!("{} isn't installed here. Set its folder in Library.", g.name))?;
        let roots = roots(pc, Some(&location));
        let mut targets = Vec::new();
        for f in &g.files {
            let paths = catalog::expand_all(&f.path, &roots)?;
            if paths.is_empty() {
                return Err(launch_once(g));
            }
            targets.extend(paths.into_iter().map(|p| (f, p)));
        }
        let previous = gs.applied.take();
        let stamp = now();
        let mut files = Vec::new();
        let mut changed = 0;
        let mut result = Ok(());
        for (f, path) in targets {
            match self.apply_file(g, f, path, pc, previous.as_ref(), stamp) {
                Ok((record, n)) => {
                    files.push(record);
                    changed += n;
                }
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        // Record whatever was written, even after a failure, so it all stays revertible.
        let rev = if result.is_ok() { g.rev.clone() } else { previous.as_ref().map(|p| p.rev.clone()).unwrap_or_default() };
        for old in previous.into_iter().flat_map(|p| p.files) {
            if !files.iter().any(|f| f.path == old.path) {
                files.push(old);
            }
        }
        gs.applied = (!files.is_empty()).then_some(Applied { rev, at: stamp, files });
        self.save_state(&state)?;
        result.map(|_| changed)
    }

    fn apply_file(
        &self,
        g: &Game,
        f: &FileTarget,
        path: PathBuf,
        pc: &Pc,
        previous: Option<&Applied>,
        stamp: u64,
    ) -> Result<(AppliedFile, usize), String> {
        let bytes = fs::read(&path).map_err(|_| launch_once(g))?;
        let (encoding, text) = patch::decode(&bytes).map_err(|e| format!("{} {e}.", file_name(&path)))?;
        let prev = previous.and_then(|a| a.files.iter().find(|x| x.path == path));

        // The first apply's values stay the originals: a re-apply must never record Chamber's own values.
        let mut originals = prev.map(|p| p.originals.clone()).unwrap_or_default();
        let mut next = text;
        let mut changed = 0;
        for s in &f.settings {
            let want = catalog::fill(&s.value, &pc.vars);
            let now = get(f.format, &next, s);
            if !originals.iter().any(|o| o.section == s.section && o.key.eq_ignore_ascii_case(&s.key)) {
                originals.push(Original { section: s.section.clone(), key: s.key.clone(), value: now.clone() });
            }
            if !now.as_deref().is_some_and(|n| patch::same_value(n, &want)) {
                next = set(f.format, &next, s.section.as_deref(), &s.key, &want)?;
                changed += 1;
            }
        }

        let backup = match prev.map(|p| p.backup.clone()).filter(|b| b.exists()) {
            Some(b) => b,
            None => {
                let dir = self.data.join("backups").join(&g.slug).join(stamp.to_string());
                fs::create_dir_all(&dir).map_err(io)?;
                // Copies that share a name (one per account) each get their own backup.
                let name = file_name(&path);
                let mut b = dir.join(&name);
                let mut n = 1;
                while b.exists() {
                    n += 1;
                    b = dir.join(format!("{n}-{name}"));
                }
                fs::write(&b, &bytes).map_err(|e| format!("Couldn't back up {}, so nothing was changed: {e}", file_name(&path)))?;
                b
            }
        };
        let was_read_only = prev.map_or_else(|| patch::is_read_only(&path), |p| p.was_read_only);
        // A re-apply over a file the game or player changed makes the old backup stale.
        let exact = prev.is_none_or(|p| p.exact && sha256_hex(&bytes) == p.written);
        let out = patch::encode(encoding, &next);
        if out != bytes {
            write_file(&path, &out, f.lock)?;
        } else if f.lock {
            patch::set_read_only(&path, true).map_err(io)?;
        }
        Ok((AppliedFile { path, format: f.format, backup, written: sha256_hex(&out), exact, was_read_only, originals }, changed))
    }

    /// Put back what Chamber changed. If nothing else touched the file, it comes back byte for byte.
    pub fn revert(&self, cat: &Catalog, pc: &Pc, slug: &str) -> Result<(), String> {
        let g = cat.game(slug)?;
        if pc.is_running(g) {
            return Err(format!("Close {} first, then try again.", g.name));
        }
        let mut state = self.load_state()?;
        let gs = state.games.get_mut(slug);
        let Some(applied) = gs.as_ref().and_then(|s| s.applied.clone()) else {
            return Err(format!("Nothing to revert for {}.", g.name));
        };
        for f in &applied.files {
            let Ok(bytes) = fs::read(&f.path) else { continue }; // file is gone: nothing to put back
            let out = if f.exact && sha256_hex(&bytes) == f.written && f.backup.exists() {
                fs::read(&f.backup).map_err(io)?
            } else {
                // The game or player changed other things since; undo only Chamber's keys.
                let (encoding, mut text) = patch::decode(&bytes).map_err(|e| format!("{} {e}.", file_name(&f.path)))?;
                for o in &f.originals {
                    text = match &o.value {
                        Some(v) => set(f.format, &text, o.section.as_deref(), &o.key, v)?,
                        None => remove(f.format, &text, o.section.as_deref(), &o.key),
                    };
                }
                patch::encode(encoding, &text)
            };
            write_file(&f.path, &out, f.was_read_only)?;
        }
        if let Some(gs) = gs {
            gs.applied = None;
        }
        self.save_state(&state)
    }

    pub fn set_folder(&self, cat: &Catalog, slug: &str, folder: &Path) -> Result<(), String> {
        cat.game(slug)?;
        if !folder.is_absolute() || !folder.is_dir() {
            return Err("That folder doesn't exist.".into());
        }
        self.update(slug, |gs| gs.folder = Some(folder.to_path_buf()))
    }

    pub fn clear_folder(&self, cat: &Catalog, slug: &str) -> Result<(), String> {
        cat.game(slug)?;
        self.update(slug, |gs| gs.folder = None)
    }

    pub fn set_hidden(&self, cat: &Catalog, slug: &str, hidden: bool) -> Result<(), String> {
        cat.game(slug)?;
        self.update(slug, |gs| gs.hidden = hidden)
    }

    fn update(&self, slug: &str, change: impl FnOnce(&mut GameState)) -> Result<(), String> {
        let mut state = self.load_state()?;
        change(state.games.entry(slug.to_string()).or_default());
        self.save_state(&state)
    }

    /// The game's settings folder, or its backups folder.
    pub fn folder_for(&self, cat: &Catalog, pc: &Pc, slug: &str, which: &str) -> Result<PathBuf, String> {
        let g = cat.game(slug)?;
        if which == "backups" {
            let dir = self.data.join("backups").join(slug);
            return if dir.is_dir() { Ok(dir) } else { Err("No backups yet.".into()) };
        }
        let state = self.load_state()?;
        let location = self.locate(g, state.games.get(slug), pc).map(|(path, _)| path);
        let roots = roots(pc, location.as_deref());
        g.files
            .iter()
            .filter_map(|f| catalog::expand_all(&f.path, &roots).ok()?.first()?.parent().map(Path::to_path_buf))
            .find(|p| p.is_dir())
            .or(location.filter(|l| l.is_dir()))
            .ok_or_else(|| format!("Couldn't find {}'s settings folder.", g.name))
    }
}

fn roots(pc: &Pc, install: Option<&Path>) -> HashMap<&'static str, PathBuf> {
    let mut roots = pc.roots.clone();
    // Only a real absolute folder can stand in for {install}; never a blank or relative path.
    if let Some(dir) = install.filter(|d| d.is_absolute()) {
        roots.insert("install", dir.to_path_buf());
    }
    roots
}

fn get(format: Format, text: &str, s: &Setting) -> Option<String> {
    match format {
        Format::Ini => patch::ini_get(text, s.section.as_deref().unwrap_or_default(), &s.key),
        Format::Kv => patch::kv_get(text, &s.key),
        Format::Cod => patch::cod_get(text, &s.key),
        Format::Convar => patch::convar_get(text, &s.key),
        Format::Spaced => patch::spaced_get(text, &s.key),
        Format::Json => patch::json_get(text, &s.key),
    }
}

fn set(format: Format, text: &str, section: Option<&str>, key: &str, value: &str) -> Result<String, String> {
    match format {
        Format::Ini => Ok(patch::ini_set(text, section.unwrap_or_default(), key, value)),
        Format::Kv => patch::kv_set(text, key, value),
        Format::Cod => patch::cod_set(text, key, value),
        Format::Convar => Ok(patch::convar_set(text, key, value)),
        Format::Spaced => Ok(patch::spaced_set(text, key, value)),
        Format::Json => patch::json_set(text, key, value),
    }
}

fn remove(format: Format, text: &str, section: Option<&str>, key: &str) -> String {
    match format {
        Format::Ini => patch::ini_remove(text, section.unwrap_or_default(), key),
        Format::Kv => patch::kv_remove(text, key),
        Format::Cod => patch::cod_remove(text, key),
        Format::Convar => patch::convar_remove(text, key),
        Format::Spaced => patch::spaced_remove(text, key),
        Format::Json => patch::json_remove(text, key),
    }
}

/// Display text: the setting's own name for a value ("0" -> "Off"), else the tidied raw value.
fn show(raw: Option<&str>, s: &Setting) -> String {
    let Some(raw) = raw else { return "Not set".into() };
    if let Some((_, name)) = s.values.iter().find(|(k, _)| patch::same_value(k, raw)) {
        return name.clone();
    }
    let tidy = raw.trim();
    if tidy.contains('.') && tidy.parse::<f64>().is_ok() {
        return tidy.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    tidy.to_string()
}

fn write_file(path: &Path, bytes: &[u8], read_only: bool) -> Result<(), String> {
    let _ = patch::set_read_only(path, false);
    patch::write_atomic(path, bytes)
        .map_err(|_| format!("Couldn't write {}. If the game is running, close it and try again.", file_name(path)))?;
    if read_only {
        patch::set_read_only(path, true).map_err(io)?;
    }
    Ok(())
}

fn launch_once(g: &Game) -> String {
    format!("Launch {} once so it creates its settings file, then try again.", g.name)
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or_default()
}

fn io(e: std::io::Error) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Vars;

    const ORIGINAL: &str = "[Video]\r\nShadows=3\r\nVSync=True\r\nName=Player One\r\n\r\n[Audio]\r\nVolume=80\r\n";

    struct Fixture {
        root: PathBuf,
        engine: Engine,
        cat: Catalog,
        pc: Pc,
        file: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = patch::set_read_only(&self.file, false);
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn fixture(name: &str) -> Fixture {
        let root = std::env::temp_dir().join(format!("chamber-engine-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let local = root.join("local");
        fs::create_dir_all(local.join("TestGame")).unwrap();
        let file = local.join("TestGame/settings.ini");
        fs::write(&file, ORIGINAL).unwrap();
        let game: Game = serde_json::from_value(serde_json::json!({
            "slug": "test-game", "name": "Test Game", "tile": "TG", "launcher": "other",
            "find": { "paths": ["{localappdata}/TestGame"] },
            "process": ["TestGame.exe"],
            "files": [{
                "path": "{localappdata}/TestGame/settings.ini", "format": "ini", "lock": true,
                "settings": [
                    { "section": "Video", "key": "Shadows", "value": "0", "label": "Shadows", "values": { "0": "Off", "3": "High" }, "why": "w" },
                    { "section": "Video", "key": "VSync", "value": "False", "label": "V-Sync", "why": "w" },
                    { "section": "Video", "key": "FrameRateLimit", "value": "{refresh}.000000", "label": "Frame rate limit", "why": "w" },
                    { "section": "Latency", "key": "Reflex", "value": "2", "label": "Reflex", "why": "w" }
                ]
            }],
            "rev": "r1"
        }))
        .unwrap();
        let cat = Catalog { version: 1, games: vec![game] };
        cat.validate().unwrap();
        let pc = Pc {
            roots: HashMap::from([("localappdata", local)]),
            vars: Vars { refresh: 165, width: 2560, height: 1440 },
            ..Pc::default()
        };
        let engine = Engine { data: root.join("data"), bundled: root.join("bundled") };
        Fixture { root, engine, cat, pc, file }
    }

    fn status(fx: &Fixture) -> (&'static str, usize) {
        let v = &fx.engine.scan(&fx.cat, &fx.pc).unwrap()[0];
        (v.status, v.differ)
    }

    #[test]
    fn apply_changes_only_listed_keys_then_revert_restores_bytes() {
        let fx = fixture("roundtrip");
        assert_eq!(status(&fx), ("not-optimized", 4));
        let view = &fx.engine.scan(&fx.cat, &fx.pc).unwrap()[0];
        assert_eq!((view.settings[0].now.as_str(), view.settings[0].want.as_str()), ("High", "Off"));
        assert_eq!(view.settings[2].want, "165", "floats display tidy");

        assert_eq!(fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap(), 4);
        let text = fs::read_to_string(&fx.file).unwrap();
        assert!(text.starts_with("[Video]\r\nShadows=0\r\nVSync=False\r\nName=Player One\r\nFrameRateLimit=165.000000\r\n\r\n[Audio]\r\nVolume=80\r\n"));
        assert!(text.ends_with("\r\n\r\n[Latency]\r\nReflex=2\r\n"));
        assert!(patch::is_read_only(&fx.file), "locked after apply");
        assert_eq!(status(&fx), ("optimized", 0));

        fx.engine.revert(&fx.cat, &fx.pc, "test-game").unwrap();
        assert_eq!(fs::read_to_string(&fx.file).unwrap(), ORIGINAL, "untouched since apply: exact bytes back");
        assert!(!patch::is_read_only(&fx.file));
        assert_eq!(status(&fx), ("not-optimized", 4));
    }

    #[test]
    fn drift_reapply_and_key_level_revert_keep_the_players_own_changes() {
        let fx = fixture("drift");
        fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap();

        // The game resets one of Chamber's keys and the player changes their own volume.
        patch::set_read_only(&fx.file, false).unwrap();
        let text = fs::read_to_string(&fx.file).unwrap().replace("Shadows=0", "Shadows=3").replace("Volume=80", "Volume=55");
        fs::write(&fx.file, text).unwrap();
        assert_eq!(status(&fx), ("changed", 1));

        assert_eq!(fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap(), 1);
        assert_eq!(status(&fx), ("optimized", 0));

        fx.engine.revert(&fx.cat, &fx.pc, "test-game").unwrap();
        let text = fs::read_to_string(&fx.file).unwrap();
        // Originals come from the first apply, not the re-apply.
        assert_eq!(patch::ini_get(&text, "Video", "Shadows").as_deref(), Some("3"));
        assert_eq!(patch::ini_get(&text, "Video", "VSync").as_deref(), Some("True"));
        assert_eq!(patch::ini_get(&text, "Video", "FrameRateLimit"), None, "keys Chamber added are removed");
        assert_eq!(patch::ini_get(&text, "Latency", "Reflex"), None);
        assert_eq!(patch::ini_get(&text, "Audio", "Volume").as_deref(), Some("55"), "player's own change survives");
        assert_eq!(patch::ini_get(&text, "Video", "Name").as_deref(), Some("Player One"));
    }

    #[test]
    fn running_games_are_never_written() {
        let mut fx = fixture("running");
        fx.pc.running.insert("testgame.exe".into());
        let err = fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap_err();
        assert!(err.contains("Close Test Game"), "{err}");
        assert_eq!(fs::read_to_string(&fx.file).unwrap(), ORIGINAL);
        assert!(fx.engine.scan(&fx.cat, &fx.pc).unwrap()[0].running);
    }

    #[test]
    fn a_new_config_rev_shows_as_an_update() {
        let mut fx = fixture("rev");
        fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap();
        fx.cat.games[0].rev = "r2".into();
        assert_eq!(status(&fx), ("update", 0));
    }

    #[test]
    fn missing_files_and_missing_games() {
        let fx = fixture("missing");
        patch::set_read_only(&fx.file, false).unwrap();
        fs::remove_file(&fx.file).unwrap();
        assert_eq!(status(&fx).0, "launch-once");
        assert!(fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap_err().contains("Launch Test Game once"));
        fs::remove_dir_all(fx.file.parent().unwrap()).unwrap();
        assert_eq!(status(&fx).0, "not-found");
    }

    #[test]
    fn library_choices_persist() {
        let fx = fixture("library");
        fx.engine.set_hidden(&fx.cat, "test-game", true).unwrap();
        fx.engine.set_folder(&fx.cat, "test-game", &fx.root).unwrap();
        let v = &fx.engine.scan(&fx.cat, &fx.pc).unwrap()[0];
        assert!(v.hidden);
        assert_eq!(v.folder.as_deref(), Some(fx.root.display().to_string().as_str()));
        assert!(fx.engine.set_folder(&fx.cat, "test-game", Path::new("relative/dir")).is_err());
        assert!(fx.engine.set_hidden(&fx.cat, "not-a-game", true).is_err());
        fx.engine.clear_folder(&fx.cat, "test-game").unwrap();
        assert_eq!(fx.engine.scan(&fx.cat, &fx.pc).unwrap()[0].folder, None);
    }

    #[test]
    fn a_wildcard_folder_covers_every_account() {
        let mut fx = fixture("wildcard");
        let siege = fx.root.join("local/Siege");
        let (a, b) = (siege.join("1111/GameSettings.ini"), siege.join("2222/GameSettings.ini"));
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        fs::create_dir_all(b.parent().unwrap()).unwrap();
        fx.cat.games[0].files = vec![serde_json::from_value(serde_json::json!({
            "path": "{localappdata}/Siege/*/GameSettings.ini", "format": "ini",
            "settings": [{ "section": "DISPLAY", "key": "VSync", "value": "0", "label": "V-Sync", "why": "w" }]
        }))
        .unwrap()];
        fx.cat.validate().unwrap();
        assert_eq!(status(&fx).0, "launch-once", "no account has played yet");
        assert!(fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap_err().contains("Launch Test Game once"));

        fs::write(&a, "[DISPLAY]\r\nVSync=1\r\n").unwrap();
        fs::write(&b, "[DISPLAY]\nVSync=0\nFPS=60\n").unwrap();
        assert_eq!(status(&fx), ("not-optimized", 1), "one account still differs");
        assert_eq!(fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap(), 1);
        assert_eq!(fs::read_to_string(&a).unwrap(), "[DISPLAY]\r\nVSync=0\r\n");
        assert_eq!(status(&fx), ("optimized", 0));
        let state = fx.engine.load_state().unwrap();
        let files = &state.games["test-game"].applied.as_ref().unwrap().files;
        assert_eq!(files.len(), 2, "each copy is recorded for revert");
        assert_ne!(files[0].backup, files[1].backup, "same file name, separate backups");

        fs::write(&b, "[DISPLAY]\nVSync=1\nFPS=60\n").unwrap();
        assert_eq!(status(&fx), ("changed", 1));
        fx.engine.revert(&fx.cat, &fx.pc, "test-game").unwrap();
        assert_eq!(fs::read_to_string(&a).unwrap(), "[DISPLAY]\r\nVSync=1\r\n");
        assert_eq!(fs::read_to_string(&b).unwrap(), "[DISPLAY]\nVSync=0\nFPS=60\n", "the first apply's value comes back");
    }

    #[test]
    fn utf16_files_stay_utf16() {
        let fx = fixture("utf16");
        fs::write(&fx.file, patch::encode(patch::Encoding::Utf16Le, ORIGINAL)).unwrap();
        fx.engine.apply(&fx.cat, &fx.pc, "test-game").unwrap();
        let (encoding, text) = patch::decode(&fs::read(&fx.file).unwrap()).unwrap();
        assert_eq!(encoding, patch::Encoding::Utf16Le);
        assert_eq!(patch::ini_get(&text, "Video", "Shadows").as_deref(), Some("0"));
        fx.engine.revert(&fx.cat, &fx.pc, "test-game").unwrap();
        assert_eq!(fs::read(&fx.file).unwrap(), patch::encode(patch::Encoding::Utf16Le, ORIGINAL));
    }
}
