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
agy_call = load("agy_call")


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
        self.assertEqual(media_intake.slugify("City Move, Sept 2026!"), "city-move-sept-2026")

    def test_browser_codecs_exclude_hevc(self):
        self.assertIn("h264", media_intake.BROWSER_CODECS)
        self.assertNotIn("hevc", media_intake.BROWSER_CODECS)


class TestAgyCallFence(unittest.TestCase):
    def test_prompt_names_absolute_paths_and_forbids_search(self):
        p = agy_call.build_prompt("Describe it.", [r"C:\x\clip.mp4"])
        self.assertIn(r"C:\x\clip.mp4", p)
        self.assertIn("do not search", p)
        self.assertTrue(p.endswith("Describe it."))
        self.assertEqual(agy_call.build_prompt("Plain.", []), "Plain.")

    def test_command_never_skips_permissions_by_default(self):
        cmd = agy_call.build_command("agy", "p", r"C:\tmp\w", None, "gemini-3.8-flash-medium", "5m", False)
        self.assertNotIn("--dangerously-skip-permissions", cmd)
        self.assertNotIn("--effort", cmd)
        self.assertIn("--add-dir", cmd)
        self.assertEqual(cmd[cmd.index("--add-dir") + 1], r"C:\tmp\w")
        self.assertIn("--disable-slash-commands", cmd)
        cmd2 = agy_call.build_command("agy", "p", "w", "{}", "gemini-3.8-flash-low", "5m", True)
        self.assertIn("--dangerously-skip-permissions", cmd2)
        self.assertIn("--json-schema", cmd2)

    def test_stage_copies_read_only_into_workspace(self):
        import stat
        import tempfile
        with tempfile.TemporaryDirectory() as src_dir, tempfile.TemporaryDirectory() as work:
            src = os.path.join(src_dir, "a.mp4")
            with open(src, "wb") as fh:
                fh.write(b"bytes")
            staged = agy_call.stage([src], work)
            self.assertEqual(len(staged), 1)
            self.assertTrue(staged[0].startswith(work))
            self.assertFalse(os.stat(staged[0]).st_mode & stat.S_IWRITE)
            with self.assertRaises(PermissionError):
                open(staged[0], "wb")
            os.chmod(staged[0], stat.S_IWRITE)



class TestAgyCallWeb(unittest.TestCase):
    """--web with the agy process mocked: no live call."""

    def _run_mocked(self, **kw):
        import json
        import subprocess
        from unittest import mock
        seen = {}

        def fake_run(cmd, **_kw):
            seen["cmd"] = cmd
            return subprocess.CompletedProcess(cmd, 0, stdout=json.dumps({"status": "SUCCESS", "response": "ok"}), stderr="")

        with mock.patch.object(agy_call, "find_agy", return_value="agy"), \
                mock.patch.object(agy_call.subprocess, "run", side_effect=fake_run):
            env = agy_call.run("Find it.", **kw)
        return seen["cmd"], env

    def test_web_prompt_leads_with_the_one_tool_and_forbids_shell(self):
        p = agy_call.build_prompt("Find it.", [], web=True)
        self.assertTrue(p.startswith(agy_call.WEB_PREAMBLE))
        self.assertTrue(p.endswith("Find it."))
        for word in ("search_web", "shell", "MCP", "browser", "no files"):
            self.assertIn(word, agy_call.WEB_PREAMBLE)
        # without --web the prompt is untouched
        self.assertEqual(agy_call.build_prompt("Find it.", []), "Find it.")

    def test_web_keeps_the_staged_file_lines_after_the_preamble(self):
        p = agy_call.build_prompt("Find it.", ["/tmp/w/a.pdf"], web=True)
        self.assertLess(p.index("search_web"), p.index("/tmp/w/a.pdf"))

    def test_web_run_adds_only_the_preamble(self):
        web_cmd, _ = self._run_mocked(web=True)
        plain_cmd, _ = self._run_mocked()
        web_prompt = web_cmd[web_cmd.index("-p") + 1]
        plain_prompt = plain_cmd[plain_cmd.index("-p") + 1]
        self.assertEqual(web_prompt, agy_call.WEB_PREAMBLE + "\n\n" + plain_prompt)

        def without_prompt(cmd):
            i = cmd.index("-p")
            rest = cmd[:i + 1] + cmd[i + 2:]
            # the temp workspace path differs per run
            j = rest.index("--add-dir")
            return rest[:j + 1] + rest[j + 2:]

        self.assertEqual(without_prompt(web_cmd), without_prompt(plain_cmd))
        self.assertIn("--add-dir", web_cmd)   # the fence stays

    def test_default_run_has_no_web_preamble(self):
        cmd, _ = self._run_mocked()
        self.assertNotIn("search_web", cmd[cmd.index("-p") + 1])

    def test_web_refuses_allow_commands(self):
        from unittest import mock
        with mock.patch.object(agy_call, "find_agy", return_value="agy"), \
                mock.patch.object(agy_call.subprocess, "run") as sp:
            with self.assertRaises(SystemExit):
                agy_call.run("Find it.", web=True, allow_commands=True)
            sp.assert_not_called()

    def test_cli_flag_reaches_run_and_refuses_the_combination(self):
        import sys
        from unittest import mock
        with mock.patch.object(agy_call, "run", return_value={"status": "SUCCESS", "response": "x"}) as r, \
                mock.patch.object(agy_call.subprocess, "run") as sp, \
                mock.patch.object(sys, "argv", ["agy_call.py", "--prompt", "q", "--web"]):
            with self.assertRaises(SystemExit) as ex:
                agy_call.main()
        self.assertEqual(ex.exception.code, 0)
        self.assertIs(r.call_args.args[-1], True)   # web
        self.assertIs(r.call_args.args[-2], False)  # allow_commands
        sp.assert_not_called()
        # a regression must never reach a real agy with --dangerously-skip-permissions
        with mock.patch.object(agy_call, "find_agy", return_value="agy"), \
                mock.patch.object(agy_call.subprocess, "run") as sp, \
                mock.patch.object(sys, "argv", ["agy_call.py", "--prompt", "q", "--web", "--allow-commands"]):
            with self.assertRaises(SystemExit) as ex:
                agy_call.main()
        self.assertNotEqual(ex.exception.code, 0)
        sp.assert_not_called()

    def test_the_docs_say_what_is_blocked_and_what_is_only_steered(self):
        doc = agy_call.__doc__
        src = open(agy_call.__file__, encoding="utf-8").read()
        self.assertNotIn("nothing broader", doc)
        self.assertNotIn("and nothing else", src)
        self.assertNotIn("let a command through", doc)
        self.assertIn("What is blocked: shell commands", doc)
        self.assertIn("only steered", doc)
        self.assertIn("deny list had no effect", doc)


if __name__ == "__main__":
    unittest.main()
