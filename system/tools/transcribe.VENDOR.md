# Transcription tool vendor record

- Tool: `system/tools/transcribe.py` (AgentFrame-owned wrapper; stdlib + `onnx-asr`, `soundfile`, `numpy`, ffmpeg)
- Runtime pin: `onnx-asr` 0.12.0 (`pip install "onnx-asr[cpu]>=0.12,<0.13" soundfile numpy`; the `cpu` extra brings `onnxruntime`, which 0.12 requires to be neither 1.24.1, 1.25.x, nor 1.26.0); verified on `onnxruntime` 1.27.0
- Model: NVIDIA Parakeet TDT 0.6b v3, int8 ONNX export
- Upstream weights: Hugging Face `istupakov/parakeet-tdt-0.6b-v3-onnx` (the repo `onnx-asr` resolves `nemo-parakeet-tdt-0.6b-v3` to), revision `8f23f0c03c8761650bdb5b40aaf3e40d2c15f1ce` (2026-02-17)
- Files: `config.json`, `encoder-model.int8.onnx`, `decoder_joint-model.int8.onnx`, `nemo128.onnx`, `vocab.txt` (~640 MB)
- Encoder checksum (sha256, matching the upstream revision and the local copy, verified 2026-10-23): `6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09`
- Last refresh check (2026-10-23): a 34 s two-window synthetic clip gave 86 words with identical text and word times on 0.11.0 and 0.12.0 under `onnxruntime` 1.27.0. Under `onnxruntime` 1.31.0 the text matched and word times moved by up to 0.08 s (one encoder frame).
- Licence: model weights CC-BY-4.0 (NVIDIA NeMo release); `onnx-asr` MIT

## Scope

Weights are never committed. They resolve from `system/models/parakeet-tdt-0.6b-v3-int8/` (gitignored), then an existing Handy install under `%APPDATA%\com.pais.handy\models\`, then a download into `system/models/`. Output is the HyperFrames transcript shape (`text`, `words[text,start,end]`) with word times merged from Parakeet's sub-word tokens.

## Refresh

Bump the `onnx-asr` pin, re-run the tool on a known clip, and compare word count and timing against the previous run. To change the model revision, delete `system/models/parakeet-tdt-0.6b-v3-int8/`, run the tool once so it re-downloads, and record the new encoder checksum here.
