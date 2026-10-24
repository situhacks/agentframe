//! Port of system/tests/test_design_language_check.py, plus the doctor stray-deck cases of
//! system/tests/test_ppt_master_guard.py (TestDoctorStrayDecks). The hook cases of that file
//! stay on the Python suite; hooks are not ported.

mod common;
use afcore::doctor::{design_language_notes_at, ppt_master_stray_issues};
use common::*;

struct Shelf {
    fx: Fx,
}

impl Shelf {
    fn new() -> Shelf {
        let fx = Fx::new();
        fx.mkdir("design-languages/demo-language/imagery");
        Shelf { fx }
    }

    fn write(&self, rel: &str, text: &str) {
        self.fx.write(&format!("design-languages/demo-language/{rel}"), text);
    }

    fn write_package(&self, spec: bool, svg: bool) {
        if spec {
            self.write("package/templates/design_spec.md", "kind: deck\n");
        }
        if svg {
            self.write("package/templates/cover.svg", "<svg/>");
        } else {
            self.fx.mkdir("design-languages/demo-language/package/templates");
        }
    }

    fn write_manifest(&self, asset: &str, licence: &str, restriction: &str, on_disk: bool) {
        if on_disk {
            self.write(&format!("imagery/{asset}"), "x");
        }
        self.write(
            "imagery/manifest.yaml",
            &format!(
                "assets:\n  - path: {asset}\n    source: derived\n    licence: {licence}\n    restriction: {restriction}\n    role: derived\n"
            ),
        );
    }

    fn manifest(&self) {
        self.write_manifest("orb.png", "pexels", "none", true);
    }

    fn notes(&self) -> Vec<String> {
        design_language_notes_at(&self.fx.p("design-languages")).unwrap()
    }
}

#[test]
fn test_conformant_package_is_silent() {
    let s = Shelf::new();
    s.write("README.md", "Provenance and reuse notes.");
    s.write_package(true, true);
    s.manifest();
    assert_eq!(s.notes(), Vec::<String>::new());
}

#[test]
fn test_missing_readme_is_reported() {
    let s = Shelf::new();
    s.write_package(true, true);
    s.manifest();
    assert!(any_contains(&s.notes(), "no README.md"));
}

#[test]
fn test_missing_manifest_is_reported() {
    let s = Shelf::new();
    s.write("README.md", "notes");
    s.write_package(true, true);
    assert!(any_contains(&s.notes(), "no imagery/manifest.yaml"));
}

#[test]
fn test_package_without_design_spec_is_reported() {
    let s = Shelf::new();
    s.write("README.md", "notes");
    s.write_package(false, true);
    s.manifest();
    assert!(any_contains(&s.notes(), "no design_spec.md"));
}

#[test]
fn test_package_without_svg_roster_is_reported() {
    let s = Shelf::new();
    s.write("README.md", "notes");
    s.write_package(true, false);
    s.manifest();
    assert!(any_contains(&s.notes(), "no SVG prototypes"));
}

#[test]
fn test_promoted_exports_are_reported() {
    let s = Shelf::new();
    s.write("README.md", "notes");
    s.write_package(true, true);
    s.manifest();
    s.write("package/exports/deck.pptx", "x");
    let notes = s.notes();
    assert!(notes.iter().any(|n| n.contains("exports were promoted") || n.contains("exports was promoted")));
}

#[test]
fn test_packageless_language_must_declare_reference_grade_with_a_row() {
    let s = Shelf::new();
    s.write("README.md", "Just a look we like.");
    s.manifest();
    assert!(any_contains(&s.notes(), "reference-grade"));
}

#[test]
fn test_declared_reference_grade_with_row_is_accepted() {
    let s = Shelf::new();
    s.write("README.md", "Reference-grade until promoted. Tracked as BB-2026-08-12-04.");
    s.manifest();
    assert_eq!(s.notes(), Vec::<String>::new());
}

#[test]
fn test_manifest_record_missing_from_disk_is_reported() {
    let s = Shelf::new();
    s.write("README.md", "notes");
    s.write_package(true, true);
    s.write_manifest("orb.png", "pexels", "none", false);
    assert!(any_contains(&s.notes(), "not on disk"));
}

#[test]
fn test_unknown_licence_marked_unrestricted_is_reported() {
    let s = Shelf::new();
    s.write("README.md", "notes");
    s.write_package(true, true);
    s.write_manifest("orb.png", "unknown", "none", true);
    assert!(any_contains(&s.notes(), "stays project-scoped"));
}

#[test]
fn test_unknown_licence_kept_project_scoped_is_accepted() {
    let s = Shelf::new();
    s.write("README.md", "notes");
    s.write_package(true, true);
    s.write_manifest("orb.png", "unknown", "project-scoped", true);
    assert_eq!(s.notes(), Vec::<String>::new());
}

#[test]
fn test_empty_shelf_is_silent() {
    // The shelf is gitignored, so a downstream copy has none of these.
    let s = Shelf::new();
    s.fx.rmtree("design-languages/demo-language");
    assert_eq!(s.notes(), Vec::<String>::new());
}

// ------------------------------------------------------------------ TestDoctorStrayDecks

fn stray_fx() -> Fx {
    let fx = Fx::new();
    fx.mkdir("system/vendor/ppt-master");
    fx
}

#[test]
fn test_stray_project_dir_is_issue() {
    let fx = stray_fx();
    fx.mkdir("system/vendor/ppt-master/projects/demo/exports");
    assert!(any_contains(&ppt_master_stray_issues(&fx.root()).unwrap(), "projects"));
}

#[test]
fn test_stray_pptx_is_issue() {
    let fx = stray_fx();
    fx.write_bytes("system/vendor/ppt-master/templates/decks/deck_20260709.pptx", b"");
    assert!(any_contains(&ppt_master_stray_issues(&fx.root()).unwrap(), "deck_20260709.pptx"));
}

#[test]
fn test_system_projects_dir_is_issue() {
    let fx = stray_fx();
    fx.mkdir("system/projects/demo/exports");
    let issues = ppt_master_stray_issues(&fx.root()).unwrap();
    assert!(issues.iter().any(|i| i.starts_with("system/projects:")));
}

#[test]
fn test_clean_tree_no_issues() {
    let fx = stray_fx();
    assert_eq!(ppt_master_stray_issues(&fx.root()).unwrap(), Vec::<String>::new());
}

#[test]
fn test_missing_skill_dir_no_issues() {
    let fx = stray_fx();
    assert_eq!(ppt_master_stray_issues(&fx.p("nope")).unwrap(), Vec::<String>::new());
}
