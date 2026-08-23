export type EngineSource = "demo" | "screen" | "camera" | "idle";
export type DemoPhase = "ready" | "paying" | "declined";

export type EngineSnapshot = {
  source: EngineSource;
  recording: boolean;
  recordingStartedAt: number | null;
  lastStill: string | null;
  lastError: string | null;
  live: boolean;
  inspecting: boolean;
  demoPhase: DemoPhase;
  couponRejected: boolean;
  taxFailed: boolean;
  version: number;
};

export const DEMO_FRAME = { w: 1280, h: 720 };

export function demoLayout(w = DEMO_FRAME.w, h = DEMO_FRAME.h) {
  const listX = 28;
  const listY = 116;
  const listW = w * 0.52;
  const px = listX + listW + 24;
  const pw = w - px - 28;
  const btnW = 72;
  const fieldW = listW - 48 - btnW - 8;
  const couponY = listY + 194;
  const taxY = listY + 232;
  return {
    couponBtn: { x: listX + 20 + fieldW + 8, y: couponY, w: btnW, h: 32 },
    taxBtn: { x: listX + 20 + fieldW + 8, y: taxY, w: btnW, h: 32 },
    pay: { x: px + 20, y: listY + 292, w: pw - 52, h: 36 },
    couponField: { x: listX + 20, y: couponY, w: fieldW, h: 32 },
    taxField: { x: listX + 20, y: taxY, w: fieldW, h: 32 },
  };
}

function toHit(r: { x: number; y: number; w: number; h: number }, w = DEMO_FRAME.w, h = DEMO_FRAME.h) {
  return { x: r.x / w, y: r.y / h, w: r.w / w, h: r.h / h };
}

export function demoPayHit() {
  return toHit(demoLayout().pay);
}
export function demoCouponHit() {
  return toHit(demoLayout().couponBtn);
}
export function demoTaxHit() {
  return toHit(demoLayout().taxBtn);
}

type Listener = () => void;

