# Evolution Authoring Standard

**Purpose:** A tiny per-template patch index. It earns its place only when a system retro or system change patches the template.

## Required Entry Shape

Each entry must strictly follow this four-line format:

```markdown
## YYYY-MM-DD - {short label}
Patched section: {one line describing the section changed}
Change: {one line describing the change}
Source: {link to the system retro or system_changes row id that earned it}
```

## Rules

- **Forbidden in entries:** Validation narratives, cluster archaeology, restated retro content, evidence campaign prose, audit-log mirrors, or multi-paragraph rationale.
- **Rule:** If the entry needs more than the four lines above, the content belongs in the system retro or `system_changes`, not here. Retros own the why and the evidence; the evolution log only points to them.
