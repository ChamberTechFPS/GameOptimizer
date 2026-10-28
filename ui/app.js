// Chamber Game Optimizer UI: renders what the Rust engine reports and sends it actions.
// The engine owns every decision about files; nothing here touches the disk.
const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);

const REPO = "ChamberTechFPS/GameOptimizer";
const CATALOG_URL = `https://raw.githubusercontent.com/${REPO}/main/catalog.json`;
const RELEASES_API = `https://api.github.com/repos/${REPO}/releases/latest`;

const LAUNCHER = { steam: "Steam", epic: "Epic", riot: "Riot", battlenet: "Battle.net", ea: "EA", ubisoft: "Ubisoft", other: "PC" };
const ORDER = { changed: 0, update: 1, "not-optimized": 2, "launch-once": 3, manual: 4, optimized: 5 };

let view = { games: [], terms: 0, libraries: [] };
let appVersion = "1.0.0";
let busy = false;
let armedRevert = null; // Revert is two-step: first click arms it for 3 s
let drawerSlug = null;

// ---------- helpers ----------

function el(tag, cls, text) {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (text != null) n.textContent = text;
  return n;
}

function icon(name) {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(ns, "use");
  use.setAttribute("href", `#i-${name}`);
  svg.append(use);
  return svg;
}

function button(label, cls, onClick, iconName) {
  const b = el("button", `btn ${cls}`.trim());
  b.type = "button";
  if (iconName) b.append(icon(iconName));
  b.append(label);
  b.onclick = onClick;
  return b;
}

function tile(g, extra = "") {
  const t = el("div", `tile ${extra}`.trim(), g.tile);
  t.setAttribute("aria-hidden", "true");
  return t;
}

const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const errText = (e) => String(e?.message ?? e).split("\n")[0].slice(0, 200);
const dateText = (secs) => new Date(secs * 1000).toLocaleDateString(undefined, { month: "short", day: "numeric" });
const actionable = (g) => ["not-optimized", "changed", "update"].includes(g.status);
const where = (g) => [LAUNCHER[g.launcher] ?? g.launcher, g.location].filter(Boolean).join(" · ");

let toastTimer;
function toast(message, kind = "") {
  const t = $("toast");
  t.textContent = message;
  t.className = `toast ${kind}`.trim();
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (t.hidden = true), 5000);
}

function openUrl(url) {
  window.__TAURI__.opener.openUrl(url).catch((e) => toast(errText(e), "error"));
}

// ---------- status copy ----------

function badgeFor(g) {
  switch (g.status) {
    case "optimized": return ["Optimized", "ok", "check"];
    case "changed": return [`${g.differ} changed`, "warn", "alert"];
    case "update": return ["New config", "violet", "download"];
    case "manual": return ["Set in-game", "violet", "list"];
    case "launch-once": return ["Not set up", "", null];
    default: return ["Not optimized", "", null];
  }
}

function proofLine(g) {
  const p = g.proof;
  return p ? `My test: 1% lows ${p.low1[0]} → ${p.low1[1]}` : "";
}

function noteFor(g) {
  if (g.running && actionable(g)) return [`Close ${g.name} to apply`, "warn-text"];
  switch (g.status) {
    case "optimized": return [g.appliedAt ? `Applied ${dateText(g.appliedAt)}` : "Already matches", ""];
    case "changed": return ["Changed since you applied", ""];
    case "update": return ["New settings since you applied", ""];
    case "manual": return [`${plural(g.manual.length, "setting")} to set`, ""];
    case "launch-once": return [`Launch ${g.name} once, then rescan`, ""];
    default: return [proofLine(g) || `${plural(g.differ, "setting")} to change`, ""];
  }
}

function badge(g) {
  const [label, tone, ic] = badgeFor(g);
  const b = el("span", `badge ${tone}`.trim());
  if (ic) b.append(icon(ic));
  b.append(label);
  return b;
}

function revertButton(g, label = "Revert") {
  const armed = armedRevert === g.slug;
  return button(armed ? "Confirm revert" : label, armed ? "btn-danger" : "btn-ghost", () => {
    if (armed) return revert(g);
    armedRevert = g.slug;
    rerender();
    setTimeout(() => {
      if (armedRevert === g.slug) { armedRevert = null; rerender(); }
    }, 3000);
  });
}

// ---------- main view ----------

