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
        result = convert.convert_to_pdf(
            self.src, self.cache, renderer="definitely-not-a-real-script"
        )
        self.assertEqual(result, target)
        self.assertEqual(target.read_bytes(), b"%PDF-cached")

    def test_missing_renderer_raises_actionable_error(self):
        with self.assertRaises(convert.ConversionError) as ctx:
            convert.convert_to_pdf(self.src, self.cache, renderer=None)
        message = str(ctx.exception)
        self.assertIn("office_render.ps1", message)
        self.assertIn("PowerPoint", message)

    def test_error_never_offers_libreoffice_as_a_fallback(self):
        """The whole point of the native path: no silent degradation route."""
        with self.assertRaises(convert.ConversionError) as ctx:
            convert.convert_to_pdf(self.src, self.cache, renderer=None)
        self.assertNotIn("soffice", str(ctx.exception).lower())

    def test_unsupported_suffix_rejected_before_launching_office(self):
        odd = Path(self.tmp.name) / "notes.txt"
        odd.write_bytes(b"plain text")
        with self.assertRaises(convert.ConversionError) as ctx:
            convert.convert_to_pdf(odd, self.cache, renderer=None)
        self.assertIn("not a PowerPoint or Word file", str(ctx.exception))

    def test_converter_version_retires_libreoffice_era_cache(self):
        """v1 keys were produced by LibreOffice; they must not be reused."""
        self.assertGreaterEqual(convert.CONVERTER_VERSION, 2)

    def test_renderer_path_points_at_the_native_tool(self):
        self.assertTrue(
            convert.RENDERER.is_file(), f"missing native renderer: {convert.RENDERER}"
        )


if __name__ == "__main__":
    unittest.main()
