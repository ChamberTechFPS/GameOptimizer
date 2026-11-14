# ChamberTech FPS Optimizer

Competitive settings for your games, applied in one click. Every change is backed up, and you can undo it exactly.

I optimize gaming PCs at [ChamberTech](https://www.chambertech.net). This app does the part of that job that's the same on every PC. It finds your games, applies the settings I use for competitive play, and shows you every value it changes. FPS Optimizer is the companion to the [ChamberTech Playbook](https://github.com/ChamberTechFPS/Playbook): the playbook tunes Windows, this tunes your games. Run the playbook first, then this.

## What it does

- **Finds your games.** It checks Steam (every library drive), Epic, Riot, Battle.net, EA, and Ubisoft. Keep games somewhere else, like `D:\Games`? Add that folder in Library and the app looks inside it. You can also point a single game at its folder.
- **Changes only what it lists.** Your sensitivity, binds, and crosshair stay yours.
- **Shows its work.** For every setting you see your value, mine, and one line on why.
- **Catches resets.** When a game update puts something back, the card says so, and Re-apply fixes it.
- **Reverts exactly.** If nothing else touched the file, you get the original back byte for byte. If the game or you changed other things since, only my settings are undone and yours stay.
- **Tunes Windows too, if you want.** Pick a profile (Esports Pro, Casual Gamer, or Security) and the app sets the matching Windows settings. No playbook needed. Reset puts them back to the Windows defaults, including on a PC the playbook tuned.
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

## Windows tweaks

Not everyone wants a full playbook install, and some people who ran it want a setting back. The Windows tweaks section covers both.

| Profile | What it's for |
|---|---|
| Esports Pro | Everything for the lowest latency. Turns off game capture and mouse acceleration. |
| Casual Gamer | The safe wins. Clips and capture keep working. |
| Security | Turns virtualization-based security and Memory Integrity back on. Valorant and FACEIT expect them. |

Every tweak is a registry value with two sides: what the tweak sets, and the Windows default. Apply writes the first. Reset writes the second. Reset doesn't depend on a record of what the app did, so it also undoes those same values after the playbook set them. It can't bring back services or apps the playbook removed: that takes a Windows reinstall.

The tweaks live in [`tweaks.json`](tweaks.json). To add one, add an entry under `tweaks` and list its id in the profiles that should apply it:

```json
{
  "id": "foreground-boost",
  "name": "Foreground boost",
  "why": "The window you're playing in gets short, boosted CPU time slices.",
  "restart": true,
  "values": [
    { "hive": "HKLM", "key": "SYSTEM\\CurrentControlSet\\Control\\PriorityControl", "name": "Win32PrioritySeparation", "type": "dword", "set": "38", "default": "2" }
  ]
}
```

- `hive` is `HKCU` (this player, no prompt) or `HKLM` (the whole PC, Windows asks for permission).
- `type` is `dword` (a number in decimal) or `sz` (short text).
- `set` is what Apply writes. `default` is what Reset writes. `null` on either side means "remove the value".
- A new profile is one more entry under `profiles`.

`cargo test` checks the file. A tweak can only touch keys under the roots listed in `HKLM_ROOTS` and `HKCU_ROOTS` in [`src-tauri/src/tweaks.rs`](src-tauri/src/tweaks.rs). A key outside them fails the test until you add its root there. That is deliberate: the list is where you stop and check what else lives under a key.

Check each `default` on a stock Windows install before you ship it. A wrong default makes Reset write the wrong value.

## The numbers

Each game shows my before and after once I've benchmarked it: average FPS and 1% lows, with the rig and the method. Until then the app shows no performance claim. No number, no claim.

## Why it's safe

- **Config files only, for games.** These are the same files the game's own menu writes. Nothing is injected or left running.
- **Admin only when you ask.** The app runs without admin rights. Windows shows its permission prompt only when you apply or reset a tweak that covers the whole PC. The step that runs with those rights takes a profile name and nothing else, and it can only write the values compiled into the app.
- **Windows tweaks aren't downloaded.** They ship inside the app, so a config update can never change a Windows setting. Each one is a number or a short word under an allowed registry key. Paths, programs, and startup entries are rejected.
- **Safe settings only.** Most values are ones the game's settings menu already offers. A few only exist in the config file, like Call of Duty's renderer threads. I use those only where players commonly change them and the game isn't known to flag it, and each one says why.
- **Signed configs.** Config updates are signed with a key that never touches GitHub, so a hacked repo or account can't push configs to your PC. The app refuses anything unsigned or older than what it already has.
- **Locked to config files.** A config can only point at settings files (`.ini`, `.cfg`, `.vcfg`, `.txt`, `.txt0`, `.txt1`, and Battlefield's `PROFSAVE_profile`) under your user folders or a game's install folder. System paths, `..` tricks, and programs are rejected before anything is read.
- **Hands off running games.** The app won't write while a game is running.
- **Backups first.** The first change to any file copies the original to `%LOCALAPPDATA%\ChamberTech\GameOptimizer\backups`. The folder kept its name from before the rename so existing backups still revert.
- **No telemetry.**

## Terms

Using the app means accepting the [terms of use](ui/terms.md). The app asks on first launch. To change them, edit `ui/terms.md` and bump its `Version N` line: every player is asked to accept the new version.

## Publishing configs

Configs ship as one signed file, `catalog.json`, from a **public** host. The addresses live in [`ui/hosts.json`](ui/hosts.json). The app repo can stay private: `raw.githubusercontent.com` returns 404 for private files, and players would never receive a catalog published only here. Creating that host, and how to sign the next catalog, are in [RELEASING.md](RELEASING.md).

After the host exists:

1. Edit or add `games/<game>.json`. Set `find.exe` (the exe's path inside the install folder) so the game turns up in players' own game folders. To finish a new game, see [`games/drafts/README.md`](games/drafts/README.md).
2. Rebuild the catalog: `node tools/build-catalog.mjs`
3. Sign it. Only the keyholder can do this. The private key lives offline, passphrase-protected, on removable media. Never commit `signing/` or `*.key`, and never write its real path in this repo.
   `node tools/sign-catalog.mjs <path-to-key>`
4. Run `node tools/verify-catalog.mjs` and `cargo test --manifest-path src-tauri/Cargo.toml`
5. Commit `catalog.json` and `catalog.json.sig` together, push, and copy those two files, unchanged, to the public catalog host.

CI fails if `catalog.json` doesn't match `games/`, or if the signature doesn't verify and the files aren't the one already-known unsigned snapshot. A release fails on any bad signature. Never put the private key in the repo or in CI secrets: a key GitHub can reach defeats the point of signing. Keep the key offline and keep a backup, on a USB stick or in a password manager. Losing it means a new key and a new app release. The app trusts the current key and one spare slot, so the next rotation doesn't have to strand anyone who already updated. See [RELEASING.md](RELEASING.md).

## Releasing the app

1. Set the same version in `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`. For a beta, use the full prerelease (`1.0.0-beta.2`), not `1.0.0`. The app reports that string, and a beta stamped `1.0.0` can't tell a newer beta from itself.
2. Tag it (`v1.0.1`, or `v1.0.0-beta.2`) and push the tag. The tag has to be `v` plus that version. CI builds the installer.
3. A stable tag becomes a draft release. Review it and publish it. A tag with a hyphen becomes a prerelease. The app reads the releases list, not `/releases/latest`, because `/latest` hides prereleases. Stable builds ignore prereleases. A build whose own version is a prerelease sees newer betas.

Each release ships `SHA256SUMS.txt`, like the playbook does. The installer isn't code-signed yet, so Windows SmartScreen warns on first run. Signing comes before public launch.

The workflows pin every action to a commit and the Tauri CLI to an exact version, and the build uses `Cargo.lock` as committed. Dependabot opens the PRs that move those pins.

Upgrading from a Chamber Game Optimizer beta: uninstall the old app first. The new name installs beside it, and both would share the same settings and backups. Update checks 404 until the repo that holds the releases is public.

## Building from source

You need [Rust](https://rustup.rs) and Node.

- Test the engine: `cargo test --manifest-path src-tauri/Cargo.toml`
- Run the app on Windows: `npx @tauri-apps/cli@2.12.4 dev`
- Build the installer: `npx @tauri-apps/cli@2.12.4 build -- --locked`
- Preview the UI in any browser with sample data: `python3 -m http.server --directory ui`

## Questions

Email [chamber@chambertech.net](mailto:chamber@chambertech.net).

## Credits

[Inter](https://rsms.me/inter/) (SIL OFL 1.1), [Lucide](https://lucide.dev) icons (ISC), and [Simple Icons](https://simpleicons.org) brand marks (CC0).
