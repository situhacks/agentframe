# `.claude/skills/`

AgentFrame-owned skills. Claude Code discovers every folder here. A skill marked operator-only carries `disable-model-invocation: true`: its description stays out of every chat's context, the operator runs it as `/name`, and agents read its `SKILL.md` by path when a process file or pack routes there. Vendored runtimes live in [`system/vendor/`](../../system/vendor/README.md).

| Skill | Owns | Load when | Invocation |
|---|---|---|---|
| [`agent-reach/`](agent-reach/SKILL.md) | Read-only routing over installed CLIs for sources the default toolchain cannot reach (Reddit, LinkedIn, X, Instagram, Facebook), plus higher-fidelity web, YouTube, semantic-search, GitHub, and RSS paths | WebFetch or WebSearch is refused or empty for a required source, or the task names one of those platforms | Model and operator |
| [`deep-research/`](deep-research/SKILL.md) | Architect, specialist, and synthesis research loop with grounded citations | A substantial sourced question needs several sections and source comparison; not a quick lookup | Model and operator |
| [`deliverable-harvest/`](deliverable-harvest/SKILL.md) | Deliverable-shape findings from finished work into template patches, local promotions, and backlog rows, applied after one batched review | A shaped deliverable goes ready or publishes, or a project closes out ([`ready-event.md`](../../library/process/ready-event.md), [`project-flow.md`](../../library/process/project-flow.md)) | Operator; agents by path |
| [`voice-harvest/`](voice-harvest/SKILL.md) | Voice signal from finished work into pairs, voice-file patches, corpus promotions, and backlog rows, applied after one batched review | A user-voiced deliverable goes ready or publishes, a project closes out, or the operator asks for a voice pass | Operator; agents by path |
| [`doc-export/`](doc-export/SKILL.md) | ATS-safe PDF and DOCX export and filing for career materials | A finished resume or cover-letter head must become submission files (careers [`production.md`](../../library/domains/careers/production.md)) | Operator; agents by path |
| [`job-scout/`](job-scout/SKILL.md) | Public-ATS job discovery and recency-first triage into the pipeline | The operator asks for role discovery against the search profile | Operator only |
| [`d2-diagrams/`](d2-diagrams/SKILL.md) | Auto-laid-out D2 graphs rendered to SVG with the vendored D2 CLI | A graph is regenerated from changing source and its layout must re-solve itself ([`diagram-production.md`](../../library/process/diagram-production.md)) | Operator; agents by path |
| [`extract-design/`](extract-design/SKILL.md) | Website design-language extraction through `designlang`: tokens, type, colour, accessibility scores | A design language needs evidence from a live site (the `design-language` template's on-ramp) | Operator; agents by path |
| [`gym/`](gym/SKILL.md) | One coached fitness session over the private fitness system in `workspace/life/systems/fitness/`: sync, the day's card, set logging, readback, progression | The operator opens a gym session with `/gym` | Operator only |

Provenance: `agent-reach`, `doc-export`, and `extract-design` adapt upstream work and carry a `VENDOR.md`; `deep-research` carries `PROVENANCE.md`.

## Rules

- A capability one project needs once is pulled into that project, not packaged here. A skill earns a row when a second project reaches for the same mechanics.
- A skill's `name` equals its folder name, and every folder has a row in this table.
- Load on the state or intent in `Load when`, not on a phrase match.
