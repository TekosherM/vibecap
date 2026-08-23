import { createFileRoute } from "@tanstack/react-router";
import { AGENT_HELP, AGENT_TOOLS, CAPTURE_ONLY_TOOLS } from "@/lib/types";
import {
  buildPack,
  cancelFeedback,
  enqueueCommand,
  getBudget,
  getCapture,
  getCommand,
  getFeedback,
  getHookPlan,
  getStudioStatus,
  ingestBackend,
  ingestDatabase,
  ingestFrontend,
  ingestLogs,
  listInbox,
  listMedia,
  listSessions,
  requestFeedback,
  setBudget,
} from "@/lib/server/evidence";

async function defaultSessionId() {
  const sessions = await listSessions();
  return sessions[0]?.id ?? null;
}

function splatFrom(params: { _splat?: string }) {
  return params._splat ?? "";
}

function dataUrlToFile(id: string, dataUrl: string, fallbackName: string) {
  const match = /^data:([^,]*?);base64,(.+)$/s.exec(dataUrl);
  if (!match) return null;
  const body = Buffer.from(match[2], "base64");
  const short = id.slice(0, 8);
  const mime = match[1] || "application/octet-stream";
  const ext = mime.includes("webm") ? "webm" : mime.includes("jpeg") || mime.includes("jpg") ? "jpg" : fallbackName;
  return new Response(body, {
    headers: {
      "content-type": mime,
      "content-disposition": `attachment; filename="vibecap-${short}.${ext}"`,
      "cache-control": "no-store",
    },
  });
}

