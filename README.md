# AgentFrame

![AgentFrame — a file-native AI workspace for context, projects, and agents](.github/readme-assets/banner.svg)

AgentFrame is a file-native AI workspace that runs inside your coding agent — Claude Code, Codex, Cursor, VS Code, Antigravity, or whatever you already use. It keeps your projects, context, deliverables, decisions, and feedback loops as plain files the model can read, so the agent picks up real work exactly where it was left, and gets out of the way for the parts the model is already good at.

I'm not a software engineer. I built this because I run long projects with AI every day, and the workspace is what makes that work — every template, process, and rule in here has been used on real shipped work. It's free to fork; take whatever is useful for your own setup.

<p align="center">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-green.svg?style=flat-square" /></a>
  <img alt="Works with" src="https://img.shields.io/badge/works%20with-claude%20code%20%7C%20codex%20%7C%20cursor%20%7C%20vscode%20%7C%20antigravity-blue?style=flat-square" />
  <img alt="Status" src="https://img.shields.io/badge/status-actively%20used-orange?style=flat-square" />
</p>

## Table of contents

- [Why this exists](#why-this-exists)
- [Key features](#key-features)
- [Quick start](#quick-start)
- [What it can run](#what-it-can-run)
- [A real project, step by step](#a-real-project-step-by-step)
- [At a glance](#at-a-glance)
- [Architecture](#architecture)
- [The Workspace Dashboard](#the-workspace-dashboard)
- [Repository map](#repository-map)
- [Integrations](#integrations)
- [Auditability](#auditability)
- [Design constraints](#design-constraints)
- [Contributing](#contributing)
- [References and lineage](#references-and-lineage)
- [License](#license)
- [Contact](#contact)

## Why this exists

I built the first version of AgentFrame to run marketing campaigns, because working on copy and strategy in raw chat windows meant losing context every session — decisions drifted, files forked, and every new chat needed the same briefing. Moving the work into structured files fixed that, and the marketing system worked well enough that I shipped a whole campaign series through it.

Then I noticed what I was actually doing with it. I kept pointing the marketing system at work that had nothing to do with marketing — PowerPoint redesigns, document deliverables, little research jobs — by dressing them up as "campaigns" so the system would accept them. It worked, and that was the tell. Managing activity state, deliverable versions, and a production pipeline was never really marketing; the system had been doing context management the whole time, and marketing was just the first thing I pointed it at.

<!-- IMAGE STUB · slot: realization-hero · shows: the context-management core revealed beneath the marketing-specific shell — same visual family as the release post's image, campaign design language -->

So I rebuilt it around that. The core unit is now the **project** — the smallest useful denominator of knowledge work, flexible enough for anything you can throw at a frontier model, because underneath, all of it is context management. Marketing became one domain pack among several instead of the whole identity. Along the way the workspace picked up the features I'm proudest of: domain-agnostic projects, long-horizon memory that consolidates instead of forgetting, a voice system trained on my real edits, versioned deliverables, a career workspace, and more — the full tour is in [Key features](#key-features).

The original marketing-only system is frozen at [agentframe-marketing](https://github.com/situhacks/agentframe-marketing). This repository is its successor.

## Key features

AgentFrame is three layers on one file substrate: **prompt engineering** (reusable personas, templates, and process files — the disciplined version of what everyone was doing in 2023), **context engineering** (a file graph that decides what the model sees, which is where most of the value lives), and **loop engineering** (the exploratory edge — bounded autonomous runs, where the market is heading now). Any coding agent that reads files can drive all three.

<!-- IMAGE STUB · slot: three-layers · shows: prompt / context / loop engineering as one visual, context as the core layer -->

### Context engineering — a project never needs a re-briefing

**Full state reconstructs from disk.** After a memory compaction, a provider switch, or a month away, a fresh agent reads the project files and continues mid-stream — no "let me catch you up" paragraph, no re-uploading the brief.

**Raw sources and distilled knowledge are kept apart.** Transcripts and briefs land in `sources/` and never get edited; the agent maintains its working understanding in `knowledge/`. When a long project accumulates resolved detail, a consolidation pass archives it instead of deleting it, so active files stay lean without losing the history.

**The workspace loads lazily.** Only the active persona loads by default; project state points to the flow, the flow points to the process the current step needs, and skills load only when the work calls for them. A large workspace with a deliberately small active context window — which also keeps token costs sane.

<!-- IMAGE STUB · slot: context-graph · shows: D2 mini-diagram — project → flow → process → skill, each loading only what the step needs -->

### Prompt engineering — templates, processes, and a voice that reads from real edits

**Templates and process files are the durable product.** Every deliverable shape and workflow procedure in the library was refined on real work. Models and tools can change around them.

**The voice system writes like me because it reads my edits.** It anchors on complete pieces I actually published, layers channel-specific registers on top, and learns moves from contrastive pairs mined from my real revision passes. I'm still refining it against the latest work on voice fidelity, and it has improved immensely the more work I run through it.

**Domain packs add structure only where repeated work earned it.** Marketing, project management, and careers ship today; a new domain is a folder of templates and a descriptor, with zero changes to the core engine. My read on the market is that the big model makers are converging on this same pattern — domain packs on a core platform, the way Anthropic ships Claude for Financial Services. AgentFrame applies it to a local workspace, on a context-management substrate that keeps it flexible.

<!-- IMAGE STUB · slot: voice-pair · shows: one real contrastive pair — a generic AI line beside the operator's rewrite, rendered as a styled snippet -->

### The deterministic spine — buttons own mechanics, agents own judgment

**State transitions run as commands.** Models are strong at research, synthesis, and drafting, and unreliable at remembering exact bookkeeping steps for months. So `system/af.py` owns project creation, versioning, locking, publishing, and doctor checks: one command changes the files and records the event atomically, then hands judgment back to the agent.

**Deliverables are versioned like code.** Immutable snapshots, a head pointer, and lock gates before anything ships — plus real exports when the destination needs a file rather than a chat message. The payoff is traceability: what changed, when, and why, for every piece of work.

**Everything leaves a paper trail at near-zero cost.** Activity logs, version notes, an append-only audit database, and `af doctor` checks that report drift without silently fixing it. That trail is what makes the dashboard renderable and gives the learning loop below its raw material.

<!-- IMAGE STUB · slot: spine-flow · shows: D2 flow — draft → version → lock → gate → publish, with the paper trail written underneath each step -->

### Self-improvement — invoked, not automatic

**The system learns from finished work.** When a project closes, harvest passes compare drafts, my manual edits, and workflow friction, then propose upgrades to templates, processes, and the voice corpus. The improvement is deliberate: I decide when a body of work has earned a harvest.

**Loop engineering is the part I'm still exploring.** Bounded-autonomy runs declare a goal, evidence of completion, iteration budgets, checkpoints, and an independent reviewer before the agent works unattended. Today that's one process file — [`bounded-autonomy.md`](library/process/bounded-autonomy.md) — and I expect it to grow as I figure out what works in practice.

### Standing on community shoulders

The production capabilities — deck generation, advanced visuals, video composition, prose humanizing, deep research — are vendored from well-known, well-used community projects rather than rebuilt from scratch. Each one carries my own tweaks and builds layered on top without forking the whole thing, so upstream improvements can still flow in while the customizations persist. The full catalog, with what's vendored and what's internal, is in [At a glance](#at-a-glance).

### The Workspace Dashboard — proof the files are coherent

There's also a dashboard: projects, attention items, calendar, and a timeline, rendered straight from the same markdown the agents use. Honestly, I mostly use it for the at-a-glance view (and a little vanity), but it proves a point I care about — the workspace state is coherent enough to become an interface **with no second database and no LLM call**. Details in [The Workspace Dashboard](#the-workspace-dashboard).

<!-- IMAGE STUB · slot: dashboard-hero · shows: one Workspace Dashboard screenshot — active projects, attention queue, calendar -->

## Quick start

1. Clone the repository:

   ```bash
   git clone https://github.com/situhacks/agentframe.git
   cd agentframe
   ```

2. Open the folder in Claude Code, Codex, Cursor, VS Code, Antigravity, or another coding agent that can read and write the workspace.

3. Copy `.env.example` to `.env` if you want the optional Gemini or Composio integrations. The workspace, CLI, project system, and dashboard all run without API keys.

4. Tell the agent: **"Swap to Operator and start a new project."** Under the hood that's:

   ```bash
   python system/af.py new-project my-project --domain project-mgmt --flow open-flow
   ```

   The agent proposes a plan scaled to your objective. Nothing governed is created unless the work needs it.

5. Optionally, start the Workspace Dashboard:

   ```bash
   pip install -r system/server/requirements.txt
   python system/server/run.py --daemon
   ```

### Modes

Two generated `AGENTS.md` personas with a hard ownership boundary:

- **Operator** runs projects and career work under `workspace/` and reads the system on demand.
- **Builder** evolves the templates, packs, processes, personas, CLI, and runtime under `library/` and `system/`.

Mode swaps copy the canonical persona and append the transition to the local audit database in one operation, and the CLI refuses Operator state transitions while Builder is active.

### Pulling upstream changes

Personal context, projects, pipeline data, and the audit database are gitignored, so your working layer never collides with updates. In Builder mode, ask the agent to **"pull upstream AgentFrame updates"** — the sync skill walks changes commit by commit, or applies a reviewed bulk migration, without touching the personal layer.

## What it can run

Three domain packs and two work topologies ship today.

| Surface | What it is for | Default shape |
|---|---|---|
| **Open project** | The domain-neutral starting point for research, planning, building, writing, analysis — any objective the model and its tools can handle | `project-mgmt` + `open-flow`; no fixed phase ladder, no mandatory governance files |
| **Governed project** | Longer engagements that benefit from a charter, RAID log, stakeholder map, decision log, and workback schedule | `project-mgmt` + opt-in governance flow |
| **Marketing project** | Research, campaign architecture, copy, visuals, video, decks, publishing, and performance capture | `marketing` + open flow or an opt-in phase ladder |
| **Career workspace** | Ongoing career context, internal progression, promotion evidence, and structured application cases | Career bank + calendar/pipeline + case folders |

The open project is the default because a project is the lowest useful denominator of knowledge work. You don't need to author a domain pack before AgentFrame helps you; packs exist for repeated work where a stronger schema or gate has earned its place.

### Technical builds without splitting the brain

When a project phase turns into a proof of concept or an application, the code lives in its own repository while AgentFrame keeps the plan, sources, decisions, and build log. At graduation, the repository receives compiled native context and AgentFrame stops orchestrating it — one brain during the build, one deliberate handoff when the project can stand on its own.

## A real project, step by step

One walkthrough per pack, same spine each time: context in, versioned work through, learning back out.

### A governed project

The project-management pack takes a charter and turns it into living context the agent maintains for months.

<table>
<tr>
<td width="50%" valign="top"><!-- IMAGE STUB · pm-walkthrough-01 · shows: Operator kickoff — scaffold from the PM skeleton, charter lands in sources/ --><br/><sub><b>1 · Kickoff</b> — Tell the agent to start a new governed project. It scaffolds the workspace from the pack skeleton and files your charter into the project's <code>sources/</code>.</sub></td>
<td width="50%" valign="top"><!-- IMAGE STUB · pm-walkthrough-02 · shows: the four governance files derived from the charter under knowledge/ --><br/><sub><b>2 · Derive the knowledge base</b> — From the charter, the agent derives living context files under <code>knowledge/</code>: a RAID log, a stakeholder map, a decision log, and a workback schedule.</sub></td>
</tr>
<tr>
<td width="50%" valign="top"><!-- IMAGE STUB · pm-walkthrough-03 · shows: living maintenance — RAID updates from a meeting transcript, decisions appending --><br/><sub><b>3 · Maintain and consolidate</b> — Meeting transcripts land in <code>sources/</code>; the agent updates the RAID log, appends decisions, and re-plans the schedule. On long projects, a consolidation pass archives resolved items so active files stay lean without losing history.</sub></td>
<td width="50%" valign="top"><!-- IMAGE STUB · pm-walkthrough-04 · shows: a versioned deliverable draft with its -v{N} snapshot trail --><br/><sub><b>4 · Draft deliverables</b> — Findings, memos, and decks are drafted in your voice from the deliverable library, and every replacement-shaped revision becomes an immutable <code>-v{N}</code> snapshot.</sub></td>
</tr>
<tr>
<td width="50%" valign="top"><!-- IMAGE STUB · pm-walkthrough-05 · shows: lock gate passing + a deck export landing in the operator's own template --><br/><sub><b>5 · Deliver</b> — Deliverables pass lock criteria before delivery is recorded, including deck exports in your own PowerPoint template when the destination needs a file.</sub></td>
<td width="50%" valign="top"><!-- IMAGE STUB · pm-walkthrough-06 · shows: the closeout harvest proposing template/voice updates --><br/><sub><b>6 · Learn</b> — Closeout harvests your manual edits and workflow friction into proposed template, process, and voice improvements.</sub></td>
</tr>
</table>

### A marketing campaign

The marketing pack runs the same spine with the most visual output — it's also the pack this workspace grew up in.

<table>
<tr>
<td width="50%" valign="top"><img src=".github/readme-assets/walkthrough-01-cmo-kickoff.png" alt="Project kickoff" /><br/><sub><b>1 · Kickoff</b> — Scaffold the project, load operator context, gather live signals, and choose the research depth.</sub></td>
<td width="50%" valign="top"><img src=".github/readme-assets/walkthrough-02-research.png" alt="Deep research" /><br/><sub><b>2 · Research</b> — Use Gemini Deep Research or the native multi-role research skill and keep the cited artifact in the project.</sub></td>
</tr>
<tr>
<td width="50%" valign="top"><img src=".github/readme-assets/walkthrough-03-post-copy.png" alt="Copy in the operator voice" /><br/><sub><b>3 · Draft</b> — Build from templates, the gold voice corpus, register guidance, and project evidence. Every replacement-shaped revision becomes a snapshot.</sub></td>
<td width="50%" valign="top"><img src=".github/readme-assets/walkthrough-04-image-production.png" alt="Media production" /><br/><sub><b>4 · Produce</b> — Route images, diagrams, decks, carousels, or video through the appropriate production process.</sub></td>
</tr>
<tr>
<td width="50%" valign="top"><img src=".github/readme-assets/walkthrough-05-published.png" alt="Publish" /><br/><sub><b>5 · Deliver</b> — Deterministic gates verify locked copy and landed exports before publishing is recorded.</sub></td>
<td width="50%" valign="top"><img src=".github/readme-assets/walkthrough-06-retro.png" alt="Harvest retro" /><br/><sub><b>6 · Learn</b> — Harvest real edits and workflow friction into proposed voice, template, and process improvements.</sub></td>
</tr>
</table>

### A career workspace

The careers pack treats your career as a long-horizon project — the internal one first.

<table>
<tr>
<td width="50%" valign="top"><!-- IMAGE STUB · career-walkthrough-01 · shows: the career bank — role expectations, KPIs, proof points, stories --><br/><sub><b>1 · Build the career bank</b> — Role expectations, KPIs, proof points, resume bullets, stories, manager context, promotion rubrics, and cycle dates live as durable files.</sub></td>
<td width="50%" valign="top"><!-- IMAGE STUB · career-walkthrough-02 · shows: the pipeline board + calendar with upcoming conversations and dates --><br/><sub><b>2 · See what's coming</b> — The calendar and pipeline board make upcoming conversations, submission dates, and follow-ups visible next to the rest of the workspace.</sub></td>
</tr>
<tr>
<td width="50%" valign="top"><!-- IMAGE STUB · career-walkthrough-03 · shows: evidence harvested from completed projects into the bank --><br/><sub><b>3 · Gather evidence</b> — Completed projects already contain your wins; harvest passes turn them into proof points and a running impact record instead of a panic the week before review season.</sub></td>
<td width="50%" valign="top"><!-- IMAGE STUB · career-walkthrough-04 · shows: a promotion case built as a rubric map + evidence + gaps plan --><br/><sub><b>4 · Run a case</b> — An internal promotion case works like any evidence-backed application: the rubric becomes the requirements source, gaps become a plan, and the final material can be a deck rather than a resume.</sub></td>
</tr>
<tr>
<td width="50%" valign="top"><!-- IMAGE STUB · career-walkthrough-05 · shows: ATS-safe resume/cover exports keyed to the destination system --><br/><sub><b>5 · Produce materials</b> — The same substrate handles external applications when you need it to: ATS-aware resume and cover-letter exports, and login-free job scouting from public feeds.</sub></td>
<td width="50%" valign="top"><!-- IMAGE STUB · career-walkthrough-06 · shows: dashboard timeline view of career activity across months --><br/><sub><b>6 · Keep the record</b> — Everything accrues back into the bank, so the next case starts from evidence instead of memory.</sub></td>
</tr>
</table>

## At a glance

The full capability catalog. Skills carry provenance — this system is deliberately built on community work where the community's version is already good; every process file and template is AgentFrame-owned and earned on real projects.

### Skills

| Skill | What it does | Provenance |
|---|---|---|
| `agentframe-structure` | Safely changes flows, deliverable types, defaults, and ownership boundaries | Internal |
| `browser-harness` | Runs local CDP-driven browser workflows | Vendored |
| `d2-diagrams` | Renders deterministic SVG diagrams | Internal (pinned D2 binary vendored) |
| `deep-research` | Architect → specialist → synthesis research on the agent's own tools | Internal (prompts adapted from upstream) |
| `deliverable-harvest` | Mines finished projects for earned template and process improvements | Internal |
| `deliverable-scaffolding` | Creates deliverable instances with the correct shape and frontmatter | Internal |
| `doc-export` | Produces ATS-safe resume and cover-letter files keyed to the destination system | Adapted |
| `docx` | Creates, inspects, and edits Word documents | Vendored |
| `extract-design` | Measures a website's design language via the `designlang` CLI | Vendored |
| `humanizer` | Detects and removes common AI-writing patterns | Vendored |
| `hyperframes` | HTML-to-video composition: Studio, CLI, engine, and routed video skills | Vendored |
| `job-scout` | Sweeps public ATS feeds against the career search profile, login-free | Adapted |
| `open-design` | Local-first advanced image and deck runtime | Vendored |
| `ppt-master` | Converts sources into designed SVG pages and native-editable PowerPoint decks | Vendored |
| `pptx` | Inspects, validates, and performs native PowerPoint edits | Vendored |
| `project-consolidate` | Archives stale project detail and promotes durable context | Internal |
| `system-improvement` | Applies scoped system patches with verification and audit discipline | Internal |
| `upstream-sync` | Adopts upstream AgentFrame changes without overwriting the personal layer | Internal |
| `voice-harvest` | Turns finished work and edit deltas into corpus examples and contrastive pairs | Internal |

Vendored skills keep a skill-local `VENDOR.md` with the upstream source and refresh procedure, so pulling the latest community version is a documented step. The live index is [`system/skills/README.md`](system/skills/README.md).

### Processes

All AgentFrame-owned. Each loads on demand when the work reaches it.

| Process | What it gives the agent |
|---|---|
| `bounded-autonomy` | Goal contracts, budgets, checkpoints, reviewer gates, and stop rules |
| `browser-fallback` | A controlled browser route when a supported API, connector, or CLI is unavailable |
| `career-harvest` | Promotion of real project wins into proof points, stories, and resume-bank bullets |
| `composio-notes` | Connected-workspace publishing and performance-capture conventions |
| `deck-production` | Central deck routing with PPT Master as the default for new PowerPoint work |
| `deliverable-versioning` | Head pointers, immutable snapshots, and revision judgment |
| `diagram-production` | Static graph-shaped explainers through D2 |
| `flow-authoring` | The standard for adding or reshaping project flows |
| `humanizer-integration` | A calibrated humanization pass where a template calls for it |
| `image-production` | Path selection across generated imagery, HTML visuals, and Open Design |
| `knowledge-base` | Source ingestion, living knowledge files, archives, and consolidation rules |
| `lock-event` | Lock mechanics and the post-lock judgment checklist |
| `operator-context-setup` | First-run generation of positioning, profile, career, and voice surfaces |
| `preview-server` | Start-or-open behaviour, deep links, and preview hygiene for the dashboard |
| `process-authoring` | The standard for reusable process files |
| `project-frontmatter` | Canonical project state, tracker schema, overrides, and drift checks |
| `research-and-signals` | Kickoff context scans and research-method selection |
| `substack-publishing` | Draft preparation, editor handoff, and live-result reconciliation |
| `technical-build` | External-repository orchestration and graduation |
| `video-production` | Talking-head, HyperFrames, generated-asset, and hybrid video routes |
| `voice-mini-retro` | The lock-time eligibility gate for harvesting meaningful voice edits |
| `voice-setup` | Corpus mining, taste interview, and initial voice-system setup |

The live catalog with load triggers is [`library/process/README.md`](library/process/README.md).

### Deliverable templates

All AgentFrame-owned, shaped by real use.

| Where | Templates |
|---|---|
| Shared (`library/deliverables/`) | design-language · image-prompts · video-spec · closeout-retro · system-retro · the generic `_meta` deliverable shape |
| Marketing pack | body-copy · business-brief · campaign-architecture · campaign-brief · post-final · research-artifact · slide-copy · substack-essay |
| Project-mgmt pack | charter · raid-log · decision-log · stakeholder-map · workback-schedule |
| Careers pack | resume · cover-letter · jd-map · company-brief |

## Architecture

<!-- IMAGE STUB · slot: architecture-flow · shows: D2 flow diagram — coding agent → active persona → workspace state → selected flow → processes/templates → skills/tools, with the dashboard reading from the same files; legible to non-technical readers -->

The rules the structure holds to:

- **The default project is domain-neutral.** `project-mgmt/open-flow` contributes no domain fields and no mandatory governance ceremony.
- **Specialization is additive.** Packs declare vocabulary, templates, valid verbs, and routes; the core engine stays blind to what a project is about.
- **Files own working truth.** Markdown and media hold project state, context, decisions, and outputs. SQLite is reserved for the append-only system-change audit.
- **Sources and knowledge are different things.** Immutable inputs live in `sources/`; distilled working context lives in `knowledge/`.
- **Prose owns judgment; mechanisms guarantee invariants.** The agent decides what good work is. The CLI and hooks protect state, exports, and repeatable gates.
- **The dashboard is a reader.** It never becomes a competing state owner.
- **Templates are the durable product.** Skills and runtimes can be replaced without rewriting the deliverable library.

## The Workspace Dashboard

`python system/server/run.py --daemon` starts or reuses the local server and opens the workspace UI. It reads deterministically from the workspace files — no model, no API key, no second database.

<!-- IMAGE STUB · slot: dashboard-tour · shows: 2-3 dashboard screenshots — dashboard home, calendar/timeline, preview workspace -->

What it provides:

- A dashboard of active projects, attention items, and recent activity
- Day, week, and month calendar views
- A multi-month swimlane timeline with active-first sorting
- Work blocks derived from actual logged activity
- Deliverable and shipped-media markers with hover previews
- An IDE-style preview workspace with tabs and splits
- Markdown, text, HTML, image, PDF, video, PPTX, and DOCX viewing
- Completed-project history and print/PDF calendar output
- File watching and LiveReload for production work

## Repository map

```text
agentframe/
├── AGENTS.md                   # generated active persona
├── AGENTS.operator.md          # project execution and routing
├── AGENTS.builder.md           # system architecture and maintenance
├── library/
│   ├── context/                 # operator, people, channel, career, and voice context
│   ├── deliverables/            # shared deliverable templates
│   ├── domains/                 # marketing, project-mgmt, careers
│   ├── process/                 # flows and on-demand procedures
│   └── assets/                  # logos and reusable deck templates
├── system/
│   ├── af.py                    # deterministic state-transition CLI
│   ├── audit/                   # append-only system-change audit
│   ├── browser/                 # browser runtime and recipes
│   ├── hooks/                   # deterministic production guards
│   ├── research/                # Gemini deep-research runtime
│   ├── server/                  # Workspace Dashboard and preview server
│   ├── skills/                  # owned and vendored capabilities
│   ├── tests/                   # CLI, guards, dashboard, and runtime tests
│   └── tools/                   # pinned local tool binaries
└── workspace/
    ├── projects/                # open-flow and structured projects
    └── pipeline/                # career board and case folders
```

## Integrations

AgentFrame runs locally; external services add capability without becoming dependencies. Your coding agent provides the model — environment keys power only the optional tools.

| Integration or runtime | Used for |
|---|---|
| Gemini | Deep Research and image generation through the local helper |
| Composio | Connected-workspace context, publishing, and performance capture |
| Open Design | Local-first advanced visual and deck production |
| PPT Master | Native-editable deck generation from source material and SVG |
| HyperFrames | HTML-to-video composition and rendering |

## Auditability

Every trail has one owner:

| Trail | Lives in |
|---|---|
| Current project state | `project.md` / `application.md` frontmatter |
| Material project events | `activity.md` |
| What changed between versions | the version files themselves |
| Low-volume system changes (mode swaps, template patches, migrations) | `system/audit/agentframe.db` |
| Schema, file, export, and pack-rule checks | `python system/af.py doctor` — reports drift, never silently fixes it |

Git carries the version history of the reusable system; personal work stays local and gitignored.

## Design constraints

AgentFrame stays useful by refusing a few tempting directions:

- It is not a replacement model or a wrapper around one provider.
- It does not copy every fact into a vector database by default.
- It does not make the dashboard a competing source of truth.
- It does not encode every possible workflow before real work earns the abstraction.
- It does not let scripts make creative or project-management judgments.

The durable asset is the context, the workflow knowledge, and the deliverable library. Models, tools, and interfaces change around it.

## Contributing

PRs for templates, process improvements, domain packs, skills, and runtime fixes are welcome. Open an issue before a major architecture change so the system doesn't grow faster than future agents can understand it.

## References and lineage

- [agentframe-marketing](https://github.com/situhacks/agentframe-marketing) — the frozen marketing-only predecessor
- [Composio](https://composio.dev)
- [Open Design](https://github.com/nexu-io/open-design)
- [PPT Master](https://github.com/hugohe3/ppt-master)
- [HyperFrames](https://github.com/heygen-com/hyperframes)
- [design-extract](https://github.com/Manavarya09/design-extract)
- [humanizer](https://github.com/blader/humanizer)
- [DeepResearch Bench](https://huggingface.co/spaces/muset-ai/DeepResearch-Bench-Leaderboard)
- [LunonAI deep research](https://github.com/LunonAI/lunon-deep-research)
- [deer-flow](https://github.com/bytedance/deer-flow)
- [open_deep_research](https://github.com/langchain-ai/open_deep_research)

## License

MIT. See [`LICENSE`](LICENSE).

## Contact

Built by Brandon Situ over many weekends — and likely many more.

- [LinkedIn](https://www.linkedin.com/in/brandonsitu/)
- Email: brandonzsitu@gmail.com
