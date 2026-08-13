import importlib.util
import os
import unittest

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
RULES_PATH = os.path.join(REPO_ROOT, "library", "domains", "careers", "rules.py")

_spec = importlib.util.spec_from_file_location("careers_rules", RULES_PATH)
rules = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(rules)


class YearOnlyRangeTests(unittest.TestCase):
    """The lint must accept the date format the resume template mandates."""

    def assert_clean(self, text):
        self.assertEqual([], rules.year_only_ranges(text), f"false positive on {text!r}")

    def assert_flagged(self, text):
        self.assertTrue(rules.year_only_ranges(text), f"missed year-only range in {text!r}")

    def test_month_year_to_present_is_clean(self):
        # The regression: every current role tripped the old pattern.
        self.assert_clean("September 2022 - Present")
        self.assert_clean("Sept. 2022 - Present")
        self.assert_clean("Sep 2022 - present")
        self.assert_clean("May 2019 - Present")

    def test_month_year_to_month_year_is_clean(self):
        self.assert_clean("August 2021 - August 2022")
        self.assert_clean("Jan 2020 - Dec 2021")
        self.assert_clean("June 2018 - March 2019")

    def test_bare_year_ranges_are_flagged(self):
        self.assert_flagged("2022 - Present")
        self.assert_flagged("2019-2021")
        self.assert_flagged("2019 – 2021")
        self.assert_flagged("Consultant, 2022 - 2023")

    def test_month_like_word_is_not_a_month(self):
        # "Marketing" starts with "mar"; it must not shield a bare year.
        self.assert_flagged("Marketing 2022 - 2023")
        self.assert_flagged("Maybe 2019 - 2021")

    def test_right_side_month_does_not_excuse_a_bare_left_year(self):
        self.assert_flagged("2021 - August 2022")

    def test_reported_match_names_the_offending_range(self):
        self.assertEqual(["2019-2021"], rules.year_only_ranges("Northwind 2019-2021, Riverton"))

    def test_resume_body_lint_end_to_end(self):
        body = "## Work Experience\n\nConsultant, September 2022 - Present\nAnalyst, 2019 - 2022\n"
        issues = rules._lint("resume-v1.md", body, is_resume=True)
        date_issues = [i for i in issues if "year-only" in i]
        self.assertEqual(1, len(date_issues), issues)
        self.assertIn("2019 - 2022", date_issues[0])


if __name__ == "__main__":
    unittest.main()