export const Route = createFileRoute("/api/agent/$")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const splat = splatFrom(params);
        if (splat === "help") {
          return new Response(AGENT_HELP, {
            headers: { "content-type": "text/plain; charset=utf-8" },
          });
        }
        if (splat === "hooks") {
          const plan = await getHookPlan({ data: {} });
          return Response.json(plan);
        }
        if (splat === "tools" || splat === "status" || splat === "") {
          const [studio, plan] = await Promise.all([
            getStudioStatus(),
            getHookPlan({ data: {} }),
          ]);
          return Response.json({
            ok: true,
            name: "vibecap",
            transport: "http",
            connected: true,
            studio,
            plan,
            capture_only: CAPTURE_ONLY_TOOLS,
            output: {
              stills: "inline data_url + GET /api/agent/still/{id}.jpg",
              packs: "POST vibecap_job or vibecap_bug_pack → Pack Download JSON / stills / clip",
              clips: "GET /api/agent/clip/{id}.webm (persisted). Not a filesystem path.",
              not: ["~/Movies/Vibecap", "~/Vibecap"],
            },
            help: "GET /api/agent/help",
            hooks: "GET /api/agent/hooks",
            tools: AGENT_TOOLS,
          });
        }
        if (splat.startsWith("result/")) {
          const id = splat.slice("result/".length);
          const row = await getCommand({ data: { id } });
          if (!row) return Response.json({ error: "not found" }, { status: 404 });
          return Response.json(row);
        }
        if (splat.startsWith("feedback/")) {
          const id = splat.slice("feedback/".length);
          const row = await getFeedback({ data: { id } });
          if (!row) return Response.json({ error: "not found" }, { status: 404 });
          return Response.json(row);
        }
        if (splat.startsWith("clip/")) {
          const raw = splat.slice("clip/".length);
          const id = raw.replace(/\.webm$/i, "");
          const row = await getCapture({ data: { id } });
          if (!row) return Response.json({ error: "not found" }, { status: 404 });
          if (!row.clip_url) return Response.json({ error: "no clip" }, { status: 404 });
          const file = dataUrlToFile(id, row.clip_url, "webm");
          if (!file) return Response.json({ error: "bad clip" }, { status: 500 });
          return file;
        }
        if (splat.startsWith("still/") || splat.startsWith("media/")) {
          const raw = splat.replace(/^(still|media)\//, "");
          const wantFile = /\.(jpg|jpeg)$/i.test(raw);
          const id = raw.replace(/\.(jpg|jpeg)$/i, "");
          const row = await getCapture({ data: { id } });
          if (!row) return Response.json({ error: "not found" }, { status: 404 });
          if (wantFile) {
            if (!row.data_url) return Response.json({ error: "no still" }, { status: 404 });
            const file = dataUrlToFile(id, row.data_url, "jpg");
            if (!file) return Response.json({ error: "bad still" }, { status: 500 });
            return file;
          }
          return Response.json({
            ...row,
            file: row.data_url ? `/api/agent/still/${row.id}.jpg` : null,
            clip: row.clip_url ? `/api/agent/clip/${row.id}.webm` : null,
          });
        }
        if (splat === "media") {
          const rows = await listMedia();
          return Response.json(
            rows.map((c) => ({
              ...c,
              file: c.data_url ? `/api/agent/still/${c.id}.jpg` : null,
              clip: c.clip_url ? `/api/agent/clip/${c.id}.webm` : null,
            })),
          );
        }
        if (splat === "spending") {
          return Response.json(await getBudget());
        }
        if (splat === "inbox") {
          return Response.json(await listInbox());
        }
        return Response.json({ error: "unknown route", help: "GET /api/agent/help" }, { status: 404 });
      },
      POST: async ({ params, request }) => {
        const splat = splatFrom(params);
        if (splat !== "call") {
          return Response.json({ error: "POST /api/agent/call" }, { status: 404 });
        }
        const body = (await request.json()) as {
          tool?: string;
          args?: Record<string, unknown>;
          sessionId?: string;
        };
        const tool = body.tool;
        if (!tool) return Response.json({ error: "tool required" }, { status: 400 });
        const sessionId = body.sessionId ?? (await defaultSessionId());
        const args: Record<string, unknown> = { ...(body.args ?? {}), sessionId };

        const serverTools: Record<string, () => Promise<unknown>> = {
          vibecap_hooks: async () => getHookPlan({ data: { sessionId: sessionId ?? undefined } }),
          vibecap_ingest_frontend: async () => {
            if (!sessionId) throw new Error("no session");
            return ingestFrontend({ data: { sessionId } });
          },
          vibecap_ingest_backend: async () => {
            if (!sessionId) throw new Error("no session");
            return ingestBackend({ data: { sessionId } });
          },
          vibecap_ingest_database: async () => {
            if (!sessionId) throw new Error("no session");
            return ingestDatabase({ data: { sessionId } });
          },
          vibecap_ingest_logs: async () => {
            if (!sessionId) throw new Error("no session");
            return ingestLogs({ data: { sessionId } });
          },
          vibecap_bug_pack: async () => {
            if (!sessionId) throw new Error("no session");
            await ingestFrontend({ data: { sessionId } });
            await ingestBackend({ data: { sessionId } });
            await ingestDatabase({ data: { sessionId } });
            await ingestLogs({ data: { sessionId } });
            const pack = await buildPack({ data: { sessionId } });
            const snap = await enqueueCommand({
              data: { tool: "vibecap_snapshot", args: { sessionId } },
            });
            return {
              packId: pack.id,
              summary: pack.summary,
              snapshotCommandId: snap.id,
              output: "Pack stage · Download JSON. Stills are inline. Not ~/Movies/Vibecap.",
            };
          },
          vibecap_get_spending: async () => getBudget(),
          vibecap_set_budget: async () =>
            setBudget({
              data: {
                analysis_tier: args.analysis_tier ? String(args.analysis_tier) : undefined,
                max_frames: typeof args.max_frames === "number" ? args.max_frames : undefined,
                max_mb: typeof args.max_mb === "number" ? args.max_mb : undefined,
                max_minutes: typeof args.max_minutes === "number" ? args.max_minutes : undefined,
              },
            }),
          vibecap_list_feedback: async () => listInbox(),
          vibecap_get_feedback: async () => {
            const id = args.request_id ? String(args.request_id) : args.id ? String(args.id) : "";
            if (!id) return listInbox();
            return getFeedback({ data: { id } });
          },
          vibecap_request_feedback: async () =>
            requestFeedback({
              data: {
                sessionId: sessionId ?? undefined,
                question: String(args.question ?? "Need a look?"),
                options: Array.isArray(args.options) ? (args.options as string[]) : undefined,
                agentLabel: args.agent_label ? String(args.agent_label) : undefined,
                priority: args.priority ? String(args.priority) : undefined,
                preferred: args.preferred_reply ? String(args.preferred_reply) : undefined,
                context: args.context ? String(args.context) : undefined,
              },
            }),
          vibecap_cancel_feedback: async () => {
            const id = args.request_id ? String(args.request_id) : String(args.id ?? "");
            if (!id) throw new Error("request_id required");
            return cancelFeedback({ data: { id } });
          },
        };

        if (serverTools[tool]) {
          try {
            const result = await serverTools[tool]();
            return Response.json({ status: "done", tool, result });
          } catch (err) {
            return Response.json(
              { status: "error", error: err instanceof Error ? err.message : "failed" },
              { status: 400 },
            );
          }
        }

        const studio = await getStudioStatus();
        const cmd = await enqueueCommand({
          data: { tool, args },
        });
        return Response.json({
          status: "pending",
          id: cmd.id,
          studio_attached: studio.attached,
          hint: studio.attached
            ? "Studio is live. Poll GET /api/agent/result/" + cmd.id
            : "Leave the studio tab open so capture tools can run. Poll GET /api/agent/result/" +
              cmd.id,
        });
      },
    },
  },
});
