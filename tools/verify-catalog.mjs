// Check catalog.json.sig against the public keys embedded in src-tauri/src/catalog.rs.
//   node tools/verify-catalog.mjs
// Exit 0 only when the committed bytes verify. Prints the re-sign commands otherwise.
// This does not have the private key and cannot produce a signature.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { normHex, readTrustedModuli, signatureMatches } from "./catalog-crypto.mjs";

const root = new URL("../", import.meta.url);
const json = readFileSync(new URL("catalog.json", root));
const sig = readFileSync(new URL("catalog.json.sig", root));
const keys = readTrustedModuli();
const problems = [];

function attr(file) {
  return execFileSync("git", ["check-attr", "text", "binary", "--", file], {
    cwd: fileURLToPath(root),
    encoding: "utf8",
  });
}

const jsonAttr = attr("catalog.json");
const sigAttr = attr("catalog.json.sig");
if (!/^catalog\.json: text: unset$/m.test(jsonAttr)) {
  problems.push("catalog.json is not marked -text in .gitattributes (line-ending conversion would break the signature).");
}
if (!/^catalog\.json\.sig: binary: set$/m.test(sigAttr) || !/^catalog\.json\.sig: text: unset$/m.test(sigAttr)) {
  problems.push("catalog.json.sig is not marked binary in .gitattributes.");
}
if (json.includes(0x0d)) problems.push("catalog.json contains CR bytes. The signed form is LF only.");
// catalog.json.sig is raw RSA output. Those bytes often include 0x0D and 0x0A; that is not a line ending.

const usable = keys.filter((k) => k.hex.length > 0);
for (const key of usable) {
  const bits = key.hex.length * 4;
  if (bits !== 4096) problems.push(`${key.name} is RSA-${bits}. Catalog keys are RSA-4096 (or empty, for the spare slot).`);
  if (normHex(key.hex) !== key.hex.toLowerCase()) problems.push(`${key.name} has a leading zero. Paste the modulus without one.`);
}
if (!keys.some((k) => k.name === "CURRENT_MODULUS_HEX" && k.hex.length > 0)) {
  problems.push("TRUSTED_MODULI must include CURRENT_MODULUS_HEX.");
}
if (!keys.some((k) => k.name === "SPARE_MODULUS_HEX")) {
  problems.push("TRUSTED_MODULI must include SPARE_MODULUS_HEX (empty until the next rotation).");
}

const matched = usable.find((k) => signatureMatches(json, sig, k.hex));
const sha = createHash("sha256").update(json).digest("hex");

console.log(`catalog.json  ${json.length} bytes  sha256 ${sha}  ${json.includes(0x0d) ? "CRLF" : "LF"}`);
console.log(`catalog.json.sig  ${sig.length} bytes`);
for (const key of keys) {
  const bits = key.hex ? `RSA-${key.hex.length * 4}` : "empty, ignored";
  console.log(`  ${key.name}  ${bits}${key.hex ? `  ${key.hex.slice(0, 8)}…${key.hex.slice(-6)}` : ""}`);
}

if (problems.length) {
  console.error("\nSigning pipeline:");
  for (const p of problems) console.error(`  - ${p}`);
}

if (matched && problems.length === 0) {
  console.log(`\nSignature OK (${matched.name}).`);
  process.exit(0);
}

if (matched) {
  console.error("\nSignature matches, but the pipeline checks above failed. Fix those before trusting this catalog.");
  process.exit(1);
}

console.error("\ncatalog.json failed its signature check. No embedded key accepts catalog.json.sig.");
const endings = json.includes(0x0d)
  ? "This catalog.json contains CR bytes. That alone breaks a signature made from LF bytes."
  : "This catalog.json is LF-only, so line endings are not why this copy fails.";
console.error(`
The signature on disk is from commit 033c850. That signature matches CURRENT_MODULUS_HEX
for the catalog.json from that commit. Later commits changed catalog.json (Fortnite view
distance, the Call of Duty config, the worker-thread cap) and did not update
catalog.json.sig. ${endings}

Re-sign the current bytes with the RSA-4096 private key whose public modulus is
CURRENT_MODULUS_HEX. Do not generate a new key unless that private key is lost.
The private key stays on your machine. Never commit it or put it in CI secrets.

  node tools/build-catalog.mjs --check
  node tools/sign-catalog.mjs ~/.config/chamber-signing/catalog-signing.key

Or with openssl directly, from the repo root:

  openssl dgst -sha256 -sign ~/.config/chamber-signing/catalog-signing.key -out catalog.json.sig catalog.json
  node tools/verify-catalog.mjs

Confirm the key first. The printed modulus must equal CURRENT_MODULUS_HEX (case insensitive, no Modulus= prefix):

  openssl pkey -in ~/.config/chamber-signing/catalog-signing.key -pubout -outform DER | openssl rsa -pubin -inform DER -modulus -noout

On Windows, Git's openssl is the same command. Pass the key path you actually use.
Then run: cargo test --manifest-path src-tauri/Cargo.toml
See RELEASING.md.
`);
process.exit(1);
