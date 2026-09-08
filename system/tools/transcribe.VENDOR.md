# Transcription tool vendor record

- Tool: `system/tools/transcribe.py` (AgentFrame-owned wrapper; stdlib + `onnx-asr`, `soundfile`, `numpy`, ffmpeg)
- Runtime pin: `onnx-asr` 0.11.0 (`pip install "onnx-asr>=0.11,<0.12" soundfile numpy`)
- Model: NVIDIA Parakeet TDT 0.6b v3, int8 ONNX export
- Upstream weights: Hugging Face `istupakov/parakeet-tdt-0.6b-v3-onnx` (the repo `onnx-asr` resolves `nemo-parakeet-tdt-0.6b-v3` to)
- Files: `config.json`, `encoder-model.int8.onnx`, `decoder_joint-model.int8.onnx`, `nemo128.onnx`, `vocab.txt` (~640 MB)
- Encoder checksum (sha256, the copy verified 2026-09-08): `6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09`
- Licence: model weights CC-BY-4.0 (NVIDIA NeMo release); `onnx-asr` MIT

## Scope

Weights are never committed. They resolve from `system/models/parakeet-tdt-0.6b-v3-int8/` (gitignored), then an existing Handy install under `%APPDATA%\com.pais.handy\models\`, then a download into `system/models/`. Output is the HyperFrames transcript shape (`text`, `words[text,start,end]`) with word times merged from Parakeet's sub-word tokens.

## Refresh

Bump the `onnx-asr` pin, re-run the tool on a known clip, and compare word count and timing against the previous run. To change the model revision, delete `system/models/parakeet-tdt-0.6b-v3-int8/`, run the tool once so it re-downloads, and record the new encoder checksum here.