function card(g) {
  const c = el("article", "card");
  const head = el("div", "card-head");
  const info = el("div", "card-info");
  const whereEl = el("div", "card-where", where(g));
  whereEl.title = g.location ?? "";
  info.append(el("div", "card-name", g.name), whereEl);
  head.append(tile(g, g.status === "optimized" ? "lit" : ""), info);

  const status = el("div", "card-status");
  const [note, noteCls] = noteFor(g);
  status.append(badge(g), el("span", noteCls, note));

  const actions = el("div", "card-actions");
  if (g.status === "manual") {
    actions.append(button("Open checklist", "", () => openDrawer(g.slug)));
  } else {
    actions.append(button("Details", "btn-ghost", () => openDrawer(g.slug)));
    if (g.status === "optimized" && g.appliedAt) actions.append(revertButton(g));
    if (g.status === "changed") actions.append(button("Re-apply", "btn-outline", () => apply(g)));
    if (g.status === "update") actions.append(button("Apply update", "btn-outline", () => apply(g)));
    if (g.status === "not-optimized") actions.append(button("Optimize", "btn-outline", () => apply(g)));
  }
  c.append(head, status, actions);
  return c;
}

function render() {
  const visible = view.games.filter((g) => !g.hidden);
  const found = visible
    .filter((g) => g.found)
    .sort((a, b) => ORDER[a.status] - ORDER[b.status] || a.name.localeCompare(b.name));
  const missing = visible.filter((g) => !g.found);
  const fileGames = found.filter((g) => g.status !== "manual");
  const optimized = fileGames.filter((g) => g.status === "optimized").length;
  const todo = fileGames.filter((g) => actionable(g) && !g.running);

  // Hero: the number leads
  if (!fileGames.length) {
    $("hero-num").textContent = "0";
    $("hero-label").textContent = "supported games found";
  } else if (optimized === fileGames.length) {
    $("hero-num").textContent = fileGames.length === 1 ? "1" : `All ${fileGames.length}`;
    $("hero-label").textContent = fileGames.length === 1 ? "game optimized" : "games optimized";
  } else {
    $("hero-num").textContent = `${optimized} of ${fileGames.length}`;
    $("hero-label").textContent = "games optimized";
  }
  const all = $("optimize-all");
  all.hidden = todo.length === 0;
  all.textContent = `Optimize all (${todo.length})`;

  $("library-count").textContent = `Your library · ${found.length} found`;
  const grid = $("grid");
  grid.replaceChildren(...found.map(card));
  if (!found.length) {
    const empty = el("div", "empty");
    empty.append(
      el("strong", null, "No supported games found yet"),
      el("p", null, "Installed somewhere unusual? Open your library and point me at the folder."),
      button("Open library", "", openSheet),
    );
    grid.append(empty);
  }

  const nf = $("not-found");
  nf.hidden = missing.length === 0;
  $("not-found-text").textContent = `Not found (${missing.length}): ${missing.map((g) => g.name).join(", ")}`;
  $("not-found-list").replaceChildren(...missing.map((g) => {
    const li = el("li", "row");
    const info = el("div", "row-info");
    info.append(el("div", "card-name", g.name), el("div", "meta", `${LAUNCHER[g.launcher] ?? g.launcher} · not detected`));
    li.append(tile(g, "sm"), info, button("Set folder", "btn-ghost", () => pickFolder(g), "folder"));
    return li;
  }));
}

// ---------- details drawer ----------

function openDrawer(slug) {
  drawerSlug = slug;
  renderDrawer();
  if (!$("drawer").open) $("drawer").showModal();
}

function settingsTable(rows, headers) {
  const table = el("table", "settings");
  const tr = el("tr");
  for (const h of headers) tr.append(el("th", null, h));
  const thead = el("thead");
  thead.append(tr);
  const tbody = el("tbody");
  for (const r of rows) tbody.append(r);
  table.append(thead, tbody);
  return table;
}

function labelCell(label, why) {
  const td = el("td", null, label);
  if (why) td.append(el("span", "why", why));
  return td;
}

