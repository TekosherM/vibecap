import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import {
  Aperture,
  Bot,
  Camera,
  Command,
  Copy,
  Download,
  Database,
  FolderOpen,
  Image as ImageIcon,
  Inbox,
  Monitor,
  Package,
  PenLine,
  Radio,
  Settings,
  Square,
  Terminal,
  ScrollText,
  CreditCard,
} from "lucide-react";
import { Toaster, toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input, Textarea } from "@/components/ui/input";
import { AnnotationCanvas } from "@/components/studio/annotation";
import { LivePreview } from "@/components/studio/live-preview";
import { captureEngine } from "@/lib/capture-engine";
import { cn } from "@/lib/cn";
import {
  addEvidence,
  answerFeedback,
  buildPack,
  cancelFeedback,
  completeCommand,
  createSession,
  enqueueCommand,
  getBudget,
  getFeedback,
  getHookPlan,
  getSessionBundle,
  ingestBackend,
  ingestDatabase,
  ingestFrontend,
  ingestLogs,
  listInbox,
  listSessions,
  pullCommands,
  requestFeedback,
  saveCapture,
  setBudget,
  subjectPay,
  touchStudio,
} from "@/lib/server/evidence";
import type {
  BudgetRow,
  CaptureRow,
  EvidenceRow,
  InboxRow,
  LogRow,
  PackRow,
  SessionRow,
  Stage,
} from "@/lib/types";
import { AGENT_HELP, AGENT_TOOLS, CAPTURE_ONLY_TOOLS } from "@/lib/types";
import { evaluateHooks, type HookPlan } from "@/lib/hooks";
import { DEMO_CONSOLE, DEMO_HTTP, DEMO_TERMINAL } from "@/lib/demo-data";

const STAGES: Array<{ id: Stage; label: string; hint: string; icon: typeof Aperture }> = [
  { id: "shutter", label: "Shutter", hint: "Live capture", icon: Aperture },
  { id: "sources", label: "Sources", hint: "When to hook", icon: Radio },
  { id: "pack", label: "Pack", hint: "Evidence bundle", icon: Package },
  { id: "media", label: "Media", hint: "Stills & clips", icon: FolderOpen },
  { id: "still", label: "Still", hint: "Annotate", icon: PenLine },
  { id: "inbox", label: "Inbox", hint: "Agent questions", icon: Inbox },
  { id: "agent", label: "Agent", hint: "Live connector", icon: Bot },
  { id: "settings", label: "Settings", hint: "Budget · theme", icon: Settings },
];

function useEngine() {
  return useSyncExternalStore(
    captureEngine.subscribe,
    captureEngine.getSnapshot,
    captureEngine.getServerSnapshot,
  );
}

function useNow(on: boolean) {
  const [t, setT] = useState(0);
  useEffect(() => {
    if (!on) return;
    const id = window.setInterval(() => setT((n) => n + 1), 1000);
    return () => window.clearInterval(id);
  }, [on]);
  return t;
}

function downloadHref(href: string, filename: string) {
  const a = document.createElement("a");
  a.href = href;
  a.download = filename;
  a.rel = "noopener";
  document.body.appendChild(a);
  a.click();
  a.remove();
}

function stillFilename(c: CaptureRow) {
  return `vibecap-still-${c.id.slice(0, 8)}.jpg`;
}

function clipFilename(c: CaptureRow) {
  return `vibecap-clip-${c.id.slice(0, 8)}.webm`;
}

