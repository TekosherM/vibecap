import { DEMO_CONSOLE, DEMO_FRONTEND_DOM, DEMO_HTTP } from "@/lib/demo-data";

export type HookMedium = "jpeg" | "webm" | "json";
export type HookLayer = "frontend" | "backend" | "database" | "logs" | "capture";

export type HookDef = {
  id: string;
  layer: HookLayer;
  title: string;
  medium: HookMedium;
  tool: string;
  bind: string;
  hook_when: string[];
  skip_when: string[];
};

/** Shared routing table — the studio UI and the agent HTTP surface both read this. */
export const HOOK_CATALOG: HookDef[] = [
  {
    id: "dom",
    layer: "frontend",
    title: "DOM",
    medium: "json",
    tool: "vibecap_ingest_frontend",
    bind: "Instrumented subject (Lumen Cart). Not a random browser tab.",
    hook_when: [
      "Wrong total, label, or CTA on screen",
      "Clipped or overflowing UI",
      "Need structure (headings, cart, buttons) — not just pixels",
    ],
    skip_when: ["Server-only 500 with no UI change — use backend"],
  },
  {
    id: "console",
    layer: "frontend",
    title: "Browser console",
    medium: "json",
    tool: "vibecap_ingest_frontend",
    bind: "Same subject as DOM. Lives in the frontend JSON bundle.",
    hook_when: [
      "Uncaught exception",
      "React / framework warning",
      "Failed fetch printed to console",
    ],
    skip_when: ["Quiet console and the bug is purely visual — a still is enough"],
  },
  {
    id: "http",
    layer: "backend",
    title: "HTTP traces",
    medium: "json",
    tool: "vibecap_ingest_backend",
    bind: "Subject API (cart, tax, checkout).",
    hook_when: [
      "Status ≥ 400",
      "Timeout or slow request",
      "Response body contradicts the UI",
    ],
    skip_when: ["UI-only copy bug with 200s — use DOM + still"],
  },
  {
    id: "terminal",
    layer: "backend",
    title: "Shell / runtime",
    medium: "json",
    tool: "vibecap_ingest_backend",
    bind: "Compose / process logs. Cloud (Neon) health is included.",
    hook_when: [
      "Stack trace",
      "Process crash",
      "Need the throwing line (pricing.ts:88)",
    ],
    skip_when: ["No server log line for this repro — HTTP + DOM may suffice"],
  },
  {
    id: "database",
    layer: "database",
    title: "Database",
    medium: "json",
    tool: "vibecap_ingest_database",
    bind: "Connected Postgres (Neon in production, PGLite in preview).",
    hook_when: [
      "Wrong stock, price, or row",
      "UI shows data the table does not",
      "Need schema + sample rows",
    ],
    skip_when: ["Bug never touches persisted state"],
  },
  {
    id: "logs",
    layer: "logs",
    title: "Session log stream",
    medium: "json",
    tool: "vibecap_ingest_logs",
    bind: "This studio session — frontend, backend, database, system lines.",
    hook_when: [
      "Need a timeline of what was collected",
      "Correlate FE + BE timestamps",
    ],
    skip_when: ["First minute of a job — collect the layers first, then logs"],
  },
  {
    id: "still",
    layer: "capture",
    title: "Still",
    medium: "jpeg",
    tool: "vibecap_snapshot",
    bind: "Live shutter (demo, screen, or camera). Studio tab must stay open.",
    hook_when: [
      "Need the pixels as they are right now",
      "During recording, at a failure frame",
      "Single-screen bug (no motion)",
    ],
    skip_when: ["Need motion or timing — use record instead"],
  },
  {
    id: "video",
    layer: "capture",
    title: "Video",
    medium: "webm",
    tool: "vibecap_record_start",
    bind: "Live shutter. Unbounded — no duration. Stop when the UI settles.",
    hook_when: [
      "Multi-step flow (cart → pay → error)",
      "Timing, animation, or race",
      "Keep the camera on until results settle",
    ],
    skip_when: ["Single static screen — one still is cheaper"],
  },
];

export type HookFacts = {
  attached: boolean;
  recording: boolean;
  inspecting: boolean;
  source: string;
  collected: string[];
  captureCount: number;
  stockZero: number;
};

export type HookStatus = HookDef & {
  live: boolean;
  available_now: boolean;
  collected: boolean;
  recommend: boolean;
  reason: string;
  signals: string[];
};

export type HookPlan = {
  subject: {
    name: string;
    url: string;
    source: string;
    attached: boolean;
    note: string;
  };
  signals: {
    console_errors: number;
    console_warns: number;
    http_fail: number;
    visual_issues: number;
    stock_zero: number;
  };
  medium: {
    jpeg: { available: boolean; why: string };
    webm: { available: boolean; why: string };
    json: { available: boolean; why: string };
  };
  hooks: HookStatus[];
  next: Array<{ tool: string; why: string }>;
  rule: string;
};

