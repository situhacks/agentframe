import os
import tempfile
import unittest

from system import voice_bundle as vb


def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(text)


def seed(voice_dir):
    for rel, text in {
        "README.md": "# procedure\n",
        "identity.md": "# identity\n",
        "voice-profile.md": "# profile\n",
        "anti-patterns.md": "# anti\n",
        "registers/informal.md": "# informal register\n",
        "registers/formal.md": "# formal register\n",
        "pairs/informal.md": "### pair-a\n",
        "pairs/plain-not-clever.md": "### pair-b\n",
        "pairs/builder-pov.md": "### pair-c\n",
        "templates/substack-essay.md": "# essay shape\n",
        "corpus/informal/2026-02-02-second.md": "Provenance: b\n\nSecond piece.\n",
        "corpus/informal/2026-01-01-first.md": "Provenance: a\n\nFirst piece.\n",
        "corpus/informal/README.md": "not a piece\n",
    }.items():
        write(os.path.join(voice_dir, rel), text)


class BundleTests(unittest.TestCase):
    def test_order_and_manifest(self):
        with tempfile.TemporaryDirectory() as tmp:
            voice, out = os.path.join(tmp, "voice"), os.path.join(tmp, "out")
            seed(voice)
            result = vb.build("informal", ["long-form"], template="substack-essay.md", voice_dir=voice, out_dir=out)
            labels = [label for label, _ in result["parts"]]
            self.assertEqual(labels, [
                "procedure", "identity", "profile", "anti-patterns", "register informal",
                "template substack-essay.md", "pairs informal.md", "pairs plain-not-clever.md",
                "corpus 2026-01-01-first.md", "corpus 2026-02-02-second.md",
            ])
            self.assertEqual(result["missing"], [])
            text = open(result["path"], encoding="utf-8").read()
            self.assertTrue(text.startswith("<!-- VOICE BUNDLE"))
            self.assertIn("Read this file whole", text)
            self.assertIn(vb.DENSITY_LINE, text)
            self.assertLess(text.index("First piece."), text.index("Second piece."))
            self.assertNotIn("not a piece", text)
            self.assertTrue(result["path"].endswith(os.path.join("out", "informal-long-form.md")))

    def test_borrow_and_context_pairs_dedupe(self):
        with tempfile.TemporaryDirectory() as tmp:
            voice, out = os.path.join(tmp, "voice"), os.path.join(tmp, "out")
            seed(voice)
            result = vb.build("informal", ["builder-pov", "long-form"], borrow=["formal"], voice_dir=voice, out_dir=out)
            labels = [label for label, _ in result["parts"]]
            self.assertIn("borrow register formal", labels)
            self.assertEqual(labels.count("pairs plain-not-clever.md"), 1)
            self.assertIn("pairs builder-pov.md", labels)
            self.assertEqual([m for m, _ in result["missing"]], ["pairs formal.md"])

    def test_missing_files_are_reported_not_fatal(self):
        with tempfile.TemporaryDirectory() as tmp:
            voice, out = os.path.join(tmp, "voice"), os.path.join(tmp, "out")
            seed(voice)
            result = vb.build("formal", voice_dir=voice, out_dir=out)
            self.assertIn("pairs formal.md", [m for m, _ in result["missing"]])
            self.assertTrue(any(label == "register formal" for label, _ in result["parts"]))
            report = vb.format_report(result)
            self.assertIn("MISSING", report)
            self.assertIn("Read it whole", report)


class RecipeTests(unittest.TestCase):
    def test_voice_block_and_type(self):
        with tempfile.TemporaryDirectory() as tmp:
            head = os.path.join(tmp, "essay-v3.md")
            write(head, "---\nstatus: drafting\ntype: substack-essay\nregister: substack-informal\nvoice:\n  base_register: informal\n  borrow_from: [formal]\n---\nBody.\n")
            base, borrow, dtype = vb.recipe_from_head(head)
            self.assertEqual((base, borrow, dtype), ("informal", ["formal"], "substack-essay"))
            self.assertEqual(vb.TYPE_TEMPLATE[dtype], "substack-essay.md")
            self.assertEqual(vb.TYPE_CONTEXT[dtype], "long-form")

    def test_register_scalar_only(self):
        with tempfile.TemporaryDirectory() as tmp:
            head = os.path.join(tmp, "note-v1.md")
            write(head, "---\nregister: formal\n---\nBody.\n")
            self.assertEqual(vb.recipe_from_head(head), ("formal", [], None))

    def test_no_recipe(self):
        with tempfile.TemporaryDirectory() as tmp:
            head = os.path.join(tmp, "plain-v1.md")
            write(head, "---\nstatus: drafting\n---\nBody.\n")
            self.assertEqual(vb.recipe_from_head(head), (None, [], None))


if __name__ == "__main__":
    unittest.main()
