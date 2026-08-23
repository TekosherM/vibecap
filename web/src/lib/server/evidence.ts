import { createServerFn } from "@tanstack/react-start";
import { z } from "zod";
import { getSql } from "@/lib/db";
import type {
  BudgetRow,
  CaptureRow,
  CatalogItem,
  CommandRow,
  EvidenceRow,
  InboxRow,
  LogRow,
  PackRow,
  SessionRow,
  StudioStatusRow,
} from "@/lib/types";
import {
  DEMO_CONSOLE,
  DEMO_FRONTEND_DOM,
  DEMO_HTTP,
  DEMO_TERMINAL,
} from "@/lib/demo-data";
import { evaluateHooks, type HookPlan } from "@/lib/hooks";

function nid() {
  return crypto.randomUUID();
}

async function ensureSeed(sql: Awaited<ReturnType<typeof getSql>>) {
  const existing = await sql<{ n: number }>`select count(*)::int as n from sessions`;
  if ((existing[0]?.n ?? 0) > 0) return;
  const id = "sess-lumen-402";
  await sql`
    insert into sessions (id, name, status, notes)
    values (
      ${id},
      ${"Lumen Cart checkout — 402"},
      ${"active"},
      ${"Repro: pay with any card. Total is wrong, tax helper throws, Stripe 402."}
    )
  `;
  const lines: Array<[string, string, string]> = [
    ["backend", "error", "TypeError: Cannot read properties of undefined (reading 'tax')"],
    ["backend", "warn", "stripe.paymentIntents.create → 402 card_declined"],
    ["frontend", "error", "Uncaught TypeError: Cannot read properties of undefined (reading 'tax')"],
    ["frontend", "info", "POST /api/checkout 402 Payment Required 184ms"],
    ["database", "info", "catalog_items.stock for LM-15 is 0 but item remains in cart"],
    ["system", "info", "Session opened. Demo subject attached: checkout.lumen.test"],
  ];
  for (const [stream, level, message] of lines) {
    await sql`
      insert into logs (session_id, stream, level, message)
      values (${id}, ${stream}, ${level}, ${message})
    `;
  }
  await sql`
    insert into inbox (
      id, session_id, question, options, priority, agent_label, preferred, context, status
    ) values (
      ${"fb-total"},
      ${id},
      ${"The checkout total reads $41.00 but the line items sum to $45.00. Is the tote being dropped when tax throws, or is this a display-only bug?"},
      ${JSON.stringify(["Display-only", "Tote dropped in computeTotal", "Need a screenshot first"])},
      ${"high"},
      ${"codex"},
      ${"choice"},
      ${"pricing.ts:88 computeTotal — tax helper undefined. Notebook (LM-15) has stock 0."},
      ${"pending"}
    )
  `;
}

export const listSessions = createServerFn({ method: "GET" }).handler(async () => {
  const sql = await getSql();
  await ensureSeed(sql);
  return sql<SessionRow>`select * from sessions order by created_at desc`;
});

export const createSession = createServerFn({ method: "POST" })
  .validator(z.object({ name: z.string().min(1).max(120) }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    const id = nid();
    const rows = await sql<SessionRow>`
      insert into sessions (id, name) values (${id}, ${data.name})
      returning *
    `;
    await sql`
      insert into logs (session_id, stream, level, message)
      values (${id}, ${"system"}, ${"info"}, ${"Session created"})
    `;
    return rows[0];
  });

