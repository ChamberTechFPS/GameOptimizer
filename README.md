# Chamber Game Optimizer

Competitive settings for your games, applied in one click. Every change is backed up, and you can undo it exactly.

I optimize gaming PCs at [ChamberTech](https://www.chambertech.net). This app does the part of that job that's the same on every PC. It finds your games, applies the settings I use for competitive play, and shows you every value it changes. It's the companion to the [Chamber Playbook](https://github.com/ChamberTechFPS/Chamber-Playbook): the playbook tunes Windows, this tunes your games.

## What it does

- **Finds your games.** It checks Steam (every library drive), Epic, Riot, Battle.net, EA, and Ubisoft. If a game is installed somewhere unusual, point the app at the folder once and it remembers.
- **Changes only what it lists.** Your sensitivity, binds, and crosshair stay yours.
- **Shows its work.** For every setting you see your value, mine, and one line on why.
- **Catches resets.** When a game update puts something back, the card says so, and Re-apply fixes it.
- **Reverts exactly.** If nothing else touched the file, you get the original back byte for byte. If the game or you changed other things since, only my settings are undone and yours stay.
- **Gets new configs.** New and improved configs come from this repo when I publish them. No reinstall.

## Supported games

| Game | What changes |
|---|---|
| Counter-Strike 2 | 8 video settings in `cs2_video.txt`. Shadows stay high so you can still see enemy shadows. |
| Fortnite | 14 settings, including Performance mode, Reflex, and view distance. |
| Rocket League | 6 effects that hide the ball or cost frames. |
| Valorant | An in-game checklist. Riot keeps these settings on its servers, so no app can write them. |

More are in progress in [`games/drafts`](games/drafts).

## The numbers

Each game shows my before and after once I've benchmarked it: average FPS and 1% lows, with the rig and the method. Until then the app shows no performance claim. No number, no claim.

## Why it's safe

- **Config files only.** These are the same files the game's own menu writes. Nothing is injected or left running, and the app never asks for admin rights.
- **Menu settings only.** Every value is one the game's settings menu already offers. No hidden tweaks.
- **Signed configs.** Config updates are signed with a key that never touches GitHub, so a hacked repo or account can't push configs to your PC. The app refuses anything unsigned or older than what it already has.
- **Locked to config files.** A config can only point at settings files (`.ini`, `.cfg`, `.vcfg`, `.txt`, `.txt0`, `.txt1`, and Battlefield's `PROFSAVE_profile`) under your user folders or a game's install folder. System paths, `..` tricks, and programs are rejected before anything is read.
- **Hands off running games.** The app won't write while a game is running.
- **Backups first.** The first change to any file copies the original to `%LOCALAPPDATA%\ChamberTech\GameOptimizer\backups`.
- **No telemetry.**

## Publishing configs

1. Edit or add `games/<game>.json`. To finish a new game, see [`games/drafts/README.md`](games/drafts/README.md).
2. Rebuild the catalog: `node tools/build-catalog.mjs`
3. Sign it. Only the keyholder can do this:
   `openssl dgst -sha256 -sign ~/.config/chamber-signing/catalog-signing.key -out catalog.json.sig catalog.json`
4. Run the tests: `cargo test --manifest-path src-tauri/Cargo.toml`
5. Commit and push. Players get the new configs the next time they open the app.

CI fails if `catalog.json` is out of date or its signature doesn't match. Never put the private key in the repo or in CI secrets: a key GitHub can reach defeats the point of signing.

## Releasing the app

1. Set the version in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`.
2. Tag it (`v1.0.1`) and push the tag. CI builds the installer into a draft release.
3. Review the draft on GitHub and publish it. The app's update check only sees published releases.

The installer isn't code-signed yet, so Windows SmartScreen warns on first run. Signing comes before public launch.

## Building from source

You need [Rust](https://rustup.rs) and Node.

- Test the engine: `cargo test --manifest-path src-tauri/Cargo.toml`
- Run the app on Windows: `npx @tauri-apps/cli@2 dev`
- Build the installer: `npx @tauri-apps/cli@2 build`
- Preview the UI in any browser with sample data: `python3 -m http.server --directory ui`

## Credits

[Inter](https://rsms.me/inter/) (SIL OFL 1.1), [Lucide](https://lucide.dev) icons (ISC), and [Simple Icons](https://simpleicons.org) brand marks (CC0).
