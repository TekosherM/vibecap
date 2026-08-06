# Feedback loop — 30 use cases

Human-in-the-loop via **Agent Inbox** (app) + MCP (`request` / `get` / `list` / `cancel`).

**Protocol (always):**

1. `vibecap_request_feedback(...)` → `request_id`
2. Poll `vibecap_get_feedback(request_id)` every 2–5s until `status≠pending`
3. Answers are **not** pushed into chat — file-based only
4. Empty text + annotated path = **open the PNG with vision**
5. Lost id? `vibecap_list_feedback`. Stale? `vibecap_cancel_feedback`

| # | Use case | Media | Prefer | Options example | Priority |
| ---: | :--- | :--- | :--- | :--- | :--- |
| 1 | Approve README / docs screenshot | image | any | approve / crop-more / blur-PII | normal |
| 2 | Mark the bug (where is the glitch?) | image | annotate | — | high |
| 3 | Crop / blur PII before share | image | annotate | done / still-leaking | high |
| 4 | Animation / motion look right? | gif | any | ship / re-record | normal |
| 5 | Full flow / onboarding walkthrough | video | voice | pass / fix-step-N | normal |
| 6 | A/B design pick | 2 captures via `context` paths | choice | A / B / neither | normal |
| 7 | Modal / button copy correct? | image | text | approve / rewrite | normal |
| 8 | Contrast / a11y glance | image | text | ok / fail | normal |
| 9 | Dark mode visual QA | image | annotate | ok / fix-region | normal |
| 10 | Mobile simulator layout | image | annotate | ok / overflow | normal |
| 11 | Game sprite / hitbox timing | gif | annotate | ok / retarget | normal |
| 12 | Scroll jank / stutter | gif or video | any | smooth / janky | normal |
| 13 | Error state UI | image | text | clear / confusing | normal |
| 14 | Empty state UI | image | text | good / needs-CTA | normal |
| 15 | Loading / skeleton state | gif | choice | ok / too-long | low |
| 16 | “Is this the fix you meant?” | image before/after in context | choice | yes / no / close | high |
| 17 | Permission for destructive step | *none* | choice | allow / deny | high |
| 18 | Choose implementation approach | *none* | choice | option-A / option-B | normal |
| 19 | Voice walkthrough of UX pain | image or video | voice | — | normal |
| 20 | Spacing / alignment nit | image | annotate | — | low |
| 21 | Brand / marketing asset sign-off | image | choice | approve / reject | normal |
| 22 | Wrong window / multi-monitor | image | text | re-capture-left / ok | normal |
| 23 | i18n string overflow | image | annotate | fits / clips | normal |
| 24 | Live session: “what am I looking at?” | live frame path | text | continue / stop | high |
| 25 | Chart / data viz sanity | image | text | correct / wrong-scale | normal |
| 26 | Secret / token visible? | image | annotate | redacted / still-visible | high |
| 27 | Onboarding step N of M | image | choice | next / stuck | normal |
| 28 | Before vs after regression | two paths in `context` | choice | better / worse / same | normal |
| 29 | Agent stuck — unblock me | *none* | text or voice | — | high |
| 30 | Batch: pick which captures keep | list paths in `context` | text | keep-all / prune | low |

## Reply channels (human)

| Channel | When | Agent sees |
| :--- | :--- | :--- |
| **Choice chips** | `options` set | `choice: …` |
| **Text** | freeform | `text: "…"` |
| **Mark up** | image + prefer annotate | `annotated_media: …` (text may be empty) |
| **Voice** | prefer voice / complex UX | `voice_note: …` |
| **Dismiss** | not useful | `status=dismissed` |
| **Agent cancel** | stale | `status=cancelled` |

## Example calls

**Visual QA with chips:**

```json
{
  "media_path": "/Users/me/Movies/Vibecap/screenshot_….jpg",
  "question": "Clean enough for README hero?",
  "options": ["approve", "crop-more", "blur-PII"],
  "preferred_reply": "any",
  "agent_label": "codex",
  "priority": "normal"
}
```

**Destructive permission (no media):**

```json
{
  "question": "Delete all selected library items permanently?",
  "options": ["allow", "deny"],
  "preferred_reply": "choice",
  "priority": "high",
  "agent_label": "grok"
}
```

**Annotate-only bug:**

```json
{
  "media_path": "/…/screenshot_….jpg",
  "question": "Circle the misaligned button.",
  "preferred_reply": "annotate",
  "priority": "high",
  "context": "macOS 14, retina, main window only"
}
```
