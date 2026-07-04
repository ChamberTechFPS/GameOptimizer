const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

const $ = (id) => document.getElementById(id);
let REPO_URL = "https://github.com/ChamberTechFPS/GameOptimizer"; // fallback; the engine owns the real value

async function engine(action, game, path) {
  const raw = await invoke("engine", { action, game: game ?? null, path: path ?? null });
  return JSON.parse(raw);
}

// One engine action at a time — concurrent applies would race on state.json
let busy = false;

// PowerShell errors arrive with stack noise; keep the first line only
function errText(e) {
  return String(e).split("\n")[0].slice(0, 200);
}

let toastTimer;
function toast(message, kind = "info") {
  const el = $("toast");
  el.textContent = message;
  el.className = `toast ${kind}`;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.add("hidden"), 5000);
}

const PLATFORM_LABEL = { steam: "Steam", epic: "Epic Games", riot: "Riot" };

const BADGE = {
  applied: { cls: "applied", label: "Applied" },
  drifted: { cls: "drifted", label: "Drifted" },
  "not-applied": { cls: "not-applied", label: "Not applied" },
  guide: { cls: "guide", label: "Guide" },
  "not-installed": { cls: "not-applied", label: "Not installed" },
};

function meta(g) {
  const platform = PLATFORM_LABEL[g.platform] ?? g.platform;
  if (g.status === "applied" && g.appliedAt)
    return `${platform} · applied ${new Date(g.appliedAt).toLocaleDateString()}`;
  if (g.status === "drifted") return `${platform} · config changed since apply`;
  if (g.status === "guide") return `${platform} · settings stored server-side`;
  if (g.status === "not-installed")
    return g.customPath ? `${platform} · custom folder` : `${platform} · not detected`;
  return `${platform} · not yet optimized`;
}

// Revert is two-step: first click arms it, second click within 3s confirms
let pendingRevert = null;
let pendingRevertTimer;

function revertAction(g) {
  return pendingRevert === g.slug
    ? { label: "Confirm revert?", act: "revert", danger: true }
    : { label: "Revert", act: "confirm-revert" };
}

function actionsFor(g) {
  const location = g.customPath
    ? [{ label: "Change folder", act: "set-path" }, { label: "Clear", act: "clear-path", ghost: true }]
    : [{ label: "Set location", act: "set-path" }];

  switch (g.status) {
    case "applied": return [revertAction(g)];
    case "drifted": return [{ label: "Re-apply", act: "apply" }, revertAction(g)];
    case "not-applied": return g.customPath || g.needsPath
      ? [{ label: "Apply", act: "apply" }, ...location]
      : [{ label: "Apply", act: "apply" }];
    case "guide": return [{ label: "View guide", act: "guide" }];
    case "not-installed": return location;
    default: return [];
  }
}

let lastGames = [];

function render(games) {
  lastGames = games;
  const list = $("game-list");
  list.innerHTML = "";
  let detected = 0, optimized = 0, drifted = 0;

  if (games.length === 0) {
    const p = document.createElement("p");
    p.className = "loading";
    p.textContent = "No game configs found — try Update configs, or reinstall the app.";
    list.append(p);
  }

  for (const g of games) {
    if (g.installed) detected++;
    if (g.status === "applied") optimized++;
    if (g.status === "drifted") drifted++;

    const row = document.createElement("div");
    row.className = "row" + (g.installed ? "" : " dim");

    const tile = document.createElement("div");
    tile.className = "tile";
    tile.textContent = g.name.slice(0, 2).toUpperCase();

    const info = document.createElement("div");
    info.className = "row-info";
    info.innerHTML = `<div class="row-name"></div><div class="row-meta"></div>`;
    info.querySelector(".row-name").textContent = g.name;
    info.querySelector(".row-meta").textContent = meta(g);
    if (g.customPath) {
      const path = document.createElement("div");
      path.className = "row-path";
      path.innerHTML = `<b>Folder:</b> `;
      path.append(g.customPath);
      path.title = g.customPath;
      info.append(path);
    }

    const badge = document.createElement("span");
    const b = BADGE[g.status] ?? BADGE["not-applied"];
    badge.className = `badge ${b.cls}`;
    badge.textContent = b.label;

    row.append(tile, info, badge);

    for (const a of actionsFor(g)) {
      const btn = document.createElement("button");
      btn.className = a.ghost ? "btn btn-ghost" : a.danger ? "btn btn-danger" : "btn";
      btn.textContent = a.label;
      btn.onclick = () => handleAction(a.act, g, btn);
      row.append(btn);
    }
    list.append(row);
  }

  $("stat-detected").textContent = detected;
  $("stat-optimized").textContent = optimized;
  $("stat-drifted").textContent = drifted;
}

