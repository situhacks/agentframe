---
name: upstream-sync
description: "Pull updates from the upstream AgentFrame repository into a customized copy. Supports commit-by-commit adoption for incremental updates, or squashed/bulk migration for major upgrades."
---

# Upstream Sync

Builder-mode procedure. Git required. The personal layer (`library/context/operator/`, real campaigns, builder backlog, audit DB, `.claude/`) is gitignored and untouched by every step below — only tracked system/library/template files sync.

## Preflight

1. Builder mode active; working tree clean — commit local work first, never sync over uncommitted changes.
2. Upstream remote exists: `git remote get-url upstream`. If missing, verify the target repository:
   - For syncing within the V2 multi-domain architecture: `git remote add upstream https://github.com/situhacks/agentframe.git`.
   - For migrating from the legacy marketing-only structure: `git remote add upstream-v1 https://github.com/situhacks/agentframe-marketing.git` and point `upstream` to the new repo `https://github.com/situhacks/agentframe.git`.
3. `git fetch upstream`.
4. Check distance: `git rev-list --count master..upstream/master`. 
   - If distance is small (< 10 commits), proceed with **Commit-by-Commit Adoption**.
   - If distance is large or represents a major architectural upgrade (e.g. legacy marketing-v1 to multi-domain v2), proceed with **Squashed/Bulk Migration**.

---

## Path A: Commit-by-Commit Adoption (Incremental)

1. Inventory candidates: `git log --oneline master..upstream/master`, minus patch-equivalent commits (`git cherry -v upstream/master` marks them `-`) and hashes in the local skip list (`.claude/upstream-sync-skips`, one hash per line). Empty list → report up to date, stop.
2. Present one row per remaining upstream commit, oldest first — **each commit is one adoption unit, never split**:
   - Hash, subject, files touched (`git show --stat <hash>`), and any `MIGRATION:` line from the commit body.
   - Tag: **clean** (no touched file was customized locally — `git diff $(git merge-base master upstream/master)..master -- <paths>` is empty) or **collides** (operator edited one of those files).
3. Walk oldest-first; recommend, then wait for the operator's decision:
   - **take** — apply as-is.
   - **skip** — append the hash to `.claude/upstream-sync-skips` so future syncs don't re-ask.
   - **merge semantically** (collisions) — read both versions, re-apply the operator's customization onto the upstream shape, and show the diff before writing.
4. Apply approved units: `git merge upstream/master` when everything was taken; otherwise cherry-pick the approved hashes in order, substituting the semantic-merge results where files collided.

---

## Path B: Squashed/Bulk Migration (Major Upgrades)

Use this path when the commit gap is too wide for step-by-step review, or when files have been extensively restructured or renamed upstream.

1. **Rename & Restructure Mapping**:
   - Run `git diff --summary master upstream/master` to identify files that were renamed, moved, or deleted upstream.
   - Map legacy paths to the new V2 layout (e.g., templates moving from `library/deliverables/` into `library/domains/marketing/deliverables/`).
2. **Local Customization Check**:
   - Find all files modified locally by comparing against the fork point: `git diff $(git merge-base master upstream/master) master --name-only`.
   - Cross-reference this list with the restructure map to identify which local customizations are on files that moved or were renamed.
3. **Present Migration Plan**:
   - Summarize the structural changes (directories added/removed, schema version changes).
   - List files that will merge cleanly.
   - List collisions and paths where local changes need to be migrated to new locations.
   - Wait for operator approval before writing any files.
4. **Apply Squashed Changes**:
   - Create a temporary migration branch: `git checkout -b tmp-migration`.
   - Merge the upstream tip using a squash or merge strategy: `git merge --no-commit upstream/master`.
   - For customized files that moved, manually copy the local changes from the old paths on `master` onto the new paths in the staged migration, applying a semantic merge.
   - Verify that the old paths are deleted and the local customizations are preserved at the new paths.
   - Once the workspace is clean and verified, merge the `tmp-migration` branch back into `master`.

---

## Verify and Record

1. Reference sweep: grep tracked files for links to anything the sync deleted or renamed; fix or surface.
2. Run `python system/af.py doctor` to verify project structures and frontmatter are valid.
3. Record one audit row:
   `python system/audit/writer.py system-change --change-type upstream_sync --actor agent --mode builder --reason "<adopted hashes, target tags, or squashed migration description>"`
4. Commit the changes, naming the upstream target or squashed migration in the commit message.
