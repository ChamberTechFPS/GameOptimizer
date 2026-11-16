// FPS Optimizer UI: renders what the Rust engine reports and sends it actions.
// The engine owns every decision about files; nothing here touches the disk.
const { invoke } = window.__TAURI__.core;
const $ = (id) => document.getElementById(id);

// Catalog and app-update URLs: ui/hosts.json. Loaded in boot().
let hosts = null;

const LAUNCHER = { steam: "Steam", epic: "Epic", riot: "Riot", battlenet: "Battle.net", ea: "EA", ubisoft: "Ubisoft", other: "PC" };
const ORDER = { changed: 0, update: 1, "not-optimized": 2, "launch-once": 3, manual: 4, optimized: 5 };

let view = { games: [], terms: 0, libraries: [] };
let appVersion = "1.0.0";
let busy = false;
let armedRevert = null; // Revert is two-step: first click arms it for 3 s
let drawerSlug = null;
// Windows tweak profiles, each tweak's state read live from the registry by the engine.
let profiles = [];
let drawerProfile = null;
let armedReset = null; // Reset is two-step, like Revert

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

// Hover text. Anything worth a sentence lives here, so the screen itself stays short.
function tip(node, text) {
  if (text) node.dataset.tip = text;
  else delete node.dataset.tip;
  return node;
}

function tile(g, extra = "") {
  const t = el("div", `tile ${extra}`.trim(), g.tile);
  t.setAttribute("aria-hidden", "true");
  return t;
}

const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const errText = (e) => String(e?.message ?? e).split("\n")[0].slice(0, 400);

// Same sentence as detect::close_before_applying. Scan and the running poll send it as
// applyBlock; this rebuilds it only when an apply error comes back wrapped.
function closeBeforeApplying(name) {
  return `Close ${name} before applying — the config can't be applied while the game is running.`;
}

function blocked(g) {
  return !!(g.running && actionable(g));
}

function blockText(g, raw = "") {
  const canonical = closeBeforeApplying(g.name);
  if (!raw) return g.applyBlock || (blocked(g) ? canonical : "");
  if (raw === canonical) return raw;
  const start = raw.indexOf("Close ");
  if (start >= 0 && raw.includes("can't be applied while the game is running")) return raw.slice(start);
  if (raw.includes("while the game is running")) return canonical;
  return "";
}
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

// [label, tone, icon, hover text]
function badgeFor(g) {
  if (blocked(g)) return ["Running", "bad", "alert", blockText(g)];
  switch (g.status) {
    case "optimized": return ["Optimized", "ok", "check", ""];
    case "changed": return [`${g.differ} changed`, "warn", "alert", "Changed since you applied. Re-apply puts them back."];
    case "update": return ["New config", "violet", "download", "New settings since you applied."];
    case "manual": return ["Set in-game", "violet", "list", `${g.name} keeps these on its servers, so they're set by hand.`];
    case "launch-once": return ["Not set up", "", null, `Launch ${g.name} once so it creates its settings file, then rescan.`];
    default: return ["Not optimized", "", null, ""];
  }
}

// The few words under a card's name: the tested gain when there is one, otherwise a count.
function noteFor(g) {
  const note = el("span", "card-note");
  const p = g.proof;
  switch (g.status) {
    case "optimized":
      note.textContent = g.appliedAt ? `Applied ${dateText(g.appliedAt)}` : "Already matches";
      break;
    case "changed":
    case "update":
      note.textContent = g.appliedAt ? `Applied ${dateText(g.appliedAt)}` : "";
      break;
    case "manual":
      note.textContent = plural(g.manual.length, "setting");
      break;
    case "launch-once":
      note.textContent = "Launch once, then rescan";
      break;
    case "not-optimized":
      if (p) {
        note.append(tip(el("span", "gain num", `1% lows ${p.low1[0]} → ${p.low1[1]}`), `Avg ${p.avg[0]} → ${p.avg[1]} FPS. ${p.rig}, ${p.method}.`));
        break;
      }
    // falls through
    default:
      note.textContent = plural(g.differ, "setting");
  }
  return note;
}