async function handleAction(act, g, btn) {
  if (act === "guide") {
    await invoke("open_url", { url: `${REPO_URL}/blob/main/games/${g.slug}/guide.md` });
    return;
  }
  if (act === "confirm-revert") {
    pendingRevert = g.slug;
    clearTimeout(pendingRevertTimer);
    pendingRevertTimer = setTimeout(() => {
      if (pendingRevert === g.slug) {
        pendingRevert = null;
        render(lastGames);
      }
    }, 3000);
    render(lastGames);
    return;
  }
  if (busy) return;
  busy = true;
  document.body.classList.add("busy");
  try {
    if (act === "set-path") {
      const picked = await open({
        directory: true,
        multiple: false,
        title: g.customLabel || `Select the folder for ${g.name}`,
      });
      if (!picked) return; // user cancelled
      try {
        await engine("set-path", g.slug, picked);
        toast(`Set ${g.name} folder`, "ok");
      } catch (e) {
        toast(`Couldn't set folder for ${g.name}: ${errText(e)}`, "error");
      }
      await scan();
      return;
    }

    if (act === "revert") pendingRevert = null;
    const VERB = { apply: "Applied", revert: "Reverted", "clear-path": "Cleared folder for" };
    btn.disabled = true;
    btn.textContent = "…";
    try {
      await engine(act, g.slug);
      toast(`${VERB[act] ?? act} ${g.name}`, "ok");
    } catch (e) {
      toast(`${act} failed for ${g.name}: ${errText(e)}`, "error");
    }
    await scan();
  } finally {
    busy = false;
    document.body.classList.remove("busy");
  }
}

async function scan() {
  try {
    render(await engine("scan"));
  } catch (e) {
    const p = document.createElement("p");
    p.className = "loading";
    p.textContent = `Scan failed: ${errText(e)}`;
    $("game-list").replaceChildren(p);
  }
}

async function loadVersion() {
  try {
    const { appVersion, repoUrl } = await engine("version");
    if (appVersion) $("app-version").textContent = appVersion;
    if (repoUrl) REPO_URL = repoUrl;
  } catch { /* leave the fallbacks from the HTML/JS */ }
}

async function checkUpdates() {
  let info;
  try {
    info = await engine("check-update");
  } catch {
    $("net-status").classList.remove("hidden");
    return;
  }
  if (!info || info.error) {
    if (info?.error) $("net-status").classList.remove("hidden");
    return;
  }
  $("net-status").classList.add("hidden");

  if (info.configUpdates && info.configUpdates.length > 0) {
    $("update-banner-text").textContent =
      `New configs available: ${info.configUpdates.join(", ")} (catalog v${info.catalogLatest})`;
    $("update-banner").classList.remove("hidden");
  }
  if (info.appUpdateAvailable) {
    $("modal-text").textContent =
      `Chamber Game Optimizer v${info.appLatest} is available (you have v${info.appCurrent}).`;
    $("modal").classList.remove("hidden");
    $("modal-download").onclick = () => invoke("open_url", { url: info.releaseUrl });
  }
}

function closeModal() {
  $("modal").classList.add("hidden");
}

$("rescan-btn").onclick = scan;
$("update-dismiss").onclick = () => $("update-banner").classList.add("hidden");
$("modal-close").onclick = closeModal;
$("modal").onclick = (e) => {
  if (e.target === e.currentTarget) closeModal();
};
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") closeModal();
});
$("update-btn").onclick = async () => {
  if (busy) return;
  busy = true;
  document.body.classList.add("busy");
  const btn = $("update-btn");
  btn.disabled = true;
  btn.textContent = "Updating…";
  try {
    await engine("update");
    $("update-banner").classList.add("hidden");
    toast("Configs updated", "ok");
    await scan();
  } catch (e) {
    toast(`Update failed: ${errText(e)}`, "error");
  } finally {
    busy = false;
    document.body.classList.remove("busy");
    btn.disabled = false;
    btn.textContent = "Update configs";
  }
};

for (const el of document.querySelectorAll(".watermark [data-url]")) {
  el.onclick = () => invoke("open_url", { url: el.dataset.url });
}

loadVersion();
scan().then(checkUpdates);
