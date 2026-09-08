"""Pure-function coverage for the studio tools: no ffmpeg, no model, no network."""
import importlib.util
import os
import unittest

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def load(name):
    path = os.path.join(REPO_ROOT, "system", "tools", f"{name}.py")
    spec = importlib.util.spec_from_file_location(f"af_tool_{name}", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


transcribe = load("transcribe")
media_intake = load("media_intake")


class TestTokensToWords(unittest.TestCase):
    # the shape onnx-asr actually emits for Parakeet: a leading space starts a word,
    # punctuation and continuations arrive bare
    TOKENS = [" You", " don", "'", "t", " like", " your", " b", "oss", "?", " Qu", "it", "."]
    TIMES = [0.16, 0.32, 0.40, 0.48, 0.48, 0.72, 0.88, 1.04, 1.20, 1.52, 1.60, 1.76]

    def test_merges_pieces_and_punctuation_into_words(self):
        words = transcribe.tokens_to_words(self.TOKENS, self.TIMES, 0.0)
        self.assertEqual([w["text"] for w in words], ["You", "don't", "like", "your", "boss?", "Quit."])

    def test_times_are_monotonic_and_non_overlapping(self):
        words = transcribe.tokens_to_words(self.TOKENS, self.TIMES, 10.0)
        self.assertEqual(words[0]["start"], 10.16)
        for a, b in zip(words, words[1:]):
            self.assertLessEqual(a["end"], b["start"])
            self.assertGreater(a["end"], a["start"])

    def test_sentencepiece_marker_is_also_a_word_start(self):
        words = transcribe.tokens_to_words(["▁hello", "▁wor", "ld"], [0.0, 0.5, 0.6], 0.0)
        self.assertEqual([w["text"] for w in words], ["hello", "world"])

    def test_blank_tokens_are_ignored(self):
        words = transcribe.tokens_to_words(["<blk>", " a", "<blk>", " b"], [0.0, 0.1, 0.2, 0.3], 0.0)
        self.assertEqual([w["text"] for w in words], ["a", "b"])


class TestIntakeHelpers(unittest.TestCase):
    def test_kind_of(self):
        self.assertEqual(media_intake.kind_of("x/IMG_1.MOV"), "video")
        self.assertEqual(media_intake.kind_of("x/IMG_1.HEIC"), "photo")
        self.assertIsNone(media_intake.kind_of("x/notes.md"))

    def test_iso6709(self):
        self.assertEqual(media_intake.parse_iso6709("+43.6532-079.3832/"), [43.6532, -79.3832])
        self.assertIsNone(media_intake.parse_iso6709(None))

    def test_dhash_and_hamming_detect_near_duplicates(self):
        from PIL import Image, ImageDraw
        a = Image.new("RGB", (200, 120), (20, 40, 200))
        ImageDraw.Draw(a).rectangle([20, 20, 120, 100], fill=(240, 200, 30))
        b = a.resize((100, 60))                      # same picture, downscaled
        c = Image.new("RGB", (200, 120), (250, 250, 250))
        ImageDraw.Draw(c).ellipse([60, 10, 190, 110], fill=(10, 10, 10))
        ha, hb, hc = media_intake.dhash(a), media_intake.dhash(b), media_intake.dhash(c)
        self.assertLessEqual(media_intake.hamming(ha, hb), media_intake.NEAR_DUP_HAMMING)
        self.assertGreater(media_intake.hamming(ha, hc), media_intake.NEAR_DUP_HAMMING)

    def test_slugify(self):
        self.assertEqual(media_intake.slugify("Toronto Move, Sept 2026!"), "toronto-move-sept-2026")

    def test_browser_codecs_exclude_hevc(self):
        self.assertIn("h264", media_intake.BROWSER_CODECS)
        self.assertNotIn("hevc", media_intake.BROWSER_CODECS)


if __name__ == "__main__":
    unittest.main()