function badge(g) {
  const [label, tone, ic, hint] = badgeFor(g);
  const b = el("span", `badge ${tone}`.trim());
  if (ic) b.append(icon(ic));
  b.append(label);
  return tip(b, hint);
}

function runningWarning(g) {
  const text = blockText(g);
  if (!blocked(g) || !text) return null;
  const note = el("p", "note running-warn");
  note.append(icon("alert"), el("span", null, text));
  return note;
}

function applyButton(label, cls, g) {
  const b = button(label, cls, () => apply(g));
  if (blocked(g)) {
    b.disabled = true;
    tip(b, blockText(g));
  }
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
  info.append(
    tip(el("div", "card-name", g.name), g.location ?? ""),
    el("div", "card-where", LAUNCHER[g.launcher] ?? g.launcher),
  );
  head.append(tile(g, g.status === "optimized" ? "lit" : ""), info, badge(g));

  const actions = el("div", "card-actions");
  if (g.status === "manual") {
    actions.append(button("Checklist", "", () => openDrawer(g.slug)));
  } else {
    actions.append(button("Details", "btn-ghost", () => openDrawer(g.slug)));
    if (g.status === "optimized" && g.appliedAt) actions.append(revertButton(g));
    if (g.status === "changed") actions.append(applyButton("Re-apply", "btn-outline", g));
    if (g.status === "update") actions.append(applyButton("Update", "btn-outline", g));
    if (g.status === "not-optimized") actions.append(applyButton("Optimize", "btn-outline", g));
  }
  const foot = el("div", "card-foot");
  foot.append(noteFor(g), actions);
  c.append(head, foot);
  return c;
}

// Share of the way there, as the bar under a hero number.
function meter(id, done, total) {
  const bar = $(id);
  bar.style.width = total ? `${Math.round((done / total) * 100)}%` : "0";
  bar.parentElement.classList.toggle("full", total > 0 && done === total);
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
  meter("hero-bar", optimized, fileGames.length);
  const all = $("optimize-all");
  all.hidden = todo.length === 0;
  all.textContent = `Optimize all (${todo.length})`;
  const count = $("tab-games-count");
  count.hidden = todo.length === 0;
  count.textContent = todo.length;

  $("library-count").textContent = plural(found.length, "game");
  const grid = $("grid");
  grid.replaceChildren(...found.map(card));
  if (!found.length) {
    const empty = el("div", "empty");
    empty.append(
      el("strong", null, "No supported games found"),
      el("p", null, "Installed somewhere unusual? Add the folder."),
      button("Open library", "", openSheet),
    );
    grid.append(empty);
  }

  const nf = $("not-found");
  nf.hidden = missing.length === 0;
  $("not-found-text").textContent = `Not found · ${missing.map((g) => g.name).join(", ")}`;
  $("not-found-list").replaceChildren(...missing.map((g) => {
    const li = el("li", "row");
    const info = el("div", "row-info");
    info.append(el("div", "card-name", g.name), el("div", "meta", LAUNCHER[g.launcher] ?? g.launcher));
    li.append(tile(g, "sm"), info, button("Set folder", "btn-ghost", () => pickFolder(g), "folder"));
    return li;
  }));
}

// ---------- Windows tweaks ----------

const TWEAK_STATE = { applied: "Applied", default: "Windows default", other: "Changed elsewhere" };
const touched = (p) => p.tweaks.some((t) => t.state !== "default");
const initials = (name) => name.split(/\s+/).map((w) => w[0]).join("").slice(0, 2).toUpperCase();

function profileBadge(p) {
  const done = p.applied === p.total;
  const b = el("span", `badge ${done ? "ok" : ""}`.trim());
  if (done) b.append(icon("check"));
  b.append(done ? "Applied" : p.applied ? `${p.applied} of ${p.total}` : "Off");
  return tip(b, done ? "" : `${p.applied} of ${plural(p.total, "tweak")} applied`);
}

