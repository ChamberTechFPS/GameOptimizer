//! Text-level edits to game config files. Only the listed keys change: every other byte,
//! the encoding (UTF-8, UTF-8 with BOM, UTF-16 LE), and the line endings stay as found.

use std::fs;
use std::io;
use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
}

const BOM8: [u8; 3] = [0xEF, 0xBB, 0xBF];
const BOM16: [u8; 2] = [0xFF, 0xFE];

/// Unreal writes plain ASCII or UTF-16 LE with a BOM. Anything else is refused untouched.
pub fn decode(bytes: &[u8]) -> Result<(Encoding, String), String> {
    let unreadable = || "uses a text encoding I can't safely edit".to_string();
    if let Some(rest) = bytes.strip_prefix(&BOM8) {
        return String::from_utf8(rest.to_vec()).map(|s| (Encoding::Utf8Bom, s)).map_err(|_| unreadable());
    }
    if let Some(rest) = bytes.strip_prefix(&BOM16) {
        if !rest.len().is_multiple_of(2) {
            return Err(unreadable());
        }
        let units: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        return String::from_utf16(&units).map(|s| (Encoding::Utf16Le, s)).map_err(|_| unreadable());
    }
    String::from_utf8(bytes.to_vec()).map(|s| (Encoding::Utf8, s)).map_err(|_| unreadable())
}

pub fn encode(enc: Encoding, text: &str) -> Vec<u8> {
    match enc {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => [&BOM8[..], text.as_bytes()].concat(),
        Encoding::Utf16Le => BOM16.iter().copied().chain(text.encode_utf16().flat_map(u16::to_le_bytes)).collect(),
    }
}

/// Settings files store the same value many ways: "True"/"true", "165"/"165.000000".
pub fn same_value(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    if a.eq_ignore_ascii_case(b) {
        return true;
    }
    matches!((a.parse::<f64>(), b.parse::<f64>()), (Ok(x), Ok(y)) if (x - y).abs() < 1e-6)
}

/// Lines with their terminators, so untouched lines round-trip byte for byte.
fn lines(text: &str) -> Vec<String> {
    text.split_inclusive('\n').map(String::from).collect()
}

fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// Split a line into (content, terminator).
fn split_eol(line: &str) -> (&str, &str) {
    let body = line.trim_end_matches(['\r', '\n']);
    (body, &line[body.len()..])
}

/// Append a new line, adding a terminator to the current last line first if it lacks one.
fn push_line(out: &mut Vec<String>, at: usize, content: String, nl: &str) {
    if at > 0 && !out[at - 1].ends_with('\n') {
        out[at - 1].push_str(nl);
    }
    out.insert(at, format!("{content}{nl}"));
}

// ---------- INI ----------

fn ini_section(line: &str) -> Option<&str> {
    let t = split_eol(line).0.trim();
    t.strip_prefix('[')?.strip_suffix(']').map(str::trim)
}

/// The key on a `key=value` line, ignoring comments.
fn ini_key(line: &str) -> Option<&str> {
    let t = split_eol(line).0;
    if t.trim_start().starts_with([';', '#']) {
        return None;
    }
    t.split_once('=').map(|(k, _)| k.trim())
}

/// Indexes of lines holding `key` inside every copy of `section`, plus where each copy ends.
fn ini_find(lines: &[String], section: &str, key: &str) -> (Vec<usize>, Option<usize>) {
    let mut hits = Vec::new();
    let mut in_section = false;
    let mut last_content = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some(name) = ini_section(line) {
            in_section = name.eq_ignore_ascii_case(section);
            if in_section {
                last_content = Some(i);
            }
            continue;
        }
        if in_section {
            if !split_eol(line).0.trim().is_empty() {
                last_content = Some(i);
            }
            if ini_key(line).is_some_and(|k| k.eq_ignore_ascii_case(key)) {
                hits.push(i);
            }
        }
    }
    (hits, last_content)
}

pub fn ini_get(text: &str, section: &str, key: &str) -> Option<String> {
    let all = lines(text);
    let (hits, _) = ini_find(&all, section, key);
    let line = &all[*hits.last()?];
    split_eol(line).0.split_once('=').map(|(_, v)| v.trim().to_string())
}

