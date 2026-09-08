#!/usr/bin/env python3
"""Local transcription with word timestamps: NVIDIA Parakeet TDT 0.6b v3 (int8 ONNX) via onnx-asr.

Usage:
    python system/tools/transcribe.py <media> [--out transcript.json] [--text] [--model-dir DIR]

Output (HyperFrames transcript shape, what media-use's transcript-cut and the caption tools read):
    {"text": "...", "words": [{"text": "w", "start": 1.23, "end": 1.61}, ...],
     "engine": "parakeet-tdt-0.6b-v3-int8", "duration": 61.2, "source": "<path>"}

Runs on CPU. Any container ffmpeg reads (mp4, mov, m4a, wav) is decoded to 16 kHz mono first.
Audio is fed in ~25 s windows with a 1 s overlap because the int8 encoder rejects long sequences;
boundary words are assigned to exactly one window.

Weights resolve in order: system/models/parakeet-tdt-0.6b-v3-int8/ (gitignored) -> an existing
Handy install under %APPDATA% -> download into system/models/. Provenance: transcribe.VENDOR.md.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
MODEL_NAME = "nemo-parakeet-tdt-0.6b-v3"
MODEL_DIRNAME = "parakeet-tdt-0.6b-v3-int8"
MODELS_DIR = os.path.join(ROOT, "system", "models", MODEL_DIRNAME)
HANDY_DIR = os.path.expandvars(r"%APPDATA%\com.pais.handy\models\parakeet-tdt-0.6b-v3-int8")
HF_REPO = "istupakov/parakeet-tdt-0.6b-v3-onnx"
REQUIRED = ("config.json", "encoder-model.int8.onnx", "decoder_joint-model.int8.onnx", "vocab.txt", "nemo128.onnx")
SAMPLE_RATE = 16000
CHUNK_S = 25.0
OVERLAP_S = 1.0
TOKEN_TAIL_S = 0.12   # a token's audible tail past its emission time, for the last word of a run


def log(msg):
    print(f"[transcribe] {msg}", file=sys.stderr)


def die(msg):
    print(f"transcribe: ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def has_weights(d):
    return d and all(os.path.isfile(os.path.join(d, f)) for f in REQUIRED)


def resolve_model_dir(explicit=None):
    if explicit:
        has_weights(explicit) or die(f"--model-dir {explicit} lacks {', '.join(REQUIRED)}")
        return explicit
    if has_weights(MODELS_DIR):
        return MODELS_DIR
    if has_weights(HANDY_DIR):
        log(f"using the Handy install at {HANDY_DIR}")
        return HANDY_DIR
    log(f"no local weights; downloading {HF_REPO} (int8 files) into {MODELS_DIR}")
    try:
        from huggingface_hub import snapshot_download
    except ImportError:
        die("huggingface_hub is not installed and no local weights were found (pip install huggingface_hub)")
    os.makedirs(MODELS_DIR, exist_ok=True)
    snapshot_download(repo_id=HF_REPO, local_dir=MODELS_DIR,
                      allow_patterns=["config.json", "*.int8.onnx", "vocab.txt", "nemo128.onnx", "*.md"])
    has_weights(MODELS_DIR) or die(f"download finished but {MODELS_DIR} lacks {', '.join(REQUIRED)}")
    return MODELS_DIR


def decode_to_wav(media):
    """Any container -> 16 kHz mono PCM wav in a temp file (caller deletes)."""
    ffmpeg = shutil.which("ffmpeg") or die("ffmpeg not on PATH")
    fd, wav = tempfile.mkstemp(suffix=".wav", prefix="transcribe-")
    os.close(fd)
    cmd = [ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-i", media,
           "-vn", "-ac", "1", "-ar", str(SAMPLE_RATE), "-c:a", "pcm_s16le", wav]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        os.unlink(wav)
        die(f"ffmpeg could not decode {media}: {r.stderr.strip()[:400]}")
    return wav


def tokens_to_words(tokens, timestamps, offset):
    """Merge sub-word tokens into words with absolute start/end.

    onnx-asr emits Parakeet's SentencePiece pieces with a leading space (or ▁) on the piece that
    starts a word; punctuation and continuations arrive bare and attach to the current word."""
    words = []
    cur, start, last_t = "", None, None
    for tok, t in zip(tokens, timestamps):
        if tok in ("<blk>", "<pad>", "<unk>", ""):
            continue
        starts_word = tok.startswith("▁") or tok.startswith(" ")
        piece = tok.lstrip("▁ ") if starts_word else tok
        if starts_word and cur:
            words.append({"text": cur, "start": start, "end": last_t + TOKEN_TAIL_S})
            cur, start = "", None
        if start is None:
            start = offset + float(t)
        cur += piece
        last_t = offset + float(t)
    if cur:
        words.append({"text": cur, "start": start, "end": last_t + TOKEN_TAIL_S})
    # a word ends no later than the next one begins
    for a, b in zip(words, words[1:]):
        if a["end"] > b["start"]:
            a["end"] = max(a["start"] + 0.04, b["start"] - 0.01)
    return [w for w in words if w["text"].strip()]


def transcribe(media, model_dir):
    import numpy as np  # noqa: F401  (soundfile returns numpy arrays)
    import soundfile as sf
    import onnx_asr

    wav = decode_to_wav(media)
    try:
        audio, sr = sf.read(wav, dtype="float32")
    finally:
        os.unlink(wav)
    if audio.ndim > 1:
        audio = audio.mean(axis=1)
    duration = len(audio) / sr
    log(f"{duration:.1f}s of audio; loading {MODEL_DIRNAME} from {model_dir}")
    model = onnx_asr.load_model(MODEL_NAME, model_dir, quantization="int8").with_timestamps()

    chunk, step, overlap = int(CHUNK_S * sr), int((CHUNK_S - OVERLAP_S) * sr), OVERLAP_S
    words = []
    i, k = 0, 0
    n_chunks = max(1, int((len(audio) - overlap * sr) // step) + 1)
    while i < len(audio):
        seg = audio[i:i + chunk]
        if len(seg) < sr // 2:
            break
        offset = i / sr
        res = model.recognize(seg, sample_rate=sr)
        toks, ts = (res.tokens or []), (res.timestamps or [])
        seg_words = tokens_to_words(toks, ts, offset)
        # each boundary region belongs to exactly one window
        lo = offset + (overlap / 2 if k > 0 else 0.0)
        hi = offset + CHUNK_S - (overlap / 2 if (i + chunk) < len(audio) else 0.0)
        words += [w for w in seg_words if lo <= w["start"] < hi]
        k += 1
        log(f"window {k}/{n_chunks} done ({min(duration, offset + CHUNK_S):.0f}s)")
        i += step
    words.sort(key=lambda w: w["start"])
    text = " ".join(w["text"] for w in words)
    return {"text": text, "words": words, "engine": f"{MODEL_DIRNAME}", "duration": round(duration, 2),
            "source": os.path.abspath(media)}


def main():
    ap = argparse.ArgumentParser(description="Local Parakeet transcription with word timestamps.")
    ap.add_argument("media", help="audio or video file")
    ap.add_argument("--out", help="write the transcript JSON here")
    ap.add_argument("--text", action="store_true", help="print plain text to stdout (default when --out is absent)")
    ap.add_argument("--model-dir", help="explicit weights folder")
    args = ap.parse_args()
    os.path.isfile(args.media) or die(f"not a file: {args.media}")
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8")
        except Exception:
            pass
    result = transcribe(args.media, resolve_model_dir(args.model_dir))
    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        with open(args.out, "w", encoding="utf-8", newline="\n") as fh:
            json.dump(result, fh, ensure_ascii=False, indent=1)
        log(f"wrote {args.out} ({len(result['words'])} words)")
    if args.text or not args.out:
        print(result["text"])


if __name__ == "__main__":
    main()
