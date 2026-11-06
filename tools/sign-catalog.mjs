// Sign catalog.json with the offline private key, then verify it against the embedded public keys.
//   node tools/sign-catalog.mjs <path-to-private-key.pem>
// Refuses a key that lives inside this repo, and a key the app does not already trust.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { modulusHexFromPem, normHex, readTrustedModuli } from "./catalog-crypto.mjs";

const root = realpathSync(fileURLToPath(new URL("../", import.meta.url)));
const keyArg = process.argv[2];

function fail(message) {
  console.error(message);
  process.exit(1);
}

if (!keyArg || keyArg === "--help" || keyArg === "-h") {
  fail("Usage: node tools/sign-catalog.mjs <path-to-private-key.pem>\nThe private key stays offline. Never commit it or store it in CI secrets.");
}

const keyResolved = resolve(keyArg);
if (!existsSync(keyResolved)) fail(`Private key not found: ${keyResolved}`);
const keyAbs = realpathSync(keyResolved);
const rel = relative(root, keyAbs);
const insideRepo = rel === "" || (!rel.startsWith(`..${sep}`) && rel !== ".." && !isAbsolute(rel));
if (insideRepo) {
  fail("Refusing to sign: the private key is inside the repository.\nMove it outside the repo (for example F:\\GameOptimizer\\signing\\catalog-signing.key) and run this again.");
}

let modulus;
try {
  modulus = modulusHexFromPem(readFileSync(keyAbs));
} catch (e) {
  fail(`Couldn't read that file as an RSA private key: ${e.message}`);
}

const trusted = readTrustedModuli().filter((k) => k.hex);
const match = trusted.find((k) => normHex(k.hex) === modulus);
if (!match) {
  fail(
    "This private key is not embedded in the app, so players would reject the signature.\n" +
    "Public modulus (paste into SPARE_MODULUS_HEX in src-tauri/src/catalog.rs, ship an app\n" +
    "release, and only then sign with this key):\n\n" +
    modulus +
    "\n\nIf the current private key is lost and no build with CURRENT_MODULUS_HEX has shipped,\n" +
    "paste it into CURRENT_MODULUS_HEX instead. See RELEASING.md.",
  );
}
if (match.name === "SPARE_MODULUS_HEX") {
  console.log("Signing with the spare key. Only app builds that already embed this modulus will accept the catalog.");
}

const check = spawnSync(process.execPath, ["tools/build-catalog.mjs", "--check"], { cwd: root, encoding: "utf8" });
if (check.status !== 0) {
  process.stderr.write(check.stdout ?? "");
  process.stderr.write(check.stderr ?? "");
  fail("Not signing. catalog.json does not match games/. Run: node tools/build-catalog.mjs");
}

const signed = spawnSync("openssl", [
  "dgst", "-sha256", "-sign", keyAbs, "-out", "catalog.json.sig", "catalog.json",
], { cwd: root, stdio: "inherit" });
if (signed.error?.code === "ENOENT") {
  fail(
    "openssl is not on PATH. From the repo root, run:\n" +
    `  openssl dgst -sha256 -sign ${keyArg} -out catalog.json.sig catalog.json\n` +
    "  node tools/verify-catalog.mjs",
  );
}
if (signed.status !== 0) fail("openssl did not sign catalog.json.");

const verified = spawnSync(process.execPath, ["tools/verify-catalog.mjs"], { cwd: root, stdio: "inherit" });
if (verified.status !== 0) process.exit(verified.status ?? 1);
console.log("Signed catalog.json and verified catalog.json.sig. Commit both files together.");
