# Gold corpus — full pieces, the imitation anchor

Not the same thing as `../intake/corpus/` (raw setup-time samples, any quality): THIS folder is curated finals only, loaded at drafting time.

Full-length finished pieces in the operator's voice, stored verbatim, grouped by register. The style pass imitates THESE — models copy what they're shown over what they're told, so these files, not the profile rules, are what makes a draft sound like the operator. Loaded by the drafting sequence in [`../README.md`](../README.md); the register overlay names which folder to load.

## Rules

- **3–5 pieces per register.** Below 3 the register is thin (say so when drafting); above 5, adding a piece means replacing the weakest — name the replaced piece when you swap.
- **Topical diversity beats count.** Five pieces on one subject teach a template, not a voice. When adding, prefer the piece that covers a new topic or deliverable shape.
- **Verbatim, operator-approved finals only.** Published text or operator-locked finals. No agent drafts, no cleaned-up intermediates. One provenance line at the top of each file (source + date); nothing else added.
- **Recency-weighted.** Newest shipped work best represents where the voice is. When pruning, the oldest or most topic-redundant piece goes first.
- **New pieces arrive via `voice-harvest`** (corpus-promotion step at publish/lock), not by hand-copying mid-draft.

## Folders

| Folder | Contents |
|---|---|
| `{register}/` — one folder per register named in `identity.md` | The register's finished pieces, verbatim |
| `aspiration/` | Pointers to admired writing — the direction of travel, NOT the operator's voice. Each file holds URL + craft breakdown; live-fetch the full text at use time. Never load into a style pass by default — used for structure reference in the content pass and for the corpus-vs-aspiration A/B comparison. |
