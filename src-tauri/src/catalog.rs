//! The signed config catalog: data model, validation, path tokens, and the signature check.
//! Everything that arrives over the update channel goes through `parse_verified` before use.

use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::signature::Verifier;
use rsa::{BigUint, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

/// RSA-3072 public key (e = 65537) that signs catalog.json. The private key stays offline
/// with the maintainer, so a compromised repo or GitHub account can't ship configs.
const MODULUS_HEX: &str = "cdb8a11eb62f5182fdc5707fcd873c78302beec9455d6095ba77c9145eaa89281dfd2670934b873d9a920a0f1999c0c07510ff0d1af6750cd926133a15e5778684a2c4bddd0a7efebe386a64b1e929fcbe8a604f238510f739d34b9216159aa7c289e3ca55956bdd552eae79c9a382641d12a021c88a55867fcae58593e6de2ef9981df90dd13aa85e679145a637513753907654007f80277fc4d295a0fec7337d4f7ae464fc6afe79af53a8539b8a3658dc27825f8b71be82491a73a3a38b0eb506f33eaa82ca160f552262baf093cf654aa95a482d5c8294b32c5cd097d2fb5a36fa1a48ec60704bbcdbfa6e83b9f7719acc620b4eddd37a8939c47cc5338d1e6c311aa737024a9c3426d615130b1a710f40ad7852cde09ef71493880eab5abdf4be91d991a6ed4c70298405a1eb6baea894421c7901d8955ecb4a4b233b52f04a189bdfcf0004b24957d85d1b804c71cebfa2210c36d82484ad6aa14e8111902d9c5fcdb8d9b03af4fac72645f6c4541ae6987942c12d0fa11ed793d76d45";

/// Path roots a game file may live under. Anything else is rejected.
pub const TOKENS: [&str; 6] = ["localappdata", "appdata", "documents", "savedgames", "steamuserdata", "install"];
const LAUNCHERS: [&str; 7] = ["steam", "epic", "riot", "battlenet", "ea", "ubisoft", "other"];
const EXTENSIONS: [&str; 6] = ["ini", "txt", "cfg", "vcfg", "txt0", "txt1"];
/// Settings files with no extension, allowed by exact name.
const FILE_NAMES: [&str; 1] = ["PROFSAVE_profile"];
const VARIABLES: [&str; 3] = ["{refresh}", "{width}", "{height}"];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: u64,
    pub games: Vec<Game>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    pub slug: String,
    pub name: String,
    pub tile: String,
    pub launcher: String,
    #[serde(default)]
    pub find: Find,
    #[serde(default)]
    pub process: Vec<String>,
    #[serde(default)]
    pub files: Vec<FileTarget>,
    #[serde(default)]
    pub manual: Vec<ManualSetting>,
    #[serde(default)]
    pub proof: Option<Proof>,
    /// Content hash added by tools/build-catalog.mjs; a change means "New config".
    #[serde(default)]
    pub rev: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Find {
    pub steam: Option<u32>,
    pub epic: Option<String>,
    pub uninstall: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    /// The game's exe relative to its install folder, e.g. `FortniteGame/Binaries/Win64/x.exe`.
    /// Finds the game inside the player's library folders.
    pub exe: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Ini,
    Kv,
    /// Call of Duty's settings file: `Name@0;id;id = value // note` lines.
    Cod,
    /// Console variables, one per line: `graphics.vsync "0"` (Rust).
    Convar,
    /// `Key value` lines (Battlefield).
    Spaced,
    /// JSON, keyed by dotted path: `DisplaySettings.FullScreenMode` (Escape from Tarkov).
    Json,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileTarget {
    pub path: String,
    /// More places the same file can be, such as another launcher's folder. Every copy found
    /// gets the same settings.
    #[serde(default)]
    pub also: Vec<String>,
    pub format: Format,
    #[serde(default)]
    pub lock: bool,
    pub settings: Vec<Setting>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    pub section: Option<String>,
    pub key: String,
    pub value: String,
    pub label: String,
    /// Display names for raw values, e.g. {"0": "Off"}.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    pub why: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManualSetting {
    pub label: String,
    pub value: String,
    pub why: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    pub rig: String,
    pub avg: [u32; 2],
    pub low1: [u32; 2],
    pub method: String,
}

/// Values filled in from the primary monitor at apply time.
#[derive(Debug, Clone, Copy)]
pub struct Vars {
    pub refresh: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for Vars {
    fn default() -> Self {
        Vars { refresh: 60, width: 1920, height: 1080 }
    }
}

pub fn fill(value: &str, vars: &Vars) -> String {
    value
        .replace("{refresh}", &vars.refresh.to_string())
        .replace("{width}", &vars.width.to_string())
        .replace("{height}", &vars.height.to_string())
}

/// Verify the signature, then parse and validate. The only way a catalog gets into the app.
pub fn parse_verified(json: &[u8], sig: &[u8]) -> Result<Catalog, String> {
    if !verify_with(json, sig, MODULUS_HEX) {
        return Err("The config catalog failed its signature check, so it wasn't used.".into());
    }
    let catalog: Catalog =
        serde_json::from_slice(json).map_err(|e| format!("The config catalog is malformed: {e}"))?;
    catalog.validate()?;
    Ok(catalog)
}

fn verify_with(data: &[u8], sig: &[u8], modulus_hex: &str) -> bool {
    let Some(modulus) = from_hex(modulus_hex) else { return false };
    let Ok(key) = RsaPublicKey::new(BigUint::from_bytes_be(&modulus), BigUint::from(65537u32)) else {
        return false;
    };
    let Ok(sig) = Signature::try_from(sig) else { return false };
    VerifyingKey::<Sha256>::new(key).verify(data, &sig).is_ok()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

impl Catalog {
    pub fn game(&self, slug: &str) -> Result<&Game, String> {
        self.games.iter().find(|g| g.slug == slug).ok_or_else(|| format!("Unknown game: {slug}"))
    }

    /// Runtime security boundary: CI runs the same rules, but the app never trusts the repo.
    pub fn validate(&self) -> Result<(), String> {
        let mut slugs = HashSet::new();
        for g in &self.games {
            let fail = |why: String| Err(format!("Config for {:?} rejected: {why}", g.slug));
            if !is_slug(&g.slug) || !slugs.insert(g.slug.as_str()) {
                return fail("bad or duplicate slug".into());
            }
            if !is_text(&g.name) {
                return fail("bad name".into());
            }
            if !(1..=3).contains(&g.tile.len()) || !g.tile.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
                return fail("tile must be 1 to 3 capitals or digits".into());
            }
            if !LAUNCHERS.contains(&g.launcher.as_str()) {
                return fail(format!("unknown launcher {:?}", g.launcher));
            }
            if g.files.is_empty() == g.manual.is_empty() {
                return fail("needs either files or a manual checklist".into());
            }
            if g.find.epic.as_deref().is_some_and(|s| !is_text(s)) || g.find.uninstall.as_deref().is_some_and(|s| !is_text(s)) {
                return fail("bad find name".into());
            }
            for p in &g.find.paths {
                if let Err(e) = check_path(p, false) {
                    return fail(e);
                }
            }
            if let Some(exe) = &g.find.exe {
                let segments: Vec<&str> = exe.split('/').collect();
                let (name, dirs) = segments.split_last().unwrap_or((&"", &[]));
                if !is_exe(name) || !dirs.iter().all(|d| is_segment(d)) {
                    return fail(format!("bad find exe {exe:?}"));
                }
            }
            if let Some(p) = g.process.iter().find(|p| !is_exe(p)) {
                return fail(format!("bad process name {p:?}"));
            }
            for f in &g.files {
                for p in f.paths() {
                    if let Err(e) = check_path(p, true) {
                        return fail(e);
                    }
                }
                if f.settings.is_empty() {
                    return fail(format!("{} lists no settings", f.path));
                }
                for s in &f.settings {
                    let section_ok = match (f.format, &s.section) {
                        (Format::Ini, Some(sec)) => is_section(sec),
                        (format, None) => format != Format::Ini,
                        _ => false,
                    };
                    if !section_ok {
                        return fail(format!("{}: ini settings need a section, other formats take none", s.key));
                    }
                    let values_ok = s.values.iter().all(|(k, v)| is_value(k, f.format) && is_text(v));
                    if !is_key(&s.key, f.format) || !is_value(&s.value, f.format) || !is_text(&s.label) || !is_text(&s.why) || !values_ok {
                        return fail(format!("bad setting {:?}", s.key));
                    }
                }
            }
            if g.manual.iter().any(|m| !is_text(&m.label) || !is_text(&m.value) || !is_text(&m.why)) {
                return fail("bad checklist entry".into());
            }
            if g.proof.as_ref().is_some_and(|p| !is_text(&p.rig) || !is_text(&p.method)) {
                return fail("bad proof".into());
            }
        }
        Ok(())
    }
}

/// `{token}/clean/relative/segments`: no drive letters, no `..`, no separators inside a segment.
/// A game file may put one `*` folder between them, e.g. for a folder per account.
pub fn check_path(path: &str, is_file: bool) -> Result<(), String> {
    let (token, rest) = split_token(path).ok_or_else(|| format!("path must start with an allowed {{token}}: {path}"))?;
    if !TOKENS.contains(&token) {
        return Err(format!("unknown path token {{{token}}}"));
    }
    let segments: Vec<&str> = rest.split('/').collect();
    let wild: Vec<usize> = (0..segments.len()).filter(|&i| segments[i] == "*").collect();
    match wild[..] {
        [] => {}
        [i] if is_file && i > 0 && i + 1 < segments.len() => {}
        _ => return Err(format!("a * may only stand for one folder inside a game file's path: {path}")),
    }
    for seg in segments.iter().filter(|s| **s != "*") {
        if !is_segment(seg) {
            return Err(format!("unsafe path segment {seg:?} in {path}"));
        }
    }
    if is_file {
        let ext = segments.last().and_then(|s| s.rsplit_once('.')).map(|(_, e)| e.to_ascii_lowercase());
        if !ext.is_some_and(|e| EXTENSIONS.contains(&e.as_str())) && !segments.last().is_some_and(|s| FILE_NAMES.contains(s)) {
            return Err(format!("not a config file type: {path}"));
        }
    }
    Ok(())
}

fn split_token(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix('{')?;
    let (token, rest) = rest.split_once('}')?;
    Some((token, rest.strip_prefix('/')?))
}

/// Expand a validated path. Segments are clean, so the result always stays under its root.
pub fn expand(path: &str, roots: &HashMap<&str, PathBuf>) -> Result<PathBuf, String> {
    check_path(path, false)?;
    let (token, rest) = split_token(path).ok_or("bad path")?;
    let mut out = roots
        .get(token)
        .cloned()
        .ok_or_else(|| format!("{{{token}}} isn't available on this PC"))?;
    for seg in rest.split('/') {
        out.push(seg);
    }
    Ok(out)
}

impl FileTarget {
    pub fn paths(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.path).chain(&self.also)
    }

    /// Every copy of the file on this PC, from `path` and `also`. If none exists yet, `path`
    /// as `expand_all` gives it, so the game reads as "Launch once" like any missing file.
    pub fn copies(&self, roots: &HashMap<&str, PathBuf>) -> Result<Vec<PathBuf>, String> {
        let mut seen = HashSet::new();
        let found: Vec<PathBuf> = self
            .paths()
            .filter_map(|p| expand_all(p, roots).ok()) // a root this PC lacks holds no copy
            .flatten()
            // Once each, ignoring case like Windows: a second edit in one apply would record
            // Chamber's own values as the originals.
            .filter(|p| p.is_file() && seen.insert(p.to_string_lossy().to_ascii_lowercase()))
            .collect();
        if found.is_empty() { expand_all(&self.path, roots) } else { Ok(found) }
    }
}

/// Every copy of one game file path. A `*` folder matches each folder there that holds the
/// file; a plain path comes back as is, even before the file exists.
fn expand_all(path: &str, roots: &HashMap<&str, PathBuf>) -> Result<Vec<PathBuf>, String> {
    check_path(path, true)?;
    let Some((head, tail)) = path.split_once("/*/") else { return Ok(vec![expand(path, roots)?]) };
    let Ok(entries) = std::fs::read_dir(expand(head, roots)?) else { return Ok(Vec::new()) };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_name().to_str().is_some_and(is_segment)) // same rules as a written segment
        .map(|e| tail.split('/').fold(e.path(), |p, seg| p.join(seg)))
        .filter(|p| p.is_file())
        .collect();
    found.sort();
    Ok(found)
}

fn is_segment(seg: &str) -> bool {
    seg.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && seg.chars().all(|c| c.is_ascii_alphanumeric() || " ._()-".contains(c))
        && !seg.ends_with(['.', ' '])
}

fn is_slug(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn is_text(s: &str) -> bool {
    !s.trim().is_empty() && !s.chars().any(char::is_control)
}

fn is_section(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "/._: -".contains(c))
}

/// Call of Duty keys name the variant too (`DxrMode@1`): one name can hold several settings.
fn is_key(s: &str, format: Format) -> bool {
    let plain = |k: &str| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
    match format {
        Format::Cod => s.split_once('@').is_some_and(|(name, n)| plain(name) && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())),
        _ => plain(s),
    }
}

