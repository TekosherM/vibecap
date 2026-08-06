# Vibecap marketing plan — X · Product Hunt · MCP market

**Positioning (one line):**  
*The visual sidecar for AI coding agents — capture, annotate, and human-in-the-loop feedback for native apps, simulators, and motion — via MCP.*

**Not:** “another Loom / CleanShot.”  
**Is:** Playwright captures the web; **Vibecap captures everything agents can’t see** (desktop, simulators, games) and lets humans mark it up for the agent.

**Primary ICP:** Cursor / Claude Code / Codex power users building UI (native, Electron, mobile sims, games).  
**Secondary:** OSS maintainers, indie hackers shipping AI tooling, MCP list curators.

**Proof assets (before launch):**
- GitHub: https://github.com/TekosherM/vibecap  
- Release binaries (macOS/Win/Linux)  
- 15–45s demo GIF: agent asks → Inbox → mark-up → poll answer  
- 1 annotated screenshot “before/after agent fix”  
- `skills/vibecap` + 30 feedback use cases (`docs/FEEDBACK_USE_CASES.md`)  
- One-liner MCP install snippet  

---

## Phase 0 — Prep (1–2 weeks before PH)

| Work | Why |
|---|---|
| Tag **v0.1.x** (or v0.2) with polished release notes + binaries | PH hunters need a “download today” path |
| Landing-ready README hero: GIF + vs browser MCPs table | PH / MCP dirs scrape GitHub |
| `llms.txt` or short `/docs/MCP.md` install block | Copy-paste into Cursor/Claude config |
| 3–5 seed users (DMs) for PH upvotes + first comments | Day-1 social proof |
| Brand kit: icon, orange-on-dark stills, #Vibecap hashtag | Consistency across X/PH/MCP |
| Clarify macOS as **best path**; Win/Linux “works” | Avoid 1-star “broken on my Linux” on launch day |

**Core message house (reuse everywhere):**

1. **Agents are blind outside the browser** → Vibecap is eyes + ears for native UI.  
2. **Human-in-the-loop Inbox** → agent asks, you draw, agent polls.  
3. **One binary, MCP + GUI** → not a SaaS, not a cloud recorder.  
4. **Budget-aware live inspection** → agents don’t burn tokens forever.  
5. **Open source (MIT/Apache)** → forkable, auditable, no lock-in.

---

## 1. X.com (Twitter) — always-on engine

### Account setup
- Handle: `@vibecap` or your name + pin **demo GIF**  
- Bio: `Screen capture + HITL feedback for AI agents · MCP · OSS · macOS/Win/Linux`  
- Link: GitHub (or short link with UTM `?utm_source=x`)  
- Pin: 30s demo + install one-liner  

### Content pillars (4 types, ~5 posts/week)

| Pillar | % | Examples |
|---|---|---|
| **Demo / show** | 40% | GIF of agent inbox, mark-up, live inspection, simulator capture |
| **Contrast** | 20% | “Browser MCP vs Vibecap” table as image; “Playwright can’t see Simulator” |
| **Builder log** | 20% | Ship notes, “we fixed record-until-focus,” feedback use cases |
| **Social / ask** | 20% | “What should agents be able to *see*?” polls; RT user clips |

### Cadence (sustainable)

- **Daily (optional, 2 weeks pre-PH):** 1 short post or reply in AI-dev threads  
- **Steady state:** 3–5 posts/week + heavy **replies** in Cursor/Claude/MCP threads (replies > original posts for distribution)  
- **Launch week:** 1 thread morning, 1 mid-day demo, 1 evening “how to install”  

### High-performing post formats (copy-ready)

**A. Problem → product (tweet)**  
> Your agent can click a web button.  
> It still can’t see your iOS Simulator, egui app, or game canvas.  
>  
> Vibecap = MCP screen capture + human mark-up inbox for coding agents.  
> OSS · one Rust binary  
> github.com/TekosherM/vibecap  

**B. Thread (launch / PH day)**  
1. Hook + GIF  
2. Why browser tools fail on native  
3. MCP tools list (12)  
4. Feedback loop (poll, not push)  
5. Install snippet  
6. Ask for stars / PH upvote  

**C. “Use case of the week”**  
Pick from `docs/FEEDBACK_USE_CASES.md` — e.g. “Agent asks: circle the misaligned button” → annotated PNG reply.

