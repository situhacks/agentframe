//! Port of system/tests/test_careers_rules.py against the compiled-in careers rules.

use afcore::packs::careers::{lint, year_only_ranges};

fn assert_clean(text: &str) {
    assert_eq!(year_only_ranges(text), Vec::<String>::new(), "false positive on {text:?}");
}

fn assert_flagged(text: &str) {
    assert!(!year_only_ranges(text).is_empty(), "missed year-only range in {text:?}");
}

#[test]
fn test_month_year_to_present_is_clean() {
    // The regression: every current role tripped the old pattern.
    assert_clean("September 2022 - Present");
    assert_clean("Sept. 2022 - Present");
    assert_clean("Sep 2022 - present");
    assert_clean("May 2019 - Present");
}

#[test]
fn test_month_year_to_month_year_is_clean() {
    assert_clean("August 2021 - August 2022");
    assert_clean("Jan 2020 - Dec 2021");
    assert_clean("June 2018 - March 2019");
}

#[test]
fn test_bare_year_ranges_are_flagged() {
    assert_flagged("2022 - Present");
    assert_flagged("2019-2021");
    assert_flagged("2019 – 2021");
    assert_flagged("Consultant, 2022 - 2023");
}

#[test]
fn test_month_like_word_is_not_a_month() {
    // "Marketing" starts with "mar"; it must not shield a bare year.
    assert_flagged("Marketing 2022 - 2023");
    assert_flagged("Maybe 2019 - 2021");
}

#[test]
fn test_right_side_month_does_not_excuse_a_bare_left_year() {
    assert_flagged("2021 - August 2022");
}

#[test]
fn test_reported_match_names_the_offending_range() {
    assert_eq!(year_only_ranges("Northwind 2019-2021, Riverton"), ["2019-2021"]);
}

#[test]
fn test_resume_body_lint_end_to_end() {
    let body = "## Work Experience\n\nConsultant, September 2022 - Present\nAnalyst, 2019 - 2022\n";
    let issues = lint("resume-v1.md", body, true);
    let date_issues: Vec<&String> = issues.iter().filter(|i| i.contains("year-only")).collect();
    assert_eq!(date_issues.len(), 1, "{issues:?}");
    assert!(date_issues[0].contains("2019 - 2022"));
}
