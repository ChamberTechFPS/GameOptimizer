import { createRequire } from "node:module";
import test from "node:test";
import assert from "node:assert/strict";

const { parseSemver, compareSemver, chooseRelease } = createRequire(import.meta.url)("../ui/updates.js");
const prefix = "https://github.com/ChamberTechFPS/GameOptimizer/";
const page = (tag) => `${prefix}releases/tag/${tag}`;
const rel = (tag, extra = {}) => ({ tag_name: tag, html_url: page(tag), draft: false, prerelease: false, ...extra });

test("semver orders releases above prereleases and compares prerelease ids", () => {
  const cmp = (a, b) => compareSemver(parseSemver(a), parseSemver(b));
  assert.ok(cmp("1.0.0", "1.0.0-beta.2") > 0);
  assert.ok(cmp("1.0.0-beta.2", "1.0.0-beta.1") > 0);
  assert.ok(cmp("1.0.0-beta.10", "1.0.0-beta.2") > 0);
  assert.ok(cmp("1.0.0-beta.1", "1.0.0-alpha.9") > 0);
  assert.ok(cmp("1.0.0-alpha", "1.0.0-alpha.1") < 0);
  assert.ok(cmp("1.0.0-1", "1.0.0-alpha") < 0);
  assert.equal(cmp("v1.2.3", "1.2.3"), 0);
  assert.equal(parseSemver("1.0.0-01"), null);
  assert.equal(parseSemver("1.0"), null);
});

test("stable builds ignore prereleases and a beta build sees them", () => {
  const listed = [rel("v1.0.1-beta.1", { prerelease: true }), rel("v1.0.0")];
  assert.equal(chooseRelease(listed, "1.0.0", { includePrereleases: false, releasePagePrefix: prefix }).status, "current");
  const beta = chooseRelease(listed, "1.0.0-beta.1", { includePrereleases: false, releasePagePrefix: prefix });
  assert.deepEqual(beta, { status: "update", version: "1.0.1-beta.1", url: page("v1.0.1-beta.1") });
});

test("a stable release is newer than the beta it replaces", () => {
  const choice = chooseRelease([rel("v1.0.0-beta.9", { prerelease: true }), rel("v1.0.0")], "1.0.0-beta.2", {
    includePrereleases: false,
    releasePagePrefix: prefix,
  });
  assert.equal(choice.version, "1.0.0");
});

test("includePrereleases offers a beta to a stable version", () => {
  const choice = chooseRelease([rel("v1.0.1-beta.2", { prerelease: true })], "1.0.0", {
    includePrereleases: true,
    releasePagePrefix: prefix,
  });
  assert.equal(choice.version, "1.0.1-beta.2");
});

test("a prerelease tag is treated as a prerelease even if GitHub didn't flag it", () => {
  const choice = chooseRelease([rel("v1.0.1-beta.1")], "1.0.0", { includePrereleases: false, releasePagePrefix: prefix });
  assert.equal(choice.status, "current");
});

test("drafts and lookalike repos are not offered", () => {
  const evil = rel("v9.9.9", { html_url: "https://github.com/ChamberTechFPS/GameOptimizer-evil/releases/tag/v9.9.9" });
  const draft = rel("v2.0.0", { draft: true });
  const choice = chooseRelease([evil, draft, rel("v1.0.1")], "1.0.0", { includePrereleases: false, releasePagePrefix: prefix });
  assert.equal(choice.version, "1.0.1");
  const onlyEvil = chooseRelease([evil], "1.0.0", { includePrereleases: false, releasePagePrefix: prefix });
  assert.equal(onlyEvil.status, "error");
});

test("a single /releases/latest object still parses, and an empty list is current", () => {
  const one = chooseRelease(rel("v1.2.0"), "1.0.0", { includePrereleases: false, releasePagePrefix: prefix });
  assert.equal(one.version, "1.2.0");
  assert.equal(chooseRelease([], "1.0.0", { includePrereleases: false, releasePagePrefix: prefix }).status, "current");
});
