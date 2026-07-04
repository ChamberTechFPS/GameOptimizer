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

- **Config updates** (new games, tweaked settings) → banner appears → one click downloads them to a local cache. Bumping a game's `version` in `catalog.json` (and re-signing) is all it takes to push an update to every user.
- **App updates** → popup links to the latest [release](../../releases).

### Update security

Config updates are **signed and pinned** — a compromised repo or GitHub account can't push configs to users:

1. `catalog.json` carries a SHA-256 for every config file; the catalog itself is signed offline with the maintainer's RSA-3072 key (`catalog.json.sig`). The public key is embedded in `engine/lib.ps1`.
2. The engine downloads the catalog + signature from immutable **release assets** (never a branch), verifies the signature, rejects version rollbacks, then fetches config files from the release's tag and verifies each file's hash before atomically swapping it into the cache. Templates are re-hashed against the signed catalog again at apply time.
3. File names, slugs, and destination paths from the update channel are validated at runtime — dests must stay under an allowlisted root (`{localappdata}`, `{documents}`, …); traversal and absolute paths are rejected.

Maintainer flow after changing any config: `pwsh tools/Update-CatalogHashes.ps1` → `pwsh tools/Sign-Catalog.ps1` (requires the private key, which never enters the repo or CI) → commit → tag a release. CI fails if hashes are stale or the signature doesn't match.

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
2. Add the game to `catalog.json` with `"version": "1.0.0"` and run `pwsh tools/Update-CatalogHashes.ps1` to fill in the file hashes.
3. PR it — CI validates the manifest, catalog, and hash freshness automatically. The maintainer re-signs the catalog when merging.

## Disclaimer

Config changes only, always backed up, always revertible — but as with any tweak, understand what a setting does before relying on it in competition.

**Chamber Game Optimizer — built for competitors.**