export const getSessionBundle = createServerFn({ method: "GET" })
  .validator(z.object({ id: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    await ensureSeed(sql);
    const sessions = await sql<SessionRow>`select * from sessions where id = ${data.id}`;
    const session = sessions[0] ?? null;
    const captures = await sql<CaptureRow>`
      select * from captures where session_id = ${data.id} order by created_at desc
    `;
    const evidence = await sql<EvidenceRow>`
      select * from evidence where session_id = ${data.id} order by created_at desc
    `;
    const packs = await sql<PackRow>`
      select * from packs where session_id = ${data.id} order by created_at desc
    `;
    const logs = await sql<LogRow>`
      select * from logs where session_id = ${data.id} order by created_at desc limit 80
    `;
    return { session, captures, evidence, packs, logs };
  });

export const saveCapture = createServerFn({ method: "POST" })
  .validator(
    z.object({
      sessionId: z.string(),
      kind: z.string(),
      label: z.string().default(""),
      mime: z.string().default("image/jpeg"),
      dataUrl: z.string().max(2_500_000).nullable().optional(),
      durationMs: z.number().optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const id = nid();
    const rows = await sql<CaptureRow>`
      insert into captures (id, session_id, kind, label, mime, data_url, duration_ms)
      values (
        ${id},
        ${data.sessionId},
        ${data.kind},
        ${data.label},
        ${data.mime},
        ${data.dataUrl ?? null},
        ${data.durationMs ?? null}
      )
      returning *
    `;
    await sql`
      update budget set
        frames_used = frames_used + 1,
        mb_used = mb_used + ${(data.dataUrl?.length ?? 0) / (1024 * 1024)}
      where id = ${"global"}
    `;
    return rows[0];
  });

export const addEvidence = createServerFn({ method: "POST" })
  .validator(
    z.object({
      sessionId: z.string(),
      source: z.string(),
      kind: z.string(),
      title: z.string(),
      body: z.string().max(200_000),
      captureId: z.string().nullable().optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const id = nid();
    const rows = await sql<EvidenceRow>`
      insert into evidence (id, session_id, source, kind, title, body, capture_id)
      values (
        ${id},
        ${data.sessionId},
        ${data.source},
        ${data.kind},
        ${data.title},
        ${data.body},
        ${data.captureId ?? null}
      )
      returning *
    `;
    return rows[0];
  });

export const addLog = createServerFn({ method: "POST" })
  .validator(
    z.object({
      sessionId: z.string().nullable().optional(),
      stream: z.string(),
      level: z.string().default("info"),
      message: z.string(),
      meta: z.string().optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const rows = await sql<LogRow>`
      insert into logs (session_id, stream, level, message, meta)
      values (
        ${data.sessionId ?? null},
        ${data.stream},
        ${data.level},
        ${data.message},
        ${data.meta ?? ""}
      )
      returning *
    `;
    return rows[0];
  });

export const subjectPay = createServerFn({ method: "POST" })
  .validator(z.object({ sessionId: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    await ensureSeed(sql);
    const items = await sql<CatalogItem>`
      select id, sku, name, price_cents, stock from catalog_items order by id
    `;
    const itemsSum = items.reduce((n, i) => n + i.price_cents, 0);
    const payload = {
      ok: false,
      status: 402,
      error: "card_declined",
      code: "generic_decline",
      ui_total_cents: 4100,
      items_sum_cents: itemsSum,
      tax: null as number | null,
      stock_zero: items.filter((i) => i.stock === 0).map((i) => i.sku),
      at: new Date().toISOString(),
    };
    const lines: Array<[string, string, string, string]> = [
      ["frontend", "error", "Uncaught TypeError: Cannot read properties of undefined (reading 'tax')", "checkout.js:214"],
      ["backend", "error", "GET /api/tax?zip=94107 500 tax is undefined", "pricing.ts:88"],
      ["backend", "error", "POST /api/checkout 402 card_declined", "routes/checkout.ts:41"],
      ["frontend", "error", "PaymentIntent failed: card_declined (generic_decline)", "stripe.js"],
      ["database", "warn", "LM-15 Graphite notebook stock 0 still in cart", "catalog_items"],
    ];
    for (const [stream, level, message, meta] of lines) {
      await sql`
        insert into logs (session_id, stream, level, message, meta)
        values (${data.sessionId}, ${stream}, ${level}, ${message}, ${meta})
      `;
    }
    return payload;
  });

export const ingestFrontend = createServerFn({ method: "POST" })
  .validator(
    z.object({
      sessionId: z.string(),
      captureId: z.string().nullable().optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const liveConsole = await sql<LogRow>`
      select * from logs
      where session_id = ${data.sessionId} and stream = ${"frontend"}
      order by created_at desc
      limit 24
    `;
    const body = JSON.stringify(
      {
        console: DEMO_CONSOLE,
        live_console: liveConsole.map((l) => ({
          level: l.level,
          message: l.message,
          meta: l.meta,
          at: l.created_at,
        })),
        dom: DEMO_FRONTEND_DOM,
        collectedAt: new Date().toISOString(),
      },
      null,
      2,
    );
    const id = nid();
    const rows = await sql<EvidenceRow>`
      insert into evidence (id, session_id, source, kind, title, body, capture_id)
      values (
        ${id},
        ${data.sessionId},
        ${"frontend"},
        ${"bundle"},
        ${"Frontend — DOM, console, network"},
        ${body},
        ${data.captureId ?? null}
      )
      returning *
    `;
    await sql`
      insert into logs (session_id, stream, level, message)
      values (${data.sessionId}, ${"frontend"}, ${"info"}, ${"Frontend evidence collected"})
    `;
    return {
      ...rows[0],
      hooked: ["dom", "console"],
      signals: {
        visual_issues: DEMO_FRONTEND_DOM.issues.length,
        console_errors: DEMO_CONSOLE.filter((l) => l.level === "error").length,
        console_warns: DEMO_CONSOLE.filter((l) => l.level === "warn").length,
      },
    };
  });

export const ingestBackend = createServerFn({ method: "POST" })
  .validator(z.object({ sessionId: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    const liveHttp = await sql<LogRow>`
      select * from logs
      where session_id = ${data.sessionId} and stream = ${"backend"}
      order by created_at desc
      limit 24
    `;
    const body = JSON.stringify(
      {
        terminal: DEMO_TERMINAL,
        http: DEMO_HTTP,
        live_http: liveHttp.map((l) => ({
          level: l.level,
          message: l.message,
          meta: l.meta,
          at: l.created_at,
        })),
        runtime: "node 22 / compose checkout-1",
        cloud: { provider: "neon+compose", region: "preview", status: "degraded" },
        collectedAt: new Date().toISOString(),
      },
      null,
      2,
    );
    const id = nid();
    const rows = await sql<EvidenceRow>`
      insert into evidence (id, session_id, source, kind, title, body)
      values (
        ${id},
        ${data.sessionId},
        ${"backend"},
        ${"bundle"},
        ${"Backend — terminal, HTTP, cloud"},
        ${body}
      )
      returning *
    `;
    await sql`
      insert into logs (session_id, stream, level, message)
      values (${data.sessionId}, ${"backend"}, ${"info"}, ${"Backend evidence collected"})
    `;
    return {
      ...rows[0],
      hooked: ["http", "terminal"],
      signals: {
        http_fail: DEMO_HTTP.filter((h) => h.status >= 400).length,
        terminal: "compose checkout-1",
      },
    };
  });

export const ingestDatabase = createServerFn({ method: "POST" })
  .validator(z.object({ sessionId: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    const tables = await sql<{ table_name: string }>`
      select table_name
      from information_schema.tables
      where table_schema = 'public'
      order by table_name
    `;
    const items = await sql<CatalogItem>`
      select id, sku, name, price_cents, stock from catalog_items order by id
    `;
    const body = JSON.stringify(
      {
        engine: "Postgres (Neon or PGLite)",
        tables: tables.map((t) => t.table_name),
        catalog_items: items,
        notes: [
          "LM-15 Graphite notebook stock is 0 but still in the demo cart.",
          "Line items 1200+1800+1500 = 4500 cents; UI shows 4100.",
        ],
        collectedAt: new Date().toISOString(),
      },
      null,
      2,
    );
    const id = nid();
    const rows = await sql<EvidenceRow>`
      insert into evidence (id, session_id, source, kind, title, body)
      values (
        ${id},
        ${data.sessionId},
        ${"database"},
        ${"snapshot"},
        ${"Database — schema + catalog_items"},
        ${body}
      )
      returning *
    `;
    await sql`
      insert into logs (session_id, stream, level, message)
      values (${data.sessionId}, ${"database"}, ${"info"}, ${"Database snapshot captured"})
    `;
    return {
      evidence: rows[0],
      items,
      tables: tables.map((t) => t.table_name),
      hooked: ["database"],
      signals: {
        stock_zero: items.filter((i) => i.stock === 0).map((i) => i.sku),
      },
    };
  });

export const ingestLogs = createServerFn({ method: "POST" })
  .validator(z.object({ sessionId: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    const logs = await sql<LogRow>`
      select * from logs where session_id = ${data.sessionId} order by created_at desc limit 100
    `;
    const body = JSON.stringify({ count: logs.length, logs }, null, 2);
    const id = nid();
    const rows = await sql<EvidenceRow>`
      insert into evidence (id, session_id, source, kind, title, body)
      values (
        ${id},
        ${data.sessionId},
        ${"logs"},
        ${"stream"},
        ${"Logs — session stream"},
        ${body}
      )
      returning *
    `;
    return { evidence: rows[0], logs };
  });

export const buildPack = createServerFn({ method: "POST" })
  .validator(
    z.object({
      sessionId: z.string(),
      title: z.string().optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const sessions = await sql<SessionRow>`select * from sessions where id = ${data.sessionId}`;
    const session = sessions[0];
    if (!session) throw new Error("Session not found");
    const captures = await sql<CaptureRow>`
      select id, kind, label, mime, duration_ms, created_at,
             case when data_url is null then null else left(data_url, 80) end as data_url
      from captures where session_id = ${data.sessionId} order by created_at desc
    `;
    const evidence = await sql<EvidenceRow>`
      select * from evidence where session_id = ${data.sessionId} order by created_at desc
    `;
    const logs = await sql<LogRow>`
      select * from logs where session_id = ${data.sessionId} order by created_at desc limit 80
    `;
    const stills = await sql<CaptureRow>`
      select * from captures
      where session_id = ${data.sessionId} and kind in ('still','snapshot')
      order by created_at desc limit 12
    `;
    const bySource = {
      frontend: evidence.filter((e) => e.source === "frontend"),
      backend: evidence.filter((e) => e.source === "backend"),
      database: evidence.filter((e) => e.source === "database"),
      logs: evidence.filter((e) => e.source === "logs"),
      capture: evidence.filter((e) => e.source === "capture"),
    };
    const summary = [
      `${stills.length} stills`,
      `${bySource.frontend.length} frontend`,
      `${bySource.backend.length} backend`,
      `${bySource.database.length} database`,
      `${bySource.logs.length} log bundles`,
      `${logs.length} raw log lines`,
    ].join(" · ");
    const payload = JSON.stringify(
      {
        session,
        summary,
        captures: captures.map((c) => ({
          id: c.id,
          kind: c.kind,
          label: c.label,
          created_at: c.created_at,
        })),
        stills: stills.map((s) => ({
          id: s.id,
          label: s.label,
          kind: s.kind,
          data_url: s.data_url,
        })),
        evidence: bySource,
        logs,
        output: {
          kind: "session-pack",
          location: "Pack stage → Download JSON / Download stills",
          not: ["~/Movies/Vibecap", "~/Vibecap", "agent workspace copies"],
          files: {
            stills: stills.map((s) => `/api/agent/still/${s.id}.jpg`),
            frontend: "evidence/frontend.json",
            backend: "evidence/backend.json",
            database: "evidence/database.json",
            logs: "evidence/logs.json",
          },
        },
        generatedAt: new Date().toISOString(),
      },
      null,
      2,
    );
    const id = nid();
    const title = data.title ?? `Bug pack — ${session.name}`;
    const rows = await sql<PackRow>`
      insert into packs (id, session_id, title, summary, payload)
      values (${id}, ${data.sessionId}, ${title}, ${summary}, ${payload})
      returning *
    `;
    await sql`update sessions set status = ${"packed"} where id = ${data.sessionId}`;
    return rows[0];
  });

export const listInbox = createServerFn({ method: "GET" }).handler(async () => {
  const sql = await getSql();
  await ensureSeed(sql);
  return sql<InboxRow>`select * from inbox order by created_at desc limit 50`;
});

export const requestFeedback = createServerFn({ method: "POST" })
  .validator(
    z.object({
      sessionId: z.string().optional(),
      question: z.string().min(1),
      options: z.array(z.string()).max(8).optional(),
      priority: z.string().optional(),
      agentLabel: z.string().optional(),
      preferred: z.string().optional(),
      context: z.string().optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const id = nid();
    const rows = await sql<InboxRow>`
      insert into inbox (
        id, session_id, question, options, priority, agent_label, preferred, context
      ) values (
        ${id},
        ${data.sessionId ?? null},
        ${data.question},
        ${JSON.stringify(data.options ?? [])},
        ${data.priority ?? "normal"},
        ${data.agentLabel ?? "agent"},
        ${data.preferred ?? "any"},
        ${data.context ?? ""}
      )
      returning *
    `;
    return rows[0];
  });

export const answerFeedback = createServerFn({ method: "POST" })
  .validator(
    z.object({
      id: z.string(),
      answerText: z.string().optional(),
      answerChoice: z.string().optional(),
      status: z.enum(["answered", "dismissed", "cancelled"]).default("answered"),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const rows = await sql<InboxRow>`
      update inbox set
        status = ${data.status},
        answer_text = ${data.answerText ?? null},
        answer_choice = ${data.answerChoice ?? null}
      where id = ${data.id}
      returning *
    `;
    return rows[0];
  });

export const getFeedback = createServerFn({ method: "GET" })
  .validator(z.object({ id: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    const rows = await sql<InboxRow>`select * from inbox where id = ${data.id}`;
    return rows[0] ?? null;
  });

export const cancelFeedback = createServerFn({ method: "POST" })
  .validator(z.object({ id: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    const rows = await sql<InboxRow>`
      update inbox set status = ${"cancelled"}
      where id = ${data.id} and status = ${"pending"}
      returning *
    `;
    return rows[0] ?? null;
  });

export const getBudget = createServerFn({ method: "GET" }).handler(async () => {
  const sql = await getSql();
  const rows = await sql<BudgetRow>`select * from budget where id = ${"global"}`;
  return rows[0];
});

export const setBudget = createServerFn({ method: "POST" })
  .validator(
    z.object({
      max_frames: z.number().optional(),
      max_mb: z.number().optional(),
      max_minutes: z.number().optional(),
      analysis_tier: z.string().optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const current = (await sql<BudgetRow>`select * from budget where id = ${"global"}`)[0];
    const rows = await sql<BudgetRow>`
      update budget set
        max_frames = ${data.max_frames ?? current.max_frames},
        max_mb = ${data.max_mb ?? current.max_mb},
        max_minutes = ${data.max_minutes ?? current.max_minutes},
        analysis_tier = ${data.analysis_tier ?? current.analysis_tier}
      where id = ${"global"}
      returning *
    `;
    return rows[0];
  });

export const enqueueCommand = createServerFn({ method: "POST" })
  .validator(
    z.object({
      tool: z.string(),
      args: z.record(z.string(), z.unknown()).optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const id = nid();
    const rows = await sql<CommandRow>`
      insert into commands (id, tool, args, status)
      values (${id}, ${data.tool}, ${JSON.stringify(data.args ?? {})}, ${"pending"})
      returning *
    `;
    return rows[0];
  });

export const pullCommands = createServerFn({ method: "GET" }).handler(async () => {
  const sql = await getSql();
  const pending = await sql<CommandRow>`
    select * from commands where status = ${"pending"} order by created_at asc limit 8
  `;
  const claimed: CommandRow[] = [];
  for (const row of pending) {
    const updated = await sql<CommandRow>`
      update commands set status = ${"running"}
      where id = ${row.id} and status = ${"pending"}
      returning *
    `;
    if (updated[0]) claimed.push(updated[0]);
  }
  return claimed;
});

export const completeCommand = createServerFn({ method: "POST" })
  .validator(
    z.object({
      id: z.string(),
      status: z.enum(["done", "error"]),
      result: z.unknown(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    const rows = await sql<CommandRow>`
      update commands set
        status = ${data.status},
        result = ${JSON.stringify(data.result)}
      where id = ${data.id}
      returning *
    `;
    return rows[0];
  });

export const getCommand = createServerFn({ method: "GET" })
  .validator(z.object({ id: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    const rows = await sql<CommandRow>`select * from commands where id = ${data.id}`;
    return rows[0] ?? null;
  });

export const listCatalog = createServerFn({ method: "GET" }).handler(async () => {
  const sql = await getSql();
  return sql<CatalogItem>`select id, sku, name, price_cents, stock from catalog_items order by id`;
});

export const listRecentPacks = createServerFn({ method: "GET" }).handler(async () => {
  const sql = await getSql();
  return sql<PackRow>`select * from packs order by created_at desc limit 12`;
});

async function ensureStudio(sql: Awaited<ReturnType<typeof getSql>>) {
  await sql`
    create table if not exists studio_status (
      id text primary key default 'global',
      recording boolean not null default false,
      inspecting boolean not null default false,
      source text not null default 'idle',
      attached_at timestamptz not null default now()
    )
  `;
  await sql`
    insert into studio_status (id) values (${"global"})
    on conflict (id) do nothing
  `;
}

export const touchStudio = createServerFn({ method: "POST" })
  .validator(
    z.object({
      recording: z.boolean(),
      source: z.string(),
      inspecting: z.boolean().optional(),
    }),
  )
  .handler(async ({ data }) => {
    const sql = await getSql();
    await ensureStudio(sql);
    await sql`
      insert into studio_status (id, recording, source, inspecting, attached_at)
      values (
        ${"global"},
        ${data.recording},
        ${data.source},
        ${data.inspecting ?? false},
        now()
      )
      on conflict (id) do update set
        recording = excluded.recording,
        source = excluded.source,
        inspecting = excluded.inspecting,
        attached_at = now()
    `;
    return { ok: true };
  });

export const getStudioStatus = createServerFn({ method: "GET" }).handler(async () => {
  const sql = await getSql();
  await ensureStudio(sql);
  const rows = await sql<StudioStatusRow>`select * from studio_status where id = ${"global"}`;
  const row = rows[0];
  if (!row) {
    return { attached: false, recording: false, inspecting: false, source: "idle", lag_ms: null };
  }
  const lag = Date.now() - new Date(row.attached_at).getTime();
  return {
    attached: lag < 8000,
    recording: row.recording,
    inspecting: row.inspecting,
    source: row.source,
    attached_at: row.attached_at,
    lag_ms: lag,
  };
});

export const getHookPlan = createServerFn({ method: "GET" })
  .validator(z.object({ sessionId: z.string().optional() }))
  .handler(async ({ data }): Promise<HookPlan> => {
    const sql = await getSql();
    await ensureSeed(sql);
    await ensureStudio(sql);
    const studioRows = await sql<StudioStatusRow>`select * from studio_status where id = ${"global"}`;
    const studio = studioRows[0];
    const lag = studio ? Date.now() - new Date(studio.attached_at).getTime() : null;
    const attached = lag !== null && lag < 8000;
    const sessions = await sql<SessionRow>`select * from sessions order by created_at desc`;
    const sessionId = data.sessionId ?? sessions[0]?.id ?? null;
    let collected: string[] = [];
    let captureCount = 0;
    if (sessionId) {
      const ev = await sql<{ source: string }>`
        select distinct source from evidence where session_id = ${sessionId}
      `;
      collected = ev.map((e) => e.source);
      const cc = await sql<{ n: number }>`
        select count(*)::int as n from captures where session_id = ${sessionId}
      `;
      captureCount = cc[0]?.n ?? 0;
    }
    const items = await sql<{ sku: string; stock: number }>`select sku, stock from catalog_items`;
    const stockZero = items.filter((i) => i.stock === 0).length;
    return evaluateHooks({
      attached,
      recording: studio?.recording ?? false,
      inspecting: studio?.inspecting ?? false,
      source: studio?.source ?? "idle",
      collected,
      captureCount,
      stockZero,
    });
  });

export const getCapture = createServerFn({ method: "GET" })
  .validator(z.object({ id: z.string() }))
  .handler(async ({ data }) => {
    const sql = await getSql();
    const rows = await sql<CaptureRow>`select * from captures where id = ${data.id}`;
    return rows[0] ?? null;
  });

export const listMedia = createServerFn({ method: "GET" }).handler(async () => {
  const sql = await getSql();
  return sql<CaptureRow>`
    select * from captures order by created_at desc limit 24
  `;
});
