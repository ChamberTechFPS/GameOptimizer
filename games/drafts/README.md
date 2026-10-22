# Draft games

These games are picked from the most-watched and most-played games of October 2026 that reward high FPS and low latency. Each draft already has its detection, process names, and config file path. You add the settings.

Drafts never ship. `tools/build-catalog.mjs` only reads `games/*.json`, so a draft goes live once you finish it and move it up a folder.

## Why these games (data pulled 2026-10-22)

Columns:
- **Twitch**: average concurrent viewers over the last 7 days ([TwitchTracker](https://twitchtracker.com/games)).
- **Kick**: live viewers at the moment I checked ([Kick categories](https://kick.com/browse/categories)).
- **Steam**: concurrent players ([Steam most played](https://store.steampowered.com/charts/mostplayed)).

A dash means the game wasn't in that ranking, or isn't on that platform.

| Game | Twitch | Kick | Steam | Status |
|---|---|---|---|---|
| League of Legends | #2 · 150K | 30.5K | not on Steam | draft (checklist) |
| Counter-Strike 2 | #6 · 65.0K | 56.7K | #1 · 1,084,013 | shipped in v1 |
| VALORANT | #7 · 61.7K | 23.5K | not on Steam | shipped in v1 |
| Fortnite | #8 · 54.5K | 9.7K | not on Steam | shipped in v1 |
| Dota 2 | #10 · 46.3K | 22.3K | #2 · 960,326 | draft |
| ARC Raiders | #11 · 41.3K | – | #14 · 81,817 | draft |
| Overwatch 2 | #13 · 29.3K | – | #25 · 57,140 | draft |
| Call of Duty: Modern Warfare 4 | #15 · 27.9K | 17.6K | #9 · 119,808 (Call of Duty app) | draft (call-of-duty) |
| Deadlock | #17 · 24.8K | – | #5 · 206,050 | draft |
| WARDOGS | #18 · 24.5K | 5.0K | #7 · 170,247 | draft |
| Marvel Rivals | #20 · 18.6K | – | #33 · 45,742 | draft |
| Apex Legends | #23 · 15.8K | – | #16 · 79,959 | draft |
| Escape from Tarkov | #31 · 10.6K | – | #71 · 21,374 | draft |
| Rocket League | #36 · 9.8K | – | #61 · 25,598 | shipped in v1 |
| Rainbow Six Siege | #38 · 9.7K | – | #20 · 62,306 | draft |
| Call of Duty: Warzone | #39 · 9.5K | – | (Call of Duty app) | draft (call-of-duty) |
| PUBG: Battlegrounds | – | – | #3 · 287,432 | draft |
| Rust | – | 6.8K | #10 · 112,054 | draft |
| Delta Force | – | – | #34 · 45,677 (peak 106,853) | draft |
| Battlefield 6 | – | – | #42 · 42,306 | draft |
| THE FINALS | – | – | #63 · 24,523 | draft |

**YouTube.** I couldn't get a per-game YouTube Live ranking: Streams Charts blocks automated reads, and Stream Hatchet's full reports are gated. The Q2 2026 cross-platform numbers point the same way as the table:
- Counter-Strike 2 was the most-watched esport, at 209M hours.
- The biggest battle royales were Free Fire (601M hours), PUBG Mobile (459M), and Fortnite (394M).
- Sources: [Stream Hatchet Q2 2026 report](https://hatchet.gg/resources/guides/q2-2026-live-streaming-trends-report/), [Streamlabs × Hatchet Q2 2026](https://streamlabs.com/content-hub/post/streamlabs-and-hatchet-q2-2026-live-streaming-report).

**Overall most played.** Roblox, Minecraft, League of Legends, and Fortnite lead monthly players across all platforms ([DemandSage](https://www.demandsage.com/most-played-games-right-now/), [Newzoo PC and console report](https://investgame.net/wp-content/uploads/2026/03/2026-03-12-newzoo_2026_pc_and_console_gaming_report_wp.pdf)). On Steam, Counter-Strike 2 leads with over a million concurrent players.

**Left out on purpose:**

| Game | Reason |
|---|---|
| GTA V, Minecraft, Roblox, World of Warcraft, AION 2 | They don't hinge on frame rate or latency. |
| EA Sports FC 27 | Capped at 60 FPS. |
| PUBG Mobile, Free Fire, BGMI | Phone games; this app is Windows-only. |
| Team Fortress 2, War Thunder, Hunt: Showdown 1896 | Smaller audiences or less latency-bound. Add later if you want. |

## What still needs checking on your PC

| Draft | Settings file | Confidence | Check |
|---|---|---|---|
| league-of-legends | none (checklist) | high | League copies settings from Riot's servers and overwrites local edits ([Riot support](https://support.riotgames.com/riot/performance/reset-configuration-files)). It works like Valorant: fill `manual` with what to set in-game. |
| dota-2 | `{install}/game/dota/cfg/video.txt` (Valve kv) | high ([Liquipedia](https://liquipedia.net/dota2/Console)) | Leave the VendorID and DeviceID lines alone. Chamber never touches keys you don't list. |
| arc-raiders | `%LOCALAPPDATA%\PioneerGame\...\WindowsClient\GameUserSettings.ini` | medium ([FandomWire](https://fandomwire.com/want-better-aim-in-arc-raiders-tweak-this-hidden-setting-right-now/)) | Process name unknown: add it from Task Manager. **Menu settings only.** Players got flagged for hidden config tweaks ([MP1st](https://mp1st.com/news/arc-raiders-config-settings-unfair-advantages)). |
| overwatch-2 | `Documents\Overwatch\Settings\Settings_v0.ini` | medium | Values may be quoted (`Key = "0"`). If they are, write the quotes into `value`; the engine allows them in INI files. |
| deadlock | `{install}/game/citadel/cfg/video.txt` (Valve kv) | high ([Steam discussion](https://steamcommunity.com/app/1422450/discussions/0/4758704348167575605)) | Same VendorID and DeviceID note as Dota 2. |
| wardogs | `%LOCALAPPDATA%\Wardogs\...\WindowsClient\GameUserSettings.ini` | low (third-party guides, early access) | Confirm the path after launching once, and add the process name. Recheck after big patches. |
| marvel-rivals | `%LOCALAPPDATA%\Marvel\Saved\Config\Windows\GameUserSettings.ini` | high ([Eklipse](https://blog.eklipse.gg/tips-trivia/disable-mouse-acceleration-marvel-rivals.html)) | The file can be missing on a fresh install; the app shows "Launch once" until it exists. |
| apex-legends | `Saved Games\Respawn\Apex\local\videoconfig.txt` (Valve kv) | high | Confirm whether the DX12 build still uses `r5apex_dx12.exe`. |
| pubg | `%LOCALAPPDATA%\TslGame\...\WindowsNoEditor\GameUserSettings.ini` | high | Graphics may sit in one `ScalabilityGroups=(...)` value. If so, the whole bracketed value is one setting. |
| delta-force | `{install}/Game/DeltaForce/Saved/Config/WindowsClient/GameUserSettings.ini` | medium ([Steam discussion](https://steamcommunity.com/app/2507950/discussions/0/595136892797804254)) | Process name unknown. Check whether an `%LOCALAPPDATA%\DeltaForce` copy wins instead. |
| the-finals | `%LOCALAPPDATA%\Discovery\...\WindowsClient\GameUserSettings.ini` | low (inferred from Unreal's layout) | Confirm the folder name first. |
| call-of-duty | `%LOCALAPPDATA%\Activision\Call of Duty\players\s.1.0.cod26.txt0` (cod) | medium (a third-party MW4 script and real Black Ops 6 files) | One draft covers MW4 and Warzone, since both run in the Call of Duty app. I couldn't confirm that Warzone reads this same file. This is where Battle.net keeps it. Steam keeps it in `Documents\Call of Duty\players`, and a game file can point at only one folder for now. If `s.1.0.cod26.txt1` sits beside it, add it as a second file with the same settings. Confirm `cod26-cod.exe` in Task Manager while MW4 runs. |
| rainbow-six-siege | `Documents\My Games\Rainbow Six - Siege\<account id>\GameSettings.ini` (ini) | high (Ubisoft support's forum replies) | The `*` folder covers every account that has played on this PC. Confirm `RainbowSix.exe` in Task Manager, since Siege X may have renamed it. |
| rust | `{install}/cfg/client.cfg` (convar) | high (real client.cfg files) | The file holds every console variable, not just menu ones. Use only what the options menu sets. Rust writes this file when it quits. |
| battlefield-6 | `Documents\Battlefield 6\settings\<launcher>\PROFSAVE_profile` (spaced) | medium (guides and a settings tool; nothing official) | The `*` folder matches the launcher folder, like `steam` or `ea`. If your file sits straight in `settings`, take `*/` out of the path. Confirm `bf6.exe` in Task Manager. |
| escape-from-tarkov | `%APPDATA%\Battlestate Games\Escape from Tarkov\Settings\Graphics.ini` (json) | high (real Graphics.ini files) | The file is JSON despite its name. Check that the Steam copy uses this same folder. |

## Finishing a draft

1. Launch the game once and set every option you care about in its own menu. Then close it.
2. Open the settings file and find the key for each option. Add an entry for each to `settings`: `key`, `value`, `label`, `values` (display names), and `why` (one plain sentence).
3. Only use settings the game's menu also exposes. Never hidden ones. That keeps Chamber anti-cheat safe.
4. Fill in any missing `process` names from Task Manager. They let Chamber refuse to write while the game is running.
5. Move the file up to `games/`. Then run:
   - `node tools/build-catalog.mjs`
   - `cargo test --manifest-path src-tauri/Cargo.toml` (checks every field)
   - the sign command it prints
6. Benchmark it and add `proof` (rig, avg and 1% lows before and after, method). Without it, the app shows no performance claim.

## File formats

Each file in `files` names its `format`. Write keys and values exactly as the file stores them.

| Format | What it edits | `key` | Notes |
|---|---|---|---|
| `ini` | `Key=Value` under `[Section]` | the key, plus `section` | |
| `kv` | `"key"  "value"` (Valve) | the key | No quotes in values. |
| `cod` | `Name@0;id;id = value // note` (Call of Duty) | `Name@0`: the name and its variant | The ids and notes stay as the game wrote them. Chamber can't add a setting the file lacks. No `//` in values. |
| `convar` | `name "value"` (Rust) | the name | No quotes in values. A missing one is added at the end. |
| `spaced` | `Key value` (Battlefield) | the key | The value is everything after the first space. A missing one is added at the end. |
| `json` | one value in a JSON file (Tarkov) | a dotted path, like `DisplaySettings.FullScreenMode` | Values are raw JSON: `false`, `144`, or `"High"` with its quotes. Nothing else gets reformatted. A missing key is added to its object, but a missing object is never created. |

One folder per account? Put `*` in its place: `{documents}/My Games/Rainbow Six - Siege/*/GameSettings.ini`. Chamber edits every copy it finds and backs up each one. A path gets one `*` at most, and never as its first folder or as the file.
