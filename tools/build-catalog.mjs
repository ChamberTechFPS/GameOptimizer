// Merge games/*.json into catalog.json, the one file the app downloads and verifies.
//   node tools/build-catalog.mjs          rebuild after editing a game, then sign it
//   node tools/build-catalog.mjs --check  CI: fail if catalog.json isn't those exact bytes
//
// `--check` never writes. The writer is deterministic given the game files and the
// version it mints: UTF-8, LF, one trailing newline, two-space JSON, games sorted by
// file name, `rev` last. `version` is a unix timestamp (or one past the previous
// version, whichever is higher) and changes only when the games change. Those exact
// bytes are what gets signed. Do not reformat the file, and do not let Git convert
// its line endings (.gitattributes marks it -text).
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const path = new URL("catalog.json", root);

function loadGames() {
  return readdirSync(new URL("games/", root))
    .filter((f) => f.endsWith(".json"))
    .sort()
    .map((file) => {
      const game = JSON.parse(readFileSync(new URL(`games/${file}`, root), "utf8"));
      if (`${game.slug}.json` !== file) throw new Error(`${file}: slug must match the file name`);
      delete game.rev;
      game.rev = createHash("sha256").update(JSON.stringify(game)).digest("hex").slice(0, 8);
      return game;
    });
}

function canonical(version, games) {
  const text = `${JSON.stringify({ version, games }, null, 2)}\n`;
  if (text.includes("\r")) throw new Error("canonical catalog contained a CR");
  return Buffer.from(text, "utf8");
}

const games = loadGames();
let raw = null;
let current = null;
try {
  raw = readFileSync(path);
  current = JSON.parse(raw.toString("utf8"));
} catch {
  // first build
}
const gamesMatch = Boolean(current) && JSON.stringify(current.games) === JSON.stringify(games);
const canonicalCurrent = current ? canonical(current.version, games) : null;
const bytesMatch = Boolean(raw && canonicalCurrent && raw.equals(canonicalCurrent));
const signHint = "  node tools/sign-catalog.mjs <path-to-private-key>";

if (process.argv.includes("--check")) {
  if (!gamesMatch || !bytesMatch) {
    console.error("catalog.json is out of date or not the canonical LF bytes. Run: node tools/build-catalog.mjs, then sign it.");
    process.exit(1);
  }
  console.log(`catalog.json matches games/ (version ${current.version}, ${games.length} games)`);
} else if (gamesMatch && bytesMatch) {
  console.log(`No changes (version ${current.version})`);
} else if (gamesMatch) {
  writeFileSync(path, canonicalCurrent);
  console.log(`Rewrote catalog.json version ${current.version} as canonical LF bytes. The signature is now invalid. Sign it:`);
  console.log(signHint);
} else {
  const version = Math.max(Math.floor(Date.now() / 1000), (current?.version ?? 0) + 1);
  writeFileSync(path, canonical(version, games));
  console.log(`Wrote catalog.json version ${version} with ${games.length} games. Now sign it:`);
  console.log(signHint);
}
