export type EngineSource = "demo" | "screen" | "camera" | "idle";

export type EngineSnapshot = {
  source: EngineSource;
  recording: boolean;
  recordingStartedAt: number | null;
  lastStill: string | null;
  lastError: string | null;
  live: boolean;
  inspecting: boolean;
  version: number;
};

type Listener = () => void;

const IDLE_SNAPSHOT: EngineSnapshot = {
  source: "idle",
  recording: false,
  recordingStartedAt: null,
  lastStill: null,
  lastError: null,
  live: false,
  inspecting: false,
  version: 0,
};

function roundRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  const rr = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + rr, y);
  ctx.arcTo(x + w, y, x + w, y + h, rr);
  ctx.arcTo(x + w, y + h, x, y + h, rr);
  ctx.arcTo(x, y + h, x, y, rr);
  ctx.arcTo(x, y, x + w, y, rr);
  ctx.closePath();
}

function paintDemo(ctx: CanvasRenderingContext2D, w: number, h: number, t: number) {
  ctx.clearRect(0, 0, w, h);
  ctx.fillStyle = "#ece7df";
  ctx.fillRect(0, 0, w, h);

  ctx.fillStyle = "#161418";
  ctx.fillRect(0, 0, w, 36);
  const lights = ["#e05555", "#d8a441", "#5ec26a"];
  lights.forEach((c, i) => {
    ctx.fillStyle = c;
    ctx.beginPath();
    ctx.arc(18 + i * 16, 18, 5, 0, Math.PI * 2);
    ctx.fill();
  });
  ctx.fillStyle = "#2a2d34";
  roundRect(ctx, w / 2 - 150, 8, 300, 20, 6);
  ctx.fill();
  ctx.fillStyle = "#9aa0ad";
  ctx.font = "11px ui-monospace, monospace";
  ctx.textAlign = "center";
  ctx.fillText("checkout.lumen.test/pay", w / 2, 22);
  ctx.textAlign = "left";
  const clock = new Date();
  ctx.fillStyle = "#c8ccd4";
  ctx.font = "11px system-ui, sans-serif";
  ctx.textAlign = "right";
  ctx.fillText(
    clock.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }),
    w - 16,
    22,
  );
  ctx.textAlign = "left";

  ctx.fillStyle = "#fbf8f3";
  ctx.fillRect(0, 36, w, 56);
  ctx.fillStyle = "#1c1e24";
  ctx.font = "600 18px system-ui, sans-serif";
  ctx.fillText("Lumen Cart", 24, 70);
  ctx.fillStyle = "#8a8074";
  ctx.font = "13px system-ui, sans-serif";
  ctx.fillText("Secure checkout  ·  3 items", 24, 86);
  ctx.fillStyle = "#1c1e24";
  roundRect(ctx, w - 148, 48, 124, 32, 8);
  ctx.fill();
  ctx.fillStyle = "#fbf8f3";
  ctx.font = "600 12px system-ui, sans-serif";
  ctx.textAlign = "center";
  ctx.fillText("Account", w - 86, 68);
  ctx.textAlign = "left";

  ctx.strokeStyle = "rgba(28,30,36,0.08)";
  ctx.beginPath();
  ctx.moveTo(0, 92);
  ctx.lineTo(w, 92);
  ctx.stroke();

  const items = [
    { name: "Linen tote", sku: "LM-12", price: "$12.00", qty: "1" },
    { name: "Safelight mug", sku: "LM-18", price: "$18.00", qty: "1" },
    { name: "Graphite notebook", sku: "LM-15", price: "$15.00", qty: "1 · stock 0" },
  ];

  const listX = 28;
  const listY = 116;
  const listW = w * 0.52;
  ctx.fillStyle = "#ffffff";
  roundRect(ctx, listX, listY, listW, 340, 14);
  ctx.fill();
  ctx.strokeStyle = "rgba(28,30,36,0.08)";
  ctx.stroke();
  ctx.fillStyle = "#1c1e24";
  ctx.font = "600 14px system-ui, sans-serif";
  ctx.fillText("Order summary", listX + 20, listY + 28);

  items.forEach((item, i) => {
    const y = listY + 56 + i * 78;
    ctx.fillStyle = "#f3eee6";
    roundRect(ctx, listX + 20, y, 56, 56, 10);
    ctx.fill();
    ctx.fillStyle = i === 2 ? "#9aa0ad" : "#1c1e24";
    ctx.font = "600 22px system-ui, sans-serif";
    ctx.fillText(item.name.slice(0, 1), listX + 40, y + 36);
    ctx.fillStyle = "#1c1e24";
    ctx.font = "600 14px system-ui, sans-serif";
    ctx.fillText(item.name, listX + 92, y + 24);
    ctx.fillStyle = "#8a8074";
    ctx.font = "12px ui-monospace, monospace";
    ctx.fillText(`${item.sku}  ·  qty ${item.qty}`, listX + 92, y + 44);
    ctx.fillStyle = "#1c1e24";
    ctx.font = "600 14px system-ui, sans-serif";
    ctx.textAlign = "right";
    ctx.fillText(item.price, listX + listW - 20, y + 32);
    ctx.textAlign = "left";
  });

  ctx.fillStyle = "#8a8074";
  ctx.font = "12px system-ui, sans-serif";
  ctx.fillText("Subtotal (3 items)", listX + 20, listY + 300);
  ctx.fillStyle = "#1c1e24";
  ctx.font = "600 13px system-ui, sans-serif";
  ctx.textAlign = "right";
  ctx.fillText("$45.00", listX + listW - 20, listY + 300);
  ctx.textAlign = "left";
  ctx.fillStyle = "#c4b8a8";
  ctx.font = "12px system-ui, sans-serif";
  ctx.fillText("Tax (undefined helper)", listX + 20, listY + 322);

  const px = listX + listW + 24;
  const pw = w - px - 28;
  ctx.fillStyle = "#ffffff";
  roundRect(ctx, px, listY, pw, 340, 14);
  ctx.fill();
  ctx.strokeStyle = "rgba(28,30,36,0.08)";
  ctx.stroke();
  ctx.fillStyle = "#1c1e24";
  ctx.font = "600 14px system-ui, sans-serif";
  ctx.fillText("Pay with card", px + 20, listY + 28);

  const fields = ["Card number", "Expiry", "CVC"];
  fields.forEach((label, i) => {
    const y = listY + 48 + i * 58;
    ctx.fillStyle = "#8a8074";
    ctx.font = "11px system-ui, sans-serif";
    ctx.fillText(label, px + 20, y);
    ctx.fillStyle = "#f6f3ee";
    roundRect(ctx, px + 20, y + 8, pw - 40, 34, 8);
    ctx.fill();
    ctx.fillStyle = "#1c1e24";
    ctx.font = "13px ui-monospace, monospace";
    const vals = ["4242  4242  4242  4242", "12 / 28", "•••"];
    ctx.fillText(vals[i] ?? "", px + 32, y + 30);
  });

  ctx.fillStyle = "#1c1e24";
  ctx.font = "600 12px system-ui, sans-serif";
  ctx.fillText("Due today", px + 20, listY + 244);
  ctx.font = "600 28px system-ui, sans-serif";
  ctx.fillText("$41.00", px + 20, listY + 276);

  ctx.fillStyle = "#1c1e24";
  roundRect(ctx, px + 20, listY + 292, pw - 52, 36, 10);
  ctx.fill();
  ctx.fillStyle = "#fbf8f3";
  ctx.font = "600 13px system-ui, sans-serif";
  ctx.save();
  ctx.beginPath();
  ctx.rect(px + 20, listY + 292, pw - 52, 36);
  ctx.clip();
  const pulse = 0.5 + Math.sin(t / 400) * 0.5;
  ctx.globalAlpha = 0.85 + pulse * 0.15;
  ctx.fillText("Pay now — encrypted checkout · processing", px + 36, listY + 315);
  ctx.restore();

  const toastY = h - 118 + Math.sin(t / 700) * 2;
  ctx.fillStyle = "#2a1616";
  roundRect(ctx, 28, toastY, w - 56, 44, 10);
  ctx.fill();
  ctx.fillStyle = "#e05555";
  ctx.beginPath();
  ctx.arc(48, toastY + 22, 5, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = "#f4d2d2";
  ctx.font = "12px ui-monospace, monospace";
  ctx.fillText(
    "POST /api/checkout  402  card_declined   ·   tax helper threw at pricing.ts:88",
    64,
    toastY + 26,
  );

  const cx = px + 90 + Math.sin(t / 900) * 18;
  const cy = listY + 310;
  ctx.fillStyle = "#121316";
  ctx.beginPath();
  ctx.moveTo(cx, cy);
  ctx.lineTo(cx + 10, cy + 14);
  ctx.lineTo(cx + 5, cy + 14);
  ctx.lineTo(cx + 8, cy + 22);
  ctx.lineTo(cx + 5, cy + 22);
  ctx.lineTo(cx, cy + 14);
  ctx.closePath();
  ctx.fill();
}

