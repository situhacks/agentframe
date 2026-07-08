import os
import tempfile
import time
import unittest
from pathlib import Path

from system.server.lib.surface import convert


class TestCacheKey(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.src = Path(self.tmp.name) / "deck.pptx"
        self.src.write_bytes(b"fake pptx bytes")
        self.cache = Path(self.tmp.name) / "cache"

    def tearDown(self):
        self.tmp.cleanup()

    def test_key_stable_for_unchanged_file(self):
        a = convert.cache_path(self.src, self.cache)
        b = convert.cache_path(self.src, self.cache)
        self.assertEqual(a, b)
        self.assertTrue(a.name.endswith(".pdf"))

    def test_key_changes_when_file_changes(self):
        a = convert.cache_path(self.src, self.cache)
        time.sleep(0.01)
        self.src.write_bytes(b"different pptx bytes!")
        os.utime(self.src)
        b = convert.cache_path(self.src, self.cache)
        self.assertNotEqual(a, b)

    def test_cache_hit_skips_conversion(self):
        target = convert.cache_path(self.src, self.cache)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(b"%PDF-cached")
        result = convert.convert_to_pdf(self.src, self.cache, soffice="definitely-not-a-real-binary")
        self.assertEqual(result, target)
        self.assertEqual(target.read_bytes(), b"%PDF-cached")

    def test_missing_soffice_raises_clear_error(self):
        with self.assertRaises(convert.ConversionError) as ctx:
            convert.convert_to_pdf(self.src, self.cache, soffice=None)
        self.assertIn("LibreOffice", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()
