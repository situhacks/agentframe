# Project Automation

## Purpose

Own standing recurring or event-driven execution that remains subordinate to one AgentFrame project. The project keeps the durable contract and small project-specific source; `system/daemon/` supplies the generic host; deployment-local queues, credentials, logs, dependencies, and heartbeat state stay outside project content.

## When To Load

Load when project work becomes a standing managed activity, or when `project.md` contains an `automations` row. A one-time bounded autonomous goal uses [`bounded-autonomy.md`](bounded-autonomy.md) instead.

## Procedure

### 1. Create the project bundle

Initialize only after the standing job is real:

```powershell
python system/af.py automation init <project> <automation-id> --job "<stable role>"
```

The button creates `automations/{id}/automation.md` and its optional `project.md` tracker row. Complete the contract sections: standing job, trigger/inputs, project route, narrower human boundaries, result, verification, and deployment notes.

Keep the bundle lean. Add `prompt.md`, `scripts/`, or `fixtures/` only when the automation needs them. Small dependency-light transforms may live here. Volatile queues, receipts, credentials, auth state, logs, package caches, virtual environments, and `node_modules` never do.

### 2. Make the contract ready

Confirm a fresh frontier agent can start from the task file, follow the project route, identify its authority boundary, and verify one run without conversational context. Then run:

```powershell
python system/af.py automation ready <project> <automation-id>
```

Readiness is judgment; the button validates shape and records state.

### 3. Join a deployment

Add one deployment row to the local daemon registry. Its `id` must match the project tracker's `deployment_id`; it maps the project automation to a queue root and body profile without placing machine paths or credentials in project content.

Test one task end to end, then activate:

```powershell
python system/af.py automation activate <project> <automation-id> --deployment <registry-id>
```

The project status is desired state. Dashboard-observed `online | busy | stale | offline` remains runtime state; a failed task never changes the automation lifecycle by itself.

### 4. Operate, pause, or retire

Use result receipts and the dashboard to compare desired and observed state. Material contract or lifecycle changes update project activity; ordinary runs do not become activity narration.

```powershell
python system/af.py automation pause <project> <automation-id>
python system/af.py automation activate <project> <automation-id>
python system/af.py automation retire <project> <automation-id>
```

Pausing preserves the contract and deployment join. Retiring preserves the historical bundle but ends standing execution.

## Verification Or Logging

- `python system/af.py doctor <project>` validates tracker pointers, contract identity/headings, lifecycle values, deployment presence for active rows, and orphan contracts.
- The daemon receipt is canonical for one run; the dashboard reconciles project declarations with registry, heartbeat, queue, and receipt state.
- The automation contract links the project files it depends on. Do not duplicate those files' content into the contract.

## Boundaries

- This process does not create a new project flow, domain pack, skill, or deliverable type.
- Existing deliverable templates still own automation outputs.
- Generic watcher/launcher behavior belongs in `system/daemon/`; project bundles never fork it.
- Code presence alone does not trigger [`technical-build.md`](technical-build.md). Evaluate promotion only when the code gains an independent user, interface, toolchain, release cycle, or purpose outside the owning project.