// One tweak on a profile card: a state mark and its name. Why it helps is the hover text.
function tweakRow(t) {
  const li = el("li", "tweak");
  const mark = el("span", `state ${t.state}`);
  if (t.state === "applied") mark.append(icon("check"));
  li.append(mark, el("span", "tweak-name", t.name));
  const extra = [
    t.state === "other" ? "Set to something else right now." : "",
    t.restart ? "Needs a restart." : "",
  ].filter(Boolean).join(" ");
  return tip(li, `${t.why}${extra ? ` ${extra}` : ""}`);
}

function resetButton(scope, label) {
  const armed = armedReset === scope;
  return button(armed ? "Confirm reset" : label, armed ? "btn-danger" : "btn-ghost", () => {
    if (armed) return resetTweaks(scope);
    armedReset = scope;
    rerender();
    setTimeout(() => {
      if (armedReset === scope) { armedReset = null; rerender(); }
    }, 3000);
  });
}

function profileCard(p) {
  const c = el("article", "card");
  const head = el("div", "card-head");
  const info = el("div", "card-info");
  // The first sentence fits the card; the whole summary is the hover text.
  info.append(el("div", "card-name", p.name), el("div", "card-where", plural(p.total, "tweak")));
  head.append(tile({ tile: initials(p.name) }, p.applied === p.total ? "lit" : ""), info, profileBadge(p));
  const summary = tip(el("p", "card-summary", p.summary.split(". ")[0].replace(/\.$/, "")), p.summary);

  const list = el("ul", "tweak-list");
  list.append(...p.tweaks.map(tweakRow));

  const note = el("span", "card-note");
  if (p.admin) note.append(tip(icon("shield"), "Windows asks for permission, because some of these apply to the whole PC."));
  const actions = el("div", "card-actions");
  actions.append(button("Details", "btn-ghost", () => openProfile(p.id)));
  if (touched(p)) actions.append(resetButton(p.id, "Reset"));
  if (p.applied < p.total) actions.append(button("Apply", "btn-outline", () => applyProfile(p)));
  const foot = el("div", "card-foot");
  foot.append(note, actions);
  c.append(head, summary, list, foot);
  return c;
}

function renderTweaks() {
  $("tweaks").replaceChildren(...profiles.map(profileCard));
  // Profiles share tweaks, so count each tweak once.
  const unique = new Map(profiles.flatMap((p) => p.tweaks).map((t) => [t.id, t]));
  const applied = [...unique.values()].filter((t) => t.state === "applied").length;
  $("pc-num").textContent = unique.size ? `${applied} of ${unique.size}` : "–";
  meter("pc-bar", applied, unique.size);

  const all = $("tweaks-reset-all");
  const armed = armedReset === "all";
  all.hidden = !profiles.some(touched);
  all.replaceChildren(...(armed ? [] : [icon("undo")]), armed ? "Confirm reset" : "Reset all");
  all.className = `btn ${armed ? "btn-danger" : ""}`.trim();
}

function openProfile(id) {
  drawerProfile = id;
  drawerSlug = null;
  renderDrawer();
  if (!$("drawer").open) $("drawer").showModal();
}