const IDLE_SNAPSHOT: EngineSnapshot = {
  source: "idle",
  recording: false,
  recordingStartedAt: null,
  lastStill: null,
  lastError: null,
  live: false,
  inspecting: false,
  demoPhase: "ready",
  couponRejected: false,
  taxFailed: false,
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

function paintDemo(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  t: number,
  phase: DemoPhase,
  couponRejected: boolean,
  taxFailed: boolean,
) {
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
    const y = listY + 40 + i * 48;
    ctx.fillStyle = "#f3eee6";
    roundRect(ctx, listX + 20, y, 40, 40, 8);
    ctx.fill();
    ctx.fillStyle = i === 2 ? "#9aa0ad" : "#1c1e24";
    ctx.font = "600 18px system-ui, sans-serif";
    ctx.fillText(item.name.slice(0, 1), listX + 34, y + 26);
    ctx.fillStyle = "#1c1e24";
    ctx.font = "600 13px system-ui, sans-serif";
    ctx.fillText(item.name, listX + 72, y + 16);
    ctx.fillStyle = "#8a8074";
    ctx.font = "11px ui-monospace, monospace";
    ctx.fillText(`${item.sku}  ·  qty ${item.qty}`, listX + 72, y + 32);
    ctx.fillStyle = "#1c1e24";
    ctx.font = "600 13px system-ui, sans-serif";
    ctx.textAlign = "right";
    ctx.fillText(item.price, listX + listW - 20, y + 28);
    ctx.textAlign = "left";
  });

  const layout = demoLayout(w, h);
  ctx.fillStyle = "#8a8074";
  ctx.font = "11px system-ui, sans-serif";
  ctx.fillText("Coupon", layout.couponField.x, layout.couponField.y - 6);
  ctx.fillStyle = "#f6f3ee";
  roundRect(ctx, layout.couponField.x, layout.couponField.y, layout.couponField.w, layout.couponField.h, 8);
  ctx.fill();
  ctx.fillStyle = "#1c1e24";
  ctx.font = "13px ui-monospace, monospace";
  ctx.fillText("LUMEN10", layout.couponField.x + 12, layout.couponField.y + 21);
  ctx.fillStyle = couponRejected ? "#7a2424" : "#1c1e24";
  roundRect(ctx, layout.couponBtn.x, layout.couponBtn.y, layout.couponBtn.w, layout.couponBtn.h, 8);
  ctx.fill();
  ctx.fillStyle = "#fbf8f3";
  ctx.font = "600 12px system-ui, sans-serif";
  ctx.textAlign = "center";
  ctx.fillText(couponRejected ? "Expired" : "Apply", layout.couponBtn.x + layout.couponBtn.w / 2, layout.couponBtn.y + 21);
  ctx.textAlign = "left";

  ctx.fillStyle = "#8a8074";
  ctx.font = "11px system-ui, sans-serif";
  ctx.fillText("ZIP", layout.taxField.x, layout.taxField.y - 6);
  ctx.fillStyle = "#f6f3ee";
  roundRect(ctx, layout.taxField.x, layout.taxField.y, layout.taxField.w, layout.taxField.h, 8);
  ctx.fill();
  ctx.fillStyle = "#1c1e24";
  ctx.font = "13px ui-monospace, monospace";
  ctx.fillText("94107", layout.taxField.x + 12, layout.taxField.y + 21);
  ctx.fillStyle = taxFailed ? "#7a2424" : "#1c1e24";
  roundRect(ctx, layout.taxBtn.x, layout.taxBtn.y, layout.taxBtn.w, layout.taxBtn.h, 8);
  ctx.fill();
  ctx.fillStyle = "#fbf8f3";
  ctx.font = "600 12px system-ui, sans-serif";
  ctx.textAlign = "center";
  ctx.fillText(taxFailed ? "500" : "Tax", layout.taxBtn.x + layout.taxBtn.w / 2, layout.taxBtn.y + 21);
  ctx.textAlign = "left";

  ctx.fillStyle = "#8a8074";
  ctx.font = "12px system-ui, sans-serif";
  ctx.fillText("Subtotal (3 items)", listX + 20, listY + 316);
  ctx.fillStyle = "#1c1e24";
  ctx.font = "600 13px system-ui, sans-serif";
  ctx.textAlign = "right";
  ctx.fillText("$45.00", listX + listW - 20, listY + 316);
  ctx.textAlign = "left";
  ctx.fillStyle = "#c4b8a8";
  ctx.font = "12px system-ui, sans-serif";
  ctx.fillText(taxFailed ? "Tax helper threw · pricing.ts:88" : "Tax (undefined helper)", listX + 20, listY + 334);

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

  ctx.fillStyle = phase === "declined" ? "#7a2424" : "#1c1e24";
  roundRect(ctx, px + 20, listY + 292, pw - 52, 36, 10);
  ctx.fill();
  ctx.fillStyle = "#fbf8f3";
  ctx.font = "600 13px system-ui, sans-serif";
  ctx.save();
  ctx.beginPath();
  ctx.rect(px + 20, listY + 292, pw - 52, 36);
  ctx.clip();
  const pulse = 0.5 + Math.sin(t / 400) * 0.5;
  ctx.globalAlpha = phase === "paying" ? 0.7 : 0.85 + pulse * 0.15;
  const cta =
    phase === "paying"
      ? "Processing payment…"
      : phase === "declined"
        ? "Card declined · 402"
        : "Pay now — encrypted checkout · processing";
  ctx.fillText(cta, px + 36, listY + 315);
  ctx.restore();

  if (phase !== "ready") {
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
      phase === "paying"
        ? "POST /api/checkout  …  tax helper running"
        : "POST /api/checkout  402  card_declined   ·   tax helper threw at pricing.ts:88",
      64,
      toastY + 26,
    );
  } else if (taxFailed || couponRejected) {
    const toastY = h - 118;
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
      taxFailed
        ? "GET /api/tax?zip=94107  500  tax is undefined   ·   pricing.ts:88"
        : "POST /api/coupon  422  coupon_expired   ·   LUMEN10",
      64,
      toastY + 26,
    );
  }

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
  demoPhase: DemoPhase = "ready";
  couponRejected = false;
  taxFailed = false;
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
      demoPhase: this.demoPhase,
      couponRejected: this.couponRejected,
      taxFailed: this.taxFailed,
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
      paintDemo(
        ctx,
        canvas.width,
        canvas.height,
        t,
        this.demoPhase,
        this.couponRejected,
        this.taxFailed,
      );
      this.raf = requestAnimationFrame(loop);
    };
    this.raf = requestAnimationFrame(loop);
  }

  async useDemo() {
    this.stopTracks(false);
    this.demoPhase = "ready";
    this.couponRejected = false;
    this.taxFailed = false;
    const canvas = this.ensureDemoCanvas();
    if (canvas) {
      const ctx = canvas.getContext("2d");
      if (ctx) {
        paintDemo(
          ctx,
          canvas.width,
          canvas.height,
          0,
          this.demoPhase,
          this.couponRejected,
          this.taxFailed,
        );
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

  setDemoPhase(phase: DemoPhase) {
    this.demoPhase = phase;
    this.emit();
  }

  rejectCoupon() {
    this.couponRejected = true;
    this.emit();
  }

  failTax() {
    this.taxFailed = true;
    this.emit();
  }

  resetDemoWalk() {
    this.demoPhase = "ready";
    this.couponRejected = false;
    this.taxFailed = false;
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
