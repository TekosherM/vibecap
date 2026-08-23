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
    summary: "Start recording. No duration. Keep the camera on until the UI settles.",
  },
  {
    name: "vibecap_subject_walk",
    summary: "Walk Lumen Cart: coupon 422, tax 500, pay 402. Does not stop REC.",
  },
  {
    name: "vibecap_snapshot",
    summary: "Still while video is rolling. Does not stop the recorder. Returns the JPEG.",
  },
  {
    name: "vibecap_record_stop",
    summary: "Stop. Clip + poster land in the pack / Media — never ~/Movies or ~/Vibecap.",
  },
  {
    name: "vibecap_capture",
    summary: "One still if you do not need motion. Returns the JPEG in the tool result.",
  },
] as const;

export const AGENT_HELP = `Vibecap — capture-only (most jobs)

This running studio IS the connector. There is no vibecap --mcp to attach.
POST /api/agent/call   GET /api/agent/help   GET /api/agent/hooks   GET /api/agent/media

0. GET /api/agent/hooks              # what's live, which medium, what to call next
1. vibecap_record_start              # no duration — keep rolling
2. vibecap_subject_walk              # coupon 422, tax 500, pay 402
3. vibecap_snapshot                  # failure frame; video stays up
4. vibecap_record_stop               # when results settle
5. vibecap_bug_pack                  # one JSON: stills + frontend + backend + db + logs

When to hook
  Pixels / layout / wrong UI copy     still or video (JPEG / WebM)
  Uncaught / console.warn             ingest_frontend  → JSON (console + DOM)
  4xx/5xx / stack / compose           ingest_backend   → JSON (HTTP + shell)
  Wrong stock / price / row           ingest_database  → JSON
  Timeline of the session             ingest_logs      → JSON
  Don't want to choose                bug_pack

JSON hooks bind to the instrumented subject (Lumen Cart).
Screen / camera only change the visual medium — not the DOM / console / HTTP taps.

Output lives in the pack (Download JSON) and Media (Download JPEG / WebM).
JPEG files: GET /api/agent/still/{id}.jpg
Do not look in ~/Movies/Vibecap or ~/Vibecap.

Inbox / annotate / poll loops are optional. Skip them unless you need a human.
`;

export const AGENT_TOOLS = [
  {
    name: "vibecap_hooks",
    summary: "What's live, when to collect it, which medium. Call this first.",
  },
  ...CAPTURE_ONLY_TOOLS,
  {
    name: "vibecap_record_video",
    summary: "Record. duration_secs optional — omit it to start unbounded, then record_stop.",
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
    name: "vibecap_bug_pack",
    summary: "One JSON pack. Stills are inline — nothing to copy out of a home directory.",
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