export function evaluateHooks(facts: HookFacts): HookPlan {
  const consoleErrors = DEMO_CONSOLE.filter((l) => l.level === "error");
  const consoleWarns = DEMO_CONSOLE.filter((l) => l.level === "warn");
  const httpFail = DEMO_HTTP.filter((h) => h.status >= 400);
  const visual = DEMO_FRONTEND_DOM.issues;
  const collected = new Set(facts.collected);
  const jpegOn = facts.attached;

  const signals = {
    console_errors: consoleErrors.length,
    console_warns: consoleWarns.length,
    http_fail: httpFail.length,
    visual_issues: visual.length,
    stock_zero: facts.stockZero,
  };

  const hooks: HookStatus[] = HOOK_CATALOG.map((def) => {
    if (def.id === "dom") {
      const live = visual.length > 0;
      return {
        ...def,
        live,
        available_now: true,
        collected: collected.has("frontend"),
        recommend: live && !collected.has("frontend"),
        reason: live ? `${visual.length} DOM issues on the pay screen` : "No visual issues flagged",
        signals: visual.map((i) => i.detail),
      };
    }
    if (def.id === "console") {
      const live = consoleErrors.length + consoleWarns.length > 0;
      return {
        ...def,
        live,
        available_now: true,
        collected: collected.has("frontend"),
        recommend: live && !collected.has("frontend"),
        reason: live
          ? `${consoleErrors.length} errors · ${consoleWarns.length} warnings`
          : "Console is quiet",
        signals: DEMO_CONSOLE.filter((l) => l.level !== "info").map(
          (l) => `${l.level}: ${l.message}`,
        ),
      };
    }
    if (def.id === "http") {
      const live = httpFail.length > 0;
      return {
        ...def,
        live,
        available_now: true,
        collected: collected.has("backend"),
        recommend: live && !collected.has("backend"),
        reason: live
          ? httpFail.map((h) => `${h.method} ${h.path} ${h.status}`).join(" · ")
          : "No failing requests",
        signals: httpFail.map((h) => `${h.method} ${h.path} ${h.status}`),
      };
    }
    if (def.id === "terminal") {
      return {
        ...def,
        live: true,
        available_now: true,
        collected: collected.has("backend"),
        recommend: !collected.has("backend"),
        reason: "pricing.ts:88 TypeError on tax · Stripe 402 in compose logs",
        signals: [
          "TypeError: Cannot read properties of undefined (reading 'tax')",
          "stripe.paymentIntents.create → 402 card_declined",
        ],
      };
    }
    if (def.id === "database") {
      const live = facts.stockZero > 0;
      return {
        ...def,
        live,
        available_now: true,
        collected: collected.has("database"),
        recommend: live && !collected.has("database"),
        reason: live
          ? `${facts.stockZero} SKU(s) at stock 0 still in the cart`
          : "Catalog looks consistent",
        signals: live ? ["LM-15 Graphite notebook stock 0, still in cart"] : [],
      };
    }
    if (def.id === "logs") {
      return {
        ...def,
        live: collected.size > 0,
        available_now: true,
        collected: collected.has("logs"),
        recommend: collected.size >= 2 && !collected.has("logs"),
        reason:
          collected.size > 0
            ? "Session has layer collections to correlate"
            : "Collect FE / BE / DB first, then the log stream",
        signals: [],
      };
    }
    if (def.id === "still") {
      return {
        ...def,
        live: jpegOn,
        available_now: jpegOn,
        collected: facts.captureCount > 0,
        recommend: jpegOn,
        reason: jpegOn
          ? facts.recording
            ? "Recording — snap at each failure frame"
            : "Studio live — stills are available now"
          : "Leave the studio tab open to enable stills",
        signals: jpegOn ? [`source ${facts.source}`] : [],
      };
    }
    return {
      ...def,
      live: facts.recording,
      available_now: jpegOn,
      collected: false,
      recommend: jpegOn && !facts.recording,
      reason: !jpegOn
        ? "Studio not attached — video medium offline"
        : facts.recording
          ? "Unbounded REC is on — snap until it settles, then record_stop"
          : "Checkout is a multi-step flow — record, don't just still",
      signals: facts.recording ? ["REC unbounded"] : [],
    };
  });

  const next: Array<{ tool: string; why: string }> = [];
  if (!facts.attached) {
    next.push({
      tool: "open_studio",
      why: "JPEG / WebM need this tab open. JSON hooks (FE / BE / DB) still work.",
    });
  } else if (!facts.recording) {
    next.push({
      tool: "vibecap_record_start",
      why: "Multi-step checkout — keep the camera on until the UI settles.",
    });
  } else {
    next.push({
      tool: "vibecap_snapshot",
      why: "Grab the failure frame. Does not stop REC.",
    });
  }
  for (const id of ["dom", "http", "database"] as const) {
    const h = hooks.find((x) => x.id === id);
    if (h?.recommend) next.push({ tool: h.tool, why: h.reason });
  }
  if (facts.recording) {
    next.push({
      tool: "vibecap_record_stop",
      why: "When results settle. JPEG poster + clip land in Media.",
    });
  }
  next.push({
    tool: "vibecap_bug_pack",
    why: "If you don't want to choose — one JSON of every live layer + stills.",
  });

  return {
    subject: {
      name: "Lumen Cart checkout",
      url: DEMO_FRONTEND_DOM.url,
      source: facts.source,
      attached: facts.attached,
      note: "JSON hooks bind to this instrumented subject. Screen / camera only change the visual medium (JPEG / WebM), not the DOM / console / HTTP / DB taps.",
    },
    signals,
    medium: {
      jpeg: {
        available: jpegOn,
        why: jpegOn ? "Shutter is live" : "Studio tab not attached",
      },
      webm: {
        available: jpegOn,
        why: jpegOn
          ? "MediaRecorder on the live stage — start/stop, no duration"
          : "Studio tab not attached",
      },
      json: {
        available: true,
        why: "Frontend, backend, database, logs are server-side and do not need the tab",
      },
    },
    hooks,
    next,
    rule: "Pixels → still / video. Console + DOM → ingest_frontend. HTTP + shell → ingest_backend. Rows → ingest_database. Unsure → bug_pack.",
  };
}
