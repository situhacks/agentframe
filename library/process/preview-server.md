# Preview Server Process

Lazy-load this file when a turn writes a hub-supported artifact under `workspace/projects/*/phase-*/`.

## When To Load

Load this process when the turn writes or revises any previewable file type that the hub can render:

- HTML (`.html`)
- Image (`.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`, `.svg`)
- PDF (`.pdf`)
- Video (`.mp4`, `.mov`, `.webm`)

## Offer Trigger

After the first previewable artifact write in a turn, offer preview options once for that turn:

1. Direct artifact URL: `http://localhost:8080/<relative-path>`
2. Hub URL: `http://localhost:8080/`
3. Start command (if not already running): `python system/server/run.py`

Do not repeat this offer after every file edit in the same turn.

## Hide Noise

When the hub gets noisy with intermediate artifacts:

- Add exclusion patterns in `system/server/config.yaml` under `exclude_globs:`.
- Drop a zero-byte `.preview-hide` file in any folder to hide that folder and descendants from default hub discovery.
- Use `http://localhost:8080/?intermediates=1` to temporarily show excluded/intermediate artifacts.
