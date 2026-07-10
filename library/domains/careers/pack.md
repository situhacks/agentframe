---
domain: careers
topology: pipeline
prefix: null
extension_fields: []
verbs: [lock, version, doctor]
exportable: [resume, cover-letter, deck]
assembly_record: null
flows: []
---

# Careers domain pack

The careers domain: job applications that ship tailored, ATS-safe materials. Careers work is **not** project-shaped — it lives on the singleton pipeline surface `workspace/pipeline/` (stage-based funnel, no lock-and-ship terminal), declared here via `topology: pipeline`. `af new-project` refuses this domain; applications scaffold with `af pipe start`.

This pack is the only artifact that knows careers. The generic spine (`system/af.py`) reads what is declared here; it names no domain.

## What this pack declares

| Slot | Artifact | What it is |
|---|---|---|
| Topology | `topology: pipeline` above | careers work scaffolds under `workspace/pipeline/applications/`, stage-tracked on the board (`pipeline.md`), never under `workspace/projects/` |
| Application skeleton | [`skeleton.md`](skeleton.md) | the `application.md` an application sprint gets (`af pipe start`) |
| Board skeleton | [`pipeline-skeleton.md`](pipeline-skeleton.md) | the `pipeline.md` created on first `af pipe save` |
| Deliverable templates | [`deliverables/`](deliverables/) | `jd-map`, `resume`, `cover-letter`, `company-brief`; non-text materials (`deck`, `demo`) use the generic deliverable shape + the routes in [`production.md`](production.md) |
| Verb applicability + hooks | `verbs` above + `rules.py` | no `publish`; every submission material locks behind jd-map verification; text materials also pass the parse-hazard lint (`on_lock`) |
| Doctor rules | `rules.py` `check_application()` | text-material parse hazards, verification freshness |
| Persona routing | [`production.md`](production.md) | the application-sprint runbook the Operator loads for careers work |
| Career bank schema | [`operator-schema/career/`](../../context/operator-schema/career/README.md) | shapes for profile, master-cv, proof-points, stories, tracks, search-profile — instances live gitignored under `library/context/operator/career/` |

## Stage model (the board owns this)

`application.md` holds fixed posting facts plus its material tracker rows. The board's `shipped` value maps to the locked primary-material version actually submitted.

**Materials are per-application.** `application.md` `materials: [..]` names the deliverable rows that constitute the submission (first = primary, drives the board's `shipped`). Default `[resume]`; a promotion case is usually `[deck]`; a coding screen adds `demo`. Every material locks behind jd-map verification; text materials additionally pass the ATS hazard lint; `exportable:` above says which need filed finals.

**Internal cases ride the same funnel.** A promotion case is an internal application: `af pipe save --company {employer} --role "{level} promotion {cycle}" --ats internal`, the leveling rubric cached as `jd.md`, the jd-map read as a rubric-map (criteria to bank evidence; the gap stop is the career-planning conversation). Stages map: applied = submitted, interviewing = committee/calibration, offer = promoted, rejected = deferred (re-run next cycle).
`saved → preparing → applied → interviewing → offer | rejected | ghosted | dropped`

Transitions are enforced by `af pipe stage` (`ghosted` may return to `interviewing` — late replies happen). Stage lives only on the board; `application.md` holds fixed posting facts plus the resume/cover-letter tracker rows. Deliverables reuse the generic shape and the ordinary `lock`/`version` machinery; the board's `shipped` value maps to the locked, exported version that was actually submitted.
