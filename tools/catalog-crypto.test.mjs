import { spawnSync } from "node:child_process";
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { readTrustedModuli, signatureMatches } from "./catalog-crypto.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));

const src = readFileSync(new URL("../src-tauri/src/catalog.rs", import.meta.url), "utf8");

function rustConst(name) {
  const m = src.match(new RegExp(`const ${name}: &str = "([0-9a-f]*)";`));
  assert.ok(m, name);
  return m[1];
}

test("the node verifier agrees with the Rust test vectors", () => {
  const message = Buffer.from("chamber test message");
  assert.equal(signatureMatches(message, Buffer.from(rustConst("TEST_SIG_HEX"), "hex"), rustConst("TEST_MODULUS_HEX")), true);
  assert.equal(signatureMatches(message, Buffer.from(rustConst("TEST_SIG_B_HEX"), "hex"), rustConst("TEST_MODULUS_B_HEX")), true);
  assert.equal(signatureMatches(message, Buffer.from(rustConst("TEST_SIG_HEX"), "hex"), rustConst("TEST_MODULUS_B_HEX")), false);
  assert.equal(signatureMatches(Buffer.from("chamber test messagE"), Buffer.from(rustConst("TEST_SIG_HEX"), "hex"), rustConst("TEST_MODULUS_HEX")), false);
});

test("trusted moduli come from TRUSTED_MODULI, including the empty spare", () => {
  const keys = readTrustedModuli(src);
  assert.deepEqual(keys.map((k) => k.name), ["CURRENT_MODULUS_HEX", "SPARE_MODULUS_HEX"]);
  assert.equal(keys[0].hex.length * 4, 4096);
  assert.equal(keys[1].hex, "");
});

test("verify fails closed, and CI's allowlist accepts only the known unsigned snapshot", () => {
  const strict = spawnSync(process.execPath, ["tools/verify-catalog.mjs"], { cwd: root, encoding: "utf8" });
  if (strict.status === 0) {
    assert.match(strict.stdout, /Signature OK/);
    return;
  }
  assert.match(strict.stderr, /failed its signature check/);
  const allowed = spawnSync(process.execPath, ["tools/verify-catalog.mjs", "--allow-known-unsigned"], { cwd: root, encoding: "utf8" });
  assert.equal(allowed.status, 0, allowed.stderr);
  assert.match(allowed.stderr, /known unsigned snapshot/);
});

test("sign-catalog refuses to run without a key and refuses a key inside the repo", () => {
  const noKey = spawnSync(process.execPath, ["tools/sign-catalog.mjs"], { cwd: root, encoding: "utf8" });
  assert.notEqual(noKey.status, 0);
  assert.match(noKey.stderr, /Usage/);

  const inside = spawnSync(process.execPath, ["tools/sign-catalog.mjs", "src-tauri/src/catalog.rs"], { cwd: root, encoding: "utf8" });
  assert.notEqual(inside.status, 0);
  assert.match(inside.stderr, /inside the repository/);
});
