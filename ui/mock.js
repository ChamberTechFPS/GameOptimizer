// ponytail: dev-only stand-in for the Tauri engine so the UI can be previewed in a plain browser.
// Inside the app window.__TAURI__ already exists and this file does nothing.
(() => {
  if (window.__TAURI__) return;

  const s = (label, now, want, why) => ({ label, now, want, why, original: now });
  const sample = { rig: "Sample data, not a real test", avg: [212, 268], low1: [142, 188], method: "CapFrameX, 3 × 60 s" };
  const games = [
    {
      slug: "fortnite", name: "Fortnite", tile: "FN", launcher: "epic", found: true, lock: true,
      location: "C:\\Program Files\\Epic Games\\Fortnite", files: ["GameUserSettings.ini"], proof: sample,
      applied: true, appliedAt: Date.parse("2026-10-20") / 1000,
      settings: [
        s("Rendering mode", "DirectX 12", "Performance", "Strips the heavy effects pipeline. Same visibility, far more frames."),
        s("Frame rate limit", "Unlimited", "165", "Capped at your monitor's refresh rate for steady frame times."),
        s("Shadows", "Epic", "Off", "Shadows redraw the scene from every light. Off removes that work."),
        s("NVIDIA Reflex", "On + boost", "On + boost", "Lowers system latency between your click and the shot."),
        s("Anti-aliasing", "Off", "Off", "Smoothing edges blurs distant players."),
        s("View distance", "Far", "Far", "Keeps far players and builds drawn."),
        s("Textures", "Low", "Low", "Frees video memory. No gameplay effect."),
        s("Effects", "Low", "Low", "Less particle clutter in fights."),
        s("Post processing", "Low", "Low", "Removes bloom and haze over the scene."),
        s("Motion blur", "Off", "Off", "Blur hides movement you need to track."),
        s("V-Sync", "Off", "Off", "V-Sync adds a frame or more of input lag."),
        s("3D resolution", "100%", "100%", "Below native makes players harder to see."),
        s("Grass", "Off", "Off", "Removes cover that only hides you on your screen."),
        s("Ray tracing", "Off", "Off", "Large frame cost, no competitive value."),
      ],
    },
    {
      slug: "rocket-league", name: "Rocket League", tile: "RL", launcher: "steam", found: true, lock: false,
      location: "D:\\SteamLibrary\\steamapps\\common\\rocketleague", files: ["TASystemSettings.ini"], proof: sample,
      settings: [
        s("Dynamic shadows", "On", "Off", "Costs frames and adds no information about the ball."),
        s("Motion blur", "On", "Off", "Blur hides fast touches."),
        s("Bloom", "On", "Off", "Glow over goal explosions hides the ball."),
        s("Lens flares", "On", "Off", "Pure decoration."),
        s("Depth of field", "On", "Off", "Blurs the far side of the field."),
        s("Ambient occlusion", "On", "Off", "Contact shadows you never use."),
        s("V-Sync", "On", "Off", "V-Sync adds input lag."),
        s("Full screen", "On", "On", "Exclusive full screen has the lowest latency."),
        s("Resolution", "1920 × 1080", "1920 × 1080", "Your monitor's native resolution."),
      ],
    },
    {
      slug: "valorant", name: "Valorant", tile: "VA", launcher: "riot", found: true,
      location: "C:\\Riot Games\\VALORANT", files: [], proof: null, settings: [],
      manual: [
        { label: "Multithreaded rendering", value: "On", why: "Spreads the render work across your cores." },
        { label: "Material quality", value: "Low", why: "No visibility difference in fights." },
        { label: "Texture quality", value: "Low", why: "Frees video memory." },
        { label: "Detail quality", value: "Low", why: "Removes map clutter." },
        { label: "Vignette", value: "Off", why: "Darkens the edges of your screen." },
        { label: "V-Sync", value: "Off", why: "Adds input lag." },
        { label: "Anti-aliasing", value: "MSAA 2x", why: "Smooth enough without blurring heads." },
        { label: "Bloom", value: "Off", why: "Glow hides enemies near lights." },
        { label: "NVIDIA Reflex", value: "On + boost", why: "Lower click-to-shot latency." },
      ],
    },
    {
      slug: "cs2", name: "Counter-Strike 2", tile: "CS", launcher: "steam", found: true, lock: false,
      location: "D:\\SteamLibrary\\steamapps\\common\\Counter-Strike Global Offensive", files: ["cs2_video.txt"], proof: null,
      applied: true, appliedAt: Date.parse("2026-10-22") / 1000,
      settings: [
        s("Boost player contrast", "Enabled", "Enabled", "Makes models stand out from the map."),
        s("Shadow quality", "High", "High", "Stays high: you see enemy shadows around corners."),
        s("Shader detail", "Low", "Low", "Frames back, no visibility cost."),
        s("NVIDIA Reflex", "Enabled", "Enabled", "Lower click-to-shot latency."),
        s("V-Sync", "Disabled", "Disabled", "Adds input lag."),
      ],
    },
    {
      slug: "overwatch-2", name: "Overwatch 2", tile: "OW", launcher: "battlenet", found: true, lock: false, running: true,
      location: "C:\\Program Files (x86)\\Overwatch", files: ["Settings_v0.ini"], proof: null,
      settings: [
        s("Reduce buffering", "Off", "On", "Cuts a frame of queued input lag."),
        s("Shadows", "High", "Off", "Shadows cost frames in team fights."),
        s("Effects", "High", "Low", "Less ability clutter on screen."),
        s("Render scale", "100%", "100%", "Below native blurs targets."),
      ],
    },
    {
      slug: "apex", name: "Apex Legends", tile: "AP", launcher: "ea", found: true, noConfig: true,
      location: "C:\\Program Files\\EA Games\\Apex", files: ["videoconfig.txt"], proof: null,
      settings: [s("Model detail", "Not set", "Low", "Frames back in fights.")],
    },
    { slug: "marvel-rivals", name: "Marvel Rivals", tile: "MR", launcher: "steam", found: false, files: [], settings: [] },
    { slug: "r6-siege", name: "Rainbow Six Siege", tile: "R6", launcher: "ubisoft", found: false, hidden: true, files: [], settings: [] },
  ];
  for (const g of games) {
    g.manual ??= [];
    g.autoFound = g.found;
    g.folder = null;
    g.hidden ??= false;
    g.running ??= false;
    g.rev = g.appliedRev = "a1b2c3d4";
  }

  function statusOf(g) {
    if (!g.found) return "not-found";
    if (g.manual.length) return "manual";
    if (g.noConfig) return "launch-once";
    const differ = g.settings.filter((x) => x.now !== x.want).length;
    if (g.applied) return differ ? "changed" : g.appliedRev !== g.rev ? "update" : "optimized";
    return differ ? "not-optimized" : "optimized";
  }

  function view(g) {
    const settings = g.settings.map(({ label, now, want, why }) => ({ label, now, want, why, match: now === want }));
    return {
      slug: g.slug, name: g.name, tile: g.tile, launcher: g.launcher, found: g.found,
      location: g.folder ?? g.location ?? null, folder: g.folder, hidden: g.hidden,
      status: statusOf(g), running: g.running, total: settings.length,
      differ: settings.filter((x) => !x.match).length, appliedAt: g.applied ? g.appliedAt : null,
      files: g.files, lock: !!g.lock, settings, manual: g.manual, proof: g.proof ?? null,
    };
  }

  // ?accepted skips the first-launch terms screen
  const mockState = { terms: location.search.includes("accepted") ? 99 : 0, libraries: [] };
  const find = (slug) => games.find((g) => g.slug === slug);
  const commands = {
    scan: () => ({ games: games.map(view), terms: mockState.terms, libraries: mockState.libraries }),
    accept_terms: ({ version }) => { mockState.terms = version; return {}; },
    add_library: ({ path }) => { if (!mockState.libraries.includes(path)) mockState.libraries.push(path); return {}; },
    remove_library: ({ path }) => { mockState.libraries = mockState.libraries.filter((l) => l !== path); return {}; },
    quit: () => { console.log("quit"); return {}; },
    apply: ({ slug }) => {
      const g = find(slug);
      if (g.running) throw new Error(`Close ${g.name} first, then try again.`);
      const changed = g.settings.filter((x) => x.now !== x.want).length;
      for (const x of g.settings) x.now = x.want;
      Object.assign(g, { applied: true, appliedAt: Date.now() / 1000, appliedRev: g.rev });
      return { changed };
    },
    revert: ({ slug }) => {
      const g = find(slug);
      for (const x of g.settings) x.now = x.original;
      g.applied = false;
      return {};
    },
    set_folder: ({ slug, path }) => { Object.assign(find(slug), { folder: path, found: true }); return {}; },
    clear_folder: ({ slug }) => { const g = find(slug); Object.assign(g, { folder: null, found: g.autoFound }); return {}; },
    set_hidden: ({ slug, hidden }) => { find(slug).hidden = hidden; return {}; },
    open_folder: (args) => { console.log("open_folder", args); return {}; },
    install_catalog: () => ({ status: "current" }),
  };

  const realFetch = window.fetch.bind(window);
  window.fetch = (url, opts) => {
    if (String(url).startsWith("https://raw.githubusercontent.com/")) return Promise.resolve(new Response("{}"));
    if (String(url).startsWith("https://api.github.com/")) return Promise.resolve(new Response("", { status: 404 }));
    return realFetch(url, opts);
  };

  window.__TAURI__ = {
    core: {
      invoke: (cmd, args = {}) => new Promise((resolve, reject) => {
        setTimeout(() => {
          try { resolve(structuredClone(commands[cmd](args))); } catch (e) { reject(e.message); }
        }, 120);
      }),
    },
    dialog: { open: async (o) => (o?.title?.startsWith("Pick a folder") ? "D:\\Games" : "D:\\Games\\Marvel Rivals") },
    opener: { openUrl: async (url) => console.log("openUrl", url) },
    app: { getVersion: async () => "1.0.0" },
  };
})();
