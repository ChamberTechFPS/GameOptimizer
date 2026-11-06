// ui/hosts.json is the only catalog and update-check configuration.
// The Tauri CSP is the network allowlist. Every fetch origin in hosts.json must appear there.
import { readFileSync } from "node:fs";

const hosts = JSON.parse(readFileSync(new URL("../ui/hosts.json", import.meta.url), "utf8"));
const tauri = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
const csp = tauri?.app?.security?.csp;
const errors = [];

function originOf(key) {
  const value = hosts[key];
  let url;
  try {
    url = new URL(value);
  } catch {
    errors.push(`${key} is not a URL.`);
    return null;
  }
  if (url.protocol !== "https:") errors.push(`${key} must use https.`);
  return url;
}

for (const key of ["catalogUrl", "catalogSigUrl", "releasesUrl", "releasePagePrefix"]) {
  if (typeof hosts[key] !== "string" || hosts[key].trim() !== hosts[key]) errors.push(`${key} must be a string with no surrounding whitespace.`);
}

const catalog = originOf("catalogUrl");
const sig = originOf("catalogSigUrl");
const releases = originOf("releasesUrl");
const page = originOf("releasePagePrefix");

if (catalog && !hosts.catalogUrl.endsWith("/catalog.json")) errors.push("catalogUrl must end with /catalog.json.");
if (sig && !hosts.catalogSigUrl.endsWith("/catalog.json.sig")) errors.push("catalogSigUrl must end with /catalog.json.sig.");
if (hosts.catalogUrl === hosts.catalogSigUrl) errors.push("catalogSigUrl must be the signature file, not the catalog.");
if (typeof hosts.includePrereleases !== "boolean") errors.push("includePrereleases must be true or false.");
if (page) {
  if (!hosts.releasePagePrefix.endsWith("/")) errors.push("releasePagePrefix must end with / so a similarly named repo cannot match.");
  if (page.pathname === "/" || page.pathname === "") errors.push("releasePagePrefix must include the repo path, not just the site.");
}
if (releases && !hosts.releasesUrl.includes("/releases")) {
  errors.push("releasesUrl must be a GitHub releases list (…/releases?per_page=30), not /releases/latest. /latest hides prereleases.");
}

if (typeof csp !== "string") {
  errors.push("src-tauri/tauri.conf.json app.security.csp must be a string.");
} else {
  const connect = csp.split(";").map((s) => s.trim()).find((s) => s.startsWith("connect-src "));
  const allowed = new Set(connect ? connect.split(/\s+/).slice(1) : []);
  for (const [key, url] of [["catalogUrl", catalog], ["catalogSigUrl", sig], ["releasesUrl", releases]]) {
    if (!url) continue;
    if (!allowed.has(url.origin)) {
      errors.push(`${key} origin ${url.origin} is missing from CSP connect-src. Add it in src-tauri/tauri.conf.json.`);
    }
  }
}

if (errors.length) {
  console.error("hosts.json / CSP:");
  for (const e of errors) console.error(`  - ${e}`);
  process.exit(1);
}
console.log("hosts.json origins are allowed by the CSP.");
