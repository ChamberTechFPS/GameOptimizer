// Merge games/*.json into catalog.json, the one file the app downloads and verifies.
//   node tools/build-catalog.mjs          rebuild after editing a game, then sign it
//   node tools/build-catalog.mjs --check  CI: fail if catalog.json doesn't match games/
// Each game gets a `rev` (hash of its content), so changing a config is enough for players
// to see "New config". `version` only ever goes up, which the app uses to refuse rollbacks.
import { createHash } from "node:crypto";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const games = readdirSync(new URL("games/", root))
  .filter((f) => f.endsWith(".json"))
  .sort()
  .map((file) => {
    const game = JSON.parse(readFileSync(new URL(`games/${file}`, root), "utf8"));
    if (`${game.slug}.json` !== file) throw new Error(`${file}: slug must match the file name`);
    delete game.rev;
    game.rev = createHash("sha256").update(JSON.stringify(game)).digest("hex").slice(0, 8);
    return game;
  });

const path = new URL("catalog.json", root);
let current = null;
try {
  current = JSON.parse(readFileSync(path, "utf8"));
} catch {
  // first build
}
const fresh = current && JSON.stringify(current.games) === JSON.stringify(games);

if (process.argv.includes("--check")) {
  if (!fresh) {
    console.error("catalog.json is out of date. Run: node tools/build-catalog.mjs, then sign it.");
    process.exit(1);
  }
  console.log(`catalog.json matches games/ (version ${current.version}, ${games.length} games)`);
} else if (fresh) {
  console.log(`No changes (version ${current.version})`);
} else {
  const version = Math.max(Math.floor(Date.now() / 1000), (current?.version ?? 0) + 1);
  writeFileSync(path, `${JSON.stringify({ version, games }, null, 2)}\n`);
  console.log(`Wrote catalog.json version ${version} with ${games.length} games. Now sign it:`);
  console.log(String.raw`  openssl dgst -sha256 -sign F:\GameOptimizer\signing\catalog-signing.key -out catalog.json.sig catalog.json`);
}
