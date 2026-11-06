# Releasing Chamber Game Optimizer

The app only applies a catalog that verifies against a public key compiled into it, and it only downloads that catalog from the host in [`ui/hosts.json`](ui/hosts.json). Two things are still on you. Nobody else has the private key, and nobody else should create the public repo.

## Still to do before players can get configs

1. **Create a public catalog host** and publish `catalog.json` and `catalog.json.sig` there. The default URL is a repo that does not exist yet. Do not create it from a bot; create it yourself.
2. **Make app releases reachable.** Update checks call `api.github.com` for this repo. That 404s while the repo is private. The app now says so, instead of failing silently. Publish the repo, or point `releasesUrl` at a public repo that actually has the releases.

The catalog in git already verifies. `catalog.json` is the same bytes as on `rebuild`, and `catalog.json.sig` is the signature from that branch for the offline RSA-4096 key. CI still allows one historical unsigned snapshot (the catalog as edited after commit `033c850`, still carrying that commit's signature) and fails any other bytes that do not verify. `release.yml` and `test-build.yml` do not allow that snapshot. A built app loads this catalog.

v1.0.0-beta.1 (about two downloads) trusts only the old RSA-3072 key. That private key is gone. Those installs will reject every newly signed catalog. They need one reinstall. The spare key slot below is so the next rotation does not do that again.

## The signing key

`CURRENT_MODULUS_HEX` in `src-tauri/src/catalog.rs` is the offline RSA-4096 public key that `rebuild` trusts. The private key stays on the Windows PC at `F:\GameOptimizer\signing\catalog-signing.key`. It is not in this repo. `.gitignore` ignores `signing/` and `*.key`.

The previous RSA-4096 private key (the one that signed commit `033c850`) is unavailable, so this branch does not trust that modulus. `SPARE_MODULUS_HEX` stays empty until the next rotation. Leave the old modulus out of both `CURRENT` and `SPARE`.

`TRUSTED_MODULI` is `CURRENT_MODULUS_HEX`, then `SPARE_MODULUS_HEX`. A signature from either non-empty key is accepted. Empty is ignored.

The known-unsigned allowlist is that older pair only. The hashes live in `tools/verify-catalog.mjs` and `catalog::tests`. The files in git are the signed pair, so the allowlist is not what makes them pass. `.gitattributes` marks `catalog.json` as `-text` and `catalog.json.sig` as binary so a later checkout cannot rewrite them. `catalog.json` is LF-only.

## Re-sign the catalog

Do this when `games/*.json` changes, from the repo root, on the machine that has the private key. Git for Windows includes `openssl`. The committed catalog already verifies, so this is the next change, not a fix for the files in git.

1. Confirm the key is the one the app embeds. The hex after `Modulus=` must equal `CURRENT_MODULUS_HEX` in `src-tauri/src/catalog.rs` (ignore case):

   ```bash
   openssl pkey -in 'F:\GameOptimizer\signing\catalog-signing.key' -pubout -outform DER \
     | openssl rsa -pubin -inform DER -modulus -noout
   ```

   A traditional PKCS#1 PEM also works:

   ```bash
   openssl rsa -in 'F:\GameOptimizer\signing\catalog-signing.key' -modulus -noout
   ```

   If it does not match, stop. You have the wrong file. Pasting a different modulus into `CURRENT_MODULUS_HEX` means every build that already shipped this key will reject catalogs signed by the new key. Use the spare-key steps further down instead of swapping `CURRENT` out from under a release.

2. Rebuild only if `games/*.json` changed, then sign the exact bytes. `--check` does not write. The signer refuses a key inside the repo and a key the app does not trust, and it will not sign a catalog that doesn't match `games/`.

   ```bash
   node tools/build-catalog.mjs --check
   node tools/sign-catalog.mjs 'F:\GameOptimizer\signing\catalog-signing.key'
   ```

   The script runs this openssl command, then `node tools/verify-catalog.mjs`:

   ```bash
   openssl dgst -sha256 -sign 'F:\GameOptimizer\signing\catalog-signing.key' -out catalog.json.sig catalog.json
   ```

3. Confirm, then commit **both** files. Do not open `catalog.json` in an editor first. Do not let a tool change its line endings.

   ```bash
   node tools/verify-catalog.mjs
   cargo test --manifest-path src-tauri/Cargo.toml
   git add catalog.json catalog.json.sig
   ```

`verify-catalog.mjs` prints the same commands when the signature does not match. CI runs it too.

## Create the public catalog host

Players fetch `catalog.json` and `catalog.json.sig` from `ui/hosts.json`. The default is:

`https://raw.githubusercontent.com/ChamberTechFPS/GameOptimizer-catalog/main/catalog.json`

`raw.githubusercontent.com` and `api.github.com` are already in the app's CSP `connect-src`. A different origin will not load until you add it there. `node tools/check-hosts.mjs` fails if a fetch origin is missing.

Do this yourself:

1. Create a **public** repo `ChamberTechFPS/GameOptimizer-catalog` (or another name, and then change `catalogUrl` and `catalogSigUrl` in `ui/hosts.json`).
2. Put the signed `catalog.json` and `catalog.json.sig` on `main`. Push them with git. Don't paste `catalog.json` into the GitHub web editor; that can change newlines and break the signature.
3. Copy these lines into that repo's `.gitattributes` so it doesn't rewrite the files either:

   ```
   catalog.json -text linguist-language=JSON
   catalog.json.sig binary
   ```

4. Open the raw URL in a private window. You should get the file, not a 404. A private repo always 404s for the app.

Other hosts, if you would rather not use a second GitHub repo:

- **GitHub Pages** for that repo: `https://chambertechfps.github.io/GameOptimizer-catalog/catalog.json` and the matching `.sig` URL. Add `https://chambertechfps.github.io` to `connect-src` in `src-tauri/tauri.conf.json`.
- **chambertech.net**: put the two files at a stable https path and add that origin (for example `https://www.chambertech.net`) to `connect-src`.

Then run `node tools/check-hosts.mjs`. If the catalog host is no longer GitHub, edit the "Your data" section of `ui/terms.md` and bump its `Version N` line. The current terms say the app contacts the catalog host and GitHub.

Changing `ui/hosts.json` is the whole host change. Don't also hardcode a URL in `ui/app.js`.

## App updates

`releasesUrl` is the GitHub releases **list** (`/releases?per_page=30`), not `/releases/latest`. `/latest` never returns a prerelease, so a beta tester would never see a newer beta.

- A stable version (`1.0.0`) is offered only newer stable releases.
- A prerelease version (`1.0.0-beta.2`) is offered newer betas and the stable release that replaces them.
- `includePrereleases: true` in `ui/hosts.json` forces betas on even for a stable version. Leave it false.
- The release page has to start with `releasePagePrefix`. Anything else is ignored, and if the only newer page is off that prefix the app says it couldn't check.

Set a beta's `version` in both `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` to the full prerelease, and tag `v` plus that string (`v1.0.0-beta.2`). v1.0.0-beta.1 was stamped `1.0.0`, so it cannot see later betas even with this logic. Those installs are the ones that need a reinstall anyway.

While this repo is private, the releases request 404s and the app shows **Couldn't check for updates**. Same for a missing catalog host: **Couldn't check for new configs**. A catalog that downloads but fails the signature shows **New configs were rejected**. Hover the chip for the status code.

## Publishing the next catalog

1. Edit `games/<game>.json`.
2. `node tools/build-catalog.mjs`
3. `node tools/sign-catalog.mjs F:\GameOptimizer\signing\catalog-signing.key`
4. `node tools/verify-catalog.mjs` and `cargo test --manifest-path src-tauri/Cargo.toml`
5. Commit `catalog.json` and `catalog.json.sig` together.
6. Copy those two files, byte for byte, to the public catalog host.

`version` in the catalog is a unix timestamp, chosen only when the games change, so two rebuilds of the same edit won't match. Sign the file the writer just produced. `--check` rebuilds nothing; CI uses it to prove `catalog.json` is the canonical LF bytes for `games/`.

## Rotating the signing key later

The app trusts `TRUSTED_MODULI` in `src-tauri/src/catalog.rs`: `CURRENT_MODULUS_HEX`, then `SPARE_MODULUS_HEX`. The spare is empty. A signature from either non-empty key is accepted. Empty is ignored.

1. Generate the next key **offline**. Don't put it in the repo or in CI.

   ```bash
   openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:4096 -out catalog-signing-next.key
   openssl pkey -in catalog-signing-next.key -pubout -outform DER \
     | openssl rsa -pubin -inform DER -modulus -noout
   ```

2. Paste the modulus hex into `SPARE_MODULUS_HEX` (not `CURRENT`). Leave the current key where it is.
3. Ship an app release and wait until the installs you care about are on that build. They now trust both keys.
4. Sign new catalogs with the new private key. `sign-catalog.mjs` allows the spare and says so. Old catalogs signed by the current key still load.
5. When you retire the old key, move the spare modulus into `CURRENT_MODULUS_HEX`, put the following key in `SPARE_MODULUS_HEX` or clear it, and ship another app release **before** you stop trusting the old modulus. Dropping a modulus strands anyone whose newest catalog was signed only by it.

v1.0.0-beta.1 never got a build that trusts the 4096-bit key, and the 3072-bit private key is lost, so there is no signature those installs will accept. Reinstalling is the fix. Putting the dead 3072-bit public key back would not let them take a new catalog.

## Don't

- Don't commit the private key, and don't add it as a CI secret. A key GitHub can read defeats the signature.
- Don't reformat `catalog.json`, and don't "fix" its line endings. Sign the bytes `tools/build-catalog.mjs` wrote.
- Don't point `catalogUrl` back at `ChamberTechFPS/GameOptimizer` while that repo is private.
- Don't use `/releases/latest` if beta testers should see updates.
