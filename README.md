# Chamber Game Optimizer

Competitive settings for your games, applied in one click. Every change is backed up, and you can undo it exactly.

I optimize gaming PCs at [ChamberTech](https://www.chambertech.net). This app does the part of that job that's the same on every PC. It finds your games, applies the settings I use for competitive play, and shows you every value it changes. It's the companion to the [Chamber Playbook](https://github.com/ChamberTechFPS/Chamber-Playbook): the playbook tunes Windows, this tunes your games.

## What it does

- **Finds your games.** It checks Steam (every library drive), Epic, Riot, Battle.net, EA, and Ubisoft. Keep games somewhere else, like `D:\Games`? Add that folder in Library and the app looks inside it. You can also point a single game at its folder.
- **Changes only what it lists.** Your sensitivity, binds, and crosshair stay yours.
- **Shows its work.** For every setting you see your value, mine, and one line on why.
- **Catches resets.** When a game update puts something back, the card says so, and Re-apply fixes it.
- **Reverts exactly.** If nothing else touched the file, you get the original back byte for byte. If the game or you changed other things since, only my settings are undone and yours stay.
- **Gets new configs.** New and improved configs download from a public signed catalog when the app opens. No reinstall.

## Supported games

| Game | What changes |
|---|---|
| Counter-Strike 2 | 8 video settings in `cs2_video.txt`. Shadows stay high so you can still see enemy shadows. |
| Call of Duty | 5 settings for Modern Warfare 4: exclusive fullscreen, your monitor's native resolution and refresh rate, and renderer threads matched to your CPU. |
| Fortnite | 14 settings, including Performance mode, Reflex, and view distance. |
| Rocket League | 6 effects that hide the ball or cost frames. |
| Valorant | An in-game checklist. Riot keeps these settings on its servers, so no app can write them. |

More are in progress in [`games/drafts`](games/drafts).

## The numbers

Each game shows my before and after once I've benchmarked it: average FPS and 1% lows, with the rig and the method. Until then the app shows no performance claim. No number, no claim.

## Why it's safe

- **Config files only.** These are the same files the game's own menu writes. Nothing is injected or left running, and the app never asks for admin rights.
- **Safe settings only.** Most values are ones the game's settings menu already offers. A few only exist in the config file, like Call of Duty's renderer threads. I use those only where players commonly change them and the game isn't known to flag it, and each one says why.
- **Signed configs.** Config updates are signed with a key that never touches GitHub, so a hacked repo or account can't push configs to your PC. The app refuses anything unsigned or older than what it already has.
- **Locked to config files.** A config can only point at settings files (`.ini`, `.cfg`, `.vcfg`, `.txt`, `.txt0`, `.txt1`, and Battlefield's `PROFSAVE_profile`) under your user folders or a game's install folder. System paths, `..` tricks, and programs are rejected before anything is read.
- **Hands off running games.** The app won't write while a game is running.
- **Backups first.** The first change to any file copies the original to `%LOCALAPPDATA%\ChamberTech\GameOptimizer\backups`.
- **No telemetry.**

## Terms

Using the app means accepting the [terms of use](ui/terms.md). The app asks on first launch. To change them, edit `ui/terms.md` and bump its `Version N` line: every player is asked to accept the new version.

## Publishing configs

Configs ship as one signed file, `catalog.json`, from a **public** host. The addresses live in [`ui/hosts.json`](ui/hosts.json). The app repo can stay private: `raw.githubusercontent.com` returns 404 for private files, and players would never receive a catalog published only here. Creating that host, signing, and what to do about the catalog that's already out of date are in [RELEASING.md](RELEASING.md).

After the host exists:

1. Edit or add `games/<game>.json`. Set `find.exe` (the exe's path inside the install folder) so the game turns up in players' own game folders. To finish a new game, see [`games/drafts/README.md`](games/drafts/README.md).
2. Rebuild the catalog: `node tools/build-catalog.mjs`
3. Sign it. Only the keyholder can do this: `node tools/sign-catalog.mjs <path-to-private-key>`
4. Run `node tools/verify-catalog.mjs` and `cargo test --manifest-path src-tauri/Cargo.toml`
5. Commit `catalog.json` and `catalog.json.sig` together, push, and copy those two files, unchanged, to the public catalog host.

CI fails if `catalog.json` doesn't match `games/` or the signature doesn't verify. Never put the private key in the repo or in CI secrets: a key GitHub can reach defeats the point of signing. Keep an offline backup of the key, on a USB stick or in a password manager. Losing it means a new key and a new app release. The app trusts the current key and one spare slot, so the next rotation doesn't have to strand anyone who already updated. See [RELEASING.md](RELEASING.md).

## Releasing the app

1. Set the same version in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`. For a beta, use the full prerelease (`1.0.0-beta.2`), not `1.0.0`. The app reports that string, and a beta stamped `1.0.0` can't tell a newer beta from itself.
2. Tag it (`v1.0.1`, or `v1.0.0-beta.2`) and push the tag. The tag has to be `v` plus that version. CI builds the installer.
3. A stable tag becomes a draft release. Review it and publish it. A tag with a hyphen becomes a prerelease. The app reads the releases list, not `/releases/latest`, because `/latest` hides prereleases. Stable builds ignore prereleases. A build whose own version is a prerelease sees newer betas.

The installer isn't code-signed yet, so Windows SmartScreen warns on first run. Signing comes before public launch. Update checks 404 until the repo that holds the releases is public.

## Building from source

You need [Rust](https://rustup.rs) and Node.

- Test the engine: `cargo test --manifest-path src-tauri/Cargo.toml`
- Run the app on Windows: `npx @tauri-apps/cli@2 dev`
- Build the installer: `npx @tauri-apps/cli@2 build`
- Preview the UI in any browser with sample data: `python3 -m http.server --directory ui`

## Credits

[Inter](https://rsms.me/inter/) (SIL OFL 1.1), [Lucide](https://lucide.dev) icons (ISC), and [Simple Icons](https://simpleicons.org) brand marks (CC0).
