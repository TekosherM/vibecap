export type Stage =
  | "shutter"
  | "sources"
  | "pack"
  | "media"
  | "still"
  | "inbox"
  | "agent"
  | "settings";

export type CaptureSource = "demo" | "screen" | "camera";

export type EvidenceSource = "frontend" | "backend" | "database" | "logs" | "capture";

export type SessionRow = {
  id: string;
  name: string;
  status: string;
  notes: string;
  created_at: string;
};

export type CaptureRow = {
  id: string;
  session_id: string;
  kind: string;
  label: string;
  mime: string;
  data_url: string | null;
  clip_url: string | null;
  duration_ms: number | null;
  created_at: string;
};

export type EvidenceRow = {
  id: string;
  session_id: string;
  source: string;
  kind: string;
  title: string;
  body: string;
  capture_id: string | null;
  created_at: string;
};

export type PackRow = {
  id: string;
  session_id: string;
  title: string;
  summary: string;
  payload: string;
  created_at: string;
};

export type InboxRow = {
  id: string;
  session_id: string | null;
  question: string;
  options: string;
  priority: string;
  agent_label: string;
  preferred: string;
  context: string;
  status: string;
  answer_text: string | null;
  answer_choice: string | null;
  media_id: string | null;
  created_at: string;
};

export type LogRow = {
  id: number;
  session_id: string | null;
  stream: string;
  level: string;
  message: string;
  meta: string;
  created_at: string;
};

export type CommandRow = {
  id: string;
  tool: string;
  args: string;
  status: string;
  result: string | null;
  created_at: string;
};

export type BudgetRow = {
  id: string;
  max_frames: number;
  max_mb: number;
  max_minutes: number;
  analysis_tier: string;
  frames_used: number;
  mb_used: string | number;
  minutes_used: string | number;
};

export type CatalogItem = {
  id: number;
  sku: string;
  name: string;
  price_cents: number;
  stock: number;
};

export type StudioStatusRow = {
  id: string;
  recording: boolean;
  inspecting: boolean;
  source: string;
  attached_at: string;
};

/** Capture-only recipe. Inbox is optional. */
export const CAPTURE_ONLY_TOOLS = [
  {
    name: "vibecap_record_start",
    summary: "Start unbounded record. Pass display/output_dir for the real screen (x11grab), not the demo shutter.",
  },
  {
    name: "vibecap_capture",
    summary: "Still. Pass display/window/output_dir to grab the target screen. Else shutter JPEG.",
  },
  {
    name: "vibecap_record_stop",
    summary: "Stop unbounded record. Native MP4 lands in output_dir; shutter path writes WebM to Media.",
  },
  {
    name: "vibecap_job",
    summary: "Lumen Cart evidence: record, walk checkout (3 stills), ingest FE/BE/DB/logs, stop, pack.",
  },
] as const;

export const AGENT_HELP = `Vibecap — capture-only

A. Signed-in desktop / Chrome flow (QuestOS, any site) — use the native capturer.
   CLI (works when MCP never attaches — Cursor / Grok Bot dynamic tools):
     vibecap record start --output-dir ./frames --display "$DISPLAY"
     vibecap --screenshot --output-dir ./frames
     vibecap record stop
   Same capturer over HTTP (no studio tab, no demo shutter):
     POST /api/agent/call {"tool":"vibecap_record_start","args":{"display":":0","output_dir":"./frames"}}
     POST /api/agent/call {"tool":"vibecap_capture","args":{"display":":0","output_dir":"./frames"}}
     POST /api/agent/call {"tool":"vibecap_record_stop"}
   Files land in output_dir. Default media dir: vibecap --paths

B. Lumen Cart evidence pack (this studio's demo subject):
     GET /api/agent/hooks
     POST /api/agent/call {"tool":"vibecap_job"}
     GET /api/agent/still/{id}.jpg
   Without display/window/output_dir, snapshot/capture is the shutter (demo pay screen).

Linux backend: ffmpeg x11grab. Need ffmpeg + DISPLAY. Window title via --window.
MCP: vibecap --mcp or ./scripts/vibecap-mcp.sh (see .cursor/mcp.json). If tools never appear, use A.

Inbox is optional.
`;

export const AGENT_TOOLS = [
  {
    name: "vibecap_hooks",
    summary: "What's live, when to collect it, which medium. Call this first.",
  },
  ...CAPTURE_ONLY_TOOLS,
  {
    name: "vibecap_snapshot",
    summary: "Still while video is rolling. Native if display/output_dir is set; else shutter JPEG.",
  },
  {
    name: "vibecap_record_video",
    summary: "Omit duration_secs to start unbounded (then record_stop). Set it for a short clip.",
  },
  {
    name: "vibecap_get_live_frame",
    summary: "Latest still JPEG in the response. No filesystem path.",
  },
  {
    name: "vibecap_start_live_inspection",
    summary: "Rolling stills while the camera stays on (interval_secs, default 3).",
  },
  {
    name: "vibecap_stop_live_inspection",
    summary: "Stop the rolling stills. Recording is unaffected.",
  },
  {
    name: "vibecap_subject_coupon",
    summary: "Apply LUMEN10. 422 coupon_expired. Does not stop REC.",
  },
  {
    name: "vibecap_subject_tax",
    summary: "Lookup ZIP 94107. Tax helper 500 at pricing.ts:88. Does not stop REC.",
  },
  {
    name: "vibecap_subject_pay",
    summary: "Pay. Tax throw + Stripe 402. Does not stop REC.",
  },
  {
    name: "vibecap_ingest_frontend",
    summary: "Collect DOM, console, and a viewport still from the subject.",
  },
  {
    name: "vibecap_ingest_backend",
    summary: "Collect terminal/shell output and HTTP traces.",
  },
  {
    name: "vibecap_ingest_database",
    summary: "Snapshot schema + sample rows from the connected database.",
  },
  {
    name: "vibecap_ingest_logs",
    summary: "Collect structured logs for the active session.",
  },
  {
    name: "vibecap_request_feedback",
    summary: "Ask the human a question in Agent Inbox (optional).",
  },
  {
    name: "vibecap_get_feedback",
    summary: "Poll an inbox request by id.",
  },
  {
    name: "vibecap_list_feedback",
    summary: "List inbox requests.",
  },
  {
    name: "vibecap_cancel_feedback",
    summary: "Abandon a pending inbox question.",
  },
  {
    name: "vibecap_get_spending",
    summary: "Read agent budget vs spend.",
  },
  {
    name: "vibecap_set_budget",
    summary: "Set analysis tier or capture caps.",
  },
] as const;

export type AgentToolName = (typeof AGENT_TOOLS)[number]["name"];
