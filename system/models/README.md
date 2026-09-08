# Local model weights

Weights the vendored tools run on. Fetched on first use, never committed: both AgentFrame repos are public and a single ASR model is hundreds of megabytes. Only this README is tracked.

| Folder | Model | Used by | Resolved from |
|---|---|---|---|
| `parakeet-tdt-0.6b-v3-int8/` | NVIDIA Parakeet TDT 0.6b v3, int8 ONNX export | `system/tools/transcribe.py` | `onnx-asr`'s Hugging Face source on first run; or copied from an existing Handy install at `%APPDATA%\com.pais.handy\models\` when present |

Resolution order for a tool: this folder → a known sibling install named in the tool → download. A tool that downloads records the source and checksum in its own `VENDOR.md`. Delete a folder here to force a re-fetch; nothing else references it.
