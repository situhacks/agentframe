import json
import os
import subprocess
import sys
import tempfile
import unittest

from system import voice_lint as vl

ROOT = vl.ROOT

VOICED = "---\nstatus: drafting\nvoice:\n  base_register: informal\n---\n"
PLAIN = "---\nstatus: drafting\n---\n"


def voiced(body):
    return VOICED + body


def write(path, text):
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(text)


class UserVoicedTests(unittest.TestCase):
    def test_voice_block_marks_user_voiced(self):
        self.assertTrue(vl.lint(voiced("Fine prose."))["user_voiced"])

    def test_register_scalar_marks_user_voiced(self):
        self.assertTrue(vl.lint("---\nregister: formal\n---\nFine prose.")["user_voiced"])

    def test_plain_deliverable_is_skipped(self):
        result = vl.lint(PLAIN + "Your brain quietly checks out.")
        self.assertFalse(result["user_voiced"])
        self.assertEqual(result["hard"], [])


class BannedTicTests(unittest.TestCase):
    def test_quietly_is_hard(self):
        result = vl.lint(voiced("An automated workflow gets things right and your brain quietly checks out."))
        codes = [f["code"] for f in result["hard"]]
        self.assertEqual(codes, ["banned-tic"])
        self.assertEqual(result["hard"][0]["line"], 6)

    def test_my_read_is_hard(self):
        result = vl.lint(voiced("My honest read is that this works."))
        self.assertEqual([f["code"] for f in result["hard"]], ["banned-tic"])

    def test_quoted_speech_is_exempt(self):
        result = vl.lint(voiced('Bosworth said "keep doing it quietly" and the dashboard died.'))
        self.assertEqual(result["hard"], [])

    def test_html_comments_and_tables_are_ignored(self):
        body = "<!-- quietly, my read -->\n| quietly | x |\n\nA clean line.\n"
        self.assertEqual(vl.lint(voiced(body))["hard"], [])


class DashTests(unittest.TestCase):
    def test_two_dashes_in_one_sentence_is_hard(self):
        result = vl.lint(voiced("That's me — the person who built it — writing an essay."))
        self.assertEqual([f["code"] for f in result["hard"]], ["double-dash"])

    def test_one_dash_per_sentence_is_fine_but_consecutive_is_soft(self):
        result = vl.lint(voiced("First point — with a pivot. Second point — another pivot."))
        self.assertEqual(result["hard"], [])
        self.assertIn("dash-consecutive", [f["code"] for f in result["soft"]])

    def test_dash_rate_alarm(self):
        body = " ".join(["Short sentence — with a dash."] * 10)
        result = vl.lint(voiced(body))
        self.assertIn("dash-rate", [f["code"] for f in result["soft"]])

    def test_number_range_en_dash_does_not_count(self):
        result = vl.lint(voiced("Runs 19–34 words and stays plain."))
        self.assertEqual(result["stats"]["em_dashes"], 0)


class HyphenNormalisationTests(unittest.TestCase):
    def test_operator_hyphen_turned_into_em_dash_is_hard(self):
        prev = voiced("All press is good press even the bad ones - right? Another line.")
        head = voiced("All press is good press even the bad ones — right? Another line.")
        result = vl.lint(head, prev)
        self.assertEqual([f["code"] for f in result["hard"]], ["hyphen-normalised"])

    def test_rewritten_sentence_is_not_flagged(self):
        prev = voiced("Sonnet is fine - but do you want to think about it? More.")
        head = voiced("Sonnet is fine, but do you want to weigh the options? More.")
        self.assertEqual(vl.lint(head, prev)["hard"], [])

    def test_hyphen_shift_is_soft(self):
        prev = voiced("One - two. Three - four. Five.")
        head = voiced("One, two. Three — and four. Five.")
        codes = [f["code"] for f in vl.lint(head, prev)["soft"]]
        self.assertIn("hyphen-shift", codes)


class SoftShapeTests(unittest.TestCase):
    def test_litotes_and_thread_are_soft(self):
        result = vl.lint(voiced("Not unreasonable. Picking up that thread later. The email thread stays."))
        codes = [f["code"] for f in result["soft"]]
        self.assertIn("litotes", codes)
        self.assertEqual(codes.count("thread-as-topic"), 1)
        self.assertEqual(result["hard"], [])

    def test_second_contrastive_pivot_is_soft(self):
        body = "The fix isn't to ration, it's to route. The bill isn't the invoice, it's the unread work."
        codes = [f["code"] for f in vl.lint(voiced(body))["soft"]]
        self.assertEqual(codes.count("contrastive"), 1)


class FileTests(unittest.TestCase):
    def test_previous_version_math(self):
        with tempfile.TemporaryDirectory() as tmp:
            for n in (1, 2, 3):
                write(os.path.join(tmp, f"essay-v{n}.md"), voiced("x."))
            self.assertEqual(vl.previous_version(os.path.join(tmp, "essay-v3.md")), os.path.join(tmp, "essay-v2.md"))
            self.assertIsNone(vl.previous_version(os.path.join(tmp, "essay-v1.md")))
            self.assertIsNone(vl.previous_version(os.path.join(tmp, "essay-v9.md")))
            self.assertIsNone(vl.previous_version(os.path.join(tmp, "essay-FINAL.md")))

    def test_cli_exit_codes(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad = os.path.join(tmp, "post-v2.md")
            write(bad, voiced("Your brain quietly checks out."))
            good = os.path.join(tmp, "post-v1.md")
            write(good, voiced("Your brain checks out."))
            plain = os.path.join(tmp, "notes.md")
            write(plain, PLAIN + "quietly")
            script = os.path.join(ROOT, "system", "voice_lint.py")
            r = subprocess.run([sys.executable, script, bad, "--json"], capture_output=True, text=True, encoding="utf-8")
            self.assertEqual(r.returncode, 1)
            self.assertEqual(json.loads(r.stdout)["hard"][0]["code"], "banned-tic")
            self.assertEqual(json.loads(r.stdout)["prev"], good)
            r = subprocess.run([sys.executable, script, good], capture_output=True, text=True, encoding="utf-8")
            self.assertEqual(r.returncode, 0)
            self.assertIn("ok: no hard findings", r.stdout)
            r = subprocess.run([sys.executable, script, plain], capture_output=True, text=True, encoding="utf-8")
            self.assertEqual(r.returncode, 0)
            self.assertIn("skipped", r.stdout)


if __name__ == "__main__":
    unittest.main()
