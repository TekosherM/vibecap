import { chromium } from "playwright";

const url = "http://127.0.0.1:8080/";
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
const errors = [];
page.on("pageerror", (e) => errors.push(String(e)));
page.on("console", (m) => {
  if (m.type() === "error") errors.push(m.text());
});
await page.goto(url, { waitUntil: "domcontentloaded" });
await page.waitForTimeout(1800);

const hooks = await page.evaluate(async () => {
  const r = await fetch("/api/agent/hooks");
  return r.json();
});

async function call(tool, args = {}) {
  const pending = await page.evaluate(async (body) => {
    const r = await fetch("/api/agent/call", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    return r.json();
  }, { tool, args });
  if (pending.status === "done") {
    return { pending, result: pending, parsed: pending.result };
  }
  let result = pending;
  for (let i = 0; i < 25; i++) {
    await page.waitForTimeout(400);
    result = await page.evaluate(async (id) => {
      const r = await fetch("/api/agent/result/" + id);
      return r.json();
    }, pending.id);
    if (result.status === "done" || result.status === "error") break;
  }
  const parsed = typeof result.result === "string" ? JSON.parse(result.result) : result.result;
  return { pending, result, parsed };
}

const job = await call("vibecap_job");
const start = { pending: { status: "skipped" }, result: { status: "done" }, parsed: null };
const walk = job;
const snap = { pending: { status: "skipped" }, result: { status: "done" }, parsed: (job.parsed?.stills ?? [])[2] };
const parsed = snap.parsed;
const stop = {
  parsed: {
    captureId: job.parsed?.captureId,
    duration_ms: job.parsed?.duration_ms,
    data_url: parsed?.data_url,
    clip: job.parsed?.clip,
  },
};

let jpeg = { ok: false, type: "", bytes: 0, status: 0 };
if (parsed?.path) {
  jpeg = await page.evaluate(async (path) => {
    const r = await fetch(path);
    const buf = await r.arrayBuffer();
    return { ok: r.ok, type: r.headers.get("content-type"), bytes: buf.byteLength, status: r.status };
  }, parsed.path);
}

const clipPath = job.parsed?.clip_path ?? job.result?.result?.clip_path;
let clipFile = { ok: false, type: "", bytes: 0, status: 0 };
if (clipPath) {
  clipFile = await page.evaluate(async (path) => {
    const r = await fetch(path);
    const buf = await r.arrayBuffer();
    return { ok: r.ok, type: r.headers.get("content-type"), bytes: buf.byteLength, status: r.status };
  }, clipPath);
  await page.reload({ waitUntil: "load", timeout: 20000 });
  await page.waitForTimeout(800);
  clipFile = {
    ...clipFile,
    afterReload: await page.evaluate(async (path) => {
      const r = await fetch(path);
      const buf = await r.arrayBuffer();
      return { ok: r.ok, type: r.headers.get("content-type"), bytes: buf.byteLength, status: r.status };
    }, clipPath),
  };
}

await page.screenshot({ path: "/workspace/screenshots/qa-pack.png", fullPage: false });

await page.getByRole("button", { name: "Sources" }).first().click();
await page.waitForTimeout(400);
await page.screenshot({ path: "/workspace/screenshots/qa-sources.png", fullPage: false });

await page.getByRole("button", { name: "Agent" }).first().click();
await page.waitForTimeout(400);
await page.screenshot({ path: "/workspace/screenshots/qa-agent.png", fullPage: false });

await page.getByRole("button", { name: "Media" }).first().click();
await page.waitForTimeout(400);
await page.screenshot({ path: "/workspace/screenshots/qa-media.png", fullPage: false });

const media = await page.evaluate(async () => {
  const r = await fetch("/api/agent/media");
  return r.json();
});
const subject = await page.evaluate(async () => {
  const cart = await fetch("/api/cart");
  const tax = await fetch("/api/tax");
  return {
    cart: { status: cart.status, body: await cart.json() },
    tax: { status: tax.status, body: await tax.json() },
    coupon: await fetch("/api/coupon", { method: "POST" }).then(async (r) => ({
      status: r.status,
      body: await r.json(),
    })),
  };
});
const status = await page.evaluate(async () => {
  const r = await fetch("/api/agent/status");
  return r.json();
});

console.log(JSON.stringify({
  errors,
  hooks: {
    jpeg: hooks.medium?.jpeg?.available,
    webm: hooks.medium?.webm?.available,
    json: hooks.medium?.json?.available,
    console_errors: hooks.signals?.console_errors,
    http_fail: hooks.signals?.http_fail,
    stock_zero: hooks.signals?.stock_zero,
    firing: (hooks.hooks ?? []).filter((h) => h.live).map((h) => h.id),
    next: (hooks.next ?? []).map((n) => n.tool),
    rule: hooks.rule,
  },
  job: {
    status: job.result?.status,
    packId: job.parsed?.packId ?? job.result?.result?.packId,
    summary: job.parsed?.summary ?? job.result?.result?.summary,
    stills: (job.parsed?.stills ?? job.result?.result?.stills ?? []).length,
    coupon: job.parsed?.coupon?.error ?? job.result?.result?.coupon?.error,
    tax: job.parsed?.tax?.status ?? job.result?.result?.tax?.status,
    pay: job.parsed?.pay?.error ?? job.result?.result?.pay?.error,
    duration_ms: job.parsed?.duration_ms ?? job.result?.result?.duration_ms,
    clip_path: job.parsed?.clip_path ?? job.result?.result?.clip_path,
  },
  walk: {
    status: walk.result?.status,
    coupon: walk.parsed?.coupon?.error ?? walk.result?.result?.coupon?.error,
    tax: walk.parsed?.tax?.status ?? walk.result?.result?.tax?.status,
    pay: walk.parsed?.pay?.error ?? walk.result?.result?.pay?.error,
    stills: (walk.parsed?.stills ?? walk.result?.result?.stills ?? []).length,
  },
  snap: {
    status: snap.pending?.status,
    resultStatus: snap.result?.status,
    ok: parsed?.ok,
    recording: parsed?.recording,
    hasJpeg: typeof parsed?.data_url === "string" && parsed.data_url.startsWith("data:image/jpeg"),
    jpegChars: (parsed?.data_url || "").length,
    path: parsed?.path,
  },
  jpegFile: jpeg,
  clipFile,
  stop: {
    captureId: stop.parsed?.captureId,
    duration_ms: stop.parsed?.duration_ms,
    hasPoster: typeof stop.parsed?.data_url === "string" && stop.parsed.data_url.startsWith("data:image/jpeg"),
    clip: stop.parsed?.clip,
  },
  media: {
    count: Array.isArray(media) ? media.length : 0,
    files: Array.isArray(media) ? media.filter((m) => typeof m.file === "string").length : 0,
    downloadButtons: await page.getByRole("button", { name: /Download (JPEG|clip)/ }).count(),
  },
  subject,
  studio: status.studio,
}, null, 2));
await browser.close();
