# Releasing Chamber Game Optimizer

## Cutting a release

1. **Bump the version** (all three must match the tag, CI enforces this):
   - `app/src-tauri/tauri.conf.json` → `version`
   - `app/src-tauri/Cargo.toml` → `version`
   - `engine/lib.ps1` → `$AppVersion`
2. **Refresh + sign the catalog** if any config changed:
   ```powershell
   pwsh tools/Update-CatalogHashes.ps1
   pwsh tools/Sign-Catalog.ps1        # requires the private key (keyholder only)
   ```
3. Commit, then tag and push:
   ```bash
   git tag v0.2.0 && git push origin main v0.2.0
   ```
4. The `Release` workflow builds the NSIS installer on `windows-latest`, creates a **draft** release, and attaches `catalog.json`, `catalog.json.sig`, and `configs.zip`.
5. Review the draft on GitHub and **publish** it. Publishing is what makes it visible to `releases/latest` — i.e., to every user's update check.

## The signing key

- Private key: `~/.config/chamber-signing/catalog-signing.key` on the keyholder's machine. **Never** commit it or put it in CI secrets — the entire point is that CI/GitHub compromise cannot ship configs.
- Keep an offline backup (USB drive). If the key is lost, users on old installs can no longer receive config updates until they install an app version with a new embedded public key.
- Rotating: generate a new keypair, update `$CatalogPubKeyModulus` in `engine/lib.ps1`, ship an app release, then start signing with the new key.

## Authenticode code signing (TODO — before 1.0)

The installer is currently unsigned, so users see SmartScreen warnings. Plan:

1. Set up [Azure Trusted Signing](https://learn.microsoft.com/en-us/azure/trusted-signing/) (individual/org validation).
2. Add to `tauri.conf.json` under `bundle.windows`:
   ```json
   "signCommand": "trusted-signing-cli -e https://eus.codesigning.azure.net -a %1"
   ```
   with `AZURE_TENANT_ID` / `AZURE_CLIENT_ID` / `AZURE_CLIENT_SECRET` as GitHub Actions secrets, and `cargo install trusted-signing-cli` as a workflow step.
3. Also sign `engine/cli.ps1` + `engine/lib.ps1` (`Set-AuthenticodeSignature`) so the scripts themselves are tamper-evident.

Per-machine install (already configured) puts the app under Program Files, so installed binaries are not writable by user-level malware.