function renderProfileDrawer(p) {
  const d = $("drawer");
  const head = el("div", "drawer-head");
  const titles = el("div", "titles");
  const h = el("h2", null, p.name);
  h.id = "drawer-title";
  titles.append(h, el("div", "meta", plural(p.total, "Windows tweak")));
  const close = el("button", "btn btn-ghost btn-icon");
  close.type = "button";
  close.setAttribute("aria-label", "Close");
  close.append(icon("x"));
  close.onclick = () => d.close();
  head.append(tile({ tile: initials(p.name) }, p.applied === p.total ? "lit" : ""), titles, profileBadge(p), close);

  const body = el("div", "drawer-body");
  const note = el("p", "note");
  note.append(icon("shield"), "Registry settings. Reset restores the Windows default for each, whoever set it.");
  const rows = [];
  for (const t of p.tweaks) {
    const group = tip(el("tr", "group"), `${t.why}${t.restart ? " Needs a restart." : ""}`);
    const label = el("td", null, t.name);
    label.colSpan = 3;
    group.append(label, el("td", t.state === "applied" ? "match" : "now", TWEAK_STATE[t.state] ?? t.state));
    rows.push(group);
    for (const v of t.values) {
      const tr = el("tr", "value");
      tr.append(el("td", null, v.name), el("td", null, v.now), el("td", null, v.set), el("td", null, v.default));
      rows.push(tr);
    }
  }
  body.append(note, settingsTable(rows, ["Setting", "Now", "Set to", "Windows default"]));

  const foot = el("div", "drawer-foot");
  if (touched(p)) foot.append(resetButton(p.id, "Reset to defaults"));
  if (p.applied < p.total) foot.append(button(`Apply ${p.name}`, "btn-primary", () => applyProfile(p)));
  else foot.append(button("Close", "", () => d.close()));
  d.replaceChildren(head, body, foot);
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

// A settings row: the label and values show, the reason is the row's hover text.
function settingRow(cls, why, ...cells) {
  const tr = tip(el("tr", cls), why);
  tr.append(...cells);
  return tr;
}

function renderDrawer() {
  const d = $("drawer");
  if (drawerProfile) {
    const p = profiles.find((x) => x.id === drawerProfile);
    if (p) return renderProfileDrawer(p);
    drawerProfile = null;
  }
  const g = view.games.find((x) => x.slug === drawerSlug);
  if (!g) { if (d.open) d.close(); return; }

  const head = el("div", "drawer-head");
  const titles = el("div", "titles");
  const h = el("h2", null, g.name);
  h.id = "drawer-title";
  const sub = g.status === "manual"
    ? `${plural(g.manual.length, "setting")} · set in-game`
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
    note.append(icon("list"), `${g.name} keeps these on its servers. Set them once in-game and they follow your account.`);
    const rows = g.manual.map((s) => settingRow("", s.why, el("td", null, s.label), el("td", "want", s.value)));
    body.append(note, settingsTable(rows, ["Setting", "Set to"]));
  } else {
    const warn = runningWarning(g);
    if (warn) body.append(warn);
    if (g.status === "launch-once") {
      body.append(el("p", "note", `Launch ${g.name} once so it creates its settings file, then rescan.`));
    }
    const sorted = [...g.settings].sort((a, b) => a.match - b.match);
    const rows = sorted.map((s) => {
      const want = el("td", "want");
      if (s.match) want.append(icon("check"), " ");
      want.append(s.want);
      return settingRow(s.match ? "match" : "", s.why, el("td", null, s.label), el("td", "now", s.now), want);
    });
    body.append(settingsTable(rows, ["Setting", "Now", "Chamber"]));

    const note = el("p", "note");
    note.append(icon("lock"), g.lock
      ? `Only these change, then the file is locked so ${g.name} can't undo it. Sensitivity, binds and crosshair stay yours.`
      : "Only these change. Sensitivity, binds and crosshair stay yours.");
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
    proof.append(el("div", "meta", "Not benchmarked yet."));
  }
  body.append(proof);

  const openBtn = button("Open folder", "btn-ghost", () => openFolder(g, "config"), "folder");
  switch (g.status) {
    case "not-optimized":
      foot.append(openBtn, applyButton(`Optimize ${plural(g.differ, "setting")}`, "btn-primary", g));
      break;
    case "changed":
      foot.append(openBtn, revertButton(g, `Revert all ${g.total}`), applyButton(`Re-apply ${g.differ}`, "btn-primary", g));
      break;
    case "update":
      foot.append(openBtn, revertButton(g), applyButton("Apply update", "btn-primary", g));
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
      info.append(tip(el("div", "card-name", path), path));
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
    $("welcome-sub").textContent = "Accept to use the app.";
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
      $("welcome-sub").textContent = "Steam, Epic, Riot, Battle.net, EA and Ubisoft are found automatically.";
      const libs = el("div", "libs");
      $("welcome-body").replaceChildren(
        el("p", null, "Games somewhere else, like D:\\Games? Add the folder. You can change this later in Library."),
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
    const meta = tip(el("div", "meta", detail), g.folder ?? g.location ?? "");
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
  hideTip(); // the element under the pointer is about to be replaced
  render();
  renderTweaks();
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
  runningEpoch++;
  try {
    view = await invoke("scan");
  } catch (e) {
    toast(`Scan failed: ${errText(e)}`, "error");
    return;
  }
  try {
    profiles = (await invoke("tweaks_scan")).profiles;
  } catch (e) {
    profiles = [];
    toast(`Couldn't read the Windows tweaks: ${errText(e)}`, "error");
  }
  rerender();
}

// Windows shows its permission prompt during these when a tweak covers the whole PC.
const restartNote = (list) => (list.some((t) => t.restart) ? " Restart Windows to finish." : "");

const applyProfile = (p) => run(async () => {
  try {
    const r = await invoke("tweaks_apply", { profile: p.id });
    toast(`${p.name} applied. ${plural(r.changed, "setting")} changed.${r.changed ? restartNote(p.tweaks) : ""}`, "ok");
  } catch (e) {
    toast(errText(e), "error");
  }
  await scan();
});

const resetTweaks = (scope) => run(async () => {
  armedReset = null;
  const list = scope === "all" ? profiles.flatMap((p) => p.tweaks) : profiles.find((p) => p.id === scope)?.tweaks ?? [];
  try {
    const r = await invoke("tweaks_reset", { scope });
    toast(`Back to Windows defaults. ${plural(r.changed, "setting")} changed.${r.changed ? restartNote(list) : ""}`, "ok");
  } catch (e) {
    toast(errText(e), "error");
  }
  await scan();
});

// Process check only, every few seconds and whenever the window is focused again.
// A full scan re-reads every config file; this just asks which exes are open.
let runningEpoch = 0;
let runningInFlight = false;
const RUNNING_POLL_MS = 3000;

async function refreshRunning({ force = false } = {}) {
  if ((busy && !force) || document.hidden || runningInFlight || !view.games.length) return;
  runningInFlight = true;
  const epoch = runningEpoch;
  try {
    const r = await invoke("running");
    if (epoch !== runningEpoch || (busy && !force)) return;
    const blocks = new Map((r.running ?? []).map((b) => [b.slug, b.message]));
    let changed = false;
    for (const g of view.games) {
      const message = blocks.has(g.slug) ? blocks.get(g.slug) : null;
      const next = message != null;
      if (!!g.running !== next || (g.applyBlock ?? null) !== message) {
        g.running = next;
        g.applyBlock = message;
        changed = true;
      }
    }
    if (changed) rerender();
  } catch {
    // The next tick tries again. Apply still refuses on the engine side.
  } finally {
    runningInFlight = false;
  }
}

function watchRunning() {
  const tick = () => refreshRunning();
  document.addEventListener("visibilitychange", tick);
  window.addEventListener("focus", tick);
  setInterval(tick, RUNNING_POLL_MS);
}

function showBlocked(g, raw) {
  const text = blockText(g, raw);
  if (!text) return false;
  g.running = true;
  g.applyBlock = text;
  rerender();
  toast(text, "error");
  return true;
}

const apply = (g) => run(async () => {
  await refreshRunning({ force: true });
  if (blocked(g)) {
    showBlocked(g);
    return;
  }
  try {
    const r = await invoke("apply", { slug: g.slug });
    toast(`Optimized ${g.name}. ${plural(r.changed, "setting")} changed.`, "ok");
  } catch (e) {
    const msg = errText(e);
    if (!showBlocked(g, msg)) toast(msg, "error");
  }
  await scan();
});

const optimizeAll = () => run(async () => {
  await refreshRunning({ force: true });
  const targets = view.games.filter((g) => !g.hidden && g.found && actionable(g));
  const skipped = [];
  let done = 0;
  for (const g of targets) {
    if (g.running) { skipped.push(blockText(g) || closeBeforeApplying(g.name)); continue; }
    try {
      await invoke("apply", { slug: g.slug });
      done++;
    } catch (e) {
      const msg = errText(e);
      const text = blockText(g, msg);
      if (text) {
        g.running = true;
        g.applyBlock = text;
        skipped.push(text);
      } else {
        skipped.push(`${g.name}: ${msg}`);
      }
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
  tip($("catalog-chip"), detail);
}

function hostOf(url) {
  try { return new URL(url).host; } catch { return "the update server"; }
}

// A 404 from GitHub is what a private repo returns to people who aren't signed in.
function httpFailure(url, status) {
  const host = hostOf(url);
  return status === 404
    ? `${host} answered 404. A private GitHub repo does that for anyone who isn't signed in, and so does a missing file.`
    : `${host} answered ${status}.`;
}

async function loadHosts() {
  const r = await fetch("hosts.json");
  if (!r.ok) throw new Error("hosts.json is missing from the app.");
  const h = await r.json();
  for (const key of ["catalogUrl", "catalogSigUrl", "releasesUrl", "releasePagePrefix"]) {
    let url;
    try { url = new URL(h[key]); } catch { throw new Error(`${key} is not a URL.`); }
    if (url.protocol !== "https:") throw new Error(`${key} must use https.`);
  }
  if (!h.catalogUrl.endsWith("/catalog.json") || !h.catalogSigUrl.endsWith("/catalog.json.sig")) {
    throw new Error("The catalog host must serve catalog.json and catalog.json.sig.");
  }
  if (!h.releasePagePrefix.endsWith("/")) throw new Error("releasePagePrefix must end with /.");
  if (typeof h.includePrereleases !== "boolean") throw new Error("includePrereleases must be true or false.");
  return h;
}

// The webview downloads; the engine verifies the signature before anything is used.
async function refreshCatalog() {
  setChip("checking", "Checking…", "Checking for new configs.");
  let json, sig;
  try {
    [json, sig] = await Promise.all([hosts.catalogUrl, hosts.catalogSigUrl].map(async (url) => {
      const r = await fetch(url, { cache: "no-store" });
      if (!r.ok) throw new Error(httpFailure(url, r.status));
      const bytes = new Uint8Array(await r.arrayBuffer());
      // The engine has the same limit; this keeps a huge download from crossing into it at all.
      if (bytes.length > 4 * 1024 * 1024) throw new Error(`${hostOf(url)} sent a catalog that is too large.`);
      return Array.from(bytes);
    }));
  } catch (e) {
    setChip("error", "Configs offline", `Couldn't check for new configs. ${errText(e)} Using the configs already on this PC.`);
    return;
  }
  try {
    const r = await invoke("install_catalog", { json, sig });
    if (r.status === "updated") await scan();
    setChip("fresh", r.status === "updated" ? "Configs updated" : "Up to date", r.status === "updated" ? "New configs were just downloaded." : "You have the latest configs.");
  } catch (e) {
    setChip("error", "Configs rejected", `New configs were rejected. ${errText(e)} Using the configs already on this PC.`);
  }
}

function showUpdateError(detail) {
  const chip = $("update-chip");
  chip.hidden = false;
  chip.disabled = true;
  chip.dataset.state = "error";
  chip.classList.remove("chip-violet");
  chip.textContent = "Update check failed";
  tip(chip, detail);
  chip.onclick = null;
}

async function checkAppUpdate() {
  const chip = $("update-chip");
  try {
    const r = await fetch(hosts.releasesUrl, {
      cache: "no-store",
      headers: { Accept: "application/vnd.github+json" },
    });
    if (!r.ok) {
      showUpdateError(httpFailure(hosts.releasesUrl, r.status));
      return;
    }
    const choice = ChamberUpdates.chooseRelease(await r.json(), appVersion, hosts);
    if (choice.status === "error") {
      showUpdateError(choice.detail);
      return;
    }
    if (choice.status !== "update") {
      chip.hidden = true;
      return;
    }
    chip.hidden = false;
    chip.disabled = false;
    chip.dataset.state = "available";
    chip.classList.add("chip-violet");
    chip.textContent = `v${choice.version} available`;
    tip(chip, "Open the release page");
    chip.onclick = () => openUrl(choice.url);
  } catch (e) {
    showUpdateError(errText(e));
  }
}

// ---------- hover text ----------

const tipBox = $("tip");
let tipFor = null;

function hideTip() {
  tipFor = null;
  if (tipBox.matches(":popover-open")) tipBox.hidePopover();
}

function showTip(target) {
  tipFor = target;
  tipBox.textContent = target.dataset.tip;
  // Shown again each time, so it lands above a dialog opened since the last one.
  if (tipBox.matches(":popover-open")) tipBox.hidePopover();
  tipBox.showPopover();
  const at = target.getBoundingClientRect();
  const box = tipBox.getBoundingClientRect();
  // Centered over small things; lined up with the start of a wide row so it covers less.
  const start = at.width > 320 ? at.left : at.left + at.width / 2 - box.width / 2;
  const left = Math.min(Math.max(8, start), window.innerWidth - box.width - 8);
  const above = at.top - box.height - 8;
  tipBox.style.left = `${left}px`;
  tipBox.style.top = `${above >= 8 ? above : at.bottom + 8}px`;
}

const tipTarget = (e) => (e.target instanceof Element ? e.target.closest("[data-tip]") : null);
document.addEventListener("mouseover", (e) => {
  const t = tipTarget(e);
  if (t === tipFor) return;
  if (t) showTip(t);
  else hideTip();
});
document.addEventListener("focusin", (e) => {
  const t = tipTarget(e);
  if (t?.matches(":focus-visible")) showTip(t);
});
document.addEventListener("focusout", hideTip);
document.addEventListener("mousedown", hideTip);
document.addEventListener("scroll", hideTip, true);
document.documentElement.addEventListener("mouseleave", hideTip);

// ---------- tabs + specs panel ----------

// Which tab and whether the specs panel is open survive a restart. Nothing else is stored here.
function recall(key) {
  try { return localStorage.getItem(`fps.${key}`); } catch { return null; }
}
function remember(key, value) {
  try { localStorage.setItem(`fps.${key}`, value); } catch { /* the choice just won't persist */ }
}

function showTab(name) {
  const tab = name === "pc" ? "pc" : "games";
  for (const t of document.querySelectorAll(".tab")) {
    const on = t.dataset.tab === tab;
    t.setAttribute("aria-selected", String(on));
    t.tabIndex = on ? 0 : -1;
  }
  $("panel-games").hidden = tab !== "games";
  $("panel-pc").hidden = tab !== "pc";
  document.querySelector("main").scrollTop = 0;
  remember("tab", tab);
}

function showSpecs(open) {
  document.body.classList.toggle("no-specs", !open);
  $("specs-btn").setAttribute("aria-expanded", String(open));
  remember("specs", open ? "open" : "closed");
}

let specs = null;

// "12th Gen Intel(R) Core(TM) i7-12700K CPU @ 3.60GHz" reads better as "Intel Core i7-12700K".
const cpuName = (s) => s
  .replace(/\((R|TM|C)\)/gi, "")
  .replace(/^\d+th Gen /i, "")
  .replace(/\s+\d+-Core Processor$/i, "")
  .replace(/\s+CPU\s*@.*$/i, "")
  .replace(/\s+/g, " ")
  .trim();

// [icon, label, value, detail] per row. The panel and Copy both read from this.
function specRows(s) {
  const rows = [["cpu", "CPU", s.cpu ? cpuName(s.cpu) : "Unknown", `${plural(s.cores, "core")} · ${plural(s.threads, "thread")}`]];
  for (const g of s.gpus) {
    rows.push(["gpu", "GPU", g.name, [g.vramGb && `${g.vramGb} GB`, g.driver && `Driver ${g.driver}`].filter(Boolean).join(" · ")]);
  }
  if (!s.gpus.length) rows.push(["gpu", "GPU", "Unknown", ""]);
  rows.push(["memory", "Memory", s.ramGb ? `${s.ramGb} GB` : "Unknown", ""]);
  rows.push(["monitor", "Display", `${s.width} × ${s.height}`, `${s.refresh} Hz`]);
  if (s.board) rows.push(["board", "Motherboard", s.board, s.boardMaker ?? ""]);
  if (s.os) rows.push(["os", "Windows", s.os, s.osVersion ?? ""]);
  return rows;
}

function renderSpecs() {
  $("specs-copy").hidden = !specs;
  if (!specs) {
    $("specs-list").replaceChildren(el("p", "meta", "Couldn't read this PC's specs."));
    return;
  }
  $("specs-list").replaceChildren(...specRows(specs).map(([ic, label, value, detail]) => {
    const row = el("div", "spec");
    const mark = el("span", "spec-icon");
    mark.append(icon(ic));
    row.append(mark, el("dt", null, label), el("dd", "spec-value", value));
    if (detail) row.append(el("dd", "spec-sub num", detail));
    return row;
  }));
}

async function loadSpecs() {
  try {
    specs = await invoke("specs");
  } catch {
    specs = null;
  }
  renderSpecs();
}

async function copySpecs() {
  if (!specs) return;
  const text = specRows(specs).map(([, label, value, detail]) => `${label}: ${value}${detail ? ` (${detail})` : ""}`).join("\n");
  try {
    await navigator.clipboard.writeText(text);
    toast("Specs copied", "ok");
  } catch {
    toast("Couldn't copy. A screenshot of the panel works too.", "error");
  }
}

// ---------- wiring ----------

for (const t of document.querySelectorAll(".tab")) {
  t.onclick = () => showTab(t.dataset.tab);
  t.onkeydown = (e) => {
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    const other = t.dataset.tab === "games" ? "pc" : "games";
    showTab(other);
    $(`tab-${other}`).focus();
  };
}
showTab(recall("tab"));
showSpecs(recall("specs") !== "closed");
$("specs-btn").onclick = () => showSpecs(document.body.classList.contains("no-specs"));
$("specs-copy").onclick = copySpecs;

$("rescan-btn").onclick = () => run(scan);
$("library-btn").onclick = openSheet;
$("optimize-all").onclick = optimizeAll;
$("tweaks-reset-all").onclick = () => {
  if (armedReset === "all") return resetTweaks("all");
  armedReset = "all";
  rerender();
  setTimeout(() => {
    if (armedReset === "all") { armedReset = null; rerender(); }
  }, 3000);
};
for (const b of document.querySelectorAll("[data-url]")) b.onclick = () => openUrl(b.dataset.url);
for (const d of [$("drawer"), $("sheet")]) {
  // A click that lands on the dialog itself is a click on its backdrop
  d.addEventListener("click", (e) => { if (e.target === d) d.close(); });
}
$("sheet").querySelector("[data-close]").onclick = () => $("sheet").close();
$("drawer").addEventListener("close", () => { drawerSlug = null; drawerProfile = null; });

(async () => {
  appVersion = await window.__TAURI__.app.getVersion().catch(() => appVersion);
  $("version").textContent = appVersion;
  loadSpecs();
  await scan();
  watchRunning();
  const terms = await loadTerms();
  if (view.terms < terms.version) await welcome(terms);
  try {
    hosts = await loadHosts();
  } catch (e) {
    setChip("error", "Configs offline", errText(e));
    showUpdateError(errText(e));
    return;
  }
  refreshCatalog();
  checkAppUpdate();
})();
