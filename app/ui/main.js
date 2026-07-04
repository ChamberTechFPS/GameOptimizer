const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

const $ = (id) => document.getElementById(id);
const REPO_URL = "https://github.com/ChamberTechFPS/GameOptimizer";

async function engine(action, game, path) {
  const raw = await invoke("engine", { action, game: game ?? null, path: path ?? null });
  return JSON.parse(raw);
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

function actionsFor(g) {
  const location = g.customPath
    ? [{ label: "Change folder", act: "set-path" }, { label: "Clear", act: "clear-path", ghost: true }]
    : [{ label: "Set location", act: "set-path" }];

  switch (g.status) {
    case "applied": return [{ label: "Revert", act: "revert" }];
    case "drifted": return [{ label: "Re-apply", act: "apply" }, { label: "Revert", act: "revert" }];
    case "not-applied": return g.customPath || g.needsPath
      ? [{ label: "Apply", act: "apply" }, ...location]
      : [{ label: "Apply", act: "apply" }];
    case "guide": return [{ label: "View guide", act: "guide" }];
    case "not-installed": return location;
    default: return [];
  }
}

function render(games) {
  const list = $("game-list");
  list.innerHTML = "";
  let detected = 0, optimized = 0, drifted = 0;

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
      btn.className = a.ghost ? "btn btn-ghost" : "btn";
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
      toast(`Couldn't set folder for ${g.name}: ${e}`, "error");
    }
    await scan();
    return;
  }

  const VERB = { apply: "Applied", revert: "Reverted", "clear-path": "Cleared folder for" };
  btn.disabled = true;
  btn.textContent = "…";
  try {
    await engine(act, g.slug);
    toast(`${VERB[act] ?? act} ${g.name}`, "ok");
  } catch (e) {
    toast(`${act} failed for ${g.name}: ${e}`, "error");
  }
  await scan();
}

async function scan() {
  try {
    render(await engine("scan"));
  } catch (e) {
    const p = document.createElement("p");
    p.className = "loading";
    p.textContent = `Scan failed: ${e}`;
    $("game-list").replaceChildren(p);
  }
}

async function loadVersion() {
  try {
    const { appVersion } = await engine("version");
    if (appVersion) $("app-version").textContent = appVersion;
  } catch { /* leave the fallback shown in the HTML */ }
}

async function checkUpdates() {
  let info;
  try {
    info = await engine("check-update");
  } catch { return; }
  if (!info || info.error) return;

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

$("rescan-btn").onclick = scan;
$("update-dismiss").onclick = () => $("update-banner").classList.add("hidden");
$("modal-close").onclick = () => $("modal").classList.add("hidden");
$("update-btn").onclick = async () => {
  const btn = $("update-btn");
  btn.disabled = true;
  btn.textContent = "Updating…";
  try {
    await engine("update");
    $("update-banner").classList.add("hidden");
    toast("Configs updated", "ok");
    await scan();
  } catch (e) {
    toast(`Update failed: ${e}`, "error");
  }
  btn.disabled = false;
  btn.textContent = "Update configs";
};

for (const el of document.querySelectorAll(".watermark [data-url]")) {
  el.onclick = () => invoke("open_url", { url: el.dataset.url });
}

loadVersion();
scan().then(checkUpdates);
