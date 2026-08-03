# PPT Master - AgentFrame Boundary Notes

Vendored deck-generation skill (see `VENDOR.md`). The vendor's `SKILL.md`, references, workflows, and docs own deck-generation knowledge. This overlay contains only AgentFrame integration boundaries and operator-specific defaults. Read it whenever AgentFrame routes work to PPT Master.

## Routing

- **Deck routing lives in `library/process/deck-production.md`.** That file owns the default PowerPoint tool and its exceptions. Once routed to PPT Master, the vendored `SKILL.md` and `workflows/routing.md` own workflow and session-mode selection; AgentFrame does not mirror them.
- **PPT Master is AgentFrame's default for new `.pptx` creation/export.** This overlay starts only after `deck-production.md` has selected PPT Master.
- **Reference-grounded redesign is a separate AgentFrame route.** When `library/process/reference-grounded-deck-redesign.md` is loaded, do not run the full source deck through `beautify-pptx`. The source clone and manifest remain authoritative; PPT Master receives only slides marked `rebuild`, one isolated complex slide at a time.
- **Project workspace.** Stage `<project_path>` inside the calling project (for example, `workspace/projects/{slug}/phase-4-production/decks/{deck-name}/`) or `C:\tmp` for throwaway runs - never in this skill folder or a repo-root `projects/` directory.

## Run contract

- **Speaker notes off by default.** Skip notes generation and export with the vendor's no-notes option. Generate notes or narration only on explicit operator request.
- **Deterministic guards.** `system/hooks/ppt_master_guard.py` (wired through `.claude/settings.json`) blocks projects staged inside `system/`, blocks exports whose `svg_output/` fails the paragraph-editability lint, and re-injects the export-promotion contract. `af doctor` backstops strays. The vendor owns the SVG authoring guidance; the guard only guarantees the AgentFrame failure cases observed in production.
- **Front-loaded confirmation.** For a repo-contained bounded-autonomy deck run, `library/process/deck-production.md` owns the sealed confirmation adapter. A valid run-bound wrapper satisfies the gate before execution; the hook blocks redundant Confirm UI waits and points to the materializer. Do not skip the vendor gate, invent partial delegation, or restate its result schema here.
- **Audience-ready copy.** Slide-visible text is written for the deck's audience: no planning labels, internal signposting, references to the source material as source material, tombstones, TODO/WIP markers, or internal rationale. Keep legitimate audience-facing citations. When the operator explicitly requests a skeleton or unfinished slide, use visibly temporary prose (`Lorem ipsum` or role-shaped placeholder text), never grey placeholder bars; otherwise keep unresolved content outside the deck.
- **Operator drafts stay canonical; pass a copy.** The vendor's move-based import may absorb inputs into its project. For operator-authored storyboards or slide content, pass a copy so the deliverable-folder draft remains the source of truth.
- **Divergence pinning.** For operator-authored storyboards or slide content, set `content_divergence` to stay close. A redesign-only request also pins no content compression; resize or reflow before cutting content.
- **Isolated reference rebuild.** Treat the origin render, source-derived shape facts, and manifest delta as the page contract. Preserve everything outside the delta; do not replace an intricate source composition with a familiar layout archetype. Return only the isolated rebuilt slide for PowerPoint-native replacement and AgentFrame verification.
- **Voice handoff.** When PPT Master will author or reword copy, load `library/context/operator/voice/` and provide it at the vendor's voice/tone confirmation. Stay-close runs preserve voice already present in the source.
- **Design language handoff.** When the calling project has a locked design language, provide its palette and typography at vendor confirmation rather than allowing a new identity.
- **Brand assets.** Before design confirmation, check `library/assets/logos/`. Follow `library/assets/README.md` when an asset is missing; never approximate a real logo as custom SVG.
- **API keys.** Use the repo-root environment configuration. Do not create a second key store inside the vendored skill.

## Outputs

- **Promote each export.** Copy the completed `.pptx` from the PPT Master project's `exports/` into the calling deliverable folder, preserving its timestamped filename. The vendor-project twin stays frozen; AgentFrame versioning and operator-edit round trips remain owned by `library/process/deck-production.md`.
