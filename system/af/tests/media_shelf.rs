//! Media-shelf notes read hand-edited cards with PyYAML's YAML 1.1 typing.

mod common;
use afcore::media_shelf::media_shelf_notes;
use common::*;

fn card(fx: &Fx, extra: &str) -> Vec<String> {
    fx.write("library/assets/media/x.png", "x");
    fx.write("library/assets/media/cards/a.md", &format!("---\nsha256: abc\npath: x.png\nkind: photo\n{extra}---\n# card\n"));
    media_shelf_notes(&fx.ctx())
}

#[test]
fn test_consent_no_is_false_so_the_consent_note_fires() {
    let fx = Fx::new();
    let notes = card(&fx, "people: others\nused_in: [p]\nconsent: no\n");
    assert_eq!(notes, vec!["media cards/abc.md: people: others used in ['p'] with no consent note".to_string()]);
}

#[test]
fn test_unquoted_timestamp_is_a_datetime_and_fails_the_check() {
    let fx = Fx::new();
    let notes = card(&fx, "review: pending\ningested_at: 2026-01-01T10:00:00-07:00\n");
    assert_eq!(
        notes,
        vec!["media shelf check failed: '<' not supported between instances of 'datetime.datetime' and 'str'".to_string()]
    );
}

#[test]
fn test_trailing_block_scalar_has_no_final_newline() {
    let fx = Fx::new();
    let notes = card(&fx, "people: others\nused_in: |\n  campaign\n");
    assert_eq!(notes, vec!["media cards/abc.md: people: others used in campaign with no consent note".to_string()]);
}

#[test]
fn test_tab_between_tokens_fails_like_pyyaml() {
    let fx = Fx::new();
    let notes = card(&fx, "people: others\t\n");
    assert_eq!(notes.len(), 1);
    assert!(notes[0].starts_with(
        "media shelf check failed: while scanning for the next token\nfound character '\\t' that cannot start any token\n  in \"<unicode string>\", line 4, column 15:"
    ));
}
