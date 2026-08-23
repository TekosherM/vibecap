import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, Highlighter, Pen, Square, Type } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/cn";

type Tool = "pen" | "arrow" | "rect" | "highlight" | "text";

type Stroke = {
  tool: Tool;
  color: string;
  points: Array<{ x: number; y: number }>;
  text?: string;
};

const COLORS = ["#e05555", "#f59e4b", "#5ec26a", "#6ba3e8", "#e8eaef"];

export function AnnotationCanvas({
  src,
  onExport,
}: {
  src: string;
  onExport: (dataUrl: string) => void;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const imgRef = useRef<HTMLImageElement | null>(null);
  const [tool, setTool] = useState<Tool>("pen");
  const [color, setColor] = useState(COLORS[0]);
  const [strokes, setStrokes] = useState<Stroke[]>([]);
  const current = useRef<Stroke | null>(null);

  useEffect(() => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      imgRef.current = img;
      draw();
    };
    img.src = src;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [src, strokes]);

  function draw() {
    const canvas = canvasRef.current;
    const img = imgRef.current;
    if (!canvas || !img) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    canvas.width = 960;
    canvas.height = Math.round((img.height / img.width) * 960) || 540;
    ctx.fillStyle = "#121316";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
    for (const s of strokes) paintStroke(ctx, s);
    if (current.current) paintStroke(ctx, current.current);
  }

  function paintStroke(ctx: CanvasRenderingContext2D, s: Stroke) {
    if (s.points.length === 0) return;
    ctx.strokeStyle = s.color;
    ctx.fillStyle = s.color;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    const a = s.points[0];
    const b = s.points[s.points.length - 1];
    if (s.tool === "pen") {
      ctx.lineWidth = 3;
      ctx.beginPath();
      s.points.forEach((p, i) => (i === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y)));
      ctx.stroke();
    } else if (s.tool === "highlight") {
      ctx.lineWidth = 16;
      ctx.globalAlpha = 0.35;
      ctx.beginPath();
      s.points.forEach((p, i) => (i === 0 ? ctx.moveTo(p.x, p.y) : ctx.lineTo(p.x, p.y)));
      ctx.stroke();
      ctx.globalAlpha = 1;
    } else if (s.tool === "rect") {
      ctx.lineWidth = 2;
      ctx.strokeRect(a.x, a.y, b.x - a.x, b.y - a.y);
    } else if (s.tool === "arrow") {
      ctx.lineWidth = 3;
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.stroke();
      const ang = Math.atan2(b.y - a.y, b.x - a.x);
      ctx.beginPath();
      ctx.moveTo(b.x, b.y);
      ctx.lineTo(b.x - 12 * Math.cos(ang - 0.4), b.y - 12 * Math.sin(ang - 0.4));
      ctx.lineTo(b.x - 12 * Math.cos(ang + 0.4), b.y - 12 * Math.sin(ang + 0.4));
      ctx.closePath();
      ctx.fill();
    } else if (s.tool === "text") {
      ctx.font = "600 16px 'DM Sans', sans-serif";
      ctx.fillText(s.text ?? "Note", a.x, a.y);
    }
  }

  function pos(e: React.PointerEvent) {
    const canvas = canvasRef.current;
    if (!canvas) return { x: 0, y: 0 };
    const r = canvas.getBoundingClientRect();
    return {
      x: ((e.clientX - r.left) / r.width) * canvas.width,
      y: ((e.clientY - r.top) / r.height) * canvas.height,
    };
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        {(
          [
            ["pen", Pen],
            ["arrow", ArrowUpRight],
            ["rect", Square],
            ["highlight", Highlighter],
            ["text", Type],
          ] as const
        ).map(([id, Icon]) => (
          <Button
            key={id}
            size="icon-sm"
            variant={tool === id ? "accent" : "subtle"}
            onClick={() => setTool(id)}
            aria-label={id}
          >
            <Icon />
          </Button>
        ))}
        <div className="mx-1 h-5 w-px bg-border" />
        {COLORS.map((c) => (
          <button
            key={c}
            type="button"
            onClick={() => setColor(c)}
            className={cn(
              "size-7 rounded-full",
              color === c ? "ring-2 ring-fg ring-offset-2 ring-offset-canvas" : "",
            )}
            style={{ background: c }}
            aria-label={c}
          />
        ))}
        <Button size="sm" variant="outline" onClick={() => setStrokes([])}>
          Clear
        </Button>
        <Button
          size="sm"
          variant="accent"
          onClick={() => {
            const canvas = canvasRef.current;
            if (canvas) onExport(canvas.toDataURL("image/jpeg", 0.82));
          }}
        >
          Save annotated
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-hidden rounded-lg bg-surface-2 shadow-[var(--shadow-border)]">
        <canvas
          ref={canvasRef}
          className="h-full w-full cursor-crosshair object-contain"
          onPointerDown={(e) => {
            const p = pos(e);
            current.current = {
              tool,
              color,
              points: [p],
              text: tool === "text" ? window.prompt("Label") || "Note" : undefined,
            };
            (e.target as HTMLCanvasElement).setPointerCapture(e.pointerId);
          }}
          onPointerMove={(e) => {
            if (!current.current) return;
            current.current.points.push(pos(e));
            draw();
          }}
          onPointerUp={() => {
            if (current.current) {
              const s = current.current;
              current.current = null;
              setStrokes((prev) => [...prev, s]);
            }
          }}
        />
      </div>
    </div>
  );
}