function renderDrawer() {
  const g = view.games.find((x) => x.slug === drawerSlug);
  const d = $("drawer");
  if (!g) { if (d.open) d.close(); return; }

  const head = el("div", "drawer-head");
  const titles = el("div", "titles");
  const h = el("h2", null, g.status === "manual" ? `${g.name} checklist` : `${g.name} details`);
  h.id = "drawer-title";
  const sub = g.status === "manual"
    ? `${plural(g.manual.length, "setting")} · set these in-game`
    : `${g.files.join(", ")} · ${plural(g.total, "setting")}`;
  titles.append(h, el("div", "meta", sub));
  const close = el("button", "btn btn-ghost btn-icon");
  close.type = "button";
  close.setAttribute("aria-label", "Close");
  close.append(icon("x"));
  close.onclick = () => d.close();
  head.append(tile(g, g.status === "optimized" ? "lit" : ""), titles);
  if (g.status !== "not-found") head.append(badge(g));
  head.append(close);

  const body = el("div", "drawer-body");
  const foot = el("div", "drawer-foot");

  if (g.status === "manual") {
    const note = el("p", "note");
    note.append(icon("list"), `${g.name} keeps these on its servers, so I can't write them for you. Set them once in-game and they follow your account.`);
    const rows = g.manual.map((s) => {
      const tr = el("tr");
      tr.append(labelCell(s.label, s.why), el("td", "want", s.value));
      return tr;
    });
    body.append(note, settingsTable(rows, ["Setting", "Set to"]));
  } else {
    if (g.running && actionable(g)) {
      const note = el("p", "note warn-text");
      note.append(icon("alert"), `Close ${g.name} to apply. Its settings file can't be changed while it runs.`);
      body.append(note);
    }
    if (g.status === "launch-once") {
      body.append(el("p", "note", `Launch ${g.name} once so it creates its settings file, then rescan.`));
    }
    const sorted = [...g.settings].sort((a, b) => a.match - b.match);
    const rows = sorted.map((s) => {
      const tr = el("tr", s.match ? "match" : "");
      const want = el("td", "want");
      if (s.match) want.append(icon("check"), " ");
      want.append(s.want);
      tr.append(labelCell(s.label, s.why), el("td", "now", s.now), want);
      return tr;
    });
    body.append(settingsTable(rows, ["Setting", "Now", "Chamber"]));

    const note = el("p", "note");
    note.append(icon("lock"), g.lock
      ? `Locked after apply so ${g.name} can't undo it. Only these settings change. Your sensitivity, binds, and crosshair stay yours.`
      : "Only these settings change. Your sensitivity, binds, and crosshair stay yours.");
    body.append(note);
  }

  const proof = el("div", "proof");
  if (g.proof) {
    const p = g.proof;
    proof.append(
      el("div", "meta", `My test · ${p.rig} · ${p.method}`),
      el("div", "proof-num num", `Avg ${p.avg[0]} → ${p.avg[1]} FPS · 1% lows ${p.low1[0]} → ${p.low1[1]}`),
    );
  } else {
    proof.append(el("div", "meta", "Not benchmarked yet. Numbers show up here once I've tested it."));
  }
  body.append(proof);

  const openBtn = button("Open folder", "btn-ghost", () => openFolder(g, "config"), "folder");
  switch (g.status) {
    case "not-optimized":
      foot.append(openBtn, button(`Optimize ${plural(g.differ, "setting")}`, "btn-primary", () => apply(g)));
      break;
    case "changed":
      foot.append(openBtn, revertButton(g, `Revert all ${g.total}`), button(`Re-apply ${g.differ}`, "btn-primary", () => apply(g)));
      break;
    case "update":
      foot.append(openBtn, revertButton(g), button("Apply update", "btn-primary", () => apply(g)));
      break;
    case "optimized":
      foot.append(openBtn);
      if (g.appliedAt) foot.append(button("Open backups", "btn-ghost", () => openFolder(g, "backups")), revertButton(g));
      break;
    case "not-found":
      foot.append(button("Set folder", "btn-primary", () => pickFolder(g), "folder"));
      break;
    default:
      foot.append(button("Close", "", () => d.close()));
  }

  d.replaceChildren(head, body, foot);
}

// ---------- library sheet ----------

function openSheet() {
  renderSheet();
  if (!$("sheet").open) $("sheet").showModal();
}

// ---------- first launch + game folders ----------

// The terms ship in ui/terms.md. Bumping its "Version N" line asks every player to accept again.
async function loadTerms() {
  const md = await fetch("terms.md").then((r) => r.text());
  return { md, version: Number(md.match(/^Version (\d+)/m)?.[1] ?? 1) };
}

