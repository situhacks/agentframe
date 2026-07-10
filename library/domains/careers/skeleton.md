---
# IDENTITY
name: "{name}"
slug: {slug}
schema_version: 1
created_at: {date}
domain: careers

# POSTING (fixed facts — funnel stage lives on the pipeline board, never here)
company: "{company}"
role: "{role}"
job_url: {url}
source: {source}
ats: {ats}
date_posted: {posted}
salary_range: "{salary}"

# LIFECYCLE
last_activity: {ts}

# DELIVERABLES
deliverables:
  resume:
    file: resume/resume-v1.md
    status: not_started
  cover-letter:
    file: cover-letter/cover-letter-v1.md
    status: not_started
---

# {name}

Sprint artifacts: `jd.md` (verbatim posting, immutable) · `company-brief.md` · `jd-map.md` · `resume/` · `cover-letter/`. Templates resolve through the careers pack.

## Notes

(Deliberately unstructured: recruiter/contact details, interview rounds, follow-up threads, salary conversation. Structured fields for these are the reason trackers die — keep them prose.)
