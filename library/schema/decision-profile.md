# Decision profile

Shapes `workspace/life/decision-profile.md` (private): his standing preferences as short dated rules, read by a chat that decides his list while he has handed over ([`act-as-me.md`](../process/act-as-me.md)). It holds no decision of its own: every rule comes from something he said or decided, and cites it.

```
---
private: true
distilled: 2026-11-16
sources: src-2026-10-01-01 to src-2026-11-15-23; log 2026-10 to 2026-11
---
# Decision profile

## Time and plans

- Prefers mornings for appointments (2026-10-12) [src-2026-10-12-02] [log 2026-10-14]

## Social
## Health
## Home and errands
## Work and projects
## How he wants things done
```

- Frontmatter: `private: true`; `distilled:` the day the file was last rebuilt; `sources:` the range of orders and month logs that rebuild read.
- Sections are these six, in this order: time and plans, social, health, home and errands, work and projects, how he wants things done. A section with no rule keeps its heading.
- A rule is one line: the preference in plain words, the date it was last confirmed in brackets, and one or more citations. A citation is a filed order `[src-YYYY-MM-DD-NN]` or a month-log line `[log YYYY-MM-DD]`. A line with no citation is cut.
- A rule states what he does, never a guess about why. When two sources disagree, keep the later one and cite both.
- The file is rebuilt, not patched: a worker reads the origin material and writes it fresh, following [`re-distill.md`](../process/re-distill.md). A chat that learns a preference files the order and leaves the profile for the next rebuild.
- A chat deciding in his name finds the closest rule, then the order or log line behind it, and names that source in the `--as-him` outcome. The rule alone is not a precedent.
