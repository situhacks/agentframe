# AgentFrame Managed Automation Host

This runtime drains multiple deployment queues through one globally single-flight managed agent. It owns launch mechanics only; project automation contracts own standing work, and task files own one occurrence.

## Durable Layers

- `project.md` + `automations/{id}/automation.md`: desired lifecycle and project-bound contract.
- Local `registry.json`: deployment ids, queue roots, body profiles, and machine paths.
- Queue task/result files: one run's input and terminal receipt.
- `status.json`: disposable heartbeat/current-task snapshot for the Workspace Dashboard.

No credentials, queues, logs, heartbeat files, package caches, or live registry are tracked.

## Queue Convention

Each registered `queue_root` receives `inbox/`, `processing/`, `archive/`, `outbox/`, and `logs/`. Producers write complete `*.task.json` files into `inbox/`. The watcher claims the oldest task across every enabled queue and runs one body at a time against the shared workspace.

Minimal task:

```json
{
  "schema_version": 1,
  "id": "email-20260712-001",
  "requested_at": "2026-07-12T10:30:00-07:00",
  "task": "Process this intake using the project automation contract.",
  "inputs": ["intake.json"]
}
```

The kickoff supplies the trusted project and automation identity from the registry; task content cannot select a different project or widen authority.

One terminal receipt lands at `outbox/{id}.result.json` with `done | blocked | failed`. Body stdout/stderr are diagnostics only. Missing/invalid receipts, launch errors, and timeouts synthesize `failed`. On watcher restart, stranded processing tasks fail closed and are never replayed automatically.

## Setup

1. Copy `registry.example.json` to `system/daemon/local/registry.json` and fill the work-machine values.
2. Verify enterprise policy permits unattended Cursor use.
3. Verify `cursor-agent -p --force` can modify a disposable file with the pinned model and existing enterprise authentication.
4. Run one fake/disposable task with `python system/daemon/watcher.py --registry system/daemon/local/registry.smoke.json --once`. Keep smoke deployments in that disposable registry and queue; `registry.json` is durable live inventory for the Workspace Dashboard.
5. Register `python system/daemon/watcher.py --registry <absolute-registry-path>` at logon in Task Scheduler. Configure it to run only while the operator is logged in; queueing remains available while the laptop is offline.
6. Add Power Automate producers only after the local lifecycle test passes.

The watcher sets `AGENTFRAME_MANAGED_RUN=1`, names exact task/result paths, and launches the configured body with the workspace as `cwd`. Cursor is the first body profile, not a system assumption.

## Multiple Sources

Add one registry row per Power Automate/OneDrive source. Rows may share the same body profile. The watcher chooses the oldest waiting task across all enabled rows; it does not run parallel workspace writers.

## Dashboard Contract

The Workspace Dashboard reads project automation rows as declared state and joins them to the local registry/status/receipts as observed state. The surface is read-only: it may report ready-not-deployed, active-offline, paused-with-queue, or runtime-orphan conditions, but it never starts, retries, edits, or deletes automation state.

## Future Gateways

OpenClaw or another autonomous harness should consume the same project-contract/task/receipt protocol through its native gateway and runtime controls. It is not required to imitate this watcher or executable adapter.