export function Studio() {
  const engine = useEngine();
  useNow(engine.recording);
  const [stage, setStage] = useState<Stage>("shutter");
  const [sessions, setSessions] = useState<SessionRow[]>([]);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [captures, setCaptures] = useState<CaptureRow[]>([]);
  const [evidence, setEvidence] = useState<EvidenceRow[]>([]);
  const [packs, setPacks] = useState<PackRow[]>([]);
  const [logs, setLogs] = useState<LogRow[]>([]);
  const [inbox, setInbox] = useState<InboxRow[]>([]);
  const [budget, setBudgetState] = useState<BudgetRow | null>(null);
  const [activeStill, setActiveStill] = useState<CaptureRow | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [videoUrls, setVideoUrls] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [toolLog, setToolLog] = useState<string[]>([]);
  const [theme, setTheme] = useState<"dark" | "light">("dark");
  const [query, setQuery] = useState("");
  const [tick, setTick] = useState(0);
  const drainingRef = useRef(false);

  const session = sessions.find((s) => s.id === sessionId) ?? sessions[0] ?? null;
  const sid = session?.id ?? sessionId;

  const refresh = useCallback(async (id?: string) => {
    const list = await listSessions();
    setSessions(list);
    const useId = id ?? sid ?? list[0]?.id;
    if (!useId) return;
    setSessionId(useId);
    const bundle = await getSessionBundle({ data: { id: useId } });
    setCaptures(bundle.captures);
    setEvidence(bundle.evidence);
    setPacks(bundle.packs);
    setLogs(bundle.logs);
    const [box, bud] = await Promise.all([listInbox(), getBudget()]);
    setInbox(box);
    setBudgetState(bud);
  }, [sid]);

  useEffect(() => {
    void refresh();
    const themeSaved = localStorage.getItem("vibecap-theme");
    if (themeSaved === "light") {
      document.documentElement.classList.add("light");
      setTheme("light");
    }
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(true);
      }
      if (e.key === "Escape") setPaletteOpen(false);
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if (e.key.toLowerCase() === "s") void onStill();
      if (e.key.toLowerCase() === "r") void onRecordToggle();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const beat = () =>
      touchStudio({
        data: {
          recording: captureEngine.recording,
          source: captureEngine.source,
          inspecting: captureEngine.inspecting,
        },
      });
    void beat();
    const id = window.setInterval(() => {
      setTick((n) => n + 1);
      void (async () => {
        await beat();
        if (drainingRef.current) return;
        drainingRef.current = true;
        try {
          const pending = await pullCommands();
          for (const cmd of pending) {
            try {
              const result = await runTool(cmd.tool, JSON.parse(cmd.args || "{}"));
              await completeCommand({
                data: { id: cmd.id, status: "done", result },
              });
              toast.message(`Agent · ${cmd.tool}`);
            } catch (err) {
              await completeCommand({
                data: {
                  id: cmd.id,
                  status: "error",
                  result: { error: err instanceof Error ? err.message : "failed" },
                },
              });
            }
          }
        } finally {
          drainingRef.current = false;
        }
      })();
    }, 500);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sid]);

  async function persistStill(kind: "still" | "snapshot", label: string) {
    if (!sid) return null;
    const dataUrl = captureEngine.grabStill(1280, 0.92);
    if (!dataUrl) {
      toast.error("No live frame yet");
      return null;
    }
    const row = await saveCapture({
      data: { sessionId: sid, kind, label, mime: "image/jpeg", dataUrl },
    });
    setCaptures((c) => [row, ...c]);
    setActiveStill(row);
    await addEvidence({
      data: {
        sessionId: sid,
        source: "capture",
        kind,
        title: label,
        body: JSON.stringify({ source: engine.source, at: new Date().toISOString() }),
        captureId: row.id,
      },
    });
    return row;
  }

  async function onStill() {
    setBusy("still");
    const row = await persistStill("still", `Screenshot ${new Date().toLocaleTimeString()}`);
    setBusy(null);
    if (row) toast.success("Still saved");
  }

  async function onSnap() {
    setBusy("snap");
    const row = await persistStill(
      "snapshot",
      engine.recording ? `Agent snap @ rec ${Date.now()}` : `Live snap ${new Date().toLocaleTimeString()}`,
    );
    setBusy(null);
    if (row) toast.success(engine.recording ? "Snapped during recording" : "Live snap saved");
  }

  async function stopAndSaveClip() {
    const clip = await captureEngine.stopRecording();
    if (!clip || !sid) return null;
    const poster = captureEngine.grabStill(1280, 0.92) ?? "";
    const row = await saveCapture({
      data: {
        sessionId: sid,
        kind: "video",
        label: `Clip ${new Date().toLocaleTimeString()}`,
        mime: clip.blob.type,
        dataUrl: poster,
        durationMs: clip.durationMs,
      },
    });
    const blobUrl = URL.createObjectURL(clip.blob);
    setVideoUrls((m) => ({ ...m, [row.id]: blobUrl }));
    setCaptures((c) => [row, ...c]);
    return { row, durationMs: clip.durationMs, blobUrl };
  }

  async function onRecordToggle() {
    if (engine.recording) {
      const saved = await stopAndSaveClip();
      if (saved) toast.success("Recording saved — clip is in Media, not a home folder");
    } else {
      captureEngine.startRecording();
      toast.message("Recording — no duration. Snap until it settles, then Stop.");
    }
  }

  async function runTool(tool: string, args: Record<string, unknown> = {}) {
    if (!sid) throw new Error("No session");
    const note = `${new Date().toISOString()}  ${tool}`;
    setToolLog((l) => [note, ...l].slice(0, 24));
    if (tool === "vibecap_capture") {
      const row = await persistStill("still", "Agent capture");
      return {
        captureId: row?.id,
        ok: Boolean(row),
        mime: "image/jpeg",
        data_url: row?.data_url ?? null,
        path: row ? `/api/agent/still/${row.id}.jpg` : null,
      };
    }
    if (tool === "vibecap_snapshot" || tool === "vibecap_get_live_frame") {
      const row = await persistStill("snapshot", "Agent snapshot (live)");
      return {
        captureId: row?.id,
        recording: captureEngine.recording,
        inspecting: captureEngine.inspecting,
        ok: Boolean(row),
        mime: "image/jpeg",
        data_url: row?.data_url ?? null,
        path: row ? `/api/agent/still/${row.id}.jpg` : null,
      };
    }
    if (tool === "vibecap_record_start") {
      if (!captureEngine.recording) captureEngine.startRecording();
      return { recording: true, duration: "unbounded", stop_with: "vibecap_record_stop" };
    }
    if (tool === "vibecap_record_stop") {
      if (!captureEngine.recording) return { recording: false };
      const saved = await stopAndSaveClip();
      return {
        recording: false,
        captureId: saved?.row.id ?? null,
        duration_ms: saved?.durationMs ?? 0,
        mime: "image/jpeg",
        data_url: saved?.row.data_url ?? null,
        path: saved?.row.id ? `/api/agent/still/${saved.row.id}.jpg` : null,
        clip: "Media stage — Download clip. JPEG poster is inline. Not ~/Movies/Vibecap.",
      };
    }
    if (tool === "vibecap_record_video") {
      const raw = Number(args.duration_secs ?? 0);
      const secs = Number.isFinite(raw) ? Math.min(Math.max(0, raw), 600) : 0;
      if (!captureEngine.recording) captureEngine.startRecording();
      if (secs > 0) {
        window.setTimeout(() => {
          if (captureEngine.recording) void onRecordToggle();
        }, secs * 1000);
        return {
          recording: true,
          auto_stop_secs: secs,
          note: "Video keeps rolling. Snapshot is allowed until auto-stop.",
        };
      }
      return {
        recording: true,
        duration: "unbounded",
        note: "No duration given — call vibecap_record_stop when the flow settles.",
        stop_with: "vibecap_record_stop",
      };
    }
    if (tool === "vibecap_start_live_inspection") {
      const interval = Number(args.interval_secs ?? 3) || 3;
      let inspectBusy = false;
      captureEngine.startLiveInspection(interval, () => {
        if (inspectBusy) return;
        inspectBusy = true;
        void persistStill("snapshot", `Inspect ${new Date().toLocaleTimeString()}`).finally(() => {
          inspectBusy = false;
        });
      });
      return { inspecting: true, interval_secs: interval, recording: captureEngine.recording };
    }
    if (tool === "vibecap_stop_live_inspection") {
      captureEngine.stopLiveInspection();
      return { inspecting: false };
    }
    if (tool === "vibecap_hooks") {
      return await getHookPlan({ data: { sessionId: sid } });
    }
    if (tool === "open_studio") {
      return { ok: true, attached: captureEngine.getSnapshot().live };
    }
    if (tool === "vibecap_subject_pay") {
      captureEngine.setDemoPhase("paying");
      try {
        const result = await subjectPay({ data: { sessionId: sid } });
        captureEngine.setDemoPhase("declined");
        await refresh();
        toast.message("Checkout 402 — tax helper threw, card declined");
        return result;
      } catch (err) {
        captureEngine.setDemoPhase("ready");
        throw err;
      }
    }
    if (tool === "vibecap_ingest_frontend") {
      const snap = await persistStill("snapshot", "Frontend still");
      const ev = await ingestFrontend({ data: { sessionId: sid, captureId: snap?.id } });
      await refresh();
      return { evidenceId: ev.id };
    }
    if (tool === "vibecap_ingest_backend") {
      const ev = await ingestBackend({ data: { sessionId: sid } });
      await refresh();
      return { evidenceId: ev.id };
    }
    if (tool === "vibecap_ingest_database") {
      const ev = await ingestDatabase({ data: { sessionId: sid } });
      await refresh();
      return ev;
    }
    if (tool === "vibecap_ingest_logs") {
      const ev = await ingestLogs({ data: { sessionId: sid } });
      await refresh();
      return { evidenceId: ev.evidence.id, count: ev.logs.length };
    }
    if (tool === "vibecap_bug_pack") {
      await persistStill("snapshot", "Pack still");
      await ingestFrontend({ data: { sessionId: sid } });
      await ingestBackend({ data: { sessionId: sid } });
      await ingestDatabase({ data: { sessionId: sid } });
      await ingestLogs({ data: { sessionId: sid } });
      const pack = await buildPack({ data: { sessionId: sid } });
      await refresh();
      return { packId: pack.id, summary: pack.summary };
    }
    if (tool === "vibecap_request_feedback") {
      const row = await requestFeedback({
        data: {
          sessionId: sid,
          question: String(args.question ?? "Does this look right?"),
          options: Array.isArray(args.options) ? (args.options as string[]) : ["Yes", "No"],
          agentLabel: String(args.agent_label ?? "agent"),
        },
      });
      await refresh();
      return { request_id: row.id };
    }
    if (tool === "vibecap_get_spending") {
      return await getBudget();
    }
    if (tool === "vibecap_set_budget") {
      const row = await setBudget({
        data: {
          analysis_tier: args.analysis_tier ? String(args.analysis_tier) : undefined,
        },
      });
      setBudgetState(row);
      return row;
    }
    if (tool === "vibecap_list_feedback") {
      return await listInbox();
    }
    if (tool === "vibecap_get_feedback") {
      const id = args.request_id ? String(args.request_id) : args.id ? String(args.id) : "";
      if (!id) return await listInbox();
      return await getFeedback({ data: { id } });
    }
    if (tool === "vibecap_cancel_feedback") {
      const id = args.request_id ? String(args.request_id) : String(args.id ?? inbox.find((i) => i.status === "pending")?.id ?? "");
      if (!id) throw new Error("request_id required");
      const row = await cancelFeedback({ data: { id } });
      await refresh();
      return row;
    }
    throw new Error(`Unknown tool ${tool}`);
  }

  const pendingInbox = inbox.filter((i) => i.status === "pending").length;

  const hookPlan = useMemo(
    () =>
      evaluateHooks({
        attached: engine.live,
        recording: engine.recording,
        inspecting: engine.inspecting,
        source: engine.source,
        collected: [...new Set(evidence.map((e) => e.source))],
        captureCount: captures.length,
        stockZero: 1,
        paid: engine.demoPhase === "declined",
      }),
    [
      engine.live,
      engine.recording,
      engine.inspecting,
      engine.source,
      engine.demoPhase,
      evidence,
      captures.length,
    ],
  );

  const paletteItems = useMemo(
    () =>
      [
        { label: "Screenshot", hint: "S", run: () => void onStill() },
        { label: "Start / stop recording", hint: "R", run: () => void onRecordToggle() },
        { label: "Agent snap while live", hint: "during rec", run: () => void onSnap() },
        { label: "Pay now (walk checkout)", hint: "402", run: () => void runTool("vibecap_subject_pay") },
        ...STAGES.map((s) => ({
          label: `Go to ${s.label}`,
          hint: s.hint,
          run: () => setStage(s.id),
        })),
        {
          label: "Bug report pack",
          hint: "Collect all sources",
          run: () => void runTool("vibecap_bug_pack"),
        },
        {
          label: "Toggle theme",
          hint: "Dark / light",
          run: () => toggleTheme(theme === "dark" ? "light" : "dark"),
        },
      ].filter((i) => i.label.toLowerCase().includes(query.toLowerCase())),
    [query, theme, sid, engine.recording],
  );

  function toggleTheme(next: "dark" | "light") {
    setTheme(next);
    document.documentElement.classList.toggle("light", next === "light");
    localStorage.setItem("vibecap-theme", next);
  }

  return (
    <div className="flex h-dvh min-h-0 flex-col bg-canvas text-fg">
      <Toaster
        theme={theme === "light" ? "light" : "dark"}
        position="top-right"
        toastOptions={{
          className: "font-[family-name:var(--font-sans)]",
        }}
      />
      <header className="flex h-12 shrink-0 items-center gap-3 border-b border-border px-3 md:px-4">
        <Aperture className="size-4 text-accent" />
        <div className="min-w-0">
          <div className="text-[13px] font-semibold tracking-tight">Vibecap</div>
          <div className="hidden text-[11px] text-dim sm:block">Safelight Studio · evidence for agents</div>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <button
            type="button"
            onClick={() => setPaletteOpen(true)}
            className="hidden h-8 items-center gap-2 rounded-md bg-surface-2 px-2.5 text-[12px] text-muted shadow-[var(--shadow-border)] md:flex"
          >
            <Command className="size-3.5" />
            Command
            <kbd className="rounded bg-surface-3 px-1.5 py-0.5 font-mono text-[10px]">⌘K</kbd>
          </button>
          {pendingInbox > 0 && (
            <Badge tone="accent">{pendingInbox} inbox</Badge>
          )}
          <Badge tone={engine.recording ? "danger" : "muted"}>
            {engine.recording ? "REC" : "Idle"}
          </Badge>
        </div>
      </header>

      <div className="flex min-h-0 flex-1">
        <nav className="hidden w-[88px] shrink-0 flex-col gap-1 border-r border-border p-2 md:flex">
          {STAGES.map((s) => {
            const Icon = s.icon;
            const on = stage === s.id;
            return (
              <button
                key={s.id}
                type="button"
                onClick={() => setStage(s.id)}
                className={cn(
                  "flex flex-col items-center gap-1 rounded-[10px] px-1 py-2.5 text-[11px] transition-colors duration-150",
                  on ? "bg-surface-2 text-fg" : "text-muted hover:bg-surface-2 hover:text-fg",
                )}
              >
                <span className="relative">
                  <Icon className="size-4" />
                  {s.id === "inbox" && pendingInbox > 0 && (
                    <span className="absolute -right-1.5 -top-1 size-1.5 rounded-full bg-accent" />
                  )}
                </span>
                {s.label}
              </button>
            );
          })}
        </nav>

        <main className="flex min-h-0 min-w-0 flex-1 flex-col p-3 md:p-4">
          {stage === "shutter" && (
            <ShutterStage
              engine={engine}
              tick={tick}
              plan={hookPlan}
              onScreen={() => captureEngine.useScreen().catch((e) => toast.error(String(e)))}
              onCamera={() => captureEngine.useCamera().catch((e) => toast.error(String(e)))}
              onDemo={() => void captureEngine.useDemo()}
              onPay={() => void runTool("vibecap_subject_pay")}
            />
          )}
          {stage === "sources" && (
            <SourcesStage
              plan={hookPlan}
              evidence={evidence}
              logs={logs}
              busy={busy}
              onCollect={async (which) => {
                if (which === "open_studio") {
                  setStage("shutter");
                  return;
                }
                setBusy(which);
                try {
                  await runTool(which);
                  if (
                    which.startsWith("vibecap_ingest") ||
                    which === "vibecap_bug_pack"
                  ) {
                    toast.success("Collected");
                  }
                } finally {
                  setBusy(null);
                }
              }}
            />
          )}
          {stage === "pack" && (
            <PackStage
              packs={packs}
              evidence={evidence}
              captures={captures}
              onBuild={() => void runTool("vibecap_bug_pack")}
            />
          )}
          {stage === "media" && (
            <MediaStage
              captures={captures}
              videoUrls={videoUrls}
              onOpen={(c) => {
                setActiveStill(c);
                setStage("still");
              }}
            />
          )}
          {stage === "still" && (
            <StillStage
              still={activeStill ?? captures.find((c) => c.data_url) ?? null}
              captures={captures}
              onPick={setActiveStill}
              onSave={async (dataUrl) => {
                if (!sid) return;
                const row = await saveCapture({
                  data: {
                    sessionId: sid,
                    kind: "still",
                    label: "Annotated still",
                    mime: "image/jpeg",
                    dataUrl,
                  },
                });
                setCaptures((c) => [row, ...c]);
                setActiveStill(row);
                toast.success("Annotated still saved");
              }}
            />
          )}
          {stage === "inbox" && (
            <InboxStage
              items={inbox}
              onAnswer={async (id, payload) => {
                await answerFeedback({ data: { id, ...payload } });
                await refresh();
              }}
            />
          )}
          {stage === "agent" && (
            <AgentStage
              engine={engine}
              plan={hookPlan}
              toolLog={toolLog}
              onRun={(tool) => void runTool(tool)}
              onEnqueue={async (tool) => {
                const cmd = await enqueueCommand({ data: { tool, args: {} } });
                toast.message(`Queued ${cmd.tool}`);
              }}
              onStill={() => void onStill()}
              onSnap={() => void onSnap()}
              onRecord={() => void onRecordToggle()}
            />
          )}
          {stage === "settings" && (
            <SettingsStage
              budget={budget}
              theme={theme}
              onTheme={toggleTheme}
              onBudget={async (patch) => {
                const row = await setBudget({ data: patch });
                setBudgetState(row);
                toast.success("Budget updated");
              }}
              onNewSession={async () => {
                const name = window.prompt("Session name", "New QA run");
                if (!name) return;
                const row = await createSession({ data: { name } });
                await refresh(row.id);
                toast.success("Session opened");
              }}
              session={session}
            />
          )}
        </main>

        <aside className="hidden w-[300px] shrink-0 flex-col gap-3 border-l border-border p-3 lg:flex">
          <div>
            <div className="text-[11px] uppercase tracking-wider text-dim">Session</div>
            <div className="mt-1 text-sm font-medium">{session?.name ?? "Loading…"}</div>
            <div className="text-[12px] text-muted">{session?.notes || "Demo subject attached"}</div>
          </div>
          <div className="rounded-lg bg-surface p-3 shadow-[var(--shadow-border)]">
            <div className="text-[11px] uppercase tracking-wider text-dim">Spend</div>
            <div className="mt-2 grid grid-cols-3 gap-2 text-center">
              <Stat n={budget?.frames_used ?? 0} d={`/${budget?.max_frames ?? 80}`} l="frames" />
              <Stat n={Number(budget?.mb_used ?? 0).toFixed(1)} d={`/${budget?.max_mb ?? 40}`} l="MB" />
              <Stat n={budget?.analysis_tier ?? "standard"} d="" l="tier" />
            </div>
          </div>
          <div className="min-h-0 flex-1 overflow-auto">
            <div className="text-[11px] uppercase tracking-wider text-dim">Recent stills</div>
            <div className="mt-2 grid grid-cols-2 gap-2">
              {captures.filter((c) => c.data_url).slice(0, 6).map((c) => (
                <button
                  key={c.id}
                  type="button"
                  onClick={() => {
                    setActiveStill(c);
                    setStage("still");
                  }}
                  className="overflow-hidden rounded-md bg-surface-2"
                >
                  <img src={c.data_url ?? ""} alt={c.label} className="aspect-video w-full object-cover outline outline-1 -outline-offset-1 outline-fg/10" />
                </button>
              ))}
            </div>
          </div>
        </aside>
      </div>

      <footer className="shrink-0 border-t border-border">
        <div className="flex items-center gap-2 px-3 py-2 md:px-4">
          <Button size="sm" variant="subtle" onClick={() => void onStill()} disabled={busy === "still"}>
            <Camera className="size-4" />
            Still
          </Button>
          <Button
            size="sm"
            variant={engine.recording ? "danger" : "accent"}
            onClick={() => void onRecordToggle()}
          >
            {engine.recording ? <Square className="size-3.5 fill-current" /> : <Radio className="size-4" />}
            {engine.recording ? "Stop" : "Record"}
          </Button>
          <Button size="sm" variant="outline" onClick={() => void onSnap()} disabled={busy === "snap"}>
            <ImageIcon className="size-4" />
            Snap
          </Button>
          <Button
            size="sm"
            variant={engine.demoPhase === "declined" ? "danger" : "subtle"}
            onClick={() => void runTool("vibecap_subject_pay")}
            disabled={busy === "vibecap_subject_pay" || engine.demoPhase === "paying"}
          >
            <CreditCard className="size-4" />
            {engine.demoPhase === "paying"
              ? "Paying…"
              : engine.demoPhase === "declined"
                ? "Declined"
                : "Pay now"}
          </Button>
          <div className="hidden text-[11px] text-dim sm:block">
            Record, Pay (walks 402), Snap the failure, Stop.
          </div>
        </div>
        <div className="flex gap-1 overflow-x-auto px-3 pb-2 md:hidden">
          {STAGES.map((s) => (
            <button
              key={s.id}
              type="button"
              onClick={() => setStage(s.id)}
              className={cn(
                "shrink-0 rounded-md px-2.5 py-2 text-[11px]",
                stage === s.id ? "bg-surface-2 text-fg" : "text-muted",
              )}
            >
              {s.label}
            </button>
          ))}
        </div>
      </footer>

      {paletteOpen && (
        <div className="fixed inset-0 z-40" onClick={() => setPaletteOpen(false)}>
          <div className="absolute inset-0 bg-canvas/70" />
          <div
            className="relative mx-auto mt-24 w-[min(480px,calc(100%-24px))] rounded-xl bg-surface p-3 shadow-[var(--shadow-border)]"
            onClick={(e) => e.stopPropagation()}
          >
            <Input
              autoFocus
              placeholder="Type a command…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <div className="mt-2 max-h-72 overflow-auto">
              {paletteItems.map((item) => (
                <button
                  key={item.label}
                  type="button"
                  className="flex w-full items-center justify-between rounded-md px-3 py-2 text-left text-sm hover:bg-surface-2"
                  onClick={() => {
                    item.run();
                    setPaletteOpen(false);
                    setQuery("");
                  }}
                >
                  <span>{item.label}</span>
                  <span className="text-[11px] text-dim">{item.hint}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function Stat({ n, d, l }: { n: string | number; d: string; l: string }) {
  return (
    <div>
      <div className="font-mono text-sm tabular-nums">
        {n}
        <span className="text-dim">{d}</span>
      </div>
      <div className="text-[10px] uppercase tracking-wider text-dim">{l}</div>
    </div>
  );
}

function ShutterStage({
  engine,
  tick,
  plan,
  onScreen,
  onCamera,
  onDemo,
  onPay,
}: {
  engine: ReturnType<typeof captureEngine.getSnapshot>;
  tick: number;
  plan: HookPlan;
  onScreen: () => void;
  onCamera: () => void;
  onDemo: () => void;
  onPay: () => void;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-center gap-2">
        <h1 className="text-lg font-semibold tracking-tight">Shutter</h1>
        <Badge tone="muted" className="hidden sm:inline-flex">
          Live stage
        </Badge>
        <div className="ml-auto flex gap-1 sm:gap-2">
          <Button size="sm" variant={engine.source === "demo" ? "accent" : "subtle"} onClick={onDemo}>
            Demo
          </Button>
          <Button
            size="sm"
            variant={engine.source === "screen" ? "accent" : "subtle"}
            onClick={onScreen}
            aria-label="Screen"
          >
            <Monitor className="size-4" />
            <span className="hidden sm:inline">Screen</span>
          </Button>
          <Button
            size="sm"
            variant={engine.source === "camera" ? "accent" : "subtle"}
            onClick={onCamera}
            aria-label="Camera"
          >
            <Camera className="size-4" />
            <span className="hidden sm:inline">Camera</span>
          </Button>
        </div>
      </div>
      {engine.lastError && <p className="text-sm text-danger">{engine.lastError}</p>}
      <LivePreview engine={engine} tick={tick} onPay={onPay} />
      <div className="flex flex-wrap items-center gap-1.5">
        <Badge tone={plan.signals.console_errors ? "danger" : "muted"}>
          {plan.signals.console_errors} console
        </Badge>
        <Badge tone={plan.signals.http_fail ? "danger" : "muted"}>
          {plan.signals.http_fail} HTTP fail
        </Badge>
        <Badge tone={plan.signals.visual_issues ? "accent" : "muted"}>
          {plan.signals.visual_issues} DOM
        </Badge>
        <Badge tone={plan.signals.stock_zero ? "agent" : "muted"}>
          {plan.signals.stock_zero} stock 0
        </Badge>
        <span className="hidden text-[12px] text-muted sm:inline">Firing now — Sources says which medium to use.</span>
      </div>
      <p className="hidden text-[12px] text-muted sm:block">
        Record, then Pay now to walk checkout. Snap the 402 frame while REC is on. Stop when it settles.
      </p>
    </div>
  );
}

function SourcesStage({
  plan,
  evidence,
  logs,
  busy,
  onCollect,
}: {
  plan: HookPlan;
  evidence: EvidenceRow[];
  logs: LogRow[];
  busy: string | null;
  onCollect: (tool: string) => void;
}) {
  const layers = [
    {
      id: "frontend",
      icon: Monitor,
      title: "Frontend",
      tool: "vibecap_ingest_frontend",
      hookIds: ["dom", "console"],
      tone: "info" as const,
    },
    {
      id: "backend",
      icon: Terminal,
      title: "Backend / shell",
      tool: "vibecap_ingest_backend",
      hookIds: ["http", "terminal"],
      tone: "agent" as const,
    },
    {
      id: "database",
      icon: Database,
      title: "Database",
      tool: "vibecap_ingest_database",
      hookIds: ["database"],
      tone: "success" as const,
    },
    {
      id: "logs",
      icon: ScrollText,
      title: "Logs",
      tool: "vibecap_ingest_logs",
      hookIds: ["logs"],
      tone: "accent" as const,
    },
  ];
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto">
      <div className="flex items-end justify-between gap-3">
        <div>
          <h1 className="text-lg font-semibold tracking-tight">Sources</h1>
          <p className="text-sm text-muted">{plan.rule}</p>
        </div>
        <Button size="sm" variant="accent" onClick={() => onCollect("vibecap_bug_pack")}>
          Collect all
        </Button>
      </div>

      <div className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
        <div className="flex flex-wrap items-center gap-2">
          <div className="text-[11px] uppercase tracking-wider text-dim">Subject</div>
          <Badge tone={plan.subject.attached ? "success" : "muted"}>
            {plan.subject.attached ? "tapped" : "JSON only"}
          </Badge>
          <span className="font-mono text-[12px] text-muted">{plan.subject.url}</span>
        </div>
        <p className="mt-2 text-sm text-muted">{plan.subject.note}</p>
        <div className="mt-3 grid gap-2 sm:grid-cols-3">
          {(
            [
              ["JPEG", plan.medium.jpeg],
              ["WebM", plan.medium.webm],
              ["JSON", plan.medium.json],
            ] as const
          ).map(([label, m]) => (
            <div key={label} className="rounded-lg bg-surface-2 px-3 py-2">
              <div className="flex items-center gap-2">
                <span className="text-[13px] font-medium">{label}</span>
                <Badge tone={m.available ? "success" : "muted"} className="ml-auto">
                  {m.available ? "available" : "offline"}
                </Badge>
              </div>
              <p className="mt-1 text-[12px] text-muted">{m.why}</p>
            </div>
          ))}
        </div>
      </div>

      <div className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
        <div className="text-[11px] uppercase tracking-wider text-dim">Do next</div>
        <ol className="mt-2 space-y-1.5">
          {plan.next.map((step, i) => (
            <li key={`${step.tool}-${i}`} className="flex flex-wrap items-baseline gap-2 text-sm">
              <span className="font-mono text-[11px] text-dim">{i + 1}.</span>
              <button
                type="button"
                className="font-mono text-[12px] text-agent hover:underline"
                onClick={() => onCollect(step.tool)}
              >
                {step.tool}
              </button>
              <span className="text-muted">{step.why}</span>
            </li>
          ))}
        </ol>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        {layers.map((c) => {
          const Icon = c.icon;
          const hooks = plan.hooks.filter((h) => c.hookIds.includes(h.id));
          const live = hooks.some((h) => h.live);
          const collected = evidence.filter((e) => e.source === c.id).length;
          const when = hooks.flatMap((h) => h.hook_when).slice(0, 2);
          const skip = hooks[0]?.skip_when[0];
          const firing = hooks.flatMap((h) => h.signals).slice(0, 3);
          return (
            <div key={c.id} className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
              <div className="flex items-center gap-2">
                <Icon className="size-4 text-muted" />
                <div className="font-medium">{c.title}</div>
                <Badge tone={live ? "danger" : "muted"} className="ml-auto">
                  {live ? "firing" : "quiet"}
                </Badge>
                <Badge tone={c.tone}>{collected} collected</Badge>
              </div>
              <div className="mt-2 text-[11px] uppercase tracking-wider text-dim">JSON · hook when</div>
              <ul className="mt-1 space-y-0.5 text-[13px] text-muted">
                {when.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
              {firing.length > 0 && (
                <ul className="mt-2 space-y-0.5 font-mono text-[11px] text-danger">
                  {firing.map((s) => (
                    <li key={s} className="truncate">
                      {s}
                    </li>
                  ))}
                </ul>
              )}
              {skip && <p className="mt-2 text-[12px] text-dim">Skip: {skip}</p>}
              <Button
                size="sm"
                variant={live && collected === 0 ? "accent" : "outline"}
                className="mt-4"
                disabled={busy === c.tool}
                onClick={() => onCollect(c.tool)}
              >
                Collect
              </Button>
            </div>
          );
        })}
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        {plan.hooks
          .filter((h) => h.layer === "capture")
          .map((h) => (
            <div key={h.id} className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
              <div className="flex items-center gap-2">
                {h.id === "video" ? (
                  <Radio className="size-4 text-muted" />
                ) : (
                  <Camera className="size-4 text-muted" />
                )}
                <div className="font-medium">{h.title}</div>
                <Badge tone={h.available_now ? "success" : "muted"} className="ml-auto">
                  {h.medium.toUpperCase()}
                </Badge>
                <Badge tone={h.live ? "danger" : "muted"}>
                  {h.live ? "live" : h.available_now ? "ready" : "offline"}
                </Badge>
              </div>
              <p className="mt-2 text-sm text-muted">{h.reason}</p>
              <p className="mt-1 text-[12px] text-dim">{h.bind}</p>
              <Button
                size="sm"
                variant="outline"
                className="mt-4"
                disabled={!h.available_now || busy === h.tool}
                onClick={() =>
                  onCollect(h.id === "video" && h.live ? "vibecap_record_stop" : h.tool)
                }
              >
                {h.id === "video" ? (h.live ? "Stop" : "Start rec") : "Snap"}
              </Button>
            </div>
          ))}
      </div>
      <div className="grid gap-3 lg:grid-cols-2">
        <details className="rounded-xl bg-surface-2 p-3 shadow-[var(--shadow-border)]">
          <summary className="cursor-pointer text-[11px] uppercase tracking-wider text-dim">
            Shell / compose
          </summary>
          <pre className="mt-2 max-h-56 overflow-auto font-mono text-[11px] leading-relaxed text-muted">
            {DEMO_TERMINAL}
          </pre>
        </details>
        <div className="rounded-xl bg-surface-2 p-3 shadow-[var(--shadow-border)]">
          <div className="text-[11px] uppercase tracking-wider text-dim">Console</div>
          <ul className="mt-2 space-y-1.5 font-mono text-[11px]">
            {DEMO_CONSOLE.map((l) => (
              <li key={l.message} className={l.level === "error" ? "text-danger" : l.level === "warn" ? "text-warn" : "text-muted"}>
                {l.level}  {l.message}
              </li>
            ))}
          </ul>
          <div className="mt-3 text-[11px] uppercase tracking-wider text-dim">HTTP</div>
          <ul className="mt-2 space-y-1 font-mono text-[11px] text-muted">
            {DEMO_HTTP.map((h) => (
              <li key={h.path}>
                {h.method} {h.path}{" "}
                <span className={h.status >= 400 ? "text-danger" : "text-success"}>{h.status}</span>{" "}
                {h.ms}ms
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}

function PackStage({
  packs,
  evidence,
  captures,
  onBuild,
}: {
  packs: PackRow[];
  evidence: EvidenceRow[];
  captures: CaptureRow[];
  onBuild: () => void;
}) {
  const latest = packs[0];
  const stills = captures.filter((c) => c.data_url && c.kind !== "video");
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto">
      <div className="flex items-end justify-between">
        <div>
          <h1 className="text-lg font-semibold tracking-tight">Evidence pack</h1>
          <p className="text-sm text-muted">
            {captures.length} captures · {evidence.length} evidence items · {packs.length} packs
          </p>
        </div>
        <Button variant="accent" onClick={onBuild}>
          Build pack
        </Button>
      </div>
      {latest ? (
        <div className="min-h-0 flex-1 overflow-auto rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <div className="font-medium">{latest.title}</div>
              <div className="text-[12px] text-muted">{latest.summary}</div>
            </div>
            <div className="flex flex-wrap gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  const blob = new Blob([latest.payload], { type: "application/json" });
                  downloadHref(URL.createObjectURL(blob), `vibecap-pack-${latest.id.slice(0, 8)}.json`);
                }}
              >
                <Download className="size-4" />
                Download JSON
              </Button>
              <Button
                size="sm"
                variant="subtle"
                disabled={!stills.length}
                onClick={() => {
                  stills.forEach((c, i) => {
                    window.setTimeout(() => {
                      if (c.data_url) downloadHref(c.data_url, stillFilename(c));
                    }, i * 90);
                  });
                }}
              >
                <Download className="size-4" />
                Download stills
              </Button>
            </div>
          </div>
          {stills.length > 0 && (
            <div className="mt-4 grid grid-cols-3 gap-2 sm:grid-cols-4">
              {stills.slice(0, 12).map((c) => (
                <div key={c.id} className="overflow-hidden rounded-lg bg-surface-2">
                  <img
                    src={c.data_url ?? ""}
                    alt={c.label}
                    className="aspect-video w-full object-cover outline outline-1 -outline-offset-1 outline-fg/10"
                  />
                  <div className="flex items-center justify-between gap-1 px-2 py-1.5">
                    <span className="truncate text-[11px] text-muted">{c.label || c.kind}</span>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      className="size-8 shrink-0"
                      aria-label={`Download JPEG ${c.label || c.kind}`}
                      onClick={() => {
                        if (c.data_url) downloadHref(c.data_url, stillFilename(c));
                      }}
                    >
                      <Download className="size-3.5" />
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
          <pre className="mt-4 max-h-[40vh] overflow-auto font-mono text-[11px] leading-relaxed text-muted">
            {latest.payload.slice(0, 8000)}
          </pre>
        </div>
      ) : (
        <Empty label="No pack yet. Collect sources, then build." />
      )}
    </div>
  );
}

function MediaStage({
  captures,
  videoUrls,
  onOpen,
}: {
  captures: CaptureRow[];
  videoUrls: Record<string, string>;
  onOpen: (c: CaptureRow) => void;
}) {
  if (!captures.length) return <Empty label="Library is empty. Take a still or record." />;
  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="mb-3 flex items-end justify-between gap-3">
        <div>
          <h1 className="text-lg font-semibold tracking-tight">Media</h1>
          <p className="text-sm text-muted">
            JPEG and WebM download here. Nothing is written to a home folder.
          </p>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-4">
        {captures.map((c) => {
          const clip = c.kind === "video" ? videoUrls[c.id] : null;
          const file = clip ?? c.data_url;
          const filename = clip ? clipFilename(c) : stillFilename(c);
          return (
            <div
              key={c.id}
              className="overflow-hidden rounded-xl bg-surface shadow-[var(--shadow-border)]"
            >
              <button type="button" onClick={() => onOpen(c)} className="relative block w-full text-left">
                <img
                  src={c.data_url || clip || ""}
                  alt={c.label}
                  className="aspect-video w-full object-cover outline outline-1 -outline-offset-1 outline-fg/10"
                />
                {clip && (
                  <span className="absolute left-2 top-2 rounded bg-canvas/80 px-1.5 py-0.5 text-[10px] uppercase tracking-wider text-fg">
                    WebM
                  </span>
                )}
              </button>
              <div className="flex items-center gap-2 px-3 py-2">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] font-medium">{c.label || c.kind}</div>
                  <div className="text-[11px] text-dim">{clip ? "video/webm" : "image/jpeg"}</div>
                </div>
                <Button
                  size="icon-sm"
                  variant="subtle"
                  className="size-9 shrink-0"
                  disabled={!file}
                  aria-label={clip ? `Download clip ${c.label || c.kind}` : `Download JPEG ${c.label || c.kind}`}
                  onClick={() => {
                    if (file) downloadHref(file, filename);
                  }}
                >
                  <Download />
                </Button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function StillStage({
  still,
  captures,
  onPick,
  onSave,
}: {
  still: CaptureRow | null;
  captures: CaptureRow[];
  onPick: (c: CaptureRow) => void;
  onSave: (dataUrl: string) => void;
}) {
  const src = still?.data_url ?? "";
  if (!still || !src) return <Empty label="Capture a still first, then mark it up." />;
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex items-center gap-2">
        <h1 className="text-lg font-semibold tracking-tight">Still</h1>
        <select
          className="ml-auto h-9 rounded-md bg-surface-2 px-2 text-sm shadow-[var(--shadow-border)]"
          value={still.id}
          onChange={(e) => {
            const next = captures.find((c) => c.id === e.target.value);
            if (next) onPick(next);
          }}
        >
          {captures.filter((c) => c.data_url).map((c) => (
            <option key={c.id} value={c.id}>
              {c.label || c.kind}
            </option>
          ))}
        </select>
        <Button
          size="sm"
          variant="outline"
          aria-label="Download JPEG"
          onClick={() => downloadHref(src, stillFilename(still))}
        >
          <Download className="size-4" />
          JPEG
        </Button>
      </div>
      <AnnotationCanvas src={src} onExport={onSave} />
    </div>
  );
}

function InboxStage({
  items,
  onAnswer,
}: {
  items: InboxRow[];
  onAnswer: (id: string, payload: { answerText?: string; answerChoice?: string; status: "answered" | "dismissed" }) => void;
}) {
  const [active, setActive] = useState(items[0]?.id ?? null);
  const current = items.find((i) => i.id === active) ?? items[0];
  const [text, setText] = useState("");
  if (!current) return <Empty label="No agent questions yet." />;
  const options = JSON.parse(current.options || "[]") as string[];
  return (
    <div className="grid min-h-0 flex-1 gap-3 md:grid-cols-[220px_1fr]">
      <div className="overflow-auto rounded-xl bg-surface p-2 shadow-[var(--shadow-border)]">
        {items.map((i) => (
          <button
            key={i.id}
            type="button"
            onClick={() => setActive(i.id)}
            className={cn(
              "mb-1 w-full rounded-md px-3 py-2 text-left",
              i.id === current.id ? "bg-surface-2" : "hover:bg-surface-2",
            )}
          >
            <div className="flex items-center gap-2">
              <Badge tone={i.status === "pending" ? "accent" : "muted"}>{i.status}</Badge>
              <span className="text-[11px] text-dim">{i.agent_label}</span>
            </div>
            <div className="mt-1 line-clamp-2 text-[13px]">{i.question}</div>
          </button>
        ))}
      </div>
      <div className="flex min-h-0 flex-col rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
        <div className="text-[11px] uppercase tracking-wider text-dim">
          {current.agent_label} · {current.priority}
        </div>
        <h2 className="mt-1 text-base font-medium leading-snug">{current.question}</h2>
        {current.context && <p className="mt-2 text-sm text-muted">{current.context}</p>}
        <div className="mt-4 flex flex-wrap gap-2">
          {options.map((o) => (
            <Button
              key={o}
              size="sm"
              variant="outline"
              onClick={() => onAnswer(current.id, { answerChoice: o, status: "answered" })}
            >
              {o}
            </Button>
          ))}
        </div>
        <Textarea
          className="mt-4"
          placeholder="Or reply in prose…"
          value={text}
          onChange={(e) => setText(e.target.value)}
        />
        <div className="mt-3 flex gap-2">
          <Button
            size="sm"
            onClick={() => {
              onAnswer(current.id, { answerText: text, status: "answered" });
              setText("");
            }}
          >
            Send
          </Button>
          <Button size="sm" variant="ghost" onClick={() => onAnswer(current.id, { status: "dismissed" })}>
            Dismiss
          </Button>
        </div>
      </div>
    </div>
  );
}

function AgentStage({
  engine,
  plan,
  toolLog,
  onRun,
  onEnqueue,
  onStill,
  onSnap,
  onRecord,
}: {
  engine: ReturnType<typeof captureEngine.getSnapshot>;
  plan: HookPlan;
  toolLog: string[];
  onRun: (tool: string) => void;
  onEnqueue: (tool: string) => void;
  onStill: () => void;
  onSnap: () => void;
  onRecord: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const curl =
    `curl -s -X POST /api/agent/call -H 'content-type: application/json' ` +
    `-d '{"tool":"vibecap_hooks"}'`;

  const restTools = AGENT_TOOLS.filter(
    (t) =>
      t.name !== "vibecap_hooks" &&
      !CAPTURE_ONLY_TOOLS.some((c) => c.name === t.name),
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto">
      <div className="flex flex-wrap items-end justify-between gap-2">
        <div>
          <h1 className="text-lg font-semibold tracking-tight">Live connector</h1>
          <p className="text-sm text-muted">
            This tab is the MCP. No <span className="font-mono text-fg">vibecap --mcp</span>, no duration, no ~/Movies.
          </p>
        </div>
        <Badge tone={engine.live ? "success" : "muted"}>
          {engine.live ? "Studio attached" : "Waiting"}
        </Badge>
      </div>

      <div className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
        <div className="flex flex-wrap items-center gap-2">
          <div className="text-[11px] uppercase tracking-wider text-dim">Hook plan</div>
          <Badge tone="info">GET /api/agent/hooks</Badge>
          <Button size="sm" variant="ghost" onClick={() => onRun("vibecap_hooks")}>
            Refresh
          </Button>
        </div>
        <p className="mt-1 text-sm text-muted">{plan.rule}</p>
        <div className="mt-2 flex flex-wrap gap-1.5">
          <Badge tone={plan.medium.jpeg.available ? "success" : "muted"}>JPEG</Badge>
          <Badge tone={plan.medium.webm.available ? "success" : "muted"}>WebM</Badge>
          <Badge tone="success">JSON</Badge>
          {plan.hooks
            .filter((h) => h.live)
            .map((h) => (
              <Badge key={h.id} tone="danger">
                {h.title}
              </Badge>
            ))}
        </div>
        <ol className="mt-3 space-y-1 font-mono text-[12px] text-muted">
          {plan.next.map((step, i) => (
            <li key={`${step.tool}-${i}`} className="flex flex-wrap items-baseline gap-2">
              <span className="text-dim">{i + 1}.</span>
              <button
                type="button"
                className="text-agent hover:underline"
                onClick={() => onRun(step.tool)}
              >
                {step.tool}
              </button>
              <span>{step.why}</span>
            </li>
          ))}
        </ol>
      </div>

      <div className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
        <div className="flex flex-wrap items-center gap-2">
          <div className="text-[11px] uppercase tracking-wider text-dim">Capture-only</div>
          {engine.recording && <Badge tone="danger">REC</Badge>}
          {engine.inspecting && <Badge tone="accent">Inspect</Badge>}
        </div>
        <p className="mt-1 text-sm text-muted">
          Start rec → Pay now → Snap the 402 → Stop. Inbox is optional.
        </p>
        <div className="mt-3 flex flex-wrap gap-2">
          <Button size="sm" variant="subtle" onClick={onStill}>
            <Camera className="size-4" />
            Still
          </Button>
          <Button size="sm" variant={engine.recording ? "danger" : "accent"} onClick={onRecord}>
            {engine.recording ? <Square className="size-3.5 fill-current" /> : <Radio className="size-4" />}
            {engine.recording ? "Stop" : "Start rec"}
          </Button>
          <Button size="sm" variant="outline" onClick={() => onRun("vibecap_subject_pay")}>
            <CreditCard className="size-4" />
            Pay now
          </Button>
          <Button size="sm" variant="outline" onClick={onSnap}>
            <ImageIcon className="size-4" />
            Snap
          </Button>
        </div>
        <ol className="mt-3 space-y-1 font-mono text-[12px] text-muted">
          {CAPTURE_ONLY_TOOLS.map((t, i) => (
            <li key={t.name} className="flex flex-wrap items-baseline gap-2">
              <span className="text-dim">{i + 1}.</span>
              <button
                type="button"
                className="text-agent hover:underline"
                onClick={() => onRun(t.name)}
              >
                {t.name}
              </button>
              <span>{t.summary}</span>
            </li>
          ))}
        </ol>
      </div>

      <div className="rounded-xl bg-surface p-4 font-mono text-[12px] leading-relaxed text-muted shadow-[var(--shadow-border)]">
        <div className="flex items-center justify-between gap-2">
          <div className="text-fg">POST /api/agent/call</div>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              void navigator.clipboard.writeText(curl);
              setCopied(true);
              window.setTimeout(() => setCopied(false), 1200);
            }}
          >
            <Copy className="size-3.5" />
            {copied ? "Copied" : "Copy"}
          </Button>
        </div>
        <div>{`{ "tool": "vibecap_hooks", "args": {} }`}</div>
        <div className="mt-2 text-fg">GET /api/agent/hooks</div>
        <div>GET /api/agent/help · GET /api/agent/status · GET /api/agent/media</div>
        <pre className="mt-3 max-h-40 overflow-auto whitespace-pre-wrap text-[11px] text-dim">
          {AGENT_HELP}
        </pre>
      </div>

      <div>
        <div className="text-[11px] uppercase tracking-wider text-dim">More tools</div>
        <div className="mt-2 grid gap-2 sm:grid-cols-2">
          {restTools.map((t) => (
            <div key={t.name} className="rounded-lg bg-surface p-3 shadow-[var(--shadow-border)]">
              <div className="font-mono text-[12px] text-agent">{t.name}</div>
              <p className="mt-1 text-[12px] text-muted">{t.summary}</p>
              <div className="mt-2 flex gap-2">
                <Button size="sm" variant="subtle" onClick={() => onRun(t.name)}>
                  Run
                </Button>
                <Button size="sm" variant="ghost" onClick={() => onEnqueue(t.name)}>
                  Queue
                </Button>
              </div>
            </div>
          ))}
        </div>
      </div>
      <div>
        <div className="text-[11px] uppercase tracking-wider text-dim">Tool log</div>
        <ul className="mt-1 font-mono text-[11px] text-muted">
          {toolLog.length === 0 && <li>No calls yet</li>}
          {toolLog.map((l) => (
            <li key={l}>{l}</li>
          ))}
        </ul>
      </div>
    </div>
  );
}

function SettingsStage({
  budget,
  theme,
  onTheme,
  onBudget,
  onNewSession,
  session,
}: {
  budget: BudgetRow | null;
  theme: "dark" | "light";
  onTheme: (t: "dark" | "light") => void;
  onBudget: (p: { analysis_tier?: string; max_frames?: number }) => void;
  onNewSession: () => void;
  session: SessionRow | null;
}) {
  return (
    <div className="mx-auto w-full max-w-xl space-y-4 overflow-auto">
      <h1 className="text-lg font-semibold tracking-tight">Settings</h1>
      <section className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
        <div className="text-sm font-medium">Appearance</div>
        <div className="mt-3 flex gap-2">
          <Button size="sm" variant={theme === "dark" ? "accent" : "subtle"} onClick={() => onTheme("dark")}>
            Graphite
          </Button>
          <Button size="sm" variant={theme === "light" ? "accent" : "subtle"} onClick={() => onTheme("light")}>
            Paper
          </Button>
        </div>
      </section>
      <section className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
        <div className="text-sm font-medium">Agent budget</div>
        <p className="mt-1 text-[12px] text-muted">Caps live inspection so vision spend stays bounded.</p>
        <div className="mt-3 flex flex-wrap gap-2">
          {(["eco", "standard", "intensive"] as const).map((tier) => (
            <Button
              key={tier}
              size="sm"
              variant={budget?.analysis_tier === tier ? "accent" : "subtle"}
              onClick={() => onBudget({ analysis_tier: tier })}
            >
              {tier}
            </Button>
          ))}
        </div>
      </section>
      <section className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
        <div className="text-sm font-medium">Session</div>
        <p className="mt-1 text-[12px] text-muted">{session?.name}</p>
        <Button size="sm" className="mt-3" variant="outline" onClick={onNewSession}>
          New session
        </Button>
      </section>
    </div>
  );
}

function Empty({ label }: { label: string }) {
  return (
    <div className="flex flex-1 items-center justify-center rounded-xl bg-surface text-sm text-muted shadow-[var(--shadow-border)]">
      {label}
    </div>
  );
}