/// Values are written into files: no line breaks or unknown {variables}, and nothing that ends
/// the value early: a quote in Valve and convar files, a `//` note in Call of Duty files. INI
/// values may carry quotes (Overwatch uses them).
fn is_value(s: &str, format: Format) -> bool {
    let mut rest = s.to_string();
    for v in VARIABLES {
        rest = rest.replace(v, "");
    }
    let fits = match format {
        Format::Ini | Format::Spaced => true,
        Format::Kv | Format::Convar => !s.contains('"'),
        Format::Cod => !s.contains("//"),
        // One JSON scalar: an object, an array, or `1, "x": 2` would add structure, not a value.
        Format::Json => serde_json::from_str::<serde_json::Value>(&fill(s, &Vars::default())).is_ok_and(|v| !v.is_object() && !v.is_array()),
    };
    fits && !s.chars().any(char::is_control) && !rest.contains(['{', '}'])
}

fn is_exe(s: &str) -> bool {
    s.to_ascii_lowercase().ends_with(".exe") && s.chars().all(|c| c.is_ascii_alphanumeric() || " ._()-".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // Throwaway 2048-bit test key, generated for these tests only (its private half was discarded).
    const TEST_MODULUS_HEX: &str = "bfe3cc26a01280d1c6fa5e61da8cfb1e16010181c2c71d2f682577a42f4c13f1c66c677dd231a45ea366a822caeac60bfedb759494aa70a973dbf5d26f3ecde5dd032a623cd35ae1e1a6e65771082ec84211c4512bdee2b10c7c09f9b132d931b953790c15206d62ee4f316871b7ff73fb028ca14b26b7fa31ef0bcfb156e5147b42889401667251910624af1aeddb500ffbaa262b28b8a1d84e1247dc1eba595f47ac99a2ad1a9e6aa65ec2759d38ed0d9b59cd05e897acc5c0c4ba70906de01fc77eba6e2486678832de1286dd639d8db35907549f91bfc09d6792f2359d4a8ec592568826b834c6716cd504adeb2f68d63f9eb543fc067ec42d8423d25f0d";
    const TEST_SIG_HEX: &str = "5c824740afe2b56d29d3de1a9d951fc14fab53d80af509b05321704359ed368e04398b4d7c1e16fa358b26ac5bffdad3f947ab846cd6b4ed569c974196229dac2f57d55ce8f10dc6ea34c89176fdeba2852e65f73c64d31356368bf526d5695ec09305faa24db93240ddb5c41cdad8e2060a583a393cbe3044c102714c3ffea8b579139bb4aaa48a5b47f2a24da1a2cb778dfaca6eddff64a7cd7b7fafe5cf81be6e2ceac0591716d16226501b06c74d98e05ec9b616c491d234e9dd3b38c24770b8b4ebe8bc8cd50c4f6f2028b6322f1d44cf112e57ce41b0cae8f2f174ea5d0df3e145303ab827f63079595c5a4c35a6e5ad9d7f0077c038e2bc06baaa1cc8";
    const TEST_MESSAGE: &[u8] = b"chamber test message";

    #[test]
    fn signature_accepts_good_and_rejects_tampered() {
        let sig = from_hex(TEST_SIG_HEX).unwrap();
        assert!(verify_with(TEST_MESSAGE, &sig, TEST_MODULUS_HEX));
        assert!(!verify_with(b"chamber test messagE", &sig, TEST_MODULUS_HEX));
        let mut bad = sig.clone();
        bad[10] ^= 1;
        assert!(!verify_with(TEST_MESSAGE, &bad, TEST_MODULUS_HEX));
        assert!(!verify_with(TEST_MESSAGE, &sig, MODULUS_HEX), "test sig must not pass the real key");
    }

    #[test]
    fn paths_reject_escapes_and_unknown_roots() {
        assert!(check_path("{localappdata}/FortniteGame/Saved/GameUserSettings.ini", true).is_ok());
        assert!(check_path("{documents}/My Games/Rocket League (x)/a.ini", true).is_ok());
        assert!(check_path("{localappdata}/Activision/Call of Duty/players/s.1.0.cod26.txt0", true).is_ok());
        assert!(check_path("{documents}/Battlefield 6/settings/*/PROFSAVE_profile", true).is_ok());
        for bad in [
            "C:/Windows/system.ini",
            "{localappdata}/../x.ini",
            "{localappdata}/a/../../x.ini",
            "{localappdata}/.hidden/x.ini",
            "{temp}/x.ini",
            "{localappdata}/a\\b.ini",
            "{localappdata}/x.exe",
            "{documents}/Battlefield 6/settings/steam/PROFSAVE",
            "{localappdata}/x.ini.",
            "{localappdata}//x.ini",
            "localappdata/x.ini",
        ] {
            assert!(check_path(bad, true).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn one_wildcard_folder_inside_a_file_path() {
        assert!(check_path("{documents}/My Games/Rainbow Six - Siege/*/GameSettings.ini", true).is_ok());
        for bad in [
            "{documents}/*/GameSettings.ini",   // every folder in Documents
            "{documents}/My Games/*",           // the file itself
            "{documents}/My Games/*/a/*/x.ini", // more than one
            "{documents}/My Games/Siege*/x.ini",
        ] {
            assert!(check_path(bad, true).is_err(), "{bad} should be rejected");
        }
        assert!(check_path("{documents}/My Games/*/Siege", false).is_err(), "only game files can use one");
    }

    #[test]
    fn expand_all_finds_every_accounts_copy() {
        let dir = std::env::temp_dir().join(format!("chamber-catalog-wild-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for account in ["b-account", "a-account", "no-file-yet", ".hidden"] {
            std::fs::create_dir_all(dir.join("Siege").join(account)).unwrap();
        }
        for account in ["b-account", "a-account", ".hidden"] {
            std::fs::write(dir.join("Siege").join(account).join("GameSettings.ini"), "").unwrap();
        }
        let roots = HashMap::from([("documents", dir.clone())]);
        let found = expand_all("{documents}/Siege/*/GameSettings.ini", &roots).unwrap();
        assert_eq!(found, [dir.join("Siege/a-account/GameSettings.ini"), dir.join("Siege/b-account/GameSettings.ini")]);
        assert!(expand_all("{documents}/Gone/*/GameSettings.ini", &roots).unwrap().is_empty());
        assert_eq!(expand_all("{documents}/Siege/x.ini", &roots).unwrap(), [dir.join("Siege/x.ini")], "a plain path, even before it exists");
        assert!(expand("{documents}/Siege/*/GameSettings.ini", &roots).is_err(), "a single path never holds a *");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn find_exe_stays_relative() {
        let game = |exe: &str| -> Game {
            serde_json::from_value(serde_json::json!({
                "slug": "g", "name": "G", "tile": "G", "launcher": "other", "find": { "exe": exe },
                "manual": [{ "label": "l", "value": "v", "why": "w" }]
            }))
            .unwrap()
        };
        for good in ["Game.exe", "FortniteGame/Binaries/Win64/FortniteClient-Win64-Shipping.exe"] {
            assert!(Catalog { version: 0, games: vec![game(good)] }.validate().is_ok(), "{good}");
        }
        for bad in ["../Game.exe", "/Game.exe", "C:/Game.exe", "bin\\Game.exe", "bin/Game.dll", "a//Game.exe"] {
            assert!(Catalog { version: 0, games: vec![game(bad)] }.validate().is_err(), "{bad}");
        }
    }

    #[test]
    fn expand_joins_under_the_root() {
        let roots = HashMap::from([("documents", PathBuf::from("/home/me/Docs"))]);
        let p = expand("{documents}/My Games/x.ini", &roots).unwrap();
        assert_eq!(p, Path::new("/home/me/Docs/My Games/x.ini"));
        assert!(expand("{steamuserdata}/730/x.txt", &roots).is_err());
    }

    #[test]
    fn values_cannot_inject_lines_or_quotes() {
        assert!(is_value("{refresh}.000000", Format::Ini));
        assert!(is_value("(X=1,Y=2)", Format::Ini));
        assert!(is_value("", Format::Kv));
        assert!(is_value("\"0\"", Format::Ini), "quoted INI values are fine");
        assert!(!is_value("1\n[Other]\nKey=2", Format::Ini));
        assert!(!is_value("1\" \"evil", Format::Kv), "a quote would end a Valve value");
        assert!(!is_value("{username}", Format::Ini));
        assert!(is_value("Enabled + boost", Format::Cod));
        assert!(!is_value("Off // On", Format::Cod), "a // note would cut a Call of Duty value short");
        assert!(is_value("(0.0, 1.0, 0.0)", Format::Convar));
        assert!(!is_value("1\" \"evil", Format::Convar), "a quote would end a convar value");
        assert!(is_value("false", Format::Json));
        assert!(is_value("\"TAA_High\"", Format::Json));
        assert!(is_value("{refresh}", Format::Json));
        assert!(!is_value("High", Format::Json), "JSON strings keep their quotes");
        assert!(!is_value("1, \"Injected\": 2", Format::Json), "one value, never more members");
        assert!(!is_value("[1, 2]", Format::Json));
    }

    #[test]
    fn cod_keys_name_their_variant() {
        assert!(is_key("DxrMode@1", Format::Cod));
        assert!(!is_key("DxrMode", Format::Cod), "the variant picks which DxrMode line");
        assert!(!is_key("DxrMode@", Format::Cod));
        assert!(!is_key("DxrMode@1;8707", Format::Cod), "the ids belong to the file");
        assert!(is_key("sg.ShadowQuality", Format::Ini));
        assert!(!is_key("Shadow@1", Format::Ini));
    }

    /// Drafts have no settings yet, but their structure and paths must already be valid.
    #[test]
    fn every_draft_is_well_formed() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../games/drafts");
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        for path in entries.map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "json")) {
            let game: Game = serde_json::from_slice(&std::fs::read(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert_eq!(path.file_stem().unwrap(), game.slug.as_str(), "{}", path.display());
            for p in game.files.iter().flat_map(FileTarget::paths) {
                check_path(p, true).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
            for p in &game.find.paths {
                check_path(p, false).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
            assert!(game.process.iter().all(|p| is_exe(p)), "{}", path.display());
        }
    }

    #[test]
    fn fill_uses_monitor_values() {
        let v = Vars { refresh: 165, width: 2560, height: 1440 };
        assert_eq!(fill("{refresh}.000000", &v), "165.000000");
        assert_eq!(fill("{width}x{height}", &v), "2560x1440");
    }

    /// Every game file in the repo must load with unknown fields denied and pass validation.
    #[test]
    fn every_game_file_is_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../games");
        let mut games = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("games/ folder") {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "json") {
                let game: Game = serde_json::from_slice(&std::fs::read(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                assert_eq!(path.file_stem().unwrap(), game.slug.as_str(), "file name must match slug");
                games.push(game);
            }
        }
        assert!(!games.is_empty());
        Catalog { version: 0, games }.validate().unwrap();
    }

    /// CI gate: the committed catalog must carry a valid signature from the real key.
    #[test]
    fn committed_catalog_is_signed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let sig_path = root.join("catalog.json.sig");
        if !sig_path.exists() && std::env::var_os("CI").is_none() {
            eprintln!("catalog.json.sig is missing: sign the catalog before pushing");
            return;
        }
        let json = std::fs::read(root.join("catalog.json")).unwrap();
        let sig = std::fs::read(sig_path).unwrap();
        parse_verified(&json, &sig).unwrap();
    }
}