### Who to engage (not spam)
- Cursor / Claude Code / MCP launch posts  
- Build-in-public founders shipping desktop/mobile  
- Lists: awesome-mcp-servers maintainers, mcp.so curators  
- Reply with **value** (GIF or one install line), not “check my tool”

### Metrics that matter on X
- Stars from X traffic (GitHub Insights)  
- Demo GIF completion / quote-tweets  
- Replies from ICP accounts (better than vanity impressions)

### Paid (optional, small)
- $50–150 boost on the **launch thread only** if organic >50 likes; target Interests: AI, Programming. Skip if organic is dead — fix creative first.

---

## 2. Product Hunt — launch event

### Goal
Top 5 **Developer Tools** day / top 20 overall is realistic for a polished niche OSS tool; #1 is lottery. Optimize for **quality hunters + comments + GitHub stars**, not rank anxiety.

### Timing
- **Tue–Thu**, hunter timezone-friendly (US morning)  
- Avoid major AI model launch days  
- Ship a **named release** the same morning (v0.2 “Agent Inbox”)

### Listing copy

**Tagline (≤60 chars):**  
`MCP screen capture & human feedback for AI agents`

**Description structure:**
1. Problem (agents blind outside DOM)  
2. What Vibecap is (native app + MCP)  
3. 3 bullets: capture · annotate/HITL · budget live inspection  
4. Who for (Cursor/Claude/Codex + native/mobile/game)  
5. OSS + platforms  
6. Link to GitHub + skill  

**First comment (you, minute 0):**  
Story: why you built it (vibe-coding native UI, Playwright not enough) → how feedback loop works → honest limitations (macOS best, poll not push) → ask for use-case ideas.

### Gallery (order)
1. Hero GIF (full loop)  
2. vs browser MCP table  
3. Agent Inbox UI  
4. Annotation studio  
5. MCP config snippet  
6. Architecture one-liner  

### Maker strategy (24h)

| Window | Action |
|---|---|
| T-7d | Soft X teaser “shipping to PH Tuesday” |
| T-2d | DM 10–20 makers/friends for **first hour** support |
| T-0 | Post PH link on X thread + relevant Discords (Cursor, Claude, MCP) — **no vote-begging** |
| +2h | Answer every PH comment within 30 min |
| +8h | Mid-day “install in 60s” X post with PH link |
| +24h | Thank-you post + ship list of requested features |

### What to avoid on PH
- Fake urgency, vote farming groups  
- Overclaiming “cross-platform perfect” if macOS is the real demo  
- Competing only on “screen recorder” — **lead with MCP + agents**

### PH success metrics
- GitHub stars (+100–300 in 48h is a solid OSS outcome)  
- Quality comments from agent users  
- MCP directory submissions that cite PH  

---

## 3. MCP market / directories — discovery moat

Directories compound longer than a PH spike. Treat them as **SEO for agents**.

### Where to list (priority order)

| Destination | Action |
|---|---|
| **mcp.so** | Submit with tags: desktop, capture, agents, vision, HITL |
| **mcpmarket.com** | Curated listing; short demo + install JSON |
| **glama.ai/mcp** | Server page + tool inventory |
| **awesome-mcp-servers** (punkpeye / wong2 etc.) | PR with one-line description + category (Desktop / Browser & Automation / Developer tools) |
| **vibehackers.io / mcpservers.org** | Submit if forms exist |
| **Official / community MCP lists** | PR when ready for scrutiny |
| **Cursor / Claude forums & Discord** | “MCP of the week” style post with config |
| **Smithery / PulseMCP / similar** | If they accept local stdio servers |

### Listing package (reuse)

**Name:** Vibecap  
**Category:** Developer tools · Desktop · Vision · Human-in-the-loop  
**One-liner:** Native screen capture, recording, and human annotation inbox for AI coding agents over MCP.  
**Install:**

```json
{
  "mcpServers": {
    "vibecap": {
      "command": "vibecap",
      "args": ["--mcp"]
    }
  }
}
```

**Tools (scannable):** capture · record_video · export_gif · live inspection · budget · request/get/list/cancel feedback  

**Differentiators for listings:**
- Not browser-only  
- Human feedback loop with annotate/voice/choices  
- Cost controls (`set_budget` / tiers)  
- Single Rust binary, OSS  

