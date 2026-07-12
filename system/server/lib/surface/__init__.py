"""AgentFrame Workspace Dashboard: deterministic dashboard/preview data layer.

Reads workspace files only — no LLM calls, no network. Modules:

- ``state``     — project scanner, attention/activity parsers
- ``artifacts`` — deliverable-group artifact index (tracker + archive)
- ``snapshot``  — cached dashboard snapshot with etag invalidation
- ``convert``   — LibreOffice PPTX/DOCX -> cached PDF
- ``daemon``    — start-or-open lifecycle, lock file
- ``api``       — Tornado handlers mounted by the preview server
"""