pub fn ini_set(text: &str, section: &str, key: &str, value: &str) -> String {
    let mut all = lines(text);
    let nl = newline(text);
    let (hits, end) = ini_find(&all, section, key);
    if !hits.is_empty() {
        for i in hits {
            let (body, eol) = split_eol(&all[i]);
            let (before, after) = body.split_once('=').expect("ini_find only returns key lines");
            let spacing = &after[..after.len() - after.trim_start().len()];
            all[i] = format!("{before}={spacing}{value}{eol}");
        }
    } else if let Some(end) = end {
        push_line(&mut all, end + 1, format!("{key}={value}"), nl);
    } else {
        // New section at the end, separated by a blank line like the game writes them.
        let at = all.len();
        let needs_gap = all.last().is_some_and(|l| !split_eol(l).0.trim().is_empty());
        if needs_gap {
            push_line(&mut all, at, String::new(), nl);
        }
        let at = all.len();
        push_line(&mut all, at, format!("[{section}]"), nl);
        push_line(&mut all, at + 1, format!("{key}={value}"), nl);
    }
    all.concat()
}

pub fn ini_remove(text: &str, section: &str, key: &str) -> String {
    let all = lines(text);
    let (hits, _) = ini_find(&all, section, key);
    all.into_iter().enumerate().filter(|(i, _)| !hits.contains(i)).map(|(_, l)| l).collect()
}

// ---------- line formats: one setting per line ----------

/// Finds the setting on one line: its key and the byte range of its value.
type Parse = fn(&str) -> Option<(&str, Range<usize>)>;

fn line_get(text: &str, key: &str, parse: Parse) -> Option<String> {
    // The last copy wins, as when the game reads the file top to bottom.
    lines(text).iter().rev().find_map(|l| parse(l).filter(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| l[v].to_string()))
}

/// Every copy of `key` gets the value. None when no line holds `key`.
fn line_set(text: &str, key: &str, value: &str, parse: Parse) -> Option<String> {
    let mut all = lines(text);
    let mut found = false;
    for line in &mut all {
        if let Some((k, range)) = parse(line) {
            if k.eq_ignore_ascii_case(key) {
                line.replace_range(range, value);
                found = true;
            }
        }
    }
    found.then(|| all.concat())
}

fn line_remove(text: &str, key: &str, parse: Parse) -> String {
    lines(text).into_iter().filter(|l| !parse(l).is_some_and(|(k, _)| k.eq_ignore_ascii_case(key))).collect()
}

// ---------- Valve KeyValues (flat files like cs2_video.txt) ----------

/// The quoted key on a `"key"  "value"` line, with the value's byte range.
fn kv_parse(line: &str) -> Option<(&str, Range<usize>)> {
    let body = split_eol(line).0;
    let start = body.find('"')?;
    if !body[..start].trim().is_empty() {
        return None;
    }
    let key_end = start + 1 + body[start + 1..].find('"')?;
    let after = key_end + 1;
    let vstart = after + body[after..].find('"')?;
    if !body[after..vstart].trim().is_empty() {
        return None;
    }
    let vend = vstart + 1 + body[vstart + 1..].find('"')?;
    Some((&body[start + 1..key_end], vstart + 1..vend))
}

pub fn kv_get(text: &str, key: &str) -> Option<String> {
    line_get(text, key, kv_parse)
}

// ponytail: flat files only. Nested blocks (vcfg "convars") need an insert that finds the block.
pub fn kv_set(text: &str, key: &str, value: &str) -> Result<String, String> {
    if let Some(out) = line_set(text, key, value, kv_parse) {
        return Ok(out);
    }
    let mut all = lines(text);
    let close = all
        .iter()
        .rposition(|l| split_eol(l).0.trim() == "}")
        .ok_or_else(|| format!("couldn't find where to add {key}"))?;
    push_line(&mut all, close, format!("\t\"{key}\"\t\t\"{value}\""), newline(text));
    Ok(all.concat())
}

pub fn kv_remove(text: &str, key: &str) -> String {
    line_remove(text, key, kv_parse)
}