### SEO / discoverability
- README H1 + first paragraph include: `MCP`, `Cursor`, `Claude`, `screen capture`, `human-in-the-loop`  
- Topics on GitHub: `mcp`, `cursor`, `claude-code`, `screen-capture`, `rust`  
- Skill file discoverable under `.agents/skills` and `skills/`  

### Ongoing MCP marketing
- Monthly: “New feedback use cases” post → update directory descriptions  
- When Claude/Cursor ships MCP UX changes → short “still works / updated config” note  
- Cross-link: every MCP listing → GitHub → PH (once) → demo GIF  

---

## 90-day calendar (integrated)

### Weeks 1–2 — Foundation
- Polish README + demo GIF  
- List on **mcp.so + glama + awesome PR**  
- Start X: 3 posts + daily reply habit  
- Soft launch to 20 builders for feedback  

### Weeks 3–4 — Product Hunt week
- Mon: freeze features, cut release  
- Tue: **PH launch** + X thread  
- Wed–Thu: comment velocity, ship one “requested” micro-fix  
- Fri: recap thread + star count  

### Weeks 5–8 — Compound
- X use-case series (1×/week from the 30 use cases)  
- Guest reply threads under “best MCP servers” roundups  
- Short blog/dev.to: “Why agents need OS-level eyes”  
- Collect 3 user quotes for next PH/README  

### Weeks 9–12 — Expand
- Windows/Linux polish post if demos improve  
- Partner with 1–2 agent YouTubers (send binary + script)  
- Optional: second PH “launch” only if major v0.3 (live multi-agent, Windows audio, etc.) — otherwise skip  

---

## Channel roles (don’t confuse them)

```text
X.com        → attention + narrative (daily)
Product Hunt → concentrated launch spike (1 day + 1 week)
MCP market   → durable discovery for people already shopping MCPs
GitHub stars → trust signal all three feed into
```

**Funnel:**  
X/PH interest → GitHub star → `cargo install` / binary → MCP config → first `request_feedback` wow moment  

**Activation moment to optimize for:**  
Human answers with **annotation-only** and agent opens the PNG. Demo that loop until it’s muscle memory in every video.

---

## Messaging do’s / don’ts

| Do | Don’t |
|---|---|
| “Eyes for agents outside the browser” | “Best screen recorder 2026” |
| Show HITL inbox + poll loop | Promise push-into-chat magic |
| Honest: macOS primary | Fake multi-platform perfection |
| Compare to browser MCPs / Playwright | Compare only to Loom/OBS |
| OSS + single binary | “Platform” / SaaS vibes |

---

## Lightweight budget

| Item | Cost |
|---|---|
| Domain (optional vibecap.dev → GitHub) | ~$12/yr |
| PH optional hunter / maker club | $0–30 |
| X boost (launch only) | $0–150 |
| Design (icon polish if needed) | $0–200 |
| **Total** | **~$0–400** |

Time > money: 5 hrs/week on X replies + directory PRs beats ads.

---

## Success metrics (simple)

| Horizon | Target |
|---|---|
| 30 days | 200–500 GitHub stars, 5 MCP listings live |
| PH day | 50+ upvotes quality comments; 100+ stars in 48h |
| 90 days | 1k stars *or* clear ICP testimonials; weekly organic MCP installs from directories |

---

## Launch-week asset checklist

- [ ] 30–45s demo GIF (capture → inbox → mark-up → agent reads)  
- [ ] PH gallery 5–6 images  
- [ ] X pin post  
- [ ] MCP config snippet in README top  
- [ ] Release with binaries  
- [ ] Seed list of 20 people for hour-0  
- [ ] First comment drafted for PH  
- [ ] Submissions queued: mcp.so, glama, awesome-mcp PR  

---

## Bottom line

Sell **agent vision + human mark-up over MCP**, not “screen recording.” Use **X** for story, **Product Hunt** for a single sharp spike, and **MCP directories** for long-term “I need an MCP for X” traffic. Optimize every channel for the same activation: first successful `request_feedback` → annotated answer.

---

## Related docs

- [FEEDBACK_USE_CASES.md](FEEDBACK_USE_CASES.md) — 30 HITL use cases for content  
- [MCP.md](MCP.md) — tool reference for listings  
- [USAGE.md](USAGE.md) — install / workflows  