// Just enough Markdown for terms.md: ## headings, paragraphs, `code`, and [links](url).
function markdown(md) {
  return md.trim().split(/\n\s*\n/).filter((b) => !b.startsWith("# ")).map((block) => {
    const heading = block.startsWith("## ");
    const n = el(heading ? "h3" : "p");
    for (const part of (heading ? block.slice(3) : block).split(/(`[^`]+`|\[[^\]]+\]\([^)]+\))/)) {
      const link = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      if (link) n.append(button(link[1], "link", () => openUrl(link[2])));
      else if (/^`.+`$/.test(part)) n.append(el("code", null, part.slice(1, -1)));
      else n.append(part);
    }
    return n;
  });
}

function renderLibraries() {
  for (const box of document.querySelectorAll(".libs")) {
    const rows = view.libraries.map((path) => {
      const li = el("div", "row");
      const info = el("div", "row-info");
      const meta = el("div", "card-name", path);
      meta.title = path;
      info.append(meta);
      li.append(info, button("Remove", "btn-ghost", () => removeLibrary(path)));
      return li;
    });
    box.replaceChildren(...rows, button("Add folder", "", addLibrary, "folder"));
  }
}

// Resolves once the player has accepted the terms and finished the folders step.
function welcome(terms) {
  const d = $("welcome");
  let accepted = false;
  return new Promise((resolve) => {
    d.addEventListener("cancel", (e) => e.preventDefault());
    d.addEventListener("close", () => { if (!accepted) d.showModal(); });

    $("welcome-title").textContent = "Terms of use";
    $("welcome-sub").textContent = "Read these before you start. Accept to use the app.";
    $("welcome-body").replaceChildren(...markdown(terms.md));
    $("welcome-foot").replaceChildren(
      button("Decline", "btn-ghost", () => invoke("quit")),
      button("Accept", "btn-primary", async () => {
        try {
          await invoke("accept_terms", { version: terms.version });
        } catch (e) {
          toast(errText(e), "error");
          return;
        }
        accepted = true;
        folders();
      }),
    );
    d.showModal();

    function folders() {
      $("welcome-title").textContent = "Where your games are";
      $("welcome-sub").textContent = "I already check the usual places. Add any others.";
      const libs = el("div", "libs");
      $("welcome-body").replaceChildren(
        el("p", null, "I look in Steam (every library drive), Epic, Riot, and Windows' list of installed apps, which covers Battle.net, EA, and Ubisoft."),
        el("p", null, "Keep games somewhere else, like D:\\Games? Add that folder and I'll look through it, including a folder per launcher like D:\\Games\\Steam. You can change this later in Library."),
        libs,
      );
      renderLibraries();
      $("welcome-foot").replaceChildren(button("Done", "btn-primary", () => { d.close(); resolve(); }));
      $("welcome-body").scrollTop = 0;
    }
  });
}

function renderSheet() {
  const games = [...view.games].sort((a, b) => a.name.localeCompare(b.name));
  $("sheet-list").replaceChildren(...games.map((g) => {
    const li = el("li", "row");
    const info = el("div", "row-info");
    const detail = g.folder ? `Your folder · ${g.folder}` : g.found ? where(g) : "Not detected";
    const meta = el("div", "meta", detail);
    meta.title = g.folder ?? g.location ?? "";
    info.append(el("div", "card-name", g.name), meta);

    const show = el("label", "show-toggle");
    const box = el("input");
    box.type = "checkbox";
    box.checked = !g.hidden;
    box.onchange = () => setHidden(g, !box.checked);
    show.append(box, "Show");

    li.append(tile(g, "sm"), info, button(g.folder ? "Change folder" : "Set folder", "btn-ghost", () => pickFolder(g), "folder"));
    if (g.folder) li.append(button("Reset to auto", "btn-ghost", () => clearFolder(g)));
    li.append(show);
    return li;
  }));
}

// ---------- actions (one engine call at a time) ----------

function rerender() {
  render();
  renderLibraries();
  if ($("drawer").open) renderDrawer();
  if ($("sheet").open) renderSheet();
}

async function run(task) {
  if (busy) return;
  busy = true;
  document.body.classList.add("busy");
  try { await task(); } finally {
    busy = false;
    document.body.classList.remove("busy");
  }
}

async function scan() {
  try {
    view = await invoke("scan");
  } catch (e) {
    toast(`Scan failed: ${errText(e)}`, "error");
    return;
  }
  rerender();
}

const apply = (g) => run(async () => {
  try {
    const r = await invoke("apply", { slug: g.slug });
    toast(`Optimized ${g.name}. ${plural(r.changed, "setting")} changed.`, "ok");
  } catch (e) {
    toast(errText(e), "error");
  }
  await scan();
});

const optimizeAll = () => run(async () => {
  const targets = view.games.filter((g) => !g.hidden && g.found && actionable(g));
  const skipped = [];
  let done = 0;
  for (const g of targets) {
    if (g.running) { skipped.push(`${g.name} is running`); continue; }
    try {
      await invoke("apply", { slug: g.slug });
      done++;
    } catch (e) {
      skipped.push(`${g.name}: ${errText(e)}`);
    }
  }
  await scan();
  const msg = `Optimized ${plural(done, "game")}.`;
  toast(skipped.length ? `${msg} Skipped ${skipped.join("; ")}.` : msg, skipped.length ? "error" : "ok");
});

const revert = (g) => run(async () => {
  armedRevert = null;
  try {
    await invoke("revert", { slug: g.slug });
    toast(`Reverted ${g.name}. Your original settings are back.`, "ok");
  } catch (e) {
    toast(errText(e), "error");
  }
  await scan();
});

const pickFolder = (g) => run(async () => {
  const path = await window.__TAURI__.dialog.open({ directory: true, title: `Where is ${g.name} installed?` });
  if (!path) return;
  try {
    await invoke("set_folder", { slug: g.slug, path });
    toast(`${g.name} folder set`, "ok");
  } catch (e) {
    toast(errText(e), "error");
  }
  await scan();
});

const addLibrary = () => run(async () => {
  const path = await window.__TAURI__.dialog.open({ directory: true, title: "Pick a folder that holds your games" });
  if (!path) return;
  try {
    await invoke("add_library", { path });
    toast(`Added ${path}`, "ok");
  } catch (e) {
    toast(errText(e), "error");
  }
  await scan();
});

const removeLibrary = (path) => run(async () => {
  await invoke("remove_library", { path }).catch((e) => toast(errText(e), "error"));
  await scan();
});

const clearFolder = (g) => run(async () => {
  await invoke("clear_folder", { slug: g.slug }).catch((e) => toast(errText(e), "error"));
  await scan();
});

const setHidden = (g, hidden) => run(async () => {
  await invoke("set_hidden", { slug: g.slug, hidden }).catch((e) => toast(errText(e), "error"));
  await scan();
});

function openFolder(g, which) {
  invoke("open_folder", { slug: g.slug, which }).catch((e) => toast(errText(e), "error"));
}

// ---------- updates ----------

function setChip(state, text, detail = "") {
  $("catalog-chip").dataset.state = state;
  $("catalog-text").textContent = text;
  $("catalog-chip").title = detail;
}

// The webview downloads; the engine verifies the signature before anything is used.
async function refreshCatalog() {
  setChip("checking", "Checking for new configs…");
  let json, sig;
  try {
    [json, sig] = await Promise.all([CATALOG_URL, `${CATALOG_URL}.sig`].map(async (url) => {
      const r = await fetch(url, { cache: "no-store" });
      if (!r.ok) throw new Error(`GitHub answered ${r.status}`);
      return Array.from(new Uint8Array(await r.arrayBuffer()));
    }));
  } catch (e) {
    setChip("saved", "Using saved configs", `Couldn't reach GitHub: ${errText(e)}`);
    return;
  }
  try {
    const r = await invoke("install_catalog", { json, sig });
    if (r.status === "updated") await scan();
    setChip("fresh", r.status === "updated" ? "New configs downloaded" : "Configs up to date");
  } catch (e) {
    setChip("saved", "Using saved configs", errText(e));
  }
}

function newer(a, b) {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    if ((pa[i] || 0) !== (pb[i] || 0)) return (pa[i] || 0) > (pb[i] || 0);
  }
  return false;
}

async function checkAppUpdate() {
  try {
    const r = await fetch(RELEASES_API, { headers: { Accept: "application/vnd.github+json" } });
    if (!r.ok) return;
    const rel = await r.json();
    const latest = String(rel.tag_name ?? "").replace(/^v/, "");
    if (!newer(latest, appVersion) || !String(rel.html_url).startsWith(`https://github.com/${REPO}/`)) return;
    const chip = $("update-chip");
    chip.textContent = `v${latest} available`;
    chip.hidden = false;
    chip.onclick = () => openUrl(rel.html_url);
  } catch {
    // Offline: the configs chip already says so
  }
}

// ---------- wiring ----------

$("rescan-btn").onclick = () => run(scan);
$("library-btn").onclick = openSheet;
$("optimize-all").onclick = optimizeAll;
for (const b of document.querySelectorAll("[data-url]")) b.onclick = () => openUrl(b.dataset.url);
for (const d of [$("drawer"), $("sheet")]) {
  // A click that lands on the dialog itself is a click on its backdrop
  d.addEventListener("click", (e) => { if (e.target === d) d.close(); });
}
$("sheet").querySelector("[data-close]").onclick = () => $("sheet").close();
$("drawer").addEventListener("close", () => (drawerSlug = null));

(async () => {
  appVersion = await window.__TAURI__.app.getVersion().catch(() => appVersion);
  $("version").textContent = appVersion;
  await scan();
  const terms = await loadTerms();
  if (view.terms < terms.version) await welcome(terms);
  refreshCatalog();
  checkAppUpdate();
})();