class CaptureEngine {
  source: EngineSource = "idle";
  stream: MediaStream | null = null;
  recorder: MediaRecorder | null = null;
  recording = false;
  recordingStartedAt: number | null = null;
  inspecting = false;
  chunks: Blob[] = [];
  lastStill: string | null = null;
  lastError: string | null = null;
  demoCanvas: HTMLCanvasElement | null = null;
  videoEl: HTMLVideoElement | null = null;
  version = 0;
  private cached: EngineSnapshot = IDLE_SNAPSHOT;
  private listeners = new Set<Listener>();
  private raf = 0;
  private inspectTimer = 0;

  subscribe = (fn: Listener) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };

  getSnapshot = (): EngineSnapshot => this.cached;

  getServerSnapshot = (): EngineSnapshot => IDLE_SNAPSHOT;

  private emit() {
    this.version += 1;
    this.cached = {
      source: this.source,
      recording: this.recording,
      recordingStartedAt: this.recordingStartedAt,
      lastStill: this.lastStill,
      lastError: this.lastError,
      live: Boolean(this.stream) || this.source === "demo",
      inspecting: this.inspecting,
      version: this.version,
    };
    this.listeners.forEach((fn) => fn());
  }

  attachVideo(el: HTMLVideoElement | null) {
    this.videoEl = el;
    if (el && this.stream) el.srcObject = this.stream;
    if (el && this.lastStill) el.poster = this.lastStill;
  }

  ensureDemoCanvas() {
    if (typeof document === "undefined") return null;
    if (this.demoCanvas) return this.demoCanvas;
    const canvas = document.createElement("canvas");
    canvas.width = 1280;
    canvas.height = 720;
    this.demoCanvas = canvas;
    return canvas;
  }

  startDemoLoop() {
    const canvas = this.ensureDemoCanvas();
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    if (this.raf) cancelAnimationFrame(this.raf);
    const loop = (t: number) => {
      paintDemo(ctx, canvas.width, canvas.height, t);
      this.raf = requestAnimationFrame(loop);
    };
    this.raf = requestAnimationFrame(loop);
  }

  async useDemo() {
    this.stopTracks(false);
    const canvas = this.ensureDemoCanvas();
    if (canvas) {
      const ctx = canvas.getContext("2d");
      if (ctx) {
        paintDemo(ctx, canvas.width, canvas.height, 0);
        this.lastStill = canvas.toDataURL("image/jpeg", 0.88);
        if (this.videoEl) this.videoEl.poster = this.lastStill;
      }
    }
    this.startDemoLoop();
    if (!canvas) {
      this.source = "demo";
      this.emit();
      return;
    }
    const stream = canvas.captureStream(30);
    this.stream = stream;
    this.source = "demo";
    this.lastError = null;
    if (this.videoEl) this.videoEl.srcObject = stream;
    this.emit();
  }

  async useScreen() {
    this.stopTracks(true);
    try {
      const stream = await navigator.mediaDevices.getDisplayMedia({
        video: { frameRate: 30 },
        audio: false,
      });
      stream.getVideoTracks()[0]?.addEventListener("ended", () => {
        void this.useDemo();
      });
      this.stream = stream;
      this.source = "screen";
      this.lastError = null;
      if (this.videoEl) this.videoEl.srcObject = stream;
      this.emit();
    } catch (err) {
      this.lastError = err instanceof Error ? err.message : "Screen share blocked";
      this.emit();
      throw err;
    }
  }

  async useCamera() {
    this.stopTracks(true);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: "user" },
        audio: false,
      });
      this.stream = stream;
      this.source = "camera";
      this.lastError = null;
      if (this.videoEl) this.videoEl.srcObject = stream;
      this.emit();
    } catch (err) {
      this.lastError = err instanceof Error ? err.message : "Camera blocked";
      this.emit();
      throw err;
    }
  }

  grabStill(maxWidth = 1280, quality = 0.92): string | null {
    const canvas = document.createElement("canvas");
    if (this.source === "demo" && this.demoCanvas) {
      const src = this.demoCanvas;
      const scale = Math.min(1, maxWidth / src.width);
      canvas.width = Math.round(src.width * scale);
      canvas.height = Math.round(src.height * scale);
      const ctx = canvas.getContext("2d");
      if (!ctx) return null;
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(src, 0, 0, canvas.width, canvas.height);
    } else {
      const video = this.videoEl;
      if (!video || video.videoWidth === 0) return this.lastStill;
      const scale = Math.min(1, maxWidth / video.videoWidth);
      canvas.width = Math.round(video.videoWidth * scale);
      canvas.height = Math.round(video.videoHeight * scale);
      const ctx = canvas.getContext("2d");
      if (!ctx) return null;
      ctx.imageSmoothingEnabled = true;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
    }
    const url = canvas.toDataURL("image/jpeg", quality);
    this.lastStill = url;
    this.emit();
    return url;
  }

  startRecording() {
    if (!this.stream || this.recording) return;
    this.chunks = [];
    const mime = MediaRecorder.isTypeSupported("video/webm;codecs=vp9")
      ? "video/webm;codecs=vp9"
      : MediaRecorder.isTypeSupported("video/webm")
        ? "video/webm"
        : "";
    const rec = new MediaRecorder(this.stream, mime ? { mimeType: mime } : undefined);
    rec.ondataavailable = (ev) => {
      if (ev.data.size) this.chunks.push(ev.data);
    };
    rec.start(250);
    this.recorder = rec;
    this.recording = true;
    this.recordingStartedAt = Date.now();
    this.emit();
  }

  async stopRecording(): Promise<{ blob: Blob; durationMs: number } | null> {
    const rec = this.recorder;
    if (!rec || !this.recording) return null;
    const started = this.recordingStartedAt ?? Date.now();
    const blob = await new Promise<Blob>((resolve) => {
      rec.onstop = () => {
        resolve(new Blob(this.chunks, { type: rec.mimeType || "video/webm" }));
      };
      rec.stop();
    });
    this.recording = false;
    this.recorder = null;
    this.recordingStartedAt = null;
    this.emit();
    return { blob, durationMs: Date.now() - started };
  }

  startLiveInspection(intervalSecs: number, onFrame: (url: string) => void) {
    this.stopLiveInspection(false);
    this.inspecting = true;
    const beat = () => {
      const url = this.grabStill();
      if (url) onFrame(url);
    };
    beat();
    const ms = Math.max(1, intervalSecs) * 1000;
    this.inspectTimer = window.setInterval(beat, ms);
    this.emit();
  }

  stopLiveInspection(emit = true) {
    if (this.inspectTimer) {
      window.clearInterval(this.inspectTimer);
      this.inspectTimer = 0;
    }
    const was = this.inspecting;
    this.inspecting = false;
    if (emit && was) this.emit();
  }

  private stopTracks(stopDemo: boolean) {
    if (this.recording) {
      try {
        this.recorder?.stop();
      } catch {
        /* ignore */
      }
      this.recording = false;
      this.recorder = null;
      this.recordingStartedAt = null;
    }
    this.stopLiveInspection(false);
    this.stream?.getTracks().forEach((t) => {
      if (stopDemo || this.source !== "demo") t.stop();
    });
    this.stream = null;
    if (stopDemo && this.raf) {
      cancelAnimationFrame(this.raf);
      this.raf = 0;
    }
  }
}

export const captureEngine = new CaptureEngine();
