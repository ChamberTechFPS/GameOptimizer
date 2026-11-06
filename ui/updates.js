// Semver compare and release picking. Classic script in the app, require()'d by the tests.
(function (root, factory) {
  const api = factory();
  if (typeof module === "object" && module.exports) module.exports = api;
  else root.ChamberUpdates = api;
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  "use strict";

  // MAJOR.MINOR.PATCH with an optional prerelease. Leading zeros in a numeric
  // prerelease identifier are rejected (they aren't semver). A leading "v" is allowed.
  function parseSemver(input) {
    const m = /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/.exec(String(input).trim());
    if (!m) return null;
    const pre = m[4] ? m[4].split(".") : null;
    if (pre && pre.some((id) => /^\d+$/.test(id) && id.length > 1 && id.startsWith("0"))) return null;
    return { major: Number(m[1]), minor: Number(m[2]), patch: Number(m[3]), pre };
  }

  function cmpId(x, y) {
    const xn = /^\d+$/.test(x);
    const yn = /^\d+$/.test(y);
    if (xn && yn) return Number(x) - Number(y);
    if (xn) return -1;
    if (yn) return 1;
    if (x < y) return -1;
    if (x > y) return 1;
    return 0;
  }

  // Positive when a is newer than b. A release is newer than its own prerelease.
  function compareSemver(a, b) {
    if (a.major !== b.major) return a.major - b.major;
    if (a.minor !== b.minor) return a.minor - b.minor;
    if (a.patch !== b.patch) return a.patch - b.patch;
    if (!a.pre && !b.pre) return 0;
    if (!a.pre) return 1;
    if (!b.pre) return -1;
    const n = Math.max(a.pre.length, b.pre.length);
    for (let i = 0; i < n; i++) {
      if (i >= a.pre.length) return -1;
      if (i >= b.pre.length) return 1;
      const c = cmpId(a.pre[i], b.pre[i]);
      if (c) return c;
    }
    return 0;
  }

  function formatSemver(v) {
    const core = `${v.major}.${v.minor}.${v.patch}`;
    return v.pre ? `${core}-${v.pre.join(".")}` : core;
  }

  /**
   * Pick the newest release the running build should offer.
   * `body` is either the /releases array or one /releases/latest object.
   * Stable builds ignore prereleases unless includePrereleases is set.
   * A build whose own version is a prerelease sees prereleases too.
   * Pages outside releasePagePrefix are never opened.
   */
  function chooseRelease(body, currentVersion, opts) {
    const current = parseSemver(currentVersion);
    if (!current) {
      return { status: "error", detail: `This build's version (${currentVersion}) isn't one the update check can compare.` };
    }
    const prefix = opts && opts.releasePagePrefix;
    if (!prefix || !prefix.startsWith("https://") || !prefix.endsWith("/")) {
      return { status: "error", detail: "The release page host isn't configured." };
    }
    const list = Array.isArray(body) ? body : (body && body.tag_name ? [body] : null);
    if (!list) return { status: "error", detail: "The update server returned something that isn't a release list." };

    const allowPre = opts.includePrereleases === true || current.pre != null;
    let best = null;
    let rejectedNewer = false;
    for (const rel of list) {
      if (!rel || rel.draft) continue;
      const parsed = parseSemver(String(rel.tag_name ?? "").replace(/^v/, ""));
      if (!parsed) continue;
      const isPre = Boolean(rel.prerelease) || parsed.pre != null;
      if (isPre && !allowPre) continue;
      if (compareSemver(parsed, current) <= 0) continue;
      const url = String(rel.html_url ?? "");
      if (!url.startsWith(prefix)) {
        rejectedNewer = true;
        continue;
      }
      if (!best || compareSemver(parsed, best.parsed) > 0) {
        best = { parsed, url, version: formatSemver(parsed) };
      }
    }
    if (best) return { status: "update", version: best.version, url: best.url };
    if (rejectedNewer) {
      return { status: "error", detail: "A newer release was listed, but its page is not on the configured site." };
    }
    return { status: "current" };
  }

  return { parseSemver, compareSemver, formatSemver, chooseRelease };
});
