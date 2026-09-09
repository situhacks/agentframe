# Intake and Dispatch

## Purpose

Turn one operator note into bounded board cards, dispatch each card to a worker session, and reconcile what comes back. The board (`workspace/board.md`) is the only state, and `python system/af.py board` owns every move. The orchestrator asks, routes, and reports; it never produces deliverable content.

## When To Load

Load when the operator hands the orchestrator a note, transcript, or list that names work for one or more projects, or says go on cards already discussed. Do not load for a single task inside an open project chat; that is Operator work under the project's own flow.

## Procedure

### 1. Read the note and ask goal-class questions only

Split the note into slices, one card each: project slug, deliverable slug, goal in one sentence, done-when as observable evidence. Ask only what changes a card's goal or done-when. Input and constraint details go into the brief or wait for the batched review. One round of questions, not an interview.

### 2. Present the inventory and wait for the go

Show the list as `project · deliverable · goal · done-when · body · model (reason)`, where body is `attached` when a chat named `<project>/<deliverable>` is already open and `background` otherwise. Propose one model per card with a one-phrase reason; the operator overrides in the same breath as the go. Starting points, not rules:

| Work | Propose |
|---|---|
| Decks, long-form or high-stakes writing, anything a client sees | `opus` |
| Research with fan-out | `opus` worker, and the brief tells it to use Sonnet subagents for the reads |
| Mechanical, short, or well-specified slices | `sonnet` |

Open questions go under the list, not inside it. The operator's go, in any wording, authorizes exactly these cards with these models. A change means a new list, never a silent edit.

### 3. Make the cards and write the briefs

For each card:

- `python system/af.py board add <project> <deliverable> --goal "..." --done-when "..." --model <agreed>`; add `--by human` when the operator will drive the chat.
- Fill `workspace/board/tasks/<id>.task.md`: the operator's verbatim ask for this slice, your clarifications, `allowed_paths`, and `context_sources` naming the project files the worker must read. Keep the Rules and Receipt sections as scaffolded.

### 4. Dispatch

- Background: `python system/af.py board dispatch <id> --launch`. The model is the one agreed on the card; `--model` overrides for one dispatch, and the board's `worker_model` is only the fallback for a card that never got one.
- Attached: the chat is named `<project>/<deliverable>`. Run `python system/af.py board dispatch <id>`, then send that session one message naming the brief path and the receipt path (`SendMessage` on Claude Code).
- The CLI refuses a second worker for the same project. Leave the card in `Queued` and tell the operator it is waiting for the slot.

### 5. Watch and report

Receipts move cards on their own: the worker's Stop hook, every session start, and the receipt watch. On each event run `board sync`, then report one line per changed card: what landed and what it needs. Never summarize a deliverable's content; name the receipt's outputs.

### 6. Review and close

- `ask: review`: the operator says approve or return. `board approve <id>` closes the card and appends the project's activity line. `board return <id> "<feedback>"` writes the feedback into the brief and returns the card to `In progress`.
- `ask: input`: answer in the worker's chat or by editing the brief, then `board resume <id>`. Or `close` or `drop` on the operator's word.
- Stale cards arrive in `Needs you` with their reason. The seven-day drop to the archive is automatic and reversible with `board reopen <id>`.

### 7. Backfill from the workspace

For a first board, or after weeks away, the note is the workspace itself. Fan out one read-only Sonnet subagent per active project: it reads `project.md`, the tail of `activity.md`, and the heads of deliverables still drafting, and returns at most three proposed cards (deliverable, goal, done-when, evidence, last touched, suggested model with reason, human or orchestrator) or says the project is dormant. Compile the proposals into the step 2 inventory, mark anything untouched for weeks as a drop candidate rather than a card, and wait for the go. Ongoing work and the pipeline and studio boards' own rows are never cards.
