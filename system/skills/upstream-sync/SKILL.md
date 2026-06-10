---
name: upstream-sync
description: "Pull updates from the upstream AgentFrame Marketing repo into a customized copy. Adoption unit = one upstream commit; operator approves each unit; collisions with local customizations get a semantic merge, not conflict markers."
---

# Upstream Sync

Builder-mode procedure. Git required. The personal layer (`library/context/operator/`, real campaigns, builder backlog, audit DB, `.claude/`) is gitignored and untouched by every step below — only tracked system/library/template files sync.

## Preflight

1. Builder mode active; working tree clean — commit local work first, never sync over uncommitted changes.
2. Upstream remote exists: `git remote get-url upstream`. If missing: `git remote add upstream https://github.com/situhacks/agentframe-marketing.git`.
3. `git fetch upstream`.
4. Inventory candidates: `git log --oneline master..upstream/master`, minus patch-equivalent commits (`git cherry -v upstream/master` marks them `-`) and hashes in the local skip list (`.claude/upstream-sync-skips`, one hash per line). Empty list → report up to date, stop.

## Inventory (present, don't apply)

One row per remaining upstream commit, oldest first — **each commit is one adoption unit, never split**:

- Hash, subject, files touched (`git show --stat <hash>`), and any `MIGRATION:` line from the commit body.
- Tag: **clean** (no touched file was customized locally — `git diff $(git merge-base master upstream/master)..master -- <paths>` is empty) or **collides** (operator edited one of those files).

## Adopt per unit (operator decides)

Walk oldest-first; recommend, then wait for the call:

- **take** — apply as-is.
- **skip** — append the hash to `.claude/upstream-sync-skips` so future syncs don't re-ask. Before applying later units, check them for references to the skipped one and surface the dependency.
- **merge semantically** (collisions) — read both versions, re-apply the operator's customization onto the upstream shape, show the result before writing. No raw conflict markers in front of the operator.

When a unit deletes or renames files, say in plain language what replaces them and what happens to local references — before applying.

## Apply, verify, record

1. Apply approved units: `git merge upstream/master` when everything was taken; otherwise cherry-pick the approved hashes in order, substituting the semantic-merge results where files collided.
2. Reference sweep: grep tracked files for links to anything the sync deleted or renamed; fix or surface.
3. One audit row — `python system/audit/writer.py system-change --change-type upstream_sync --actor agent --mode builder --reason "<adopted hashes + skips>"` — and one commit naming the adopted hashes.
