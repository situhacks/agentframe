import unittest

from system.tools import docx_validate


class DocxValidateWrapperTests(unittest.TestCase):
    def test_child_process_forces_utf8_mode(self):
        self.assertEqual(docx_validate.validator_env()["PYTHONUTF8"], "1")

    def test_vendor_validator_path_exists(self):
        self.assertTrue(docx_validate.VALIDATOR.is_file())

    def test_compatible_runtime_is_available(self):
        self.assertTrue(docx_validate.compatible_runtime())


if __name__ == "__main__":
    unittest.main()