// ---------- Call of Duty (s.1.0.cod26.txt0: `Name@0;id;id = value // note`) ----------

/// The `Name@variant` key on a setting line, with the value's byte range (before any `//` note).
fn cod_parse(line: &str) -> Option<(&str, Range<usize>)> {
    let (head, rest) = split_eol(line).0.split_once(" = ")?;
    let start = head.len() + 3;
    let end = start + rest.find(" //").unwrap_or(rest.len());
    Some((head.split_once(';').map_or(head, |(name, _)| name), start..end))
}

pub fn cod_get(text: &str, key: &str) -> Option<String> {
    line_get(text, key, cod_parse)
}

/// The ids after each name are the game's own, so a setting the file lacks is refused, never invented.
pub fn cod_set(text: &str, key: &str, value: &str) -> Result<String, String> {
    line_set(text, key, value, cod_parse).ok_or_else(|| format!("couldn't find {key} in the settings file"))
}

pub fn cod_remove(text: &str, key: &str) -> String {
    line_remove(text, key, cod_parse)
}

// ---------- files ----------

/// Write through a temp file and rename, so a crash never leaves a half-written config.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".chamber-tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

pub fn is_read_only(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.permissions().readonly())
}

#[allow(clippy::permissions_set_readonly_false)] // Windows read-only attribute; Unix only runs tests
pub fn set_read_only(path: &Path, read_only: bool) -> io::Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    if perms.readonly() != read_only {
        perms.set_readonly(read_only);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UE: &str = "[/Script/FortniteGame.FortGameUserSettings]\r\nbUseVSync=True\r\nFrameRateLimit=0.000000\r\n\r\n[ScalabilityGroups]\r\nsg.ShadowQuality=3\r\n; comment=1\r\nsg.EffectsQuality = 3\r\n";

    #[test]
    fn ini_reads_values_and_ignores_comments() {
        assert_eq!(ini_get(UE, "ScalabilityGroups", "sg.ShadowQuality").as_deref(), Some("3"));
        assert_eq!(ini_get(UE, "scalabilitygroups", "SG.EFFECTSQUALITY").as_deref(), Some("3"));
        assert_eq!(ini_get(UE, "ScalabilityGroups", "comment"), None);
        assert_eq!(ini_get(UE, "ScalabilityGroups", "bUseVSync"), None, "wrong section");
    }

    #[test]
    fn ini_set_changes_only_that_value() {
        let out = ini_set(UE, "ScalabilityGroups", "sg.ShadowQuality", "0");
        assert_eq!(out, UE.replace("sg.ShadowQuality=3", "sg.ShadowQuality=0"));
        let spaced = ini_set(UE, "ScalabilityGroups", "sg.EffectsQuality", "0");
        assert_eq!(spaced, UE.replace("sg.EffectsQuality = 3", "sg.EffectsQuality = 0"));
    }

    #[test]
    fn ini_set_adds_missing_keys_and_sections_with_crlf() {
        let out = ini_set(UE, "/Script/FortniteGame.FortGameUserSettings", "bMotionBlur", "False");
        assert!(out.contains("FrameRateLimit=0.000000\r\nbMotionBlur=False\r\n\r\n[ScalabilityGroups]"));
        let out = ini_set(UE, "D3DRHIPreference", "PreferredRHI", "dx11");
        assert!(out.ends_with("sg.EffectsQuality = 3\r\n\r\n[D3DRHIPreference]\r\nPreferredRHI=dx11\r\n"));
        assert!(!out.replace("\r\n", "").contains('\n'), "no bare LF added to a CRLF file");
    }

    #[test]
    fn ini_set_then_remove_restores_the_original() {
        let added = ini_set(UE, "ScalabilityGroups", "sg.FoliageQuality", "0");
        assert_eq!(ini_remove(&added, "ScalabilityGroups", "sg.FoliageQuality"), UE);
    }

    #[test]
    fn ini_handles_a_last_line_without_newline() {
        let out = ini_set("[A]\nx=1", "A", "y", "2");
        assert_eq!(out, "[A]\nx=1\ny=2\n");
    }

    const VIDEO: &str = "\"video.cfg\"\n{\n\t\"Version\"\t\t\"14\"\n\t\"setting.mat_vsync\"\t\t\"1\"\n}\n";

    #[test]
    fn kv_set_get_and_insert() {
        assert_eq!(kv_get(VIDEO, "setting.mat_vsync").as_deref(), Some("1"));
        let out = kv_set(VIDEO, "setting.mat_vsync", "0").unwrap();
        assert_eq!(out, VIDEO.replace("\"1\"", "\"0\""));
        let added = kv_set(VIDEO, "setting.r_low_latency", "1").unwrap();
        assert!(added.ends_with("\t\"setting.r_low_latency\"\t\t\"1\"\n}\n"));
        assert_eq!(kv_remove(&added, "setting.r_low_latency"), VIDEO);
        assert_eq!(kv_get(VIDEO, "video.cfg"), None, "block names aren't settings");
    }

    // Lines from a real Black Ops 6 file (MW4's cod26 file uses the same format), with a non-ASCII note.
    const COD: &str = "1\n\n//\n// Display\n\n// Réduit la latence\nNvidiaReflex@0;33761;11445 = Enabled // one of Disabled, Enabled, Enabled + boost\n\n// Ray tracing\nDxrMode@0;37334;7764 = Off // one of Off, On\nDxrMode@1;8707;7764 = Low // one of Off, Low, Medium, High, Ultra\nShowBlood@0;57752;20945 = true\n";

    #[test]
    fn cod_reads_values_by_name_and_variant() {
        assert_eq!(cod_get(COD, "NvidiaReflex@0").as_deref(), Some("Enabled"));
        assert_eq!(cod_get(COD, "nvidiareflex@0").as_deref(), Some("Enabled"));
        assert_eq!(cod_get(COD, "DxrMode@1").as_deref(), Some("Low"), "each variant is its own setting");
        assert_eq!(cod_get(COD, "ShowBlood@0").as_deref(), Some("true"), "a value with no note");
        assert_eq!(cod_get(COD, "DxrMode"), None, "the variant is part of the key");
    }

    #[test]
    fn cod_set_keeps_ids_notes_and_line_endings() {
        let out = cod_set(COD, "DxrMode@1", "Off").unwrap();
        assert_eq!(out, COD.replace("DxrMode@1;8707;7764 = Low //", "DxrMode@1;8707;7764 = Off //"));
        let crlf = COD.replace('\n', "\r\n");
        let out = cod_set(&crlf, "NvidiaReflex@0", "Enabled + boost").unwrap();
        assert_eq!(out, crlf.replace("= Enabled //", "= Enabled + boost //"));
        assert_eq!(cod_set(&out, "NvidiaReflex@0", "Enabled").unwrap(), crlf, "setting it back restores every byte");
    }

    #[test]
    fn cod_refuses_settings_the_file_lacks() {
        // The ids after each name come from the game, so Chamber can't write a new line.
        assert!(cod_set(COD, "MotionBlur@0", "Off").is_err());
        assert_eq!(cod_remove(COD, "ShowBlood@0"), COD.replace("ShowBlood@0;57752;20945 = true\n", ""));
    }

    #[test]
    fn encodings_round_trip_byte_for_byte() {
        for text in [UE, VIDEO, COD] {
            for enc in [Encoding::Utf8, Encoding::Utf8Bom, Encoding::Utf16Le] {
                let bytes = encode(enc, text);
                let (found, decoded) = decode(&bytes).unwrap();
                assert_eq!(found, enc);
                assert_eq!(encode(found, &decoded), bytes);
            }
        }
        assert!(decode(&[0x80, 0x81]).is_err(), "invalid UTF-8 is refused");
    }

    #[test]
    fn values_compare_loosely() {
        assert!(same_value("True", "true"));
        assert!(same_value("165.000000", "165"));
        assert!(!same_value("0", "1"));
        assert!(!same_value("dx11", "dx12"));
    }

    #[test]
    fn atomic_write_replaces_the_file() {
        let dir = std::env::temp_dir().join(format!("chamber-patch-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.ini");
        fs::write(&file, "old").unwrap();
        write_atomic(&file, b"new").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"new");
        assert!(!dir.join("a.ini.chamber-tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
