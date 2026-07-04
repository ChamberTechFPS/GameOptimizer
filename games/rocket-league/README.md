# Rocket League — Chamber config

Applies competitive video settings via `Documents\My Games\Rocket League\TAGame\Config\TASystemSettings.ini`.

## What it changes

| Setting | Value | Why |
|---|---|---|
| Motion blur / radial blur | Off | Removes smearing during fast aerials and flicks |
| Depth of field, bloom, lens flares, light shafts | Off | Cleaner ball and boost visibility, fewer GPU spikes |
| Distortion | Off | Boost/explosion effects stop warping the screen |
| Dynamic shadows | Off | Big frame-time win; car shadow ("blob") remains for reads |
| Ambient occlusion | Off | GPU cost with no competitive value |
| VSync | Off | Input latency |
| FPS cap (`MaxSmoothedFrameRate`) | 250 | High, stable cap instead of uncapped frame-time swings |
| Resolution | Auto-filled from your display | Native res, exclusive fullscreen |

## Notes

- **Apply with the game closed.** Rocket League rewrites this file when you change settings in-game — if the status shows *Drifted* after playing, that's why. Re-apply to restore the Chamber values (your first backup is kept for revert).
- Anti-cheat safe: plain config file that the game itself reads and writes; no injection.
- Works for both the Steam and Epic versions — the file lives in `Documents` either way.
