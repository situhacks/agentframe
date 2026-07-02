# AgentFrame

AgentFrame is a file-native workspace harness designed to run inside AI coding agents like Claude Code, Cursor, VS Code, or Antigravity. The harness itself is completely domain-neutral. Currently, it supports marketing campaigns and project-management projects, utilizing modular domain packs that can be expanded in the future. The workflow runs through two `AGENTS.md` modes: **Operator** handles project execution, and **Builder** manages system architecture and templates.

<p align="center">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" /></a>
  <img alt="Works with" src="https://img.shields.io/badge/works%20with-claude%20code%20%7C%20codex%20%7C%20cursor%20%7C%20vscode%20%7C%20antigravity-blue?style=flat-square" />
  <img alt="Status" src="https://img.shields.io/badge/status-actively%20used-orange?style=flat-square" />
</p>

This is my workspace where I build and run my projects—longer-term projects, campaigns, and tools. Evolving this codebase is a direct reflection of the type of work that I do day-to-day. Evolving this workspace helps keep the agent focused, fast, and organized. It is free to fork, so feel free to take whatever is useful for your own setups.

---

## Table of contents

- [Quick start](#quick-start)
- [Why this exists](#why-this-exists)
- [Domains are packs](#domains-are-packs)
- [A real project, step by step](#a-real-project-step-by-step)
- [Key features](#key-features)
- [At a glance](#at-a-glance)
- [Connectors](#connectors)
- [Architecture](#architecture)
- [Repository structure](#repository-structure)
- [Preview server](#preview-server)
- [Auditability and state](#auditability-and-state)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [References and lineage](#references-and-lineage)
- [License](#license)
- [Contact](#contact)

---

## Quick start

1. **Clone the repository:**

   ```bash
   git clone https://github.com/situhacks/agentframe.git
   cd agentframe
   ```

2. **Open the folder in your coding agent** of choice (Claude Code, Codex, Cursor, VS Code, or Antigravity). If you are using Claude Code, it automatically loads `CLAUDE.md` which imports the active `AGENTS.md` persona, pinning the agent's behavior so it survives memory compaction.

3. **Configure environment variables.** Copy `.env.example` to `.env` and add optional keys for Gemini (used for Deep Research and image generation) and Composio (used for platform publishing). Both are optional; the system will run locally without them, but you will lose external API connections. If you intend to use Open Design for local visual rendering, run `corepack pnpm install` inside `system/skills/open-design/source/`.

4. **Scaffold a new project.** Instruct the agent to **"start a new marketing campaign"** or **"start a new project-management project"**. Under the hood, the agent runs `af new-project <slug> --domain <marketing|project-mgmt>` to read the domain pack and scaffold the directory structure.

### Mode swaps

AgentFrame uses two distinct `AGENTS.md` personas to establish clean boundaries:

- **Operator mode** runs active projects. It is locked to `workspace/projects/` so the agent cannot edit templates, rules, or core processes while executing a project.
- **Builder mode** modifies the system. It has write access to `system/` and `library/` for editing templates, adding process files, or authoring domain packs.

You can swap modes by telling the agent `swap to Builder` or `swap to Operator`. The agent will handle the file copy and log the transition to the local audit database automatically. The boundary is enforced, not just described: the swap refuses to overwrite a drifted `AGENTS.md`, and the CLI's operator verbs (`lock`, `publish`, `version`, `new-project`) refuse to run while the Builder persona is active.

### Syncing upstream changes

This system evolves as I run more projects. To pull the latest system updates without losing your local changes, swap to Builder and tell the agent to **"pull upstream updates"**. The sync skill fetches this repository, walks you through changes commit-by-commit, prompts you for how to resolve local customization conflicts, and applies the updates safely. All personal layers—including project workspaces, local logs, and audit databases—are gitignored and remain untouched.

[Back to top](#agentframe)

---

## Why this exists

I built the original AgentFrame system to run marketing campaigns because I realized that working on copy and strategy in raw chat windows meant losing context and having files drift over a long campaign. Keeping everything structured and anchored as files in a local workspace makes it much easier to coordinate work over long horizons.

Over time, I wanted to use this workspace and its skills for other tasks: making slide decks, writing emails, and archiving or tracking project details—all related to project management. At first, I jerry-rigged these skills and processes into the `open-flow` style to make it work. It worked fine for short-term tasks, but it wasn't scalable for long-term projects.

To solve this, I rebuilt AgentFrame to support structured projects and long-horizon work. The new architecture introduces context components—like global profiles for people and channels—along with a dedicated memory and consolidation system.

This design aligns with where the industry is moving, where specialized domain packs (like Claude for Financial Services or Claude for Small Business) are added on top of a core platform. AgentFrame applies that same modular pattern to local workspaces, but adds a robust context and memory layer to keep the workspace lightweight and cost-effective as it evolves over time.

The original marketing-only project is frozen at [agentframe-marketing](https://github.com/situhacks/agentframe-marketing). This repository is its multi-domain successor.

[Back to top](#agentframe)

---

## Domains are packs

Everything in AgentFrame is built around domain separation. 

The core runner (`system/af.py`) and the routing files (`AGENTS.md`) carry no domain-specific code. If you search the python scripts for marketing terms or publishing platforms, you will find nothing; the engine is completely blind to what the project is actually about. All domain logic is declared in `library/domains/{domain}/`.

These are not code-heavy plugins. A domain pack is primarily a collection of deliverable templates—markdown shapes and rules for the files the project produces—along with a small metadata descriptor. All generic skills like research, voice harvesting, and document generation live outside the pack, allowing any domain to use them.

A pack defines its scope through a few key files:

- **Deliverable templates** (`deliverables/`): The templates for the artifacts this domain generates.
- **A pack descriptor** (`pack.md`): Declares custom frontmatter fields, valid CLI verbs, and folder structures. The python runner parses this directly using standard library helpers.
- **A scaffold skeleton** (`skeleton.md`): Defines the initial structure of `project.md` when the project is created.
- **An optional rules module** (`rules.py`): Python validation rules loaded dynamically by `af doctor` (for example, verifying character limits or document groupings).
- **An optional routing fragment** (`production.md`): Domain-specific step instructions that the Operator mode loads on demand during execution.

Adding support for a new workflow only requires adding a folder under `library/domains/`; the Accept-Test for the design is that a new domain requires zero modifications to the core engine.

[Back to top](#agentframe)

---

## A real project, step by step

Here is what an end-to-end run looks like in each of the two domains that ship today—one operator, six steps, no handoffs.

### Project management — a project

The PM pack takes a charter, derives living project context files, and tracks deliverables ad-hoc.

<table>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-01-kickoff.png" alt="01 · Operator kickoff" /><br/>
<sub><b>01 · Operator kickoff</b> — Tell your agent <code>start a new project-management project</code>. The Operator scaffolds the workspace from the PM pack's skeleton, setting up folders for your inputs and outputs, and imports your project charter into the project's <code>sources/</code> directory.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-02-governance.png" alt="02 · Derive the governance docs" /><br/>
<sub><b>02 · Derive project context files</b> — From the charter, the agent parses the requirements and derives four living project context files under <code>knowledge/</code>: a RAID log (Risks, Assumptions, Issues, Dependencies), a stakeholder map, a decision log, and a workback schedule.</sub>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-03-living.png" alt="03 · Keep them living" /><br/>
<sub><b>03 · Maintain context & consolidate</b> — These are not static files. The RAID log moves on a weekly cadence, decisions append as they are made, and the schedule re-plans against deadlines. For long-horizon projects, you can trigger a consolidation pass (the "dream" workflow) to archive resolved items and prune active files, keeping them lean while retaining a complete history of facts and decisions.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-04-deliverables.png" alt="04 · Ad-hoc deliverables in your voice" /><br/>
<sub><b>04 · Ad-hoc deliverables in your voice</b> — Findings, recommendations, decks, and memos are generated from the generic deliverable shape. Drafts inherit your voice parameters from <code>library/context/operator/voice/</code>, and every revision is saved as a versioned snapshot (<code>-v{N}.md</code>) so you can track what changed and rollback if needed.</sub>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-05-deliver.png" alt="05 · Deliver" /><br/>
<sub><b>05 · Deliver</b> — Deliverables are checked against lock criteria, versioned, and delivered. The PM pack rejects publishing verbs, ensuring files remain local to your workspace.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/pm-walkthrough-06-retro.png" alt="06 · Retro" /><br/>
<sub><b>06 · Retro</b> — The project closes with a harvest pass. The agent reviews all your manual edits and template overrides, then proposes updates to your core library templates and voice example pairs so the system gets smarter with every run.</sub>
</td>
</tr>
</table>

### Marketing — a campaign

The marketing pack coordinates research, copy creation, image generation, and direct publishing.

<table>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-01-cmo-kickoff.png" alt="01 · Operator kickoff" /><br/>
<sub><b>01 · Operator kickoff</b> — Tell your agent <code>start a new marketing campaign</code>. Operator reads your profile, scaffolds the campaign from the marketing pack's skeleton, and calls Composio for workplace context — recent emails, meeting notes, doc activity — so the work starts from what you care about this week, not a cold prompt.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-02-research.png" alt="02 · Gemini Deep Research" /><br/>
<sub><b>02 · Deep research, your method</b> — Pick Gemini Deep Research over the API, or the native benchmark-lifted <code>deep-research</code> skill on the agent's own search and fetch tools. Either way, a structured brief and competitor analysis artifact lands at <code>phase-1-research/research-artifact-v{N}.md</code>.</sub>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-03-post-copy.png" alt="03 · Copy in your voice" /><br/>
<sub><b>03 · Copy in your voice</b> — Drafts inherit your voice system from <code>library/context/operator/voice/</code>, then run through the humanizer before lock. Every revision snapshots as its own <code>-v{N}.md</code>, so you can roll back, compare, or read why the copy changed.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-04-image-production.png" alt="04 · Media, your pick" /><br/>
<sub><b>04 · Media, your pick</b> — HTML render in your agent for slide-shaped visuals, Gemini Nano Banana for raster variants, Open Design for higher-fidelity decks and carousels, or HyperFrames for HTML-to-video. For Open Design, AgentFrame stages the project for you — design language, mode, and first prompt already loaded.</sub>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-05-published.png" alt="05 · Publish via Composio" /><br/>
<sub><b>05 · Publish via Composio</b> — Composio's connector to LinkedIn (or X, Instagram, TikTok) sends or schedules the post; <code>af publish</code> records the delivered state and live URL back into the post folder.</sub>
</td>
<td width="50%" valign="top">
<img src=".github/readme-assets/walkthrough-06-retro.png" alt="06 · Retro" /><br/>
<sub><b>06 · Retro</b> — The agent proposes patches to your voice, templates, processes, and skills based on what actually happened. You approve or reject each one; the library evolves.</sub>
</td>
</tr>
</table>

[Back to top](#agentframe)

---

## Key features

### A deterministic harness

**The agent owns creative judgment; a Python CLI owns state tracking and bookkeeping.**

LLMs are reliable at drafting and unreliable at exact procedure following — mine skipped its versioning and lock steps three times in a single evening. AgentFrame therefore does not ask the model to remember state transitions. Commands in `system/af.py` (`new-project`, `version`, `lock`, `doctor`) run the file modifications, frontmatter updates, and audit logs atomically, then print the checklists the agent needs to follow next. All resulting state lives in markdown files rather than a chat session, so the agent can read the project directory and resume work after a provider switch or months away.

---

### A voice system built from your own writing

**Voice is defined by annotated example pairs from your own writing, not lists of banned words.**

AgentFrame compiles your actual writing into example pairs: a generic version, your rewritten version, and an annotation naming the difference. When the agent drafts, it loads these pairs into context as stylistic anchors — substance first, then a dedicated style pass that applies the cadences and markers found in the pairs. Negative rule lists ("don't use the word leverage") are avoided because agents stop following them within a few revisions.

---

### A structured project knowledge substrate

**Raw inputs and distilled working files are kept separate, and the working set is periodically consolidated.**

Immutable inputs (transcripts, briefings, raw docs) live in `sources/`; agent-distilled living files (logs and plans) live in `knowledge/`. A user-triggered consolidation pass (the "dream" workflow) archives resolved risks, closed decisions, and completed milestones into compressed monthly archives, keeping the active files small. The same rule applies to the system itself: `AGENTS.md` is the only file loaded by default, and domain packs, flows, processes, and skills load only when the current step requires them.

---

### Deep research built on an open benchmark

**Two research methods: Gemini Deep Research over the API, or a native `deep-research` skill that runs on the agent's own tools.**

The native method uses web search, page fetch, and file I/O only, so it has no research-API dependency and no per-run cost beyond session tokens. Its research loop is adopted from the top open-source entries on [DeepResearch Bench](https://huggingface.co/spaces/muset-ai/DeepResearch-Bench-Leaderboard): the specialist-role architecture from [LunonAI/lunon-deep-research](https://github.com/LunonAI/lunon-deep-research) (an adaptation of NVIDIA's AI-Q blueprint, the benchmark's top entry), the synthesis-gate checklist from [deer-flow](https://github.com/bytedance/deer-flow), and the compression pattern from [open_deep_research](https://github.com/langchain-ai/open_deep_research). Each adopted element is pinned to an upstream commit SHA in [`PROVENANCE.md`](system/skills/deep-research/PROVENANCE.md) so the skill can be updated by diffing against the pin. Benchmark-judge tuning and multi-model ensembles were not adopted, and query budgets are reduced to three operator-sized tiers (`quick` / `standard` / `deep`).

---

### A learning loop

**Every project closeout proposes reviewed updates to templates, processes, and voice.**

At closeout, two system skills analyze the project's version history and your manual edits: one extracts new voice example pairs from your edits to the agent's drafts, the other identifies template and process gaps from how the workflow actually ran. Both propose their updates as git-patch suggestions for you to approve or reject.

[Back to top](#agentframe)

---

## At a glance

The system is composed of a few simple layers: 2 domain packs, 5 shared deliverables, 18 process files, 18 skill bundles, 4 flows, a reusable asset library, a two-mode persona configuration, a deterministic CLI, a local preview server, and a two-layer audit trail.

Everything in `library/` and `system/skills/` is plain text or standard Python, meant to be modified. You set your voice and positioning once under `library/context/operator/` (generated on first run from the canonical shapes in `library/context/operator-schema/`) and they are reused automatically across all projects.

### Domain packs

| Pack | What it ships |
|---|---|
| `library/domains/marketing/` | Campaigns that ship posts. Deliverables: research, business brief, campaign brief, campaign architecture, slide-copy, body-copy, post-final. The `publish` verb and the post-FINAL assembly live here. |
| `library/domains/project-mgmt/` | Consulting / PM projects. Opt into `project-mgmt-open-flow` and a charter goes in, then four living governance docs are derived from it: RAID log, stakeholder map, decision log, workback schedule. One-off PM deliverables run on plain `open-flow` with no governance overhead. No posts, no publish. |
| `library/domains/TBD/` | The harness is domain-neutral. More domain packs can be added at any time to support any kind of workflow. |

### Flows

`open-flow` is the default across every domain. Add or edit any flow under `library/process/flows/`.

| Flow | Purpose |
|---|---|
| `open-flow` (default) | Build-as-you-go. The agent proposes a plan scaled to the objective; you narrow it and set the tempo. The default for every domain. |
| `marketing-solo-flow` | Marketing, opt-in: a lean fixed phase ladder with one accountable owner. |
| `marketing-standard-flow` | Marketing, opt-in: a fuller campaign with stakeholder review gates. |
| `project-mgmt-open-flow` | Project-mgmt, opt-in: open-flow plus a governance kickoff that derives the charter-driven governance docs. One-off PM deliverables stay on plain `open-flow`. |

### Shared deliverables

Cross-domain templates under `library/deliverables/`.

| Template | Output |
|---|---|
| `design-language` | Visual and style direction |
| `image-prompts` | Per-asset generation prompts — treatment block + per-asset deltas |
| `video-spec` | Video concept, scenes, and production plan |
| `closeout-retro` | Project-level learnings and improvements |
| `system-retro` | System-level process and architecture improvements |
| `_meta` | The generic deliverable shape every template inherits |

### Process files

Process files load on demand — only when the workflow they describe is in play.

| Process | Purpose |
|---|---|
| `flow-authoring` | Designing or evolving flows |
| `process-authoring` | Designing or evolving process files |
| `research-and-signals` | Shared kickoff research: workspace context, live MCP scan, research-method offer |
| `deliverable-versioning` | Surgical-vs-replacement judgment for `*-v{N}.md`; bumps run through `af version` |
| `lock-event` | Lock trigger and judgment gates; mechanics run through the `af` CLI |
| `project-frontmatter` | Frontmatter schema and state handling |
| `knowledge-base` | Project knowledge substrate schema, storage principles, and ingest workflow |
| `operator-context-setup` | First-run generation of your operator context from the canonical schemas |
| `voice-setup` | Build your voice system from your own writing |
| `voice-mini-retro` | Lock-time gate that routes your edit-diffs to the voice-harvest skill |
| `humanizer-integration` | The humanization pass |
| `image-production` | Image-generation workflow |
| `video-production` | Video workflow from spec to renders |
| `deck-production` | Deck path selection (PPTX skill, PPT Master, Open Design) |
| `preview-server` | When and how to use the local preview hub |
| `composio-notes` | Connector usage notes and caveats |
| `browser-fallback` | Browser-automation fallback strategy |
| `substack-distribution-notes` | Substack publishing conventions |

### Skills

My current production stack — swap any one for a sharper tool without touching templates or processes.

| Skill | Source |
|---|---|
| `agentframe-structure` | Project skill — navigates and understands the overall project architecture and state |
| `deliverable-scaffolding` | Project skill — scaffolds new deliverables from templates with correct frontmatter |
| `system-improvement` | Project skill — applies patches and improvements to the system itself |
| `upstream-sync` | Project skill — pulls upstream updates into your copy, commit by commit with approval |
| `voice-harvest` | Project skill — mines finished work and edit-diffs into voice example pairs |
| `deliverable-harvest` | Project skill — mines template and process patches from finished projects |
| `project-consolidate` | Project skill — consolidates and prunes project knowledge and history (dream workflow) |
| `deep-research` | Project skill — native multi-role deep research on the agent's own tools; loop lifted from [DeepResearch Bench](https://huggingface.co/spaces/muset-ai/DeepResearch-Bench-Leaderboard) leaders, provenance pinned in [`PROVENANCE.md`](system/skills/deep-research/PROVENANCE.md) |
| `docx` | Project skill — generates Word documents from markdown deliverables |
| `pptx` | Project skill — generates PowerPoint presentations from markdown deliverables |
| `humanizer` | Vendored from [blader/humanizer](https://github.com/blader/humanizer) |
| `hyperframes` | Vendored from [heygen-com/hyperframes](https://github.com/heygen-com/hyperframes) |
| `hyperframes-cli` | Vendored from [heygen-com/hyperframes](https://github.com/heygen-com/hyperframes) |
| `gsap` | Vendored animation skill for HyperFrames workflows |
| `ppt-master` | Vendored from [hugohe3/ppt-master](https://github.com/hugohe3/ppt-master) — native-editable deck generation via SVG→PPTX |
| `extract-design` | Thin skill over [Manavarya09/design-extract](https://github.com/Manavarya09/design-extract); CLI runs via `npx designlang` |
| `open-design` | Vendored local-first runtime from [nexu-io/open-design](https://github.com/nexu-io/open-design) for image/deck production |
| `browser-harness` | Vendored CDP-driven browser workflows via Edge |

### Everything else in the box

- Two-mode routing via `AGENTS.operator.md` and `AGENTS.builder.md`.
- The deterministic CLI at `system/af.py` — `new-project`, `lock`, `version`, `doctor`, plus the domain-dispatched `publish`. The buttons do the bookkeeping atomically and write the paper trail; the agent keeps the judgment.
- Project state and outputs as markdown under `workspace/projects/`.
- A reusable asset library at `library/assets/` — a flat logo inventory plus ppt-master deck-template packages, shared across every deck the projects produce.
- A two-layer audit trail: `activity.md` per project plus an append-only SQLite DB at `system/audit/agentframe.db`.
- A local preview server at `system/server/` for HTML, image, video, PDF, PPTX, and DOCX.
- A browser harness at `system/browser/` for CDP-driven sessions, with documented fallbacks when a workflow needs a hand.

[Back to top](#agentframe)

---

## Connectors

AgentFrame integrates with external services to support research, media generation, and publishing. All integrations are optional, and the system runs locally without them.

### Gemini Deep Research

- Generates detailed research briefs, competitor analysis, and signal maps at project kickoff.
- Outputs are saved directly to `phase-1-research/` as structured markdown.
- Uses `GEMINI_API_KEY` (which can be obtained through a standard Google AI Studio developer account).
- No key? The native `deep-research` skill covers the same phase on the agent's own tools — see [Deep research built on an open benchmark](#deep-research-built-on-an-open-benchmark).

### Gemini Image Generation (Nano & Pro)

- Used to generate quick layout variants or high-fidelity images with inline text.
- Integrated through the local preview hub and runs using your shared `GEMINI_API_KEY`.

### Composio

- Acts as a unified MCP gateway to connect your agent to tools like Google Workspace, Slack, LinkedIn, or YouTube.
- Collects live context signals for project kickoff research and publishes final content directly to social channels.

### Open Design

- A bundled local-first design runtime at `system/skills/open-design/source/` used to compile SVGs, slides, and carousels.
- Requires Node 24 and local dependencies (`corepack pnpm install`).

### `.env` shape

```bash
GEMINI_API_KEY=
COMPOSIO_API_KEY=
COMPOSIO_MCP_URL=https://connect.composio.dev/mcp
```

Your coding agent provides the LLM. These keys power the non-LLM tools — research, image generation, and publishing.

[Back to top](#agentframe)

---

## Architecture

### Mode boundary

```text
                                  ┌─── owns ──▶  workspace/projects
                  ┌──── Operator ─────┤
                  │              └─── reads ─▶  system + library
   Operator ──────┤
                  │              ┌─── owns ──▶  system + library
                  └─── Builder ──┤
                                 └─── reads ──▶  workspace/projects

   ── swap AGENTS.md to flip between modes ──
```

### The load path — one domain parameter, one resolution point

```text
   CLAUDE.md  ── imports ──▶  AGENTS.md   (the active persona; domain-agnostic)
                                  │
                                  ▼
                          project.md  →  { domain, flow, deliverables[] }
                                  │
              ┌───────────────────┼────────────────────────────┐
              ▼                   ▼                             ▼
   library/domains/{domain}/   library/process/flows/{flow}.md   library/process/* (shared)
        (the pack)              (domain-agnostic; open-flow)      (lock, versioning, frontmatter)
              │
              ▼
   draft X → pack's template ▸ shared template ▸ generic _meta shape
```

`af.py` reads `project.md` → `domain` → loads the pack and dispatches. The persona, the flows, and the shared processes name no domain; `{domain}` is the single point where the work is parameterized.

[Back to top](#agentframe)

---

## Repository structure

```text
agentframe/
├── CLAUDE.md                 # thin shim → imports AGENTS.md (pins the persona)
├── AGENTS.md                 # active persona (Operator | Builder)
├── AGENTS.operator.md
├── AGENTS.builder.md
├── README.md
├── .env.example
├── library/
│   ├── domains/
│   │   ├── marketing/        # pack.md, skeleton.md, rules.py, production.md, deliverables/
│   │   └── project-mgmt/     # pack.md, skeleton.md, deliverables/
│   ├── deliverables/         # shared cross-domain deliverables + _meta shape
│   ├── process/
│   │   └── flows/
│   ├── context/
│   │   ├── _meta/            # channel and person profile schemas
│   │   └── operator-schema/  # canonical shapes; setup generates operator/ context from these
│   └── assets/
│       ├── logos/            # flat brand-mark inventory (filename is the index)
│       └── deck-templates/   # reusable ppt-master template packages
├── system/
│   ├── af.py                 # the generic plugin-host spine
│   ├── skills/
│   ├── server/
│   ├── audit/
│   ├── browser/              # CDP-driven browser runtime
│   ├── research/             # Gemini deep-research runtime
│   └── builder-backlog.md
└── workspace/
    └── projects/           # your projects live here (gitignored)
```

[Back to top](#agentframe)

---

## Preview server

<details>
<summary>Show preview server details</summary>

- Local preview hub at `system/server/`.
- Previews HTML, images, video, PDF, PPTX, and DOCX.
- Folder-tree navigation with hide rules to keep noise down.
- Run with `py -3 system/server/run.py`.

</details>

## Auditability and state

<details>
<summary>Show auditability details</summary>

- Project layer: `activity.md` in each project for the human-readable timeline — state transitions through `system/af.py` write it automatically.
- System layer: append-only SQLite audit DB at `system/audit/agentframe.db` (writer: `system/audit/writer.py`).
- Books check: `python system/af.py doctor [project]` verifies schema, head pointers, and per-domain rules; it surfaces drift and never auto-fixes.

</details>

[Back to top](#agentframe)

---

## Roadmap

Small, demand-driven additions. Nothing here is a rewrite.

- [ ] Embedded / RAG indexing over the markdown corpus, so retrieval scales as the library grows past what fits in context.
- [ ] Scheduled runs — cron-triggered jobs for recurring work, like a weekly research pull or a digest.
- [ ] Agent-to-agent communication over an API, so projects and agents can hand work to each other.
- [x] Per-project knowledge substrate (`sources/` immutable + agent-owned `knowledge/`) with a user-triggered consolidation pass.
- [x] Cross-project context entities (channels, people) referenced by slug.
- [ ] More domain packs as the work demands them.
- [ ] Preview server v2: better search, nested live reload, stronger video UX.

[Back to top](#agentframe)

---

## Contributing

- PRs for templates, processes, packs, and skills are welcome.
- Open an issue first for major architecture changes.

## References and lineage

- [agentframe-marketing](https://github.com/situhacks/agentframe-marketing) — the frozen marketing-only predecessor this grew out of
- [Composio](https://composio.dev)
- [heygen-com/hyperframes](https://github.com/heygen-com/hyperframes)
- [nexu-io/open-design](https://github.com/nexu-io/open-design) (Apache-2.0, vendored under `system/skills/open-design/source/`)
- [hugohe3/ppt-master](https://github.com/hugohe3/ppt-master) (MIT, vendored under `system/skills/ppt-master/`)
- [Manavarya09/design-extract](https://github.com/Manavarya09/design-extract) (MIT, thin skill; CLI runs via `npx designlang`)
- [GreenSock GSAP](https://greensock.com/gsap/)
- [Google AI Studio / Gemini](https://aistudio.google.com)
- [blader/humanizer](https://github.com/blader/humanizer)
- [DeepResearch Bench leaderboard](https://huggingface.co/spaces/muset-ai/DeepResearch-Bench-Leaderboard) — the open benchmark the native `deep-research` skill's architecture was selected from
- [LunonAI/lunon-deep-research](https://github.com/LunonAI/lunon-deep-research) (MIT, role prompts and architect pattern lifted into `system/skills/deep-research/`; lineage: NVIDIA's AI-Q blueprint, the benchmark #1)
- [bytedance/deer-flow](https://github.com/bytedance/deer-flow) (MIT, synthesis-gate checklist)
- [langchain-ai/open_deep_research](https://github.com/langchain-ai/open_deep_research) (MIT, compression-at-boundary pattern)

## License

MIT. See [`LICENSE`](LICENSE).

## Contact

Built by Brandon Situ over many weekends — and likely many more.

- LinkedIn: [linkedin.com/in/brandonsitu](https://www.linkedin.com/in/brandonsitu/)
- Email: brandonzsitu@gmail.com

[Back to top](#agentframe)
