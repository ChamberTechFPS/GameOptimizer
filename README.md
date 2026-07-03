# Chamber Game Optimizer

**Per-game competitive settings, applied and tracked with one click.** Companion to the [Chamber Playbook](https://github.com/ChamberTechFPS/Chamber-Playbook) — the playbook tunes Windows, this tunes your games.

- ✅ **Status tracking** — see per game whether Chamber settings are Applied, Drifted (changed since apply), or Not applied
- 📁 **Custom locations** — installed somewhere non-standard? Auto-detection always runs first, but you can point Chamber at any folder per game and it's remembered
- 🔄 **Live config updates** — new and updated game configs are pulled from this repo automatically; the app shows a banner when they're available, no reinstall needed
- 💾 **Every change backed up** — one-click revert restores your original files
- 🛡️ **Anti-cheat safe** — config files only; no injection, no memory modification. VAC / EAC / Vanguard safe

## How it works

```
Chamber-GameOptimizer/
├─ engine/cli.ps1        # PowerShell engine: detect, apply, revert, update (CLI + GUI both use it)
├─ games/<slug>/         # manifest.json + config template + benchmark README per game
├─ catalog.json          # versions per game config — drives update checks
├─ schema/               # manifest JSON schema (CI-enforced)
└─ app/                  # Tauri desktop app (dark UI)
```

Each game is a folder with a `manifest.json` declaring how to detect the install (Steam appId, Epic app name, or paths) and which files to write where. The engine backs up the existing file, applies the template (auto-filling `{{refreshRate}}` etc. from your hardware), and records a SHA-256 hash. Status is hash comparison — no guesswork:

Path tokens in `dest`/`detect.paths` resolve at apply time: `{steamuserdata}` (most-recently-used Steam account), `{localappdata}`, `{appdata}`, `{documents}`, and `{custom}` — the folder the user picked for that game via **Set location** (auto-detection is always tried first; the custom folder is a fallback). Set `detect.customLabel` to control the folder-picker prompt.

| Status | Meaning |
|---|---|
| **Applied** | Live file matches what Chamber wrote |
| **Drifted** | File changed since apply (game update or manual edit) — re-apply or revert |
| **Not applied** | No Chamber config recorded |
| **Guide** | Settings are server-side (e.g. Valorant) — a settings card is provided instead |

## Updates without reinstalling

On launch the app checks GitHub:

- **Config updates** (new games, tweaked settings) → banner appears → one click downloads them to a local cache. Bumping a game's `version` in `catalog.json` is all it takes to push an update to every user.
- **App updates** → popup links to the latest [release](../../releases).

## CLI usage (no app needed)

```powershell
.\engine\cli.ps1 -Action scan            # JSON status of all games
.\engine\cli.ps1 -Action apply -Game cs2
.\engine\cli.ps1 -Action revert -Game cs2
.\engine\cli.ps1 -Action set-path -Game cs2 -Path 'D:\Games\cs2'   # custom folder (fallback for detection + {custom})
.\engine\cli.ps1 -Action clear-path -Game cs2                     # forget the custom folder
.\engine\cli.ps1 -Action update          # pull latest configs from GitHub
```

## Building the app

Requires [Rust](https://rustup.rs) and the [Tauri CLI](https://v2.tauri.app) (`cargo install tauri-cli`).

```powershell
cd app/src-tauri
cargo tauri dev      # run with hot reload
cargo tauri build    # produce the NSIS installer
```

## Adding a game

1. Create `games/<slug>/` with `manifest.json` (see `schema/manifest.schema.json`), the config template, and a README with benchmark receipts.
2. Add the game to `catalog.json` with `"version": "1.0.0"`.
3. PR it — CI validates the manifest automatically.

## Disclaimer

Config changes only, always backed up, always revertible — but as with any tweak, understand what a setting does before relying on it in competition.

**Chamber Game Optimizer — built for competitors.**
