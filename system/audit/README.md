# AgentFrame Marketing System Change Audit

`system/audit/` is the narrow SQLite exception in AgentFrame Marketing.

- Markdown remains canonical for campaign state, campaign activity, content, and operator-facing work.
- SQLite is canonical only for append-only `system_changes` audit rows.
- The live database file is `system/audit/agentframe.db` and should stay untracked. If a local `system/audit/marketingos.db` exists, `writer.py` migrates it to the new filename on the next default-path initialization.

## Files

| File | Job |
|---|---|
| `schema.sql` | Canonical `system_changes` table and indexes. |
| `writer.py` | Deterministic writer utility and CLI for system-change rows. |
| `agentframe.db` | Runtime SQLite database (created on first write; not committed). |
| `marketingos.db` | Legacy local filename, migrated automatically when the default DB path initializes. |

Retired telemetry modules (`user_interactions`, `daemon_runs`, hooks, reconciler observations, and post timeline reports) are no longer part of the live contract. Campaign progress history belongs in `campaign.md` and `activity.md`.

## Table: `system_changes`

Low-volume, semantic change audit for system patches.

Core fields:

- `created_at`
- `actor`
- `mode`
- `change_type`
- `target_kind`
- `target_path`
- `campaign_slug`
- `reason`
- `summary`
- `payload_json`
- `source`

Use this for template/process/schema/skill/persona/runtime changes where the `reason` and `summary` matter. Do not use it for routine campaign progress, deliverable locks, publishing, daemon runs, browser runs, or mode/session telemetry.

### `target_kind` vocabulary

Use singular, lowercase, snake_case values for `target_kind`. Recommended canonical values:

- `system`
- `library`
- `process_file`
- `persona_file`
- `deliverable_template`
- `skill_bundle`

If a historical row used a different label, correct future rows and document the drift in a follow-up `system_changes` errata row rather than rewriting history.

## CLI

Initialize the database:

```powershell
py -3 -m system.audit.writer init
```

Append a system change row:

```powershell
# 1. Write the payload to a temp file (prevents PowerShell quote mangling)
Set-Content "system\audit\_payload.json" '{"key": "value"}'

# 2. Append the row
py -3 -m system.audit.writer system-change `
  --change-type principle_refinement `
  --actor agent `
  --mode Builder `
  --target-kind persona_file `
  --target-path AGENTS.builder.md `
  --reason "Operator identified an unearned-runtime-rule failure mode." `
  --summary "Good current examples now count against adding new runtime constraints." `
  --payload-json-file "system\audit\_payload.json"

# 3. Clean up
Remove-Item "system\audit\_payload.json"
```

## Querying

Example: recent system changes

```sql
SELECT created_at, change_type, target_path, summary
FROM system_changes
ORDER BY created_at DESC
LIMIT 20;
```

Example: recent changes to one target

```sql
SELECT created_at, change_type, reason, summary
FROM system_changes
WHERE target_path = 'AGENTS.builder.md'
ORDER BY created_at DESC
LIMIT 10;
```

## Retention

`system_changes` is permanent and low-volume. Historical markdown logs stay frozen for browsing only; do not revive them as live sinks.

## Workflow Guidance

- Write `system_changes` only when the system itself changes: process files, templates, personas, skills, schema, runtime machinery, or Builder-owned docs.
- Keep campaign-facing markdown (`campaign.md`, `activity.md`, deliverable `*-v{N}.md` / `*-vN.md`) as the human-readable working surface.
- Do not add DB writes for campaign activity unless the operator explicitly reopens the DB telemetry design.
