import { useEffect, useRef } from "react";
import { captureEngine, type EngineSnapshot } from "@/lib/capture-engine";
import { Badge } from "@/components/ui/badge";

function formatRec(started: number | null) {
  if (!started) return "00:00";
  const s = Math.max(0, Math.floor((Date.now() - started) / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  if (h > 0) return `${h}:${pad(m)}:${pad(sec)}`;
  return `${pad(m)}:${pad(sec)}`;
}

export function LivePreview({
  engine,
  tick,
}: {
  engine: EngineSnapshot;
  tick: number;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    captureEngine.attachVideo(videoRef.current);
    if (captureEngine.source === "idle" || captureEngine.source === "demo") {
      void captureEngine.useDemo();
    }
    return () => captureEngine.attachVideo(null);
  }, []);

  useEffect(() => {
    const el = videoRef.current;
    if (el && captureEngine.stream && el.srcObject !== captureEngine.stream) {
      el.srcObject = captureEngine.stream;
    }
  }, [engine.version]);

  return (
    <div className="relative min-h-0 flex-1 overflow-hidden rounded-xl bg-surface-2 shadow-[var(--shadow-border)]">
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        poster={engine.lastStill ?? undefined}
        className="h-full w-full object-contain"
      />
      <div className="pointer-events-none absolute inset-x-0 top-0 flex items-start justify-between p-3">
        <div className="flex items-center gap-2">
          {engine.recording ? (
            <Badge tone="danger">
              <span
                className="size-1.5 rounded-full bg-danger"
                style={{ animation: "rec-pulse 1s ease-in-out infinite" }}
              />
              REC {formatRec(engine.recordingStartedAt)}
            </Badge>
          ) : (
            <Badge tone="accent">
              <span className="size-1.5 rounded-full bg-accent" />
              LIVE
            </Badge>
          )}
          <Badge tone="muted">{engine.source === "idle" ? "demo" : engine.source}</Badge>
          {engine.inspecting && <Badge tone="accent">Inspect</Badge>}
        </div>
        <Badge tone="agent">
          {engine.recording ? "Unbounded · Snap until it settles" : "Start/stop · no duration"}
        </Badge>
      </div>
      <div className="pointer-events-none absolute inset-x-0 bottom-0 flex justify-between p-3 text-[11px] text-muted">
        <span>1280×720 · 30 fps</span>
        <span className="tabular-nums">t {tick}s</span>
      </div>
    </div>
  );
}
