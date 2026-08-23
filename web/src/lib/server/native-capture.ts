/**
 * Server-side native capturer — same backend as `vibecap` CLI (ffmpeg x11grab
 * on Linux). Used when an agent names a display, window, or output_dir so
 * stills are the target screen, not the Lumen Cart demo shutter.
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const NATIVE_TOOLS = new Set([
  "vibecap_capture",
  "vibecap_snapshot",
  "vibecap_get_live_frame",
  "vibecap_record_start",
  "vibecap_record_stop",
  "vibecap_record_status",
  "vibecap_record_video",
]);

export function wantsNativeSource(args?: Record<string, unknown> | null): boolean {
  if (!args) return false;
  const source = String(args.source ?? "").toLowerCase();
  if (["display", "native", "x11", "x11grab"].includes(source)) return true;
  for (const key of ["display", "window", "output_dir"] as const) {
    const v = args[key];
    if (typeof v === "string" && v.trim()) return true;
  }
  return false;
}

export function isNativeCaptureTool(tool: string): boolean {
  return NATIVE_TOOLS.has(tool);
}

export function findVibecapBin(): string | null {
  if (process.env.VIBECAP_BIN && existsSync(process.env.VIBECAP_BIN)) {
    return process.env.VIBECAP_BIN;
  }
  const here = dirname(fileURLToPath(import.meta.url));
  // web/src/lib/server → repo root
  const root = resolve(here, "../../../..");
  for (const rel of ["target/release/vibecap", "target/debug/vibecap"]) {
    const p = resolve(root, rel);
    if (existsSync(p)) return p;
  }
  const which = spawnSync("which", ["vibecap"], { encoding: "utf8" });
  const hit = which.stdout?.trim();
  if (which.status === 0 && hit && existsSync(hit)) return hit;
  return null;
}

export function nativeCliArgs(
  tool: string,
  args: Record<string, unknown> = {},
): string[] {
  const out: string[] = [];
  switch (tool) {
    case "vibecap_capture":
    case "vibecap_snapshot":
    case "vibecap_get_live_frame":
      out.push("--screenshot");
      break;
    case "vibecap_record_start":
      out.push("--record-start");
      break;
    case "vibecap_record_stop":
      out.push("--record-stop");
      break;
    case "vibecap_record_status":
      out.push("--record-status");
      break;
    case "vibecap_record_video": {
      const dur = Number(args.duration_secs ?? 0);
      if (Number.isFinite(dur) && dur > 0) {
        // Timed clip: start+we do not have a duration flag; use start and tell
        // the caller to stop, or shell a timed ffmpeg. Prefer start.
        out.push("--record-start");
      } else {
        out.push("--record-start");
      }
      break;
    }
    default:
      out.push("--screenshot");
  }
  if (typeof args.output_dir === "string" && args.output_dir.trim()) {
    out.push("--output-dir", args.output_dir.trim());
  }
  if (typeof args.display === "string" && args.display.trim()) {
    out.push("--display", args.display.trim());
  }
  const window =
    (typeof args.window === "string" && args.window.trim()) ||
    (typeof args.app_name === "string" && args.app_name.trim()) ||
    "";
  if (window) out.push("--window", window);
  if (args.gif === true) out.push("--gif");
  return out;
}

export type NativeCaptureResult = {
  ok: boolean;
  tool: string;
  stdout: string;
  stderr: string;
  path: string | null;
  backend: "vibecap-cli";
  hint: string;
};

function parsePath(stdout: string): string | null {
  const lines = stdout
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  for (const line of lines) {
    const m = line.match(/(\/\S+\.(?:jpg|jpeg|mp4|gif))/i);
    if (m) return m[1];
    if (line.startsWith("/") && !line.includes(" ")) return line;
  }
  const mp4 = stdout.match(/mp4=(\/\S+\.mp4)/);
  return mp4 ? mp4[1] : null;
}

export function runNativeCapture(
  tool: string,
  args: Record<string, unknown> = {},
): NativeCaptureResult {
  const bin = findVibecapBin();
  if (!bin) {
    return {
      ok: false,
      tool,
      stdout: "",
      stderr: "vibecap binary not found. cargo build --release, or set VIBECAP_BIN.",
      path: null,
      backend: "vibecap-cli",
      hint: "CLI and web studio share this capturer. Build the native binary first.",
    };
  }
  const cli = nativeCliArgs(tool, args);
  const ran = spawnSync(bin, cli, { encoding: "utf8", timeout: 60_000 });
  const stdout = (ran.stdout ?? "").trim();
  const stderr = (ran.stderr ?? "").trim();
  return {
    ok: ran.status === 0,
    tool,
    stdout,
    stderr,
    path: parsePath(stdout),
    backend: "vibecap-cli",
    hint: "File is on disk (output_dir). Not the demo shutter. GET /api/agent/still is only for pack JPEGs.",
  };
}
