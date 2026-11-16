//! This PC's hardware, for the specs panel. Read-only: it reads the registry and asks Windows
//! two questions. Nothing here writes, and nothing leaves the PC.

use serde::Serialize;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Specs {
    pub cpu: Option<String>,
    pub cores: u32,
    pub threads: u32,
    /// Largest video memory first, so a laptop's dedicated GPU leads.
    pub gpus: Vec<Gpu>,
    pub ram_gb: Option<u32>,
    pub width: u32,
    pub height: u32,
    pub refresh: u32,
    /// The motherboard's model, e.g. "ROG STRIX B650-A GAMING WIFI".
    pub board: Option<String>,
    pub board_maker: Option<String>,
    /// e.g. "Windows 11 Pro".
    pub os: Option<String>,
    /// e.g. "24H2 · build 26100".
    pub os_version: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gpu {
    pub name: String,
    pub vram_gb: Option<u32>,
    pub driver: Option<String>,
}

/// Trimmed, with runs of spaces collapsed: CPU names come padded.
#[cfg_attr(not(windows), allow(dead_code))]
fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whole gigabytes, rounded. None for zero, which is what a missing value reads as.
#[cfg_attr(not(windows), allow(dead_code))]
fn gb(bytes: u64) -> Option<u32> {
    (bytes > 0).then(|| (bytes as f64 / (1u64 << 30) as f64).round() as u32).filter(|&n| n > 0)
}

/// Little-endian number from a registry value of up to 8 bytes (REG_QWORD, REG_DWORD or REG_BINARY).
#[cfg_attr(not(windows), allow(dead_code))]
fn le_u64(bytes: &[u8]) -> u64 {
    bytes.iter().take(8).rev().fold(0, |n, &b| (n << 8) | u64::from(b))
}

/// The driver version as its vendor prints it. Windows stores NVIDIA's 560.94 as 32.0.15.6094:
/// the last five digits are the version.
#[cfg_attr(not(windows), allow(dead_code))]
fn driver_label(gpu: &str, version: &str) -> String {
    let version = version.trim();
    if gpu.to_ascii_uppercase().contains("NVIDIA") {
        let digits: String = version.split('.').skip(2).collect();
        if digits.len() >= 5 && digits.bytes().all(|b| b.is_ascii_digit()) {
            let tail = &digits[digits.len() - 5..];
            return format!("{}.{}", &tail[..3], &tail[3..]);
        }
    }
    version.to_string()
}

/// Windows 11 still calls itself Windows 10 in the registry; the build number tells them apart.
#[cfg_attr(not(windows), allow(dead_code))]
fn os_name(build: u32, edition: Option<&str>) -> String {
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    match edition {
        Some("Professional") => format!("{name} Pro"),
        Some("Core") => format!("{name} Home"),
        Some(e) if !e.is_empty() => format!("{name} {e}"),
        _ => name.to_string(),
    }
}

#[cfg(windows)]
pub fn read() -> Specs {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    const GPU_CLASS: &str = "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}";
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let text = |key: &str, name: &str| -> Option<String> {
        hklm.open_subkey(key).ok()?.get_value::<String, _>(name).ok().map(|s| clean(&s)).filter(|s| !s.is_empty())
    };

    let mut gpus: Vec<Gpu> = Vec::new();
    if let Ok(class) = hklm.open_subkey(GPU_CLASS) {
        for name in class.enum_keys().flatten() {
            let Ok(adapter) = class.open_subkey(&name) else { continue };
            let Ok(desc) = adapter.get_value::<String, _>("DriverDesc") else { continue };
            let desc = clean(&desc);
            // "Microsoft Basic Display Adapter" and the remote-desktop adapters aren't hardware.
            if desc.is_empty() || desc.starts_with("Microsoft ") || gpus.iter().any(|g| g.name == desc) {
                continue;
            }
            let memory = adapter
                .get_raw_value("HardwareInformation.qwMemorySize")
                .or_else(|_| adapter.get_raw_value("HardwareInformation.MemorySize"))
                .map(|v| le_u64(&v.bytes))
                .unwrap_or(0);
            let driver = adapter.get_value::<String, _>("DriverVersion").ok().map(|v| driver_label(&desc, &v));
            gpus.push(Gpu { name: desc, vram_gb: gb(memory), driver });
        }
    }
    gpus.sort_by_key(|g| std::cmp::Reverse(g.vram_gb));

    const BIOS: &str = "HARDWARE\\DESCRIPTION\\System\\BIOS";
    const NT: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
    let build = text(NT, "CurrentBuildNumber").and_then(|b| b.parse::<u32>().ok());
    let os_version = match (text(NT, "DisplayVersion"), build) {
        (Some(v), Some(b)) => Some(format!("{v} · build {b}")),
        (v, b) => v.or(b.map(|b| format!("Build {b}"))),
    };

    let mut kb = 0u64;
    // SAFETY: a plain out-pointer call; `kb` is only read when Windows reports success.
    let ram_ok = unsafe { windows_sys::Win32::System::SystemInformation::GetPhysicallyInstalledSystemMemory(&mut kb) } != 0;

    let vars = crate::detect::hardware();
    Specs {
        cpu: text("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0", "ProcessorNameString"),
        cores: vars.cores,
        threads: vars.threads,
        gpus,
        ram_gb: if ram_ok { gb(kb.saturating_mul(1024)) } else { None },
        width: vars.width,
        height: vars.height,
        refresh: vars.refresh,
        board: text(BIOS, "BaseBoardProduct"),
        board_maker: text(BIOS, "BaseBoardManufacturer"),
        os: build.map(|b| os_name(b, text(NT, "EditionID").as_deref())),
        os_version,
    }
}

/// Nothing to read off Windows; the app only ships for Windows.
#[cfg(not(windows))]
pub fn read() -> Specs {
    Specs::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_sizes_are_tidied() {
        assert_eq!(clean("  Intel(R) Core(TM)   i7-14700K "), "Intel(R) Core(TM) i7-14700K");
        assert_eq!(gb(0), None);
        assert_eq!(gb(12 * (1 << 30)), Some(12));
        assert_eq!(gb(34_246_492_160), Some(32)); // 31.9 GB reads as the 32 GB that's installed
        assert_eq!(le_u64(&[0, 0, 0, 0, 3, 0, 0, 0]), 3 << 32);
        assert_eq!(le_u64(&[0, 0, 0, 0x80]), 0x8000_0000);
        assert_eq!(le_u64(&[]), 0);
    }

    #[test]
    fn driver_versions_read_the_way_the_vendor_prints_them() {
        assert_eq!(driver_label("NVIDIA GeForce RTX 4070", "32.0.15.6094"), "560.94");
        assert_eq!(driver_label("NVIDIA GeForce RTX 4070", "31.0.15.3623"), "536.23");
        assert_eq!(driver_label("AMD Radeon RX 7800 XT", "32.0.12033.1030"), "32.0.12033.1030");
        assert_eq!(driver_label("NVIDIA GeForce RTX 4070", "odd"), "odd");
    }

    #[test]
    fn windows_11_is_told_apart_by_build() {
        assert_eq!(os_name(26100, Some("Professional")), "Windows 11 Pro");
        assert_eq!(os_name(19045, Some("Core")), "Windows 10 Home");
        assert_eq!(os_name(22631, Some("Enterprise")), "Windows 11 Enterprise");
        assert_eq!(os_name(22631, None), "Windows 11");
    }

    #[test]
    fn reading_this_pc_never_fails() {
        let s = read();
        assert!(s.gpus.iter().all(|g| !g.name.is_empty()));
    }
}
