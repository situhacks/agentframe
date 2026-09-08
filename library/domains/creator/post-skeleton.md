---
# IDENTITY
name: "{name}"
slug: {slug}
schema_version: 1
created_at: {date}
domain: creator

# POST (fixed facts; state lives on the calendar board, never here)
type: video
platform: {platform}
date: {date_slot}
series: {series}
hook: null
source_project: null
source_file: {source_file}

# RECEIPT (written by `af studio post`)
posted_at: null
url: null
metrics_captured_at: null

# LIFECYCLE
last_activity: {ts}

# DELIVERABLES (only a drafted script.md earns a row; edit.md, media and renders are never rows)
deliverables: {{}}
---

# {name}

Post files: `edit.md` (the edit decision list, edited in place until posted) · `video/` (source, transcript, cut, the HyperFrames project) · `renders/final.mp4` (what ships) · `script.md` only when the piece is scripted.

## Notes

(Deliberately unstructured: what prompted it, what to cut, what the comments said afterward.)
