import os
import tempfile
import unittest
from unittest.mock import patch

from system import af


MANIFEST = """assets:
  - path: {asset}
    source: derived
    licence: {licence}
    restriction: {restriction}
    role: derived
"""


class DesignLanguageNotesTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.shelf = os.path.join(self.tmp.name, "design-languages")
        self.base = os.path.join(self.shelf, "demo-language")
        os.makedirs(os.path.join(self.base, "imagery"))
        self.patcher = patch.object(af, "DESIGN_LANGUAGES", self.shelf)
        self.patcher.start()

    def tearDown(self):
        self.patcher.stop()
        self.tmp.cleanup()

    # ---------------------------------------------------------------- helpers
    def write(self, rel, text=""):
        path = os.path.join(self.base, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(text)
        return path

    def write_package(self, spec=True, svg=True):
        if spec:
            self.write("package/templates/design_spec.md", "kind: deck\n")
        if svg:
            self.write("package/templates/cover.svg", "<svg/>")
        else:
            os.makedirs(os.path.join(self.base, "package", "templates"), exist_ok=True)

    def write_manifest(self, asset="orb.png", licence="pexels", restriction="none", on_disk=True):
        if on_disk:
            self.write(os.path.join("imagery", asset), "x")
        self.write(
            "imagery/manifest.yaml",
            MANIFEST.format(asset=asset, licence=licence, restriction=restriction),
        )

    def notes(self):
        return af.design_language_notes()

    # ------------------------------------------------------- the clean baseline
    def test_conformant_package_is_silent(self):
        self.write("README.md", "Provenance and reuse notes.")
        self.write_package()
        self.write_manifest()
        self.assertEqual(self.notes(), [])

    # ------------------------------------------------------------- structural
    def test_missing_readme_is_reported(self):
        self.write_package()
        self.write_manifest()
        self.assertTrue(any("no README.md" in n for n in self.notes()))

    def test_missing_manifest_is_reported(self):
        self.write("README.md", "notes")
        self.write_package()
        self.assertTrue(any("no imagery/manifest.yaml" in n for n in self.notes()))

    def test_package_without_design_spec_is_reported(self):
        self.write("README.md", "notes")
        self.write_package(spec=False)
        self.write_manifest()
        self.assertTrue(any("no design_spec.md" in n for n in self.notes()))

    def test_package_without_svg_roster_is_reported(self):
        self.write("README.md", "notes")
        self.write_package(svg=False)
        self.write_manifest()
        self.assertTrue(any("no SVG prototypes" in n for n in self.notes()))

    def test_promoted_exports_are_reported(self):
        self.write("README.md", "notes")
        self.write_package()
        self.write_manifest()
        self.write("package/exports/deck.pptx", "x")
        self.assertTrue(any("exports were promoted" in n or "exports was promoted" in n
                            for n in self.notes()))

    # ------------------------------------------- the reference-grade waypoint
    def test_packageless_language_must_declare_reference_grade_with_a_row(self):
        self.write("README.md", "Just a look we like.")
        self.write_manifest()
        self.assertTrue(any("reference-grade" in n for n in self.notes()))

    def test_declared_reference_grade_with_row_is_accepted(self):
        self.write("README.md", "Reference-grade until promoted. Tracked as BB-2026-08-12-04.")
        self.write_manifest()
        self.assertEqual(self.notes(), [])

    # --------------------------------------------------------- licence hygiene
    def test_manifest_record_missing_from_disk_is_reported(self):
        self.write("README.md", "notes")
        self.write_package()
        self.write_manifest(on_disk=False)
        self.assertTrue(any("not on disk" in n for n in self.notes()))

    def test_unknown_licence_marked_unrestricted_is_reported(self):
        self.write("README.md", "notes")
        self.write_package()
        self.write_manifest(licence="unknown", restriction="none")
        self.assertTrue(any("stays project-scoped" in n for n in self.notes()))

    def test_unknown_licence_kept_project_scoped_is_accepted(self):
        self.write("README.md", "notes")
        self.write_package()
        self.write_manifest(licence="unknown", restriction="project-scoped")
        self.assertEqual(self.notes(), [])

    # ------------------------------------------------------------- empty shelf
    def test_empty_shelf_is_silent(self):
        # The shelf is gitignored, so a downstream copy has none of these.
        import shutil
        shutil.rmtree(self.base)
        self.assertEqual(self.notes(), [])


if __name__ == "__main__":
    unittest.main()
