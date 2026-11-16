---
checked: 2026-11-14
sources: Anthropic's model overview and pricing pages; each model's overview and "Prompting Claude" guide; Claude Code's model-config and sub-agents pages; the choosing-a-model and effort guides
---

# The model rule and the model facts

The one place that says which model to choose and what each model is. Every other file links here.

## The rule

Recommended, not enforced. For an orchestrator that is not Fable:

- Plan with the strongest model: the first, large design on Fable; a section design on Opus, or Fable when the build matters most.
- Build on Sonnet, from a plan that Opus or Fable wrote.
- When a build fails its tests or its review, rebuild on Opus, then on Fable.
- Scripted work with exact instructions (a check, an extraction, a routing step) goes to Haiku.
- Put the effort into the plan, so a cheaper model can build from it.
- A Fable orchestrator chooses its own agents and may depart from this.
- Changing the effort level is often a better first step than changing the model.

A brief's `model` field, a registry row's `model` column and `--model` take one of the four aliases below. The ids the tools pass to `claude -p` are in `system/models.json`.

## The models

| Alias | API id | Context / output | $ per million tokens, in / out / cache read | Anthropic says it is for |
|---|---|---|---|---|
| `fable` | `claude-fable-5-2` | 1M / 128K | 8 / 40 / 0.20 | Demanding reasoning and long-horizon agentic work |
| `opus` | `claude-opus-5-5` | 1M / 128K | 4 / 20 / 0.20 | Long-running agentic coding and knowledge work; the "start here" model |
| `sonnet` | `claude-sonnet-5-5` | 1M / 128K | 2 / 10 / 0.10 | The best speed and intelligence mix for everyday coding and agent work |
| `haiku` | `claude-haiku-5-5` | 1M / 128K | 0.10 / 0.50 (prompts over 100K: 0.50 / 2.50) | High-volume, latency-sensitive work: classification, extraction, routing, subagent tasks |

Claude Code compacts a chat at about 967K tokens by default, and AgentFrame nudges a chat sooner by its class ([`sessions-registry.md`](sessions-registry.md#session-classes)).

## Briefing each model

- Fable: it ends turns early on long tasks at low or medium effort, so say "finish the whole task". It treats "never" as absolute, so scope such words to their reason. Told it owns a codebase, it consolidates and changes more than was asked. Its subagents can dominate cost.
- Opus: a message with no tool call can be a progress report that ends the turn, so an unattended loop checks the task list and nudges, at most three times. On loosely specified work, tell it to read first. Lower effort cuts its thinking more reliably than prompt text.
- Sonnet: at low or medium effort it checks in before finishing; at higher effort it adds tests and documents nobody asked for. Say "finish the whole task" and "add nothing that was not asked". At the highest efforts it starts review rounds; say not to. For the hardest long work Anthropic says to use Opus.
- Haiku: give it today's date, or it may skip a search it needs. At low effort on a long prompt it stops early, and it may report a change done without checking it. A refused request refuses again when resent.

A model card in [`library/models/`](../models/) is a short list of instructions addressed to one model. `af hook session-start` (and `af session bundle`) adds the card of a build chat's model at the start. Only Fable and Opus have a card; other models get none. A model changed in the middle of a chat gets its card at the next session start or compaction.

## Keeping this file true

The harness review flags this file when `checked` is more than 60 days old; a builder chat then reads Anthropic's model pages again and updates the table, the cards and the date.

## Other models

Gemini 4 Hydrogen: second drafts, second-opinion reviews and long reading in research, to spread usage across his subscriptions. Call it through `system/tools/agy_call.py --model gemini-4-hydrogen-high`; limits are in `agy_call.VENDOR.md`.
