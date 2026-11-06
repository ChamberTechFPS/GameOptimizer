// Shared by the sign and verify scripts. The app's trusted moduli live in catalog.rs;
// this reads that list so a signature check uses the same keys the binary embeds.
import { createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";

const rsaOid = Buffer.from("300d06092a864886f70d0101010500", "hex");

function derLen(n) {
  if (n < 128) return Buffer.from([n]);
  const bytes = [];
  let x = n;
  while (x) {
    bytes.unshift(x & 0xff);
    x >>= 8;
  }
  return Buffer.from([0x80 | bytes.length, ...bytes]);
}

function derInt(buf) {
  let v = Buffer.from(buf);
  if (v[0] & 0x80) v = Buffer.concat([Buffer.from([0x00]), v]);
  while (v.length > 1 && v[0] === 0x00 && (v[1] & 0x80) === 0) v = v.subarray(1);
  return Buffer.concat([Buffer.from([0x02]), derLen(v.length), v]);
}

function derSeq(content) {
  return Buffer.concat([Buffer.from([0x30]), derLen(content.length), content]);
}

export function normHex(hex) {
  return hex.toLowerCase().replace(/^0+/, "");
}

/** RSA public key (e = 65537) from a modulus hex string, as the app builds it. */
export function publicKeyFromModulus(hex) {
  if (!/^[0-9a-fA-F]+$/.test(hex) || hex.length % 2 !== 0) {
    throw new Error("modulus is not even-length hex");
  }
  const pkcs1 = derSeq(Buffer.concat([
    derInt(Buffer.from(hex, "hex")),
    derInt(Buffer.from([0x01, 0x00, 0x01])),
  ]));
  const bitString = Buffer.concat([
    Buffer.from([0x03]),
    derLen(pkcs1.length + 1),
    Buffer.from([0x00]),
    pkcs1,
  ]);
  const spki = derSeq(Buffer.concat([rsaOid, bitString]));
  return createPublicKey({ key: spki, format: "der", type: "spki" });
}

export function signatureMatches(data, sig, modulusHex) {
  try {
    return verify("sha256", data, publicKeyFromModulus(modulusHex), sig);
  } catch {
    return false;
  }
}

/**
 * `TRUSTED_MODULI` in src-tauri/src/catalog.rs. Each entry is a named const
 * (`CURRENT_MODULUS_HEX`) or an inline hex literal. Empty strings are spare slots.
 */
export function readTrustedModuli(src = readFileSync(new URL("../src-tauri/src/catalog.rs", import.meta.url), "utf8")) {
  const list = src.match(/const TRUSTED_MODULI:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\]\s*;/);
  if (!list) throw new Error("TRUSTED_MODULI is missing from src-tauri/src/catalog.rs");
  const parts = list[1].split(",").map((s) => s.trim()).filter(Boolean);
  if (parts.length === 0) throw new Error("TRUSTED_MODULI is empty");
  return parts.map((part) => {
    const literal = part.match(/^"([0-9a-fA-F]*)"$/);
    if (literal) return { name: literal[1] ? "inline" : "empty", hex: literal[1] };
    const id = part.match(/^([A-Z0-9_]+)$/);
    if (!id) throw new Error(`Can't read trusted key entry ${part}`);
    const value = src.match(new RegExp(`const ${id[1]}:\\s*&str\\s*=\\s*"([0-9a-fA-F]*)"\\s*;`));
    if (!value) throw new Error(`const ${id[1]} is not a hex string in catalog.rs`);
    return { name: id[1], hex: value[1] };
  });
}

/** Public modulus hex (no leading zeros) of a PEM private or public key. */
export function modulusHexFromPem(pem) {
  const jwk = createPublicKey(pem).export({ format: "jwk" });
  if (jwk.kty !== "RSA" || jwk.e !== "AQAB") {
    throw new Error("The key must be RSA with public exponent 65537.");
  }
  return normHex(Buffer.from(jwk.n, "base64url").toString("hex"));
}
