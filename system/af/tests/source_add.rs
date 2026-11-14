//! `af source add`: the next source id is taken under a lock, the file and its INDEX row are written
//! together, and a refusal writes nothing.

mod common;
use common::*;

const DATE: &str = "2026-11-14";

fn layer(fx: &Fx) {
    fx.mkdir("workspace/life/sources");
}

fn body_file(fx: &Fx, name: &str, bytes: &[u8]) -> String {
    fx.write_bytes(&format!("in/{name}"), bytes)
}

fn add(fx: &Fx, slug: &str, extra: &[&str]) -> (i32, String, String) {
    let input = body_file(fx, "body.txt", b"the input\n");
    let mut args = vec!["source", "add", "--type", "note", "--date", DATE, "--slug", slug, "--summary", "a note", "--body-file", &input];
    args.extend_from_slice(extra);
    fx.run(&args)
}

fn ids(fx: &Fx) -> Vec<String> {
    fx.read("workspace/life/sources/INDEX.md")
        .lines()
        .filter_map(|l| l.split('|').nth(1))
        .map(|c| c.trim().trim_matches('`').to_string())
        .filter(|c| c.starts_with("src-"))
        .collect()
}

fn files_in_sources(fx: &Fx) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let base = fx.path("workspace/life/sources");
    if base.is_dir() {
        walk(&base, &base, &mut out);
    }
    out.sort();
    out
}

#[test]
fn two_calls_for_one_date_give_01_and_02_two_files_two_rows_and_a_clean_doctor() {
    let fx = Fx::new();
    layer(&fx);
    let first = fx.ok(&["source", "add", "--type", "note", "--date", DATE, "--slug", "one", "--summary", "first", "--body-file", &body_file(&fx, "a.txt", b"alpha\n")]);
    let second = fx.ok(&["source", "add", "--type", "receipt", "--date", DATE, "--slug", "two", "--summary", "second", "--body-file", &body_file(&fx, "b.txt", b"beta\n")]);
    assert_eq!(first.replace("\r\n", "\n"), format!("src-{DATE}-01\nworkspace/life/sources/note-{DATE}-one.md\n"));
    assert_eq!(second.replace("\r\n", "\n"), format!("src-{DATE}-02\nworkspace/life/sources/receipt-{DATE}-two.md\n"));
    assert_eq!(ids(&fx), [format!("src-{DATE}-01"), format!("src-{DATE}-02")]);
    assert_eq!(files_in_sources(&fx), ["INDEX.md".to_string(), format!("note-{DATE}-one.md"), format!("receipt-{DATE}-two.md")]);
    let index = fx.read("workspace/life/sources/INDEX.md");
    assert!(index.contains(&format!("| `src-{DATE}-01` | [`note-{DATE}-one.md`](note-{DATE}-one.md) | {DATE} | first |")), "{index}");
    let doctor = fx.af().arg("doctor").output().unwrap();
    let text = String::from_utf8_lossy(&doctor.stdout).into_owned();
    assert!(!text.contains("src-") && !text.contains("INDEX.md"), "{text}");
}

#[test]
fn an_id_used_only_in_a_subfolders_frontmatter_is_skipped() {
    let fx = Fx::new();
    layer(&fx);
    fx.write("workspace/life/sources/inbox/old.md", &format!("---\nid: src-{DATE}-03\ntype: note\n---\nbody\n"));
    let out = fx.ok(&["source", "add", "--type", "note", "--date", DATE, "--slug", "x", "--summary", "s", "--body-file", &body_file(&fx, "a.txt", b"x")]);
    assert!(out.starts_with(&format!("src-{DATE}-04")), "{out}");
}

#[test]
fn an_id_used_only_in_the_index_is_skipped() {
    let fx = Fx::new();
    layer(&fx);
    fx.write(
        "workspace/life/sources/INDEX.md",
        &format!("# Source Index\n\n| ID | File | Date | Description |\n|---|---|---|---|\n| `src-{DATE}-07` | [`gone.md`](gone.md) | {DATE} | moved away |\n"),
    );
    let out = fx.ok(&["source", "add", "--type", "note", "--date", DATE, "--slug", "x", "--summary", "s", "--body-file", &body_file(&fx, "a.txt", b"x")]);
    assert!(out.starts_with(&format!("src-{DATE}-08")), "{out}");
}

#[test]
fn an_id_used_only_in_an_orders_file_name_is_skipped() {
    let fx = Fx::new();
    layer(&fx);
    fx.write(&format!("workspace/life/sources/orders/order-{DATE}-05-groceries.md"), "no frontmatter here\n");
    let out = fx.ok(&["source", "add", "--type", "order", "--date", DATE, "--slug", "next", "--summary", "s", "--body-file", &body_file(&fx, "a.txt", b"buy milk\n")]);
    assert!(out.starts_with(&format!("src-{DATE}-06")), "{out}");
    assert!(fx.isfile(&format!("workspace/life/sources/orders/order-{DATE}-06-next.md")));
    assert!(fx.read(&format!("workspace/life/sources/orders/order-{DATE}-06-next.md")).starts_with(&format!("---\nid: src-{DATE}-06\ntype: order\n")));
}

#[test]
fn another_date_does_not_count_and_a_hundredth_id_prints_as_is() {
    let fx = Fx::new();
    layer(&fx);
    fx.write("workspace/life/sources/other.md", "---\nid: src-2026-11-13-09\n---\n");
    let out = fx.ok(&["source", "add", "--type", "note", "--date", DATE, "--slug", "x", "--summary", "s", "--body-file", &body_file(&fx, "a.txt", b"x")]);
    assert!(out.starts_with(&format!("src-{DATE}-01\n")) || out.starts_with(&format!("src-{DATE}-01\r\n")), "{out}");
    fx.write("workspace/life/sources/big.md", &format!("---\nid: src-{DATE}-99\n---\n"));
    let out = fx.ok(&["source", "add", "--type", "note", "--date", DATE, "--slug", "y", "--summary", "s", "--body-file", &body_file(&fx, "a.txt", b"x")]);
    assert!(out.starts_with(&format!("src-{DATE}-100")), "{out}");
}

#[test]
fn eight_processes_at_once_end_with_eight_ids_files_and_rows() {
    let fx = Fx::new();
    layer(&fx);
    let input = body_file(&fx, "a.txt", b"same input\n");
    let start = std::sync::Barrier::new(8);
    let mut printed: Vec<String> = std::thread::scope(|scope| {
        let runs: Vec<_> = (0..8)
            .map(|i| {
                let (fx, input, start) = (&fx, &input, &start);
                scope.spawn(move || {
                    start.wait();
                    fx.run(&["source", "add", "--type", "note", "--date", DATE, "--slug", &format!("n{i}"), "--summary", &format!("row {i}"), "--body-file", input])
                })
            })
            .collect();
        runs.into_iter()
            .map(|r| {
                let (code, out, err) = r.join().unwrap();
                assert_eq!(code, 0, "{err}");
                out.lines().next().unwrap().to_string()
            })
            .collect()
    });
    printed.sort();
    let want: Vec<String> = (1..=8).map(|n| format!("src-{DATE}-{n:02}")).collect();
    assert_eq!(printed, want);
    let mut rows = ids(&fx);
    rows.sort();
    assert_eq!(rows, want);
    assert_eq!(files_in_sources(&fx).len(), 9, "{:?}", files_in_sources(&fx));
}

#[test]
fn a_body_file_comes_back_byte_for_byte_after_the_frontmatter() {
    let fx = Fx::new();
    layer(&fx);
    let body: &[u8] = b"  leading\r\nline two\r\n\r\n\r\n";
    fx.ok(&["source", "add", "--type", "email", "--date", DATE, "--slug", "mail", "--summary", "s", "--origin", "Gmail thread: abc", "--private", "--body-file", &body_file(&fx, "mail.txt", body)]);
    let bytes = std::fs::read(fx.path(&format!("workspace/life/sources/email-{DATE}-mail.md"))).unwrap();
    let head = format!("---\nid: src-{DATE}-01\ntype: email\ndate: {DATE}\norigin: \"Gmail thread: abc\"\nverbatim: true\nprivate: true\n---\n");
    assert_eq!(&bytes[..head.len()], head.as_bytes());
    assert_eq!(&bytes[head.len()..], body);
}

#[test]
fn the_default_origin_and_no_private_flag() {
    let fx = Fx::new();
    layer(&fx);
    add(&fx, "x", &[]);
    let text = fx.read(&format!("workspace/life/sources/note-{DATE}-x.md"));
    assert!(text.contains("origin: filed with af source add\nverbatim: true\n---\n"), "{text}");
    assert!(!text.contains("private"), "{text}");
}

#[test]
fn a_file_is_copied_unchanged_and_move_removes_the_original() {
    let fx = Fx::new();
    layer(&fx);
    let bytes: Vec<u8> = (0..=255u8).chain([0, 13, 10, 255]).collect();
    let pdf = fx.write_bytes("in/scan.pdf", &bytes);
    fx.ok(&["source", "add", "--type", "document", "--date", DATE, "--slug", "scan", "--summary", "a scan", "--file", &pdf]);
    assert_eq!(std::fs::read(fx.path(&format!("workspace/life/sources/document-{DATE}-scan.pdf"))).unwrap(), bytes);
    assert!(fx.isfile("in/scan.pdf"), "a copy leaves the original");
    let index = fx.read("workspace/life/sources/INDEX.md");
    assert!(index.contains(&format!("[`document-{DATE}-scan.pdf`](document-{DATE}-scan.pdf)")), "{index}");

    let again = fx.write_bytes("in/photo.png", &bytes);
    fx.ok(&["source", "add", "--type", "receipt", "--date", DATE, "--slug", "photo", "--summary", "a photo", "--file", &again, "--move"]);
    assert!(!fx.isfile("in/photo.png"), "--move removes the original");
    assert_eq!(std::fs::read(fx.path(&format!("workspace/life/sources/receipt-{DATE}-photo.png"))).unwrap(), bytes);
    assert_eq!(ids(&fx), [format!("src-{DATE}-01"), format!("src-{DATE}-02")]);
}

#[test]
fn an_order_is_text_and_a_file_for_it_is_refused() {
    let fx = Fx::new();
    layer(&fx);
    let f = fx.write("in/o.txt", "do it\n");
    fx.refused(&["source", "add", "--type", "order", "--date", DATE, "--slug", "o", "--summary", "s", "--file", &f]);
    assert_eq!(files_in_sources(&fx), Vec::<String>::new());
    assert!(fx.isfile("in/o.txt"));
}

#[test]
fn an_existing_target_name_is_refused_and_the_id_is_not_used_up() {
    let fx = Fx::new();
    layer(&fx);
    let (code, ..) = add(&fx, "same", &[]);
    assert_eq!(code, 0);
    let index_before = fx.read("workspace/life/sources/INDEX.md");
    let (code, _, err) = add(&fx, "same", &[]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("already exists"), "{err}");
    assert_eq!(fx.read("workspace/life/sources/INDEX.md"), index_before);
    assert_eq!(files_in_sources(&fx).len(), 2);
    let (code, out, _) = add(&fx, "other", &[]);
    assert_eq!(code, 0);
    assert!(out.starts_with(&format!("src-{DATE}-02")), "{out}");
}

#[test]
fn a_bad_slug_date_summary_or_missing_layer_is_refused_with_nothing_written() {
    let fx = Fx::new();
    let body = body_file(&fx, "a.txt", b"x");
    let run = |fx: &Fx, date: &str, slug: &str, summary: &str| {
        fx.run(&["source", "add", "--type", "note", "--date", date, "--slug", slug, "--summary", summary, "--body-file", &body])
    };
    let (code, _, err) = run(&fx, DATE, "x", "s");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("workspace/life/sources"), "{err}");
    assert!(!fx.exists("workspace"), "a missing layer is not created");

    layer(&fx);
    for (date, slug, summary) in [
        (DATE, "Bad Slug", "s"),
        (DATE, "under_score", "s"),
        (DATE, "", "s"),
        ("2026-02-30", "x", "s"),
        ("2026-2-3", "x", "s"),
        ("tomorrow", "x", "s"),
        (DATE, "x", "a | b"),
        (DATE, "x", "two\nlines"),
        (DATE, "x", ""),
    ] {
        let (code, _, err) = run(&fx, date, slug, summary);
        assert_eq!(code, 1, "{date} {slug} {summary:?}: {err}");
    }
    assert_eq!(files_in_sources(&fx), Vec::<String>::new());
}

#[test]
fn an_index_with_crlf_endings_gets_a_crlf_row_and_a_missing_index_gets_its_header() {
    let fx = Fx::new();
    layer(&fx);
    let (code, ..) = add(&fx, "first", &[]);
    assert_eq!(code, 0);
    let index = fx.read("workspace/life/sources/INDEX.md");
    assert!(index.starts_with("# Source Index\n\n| ID | File | Date | Description |\n|---|---|---|---|\n| `src-"), "{index}");

    let crlf = "# Source Index\r\n\r\n| ID | File | Date | Description |\r\n|---|---|---|---|\r\n";
    fx.write_bytes("workspace/life/sources/INDEX.md", crlf.as_bytes());
    let (code, ..) = add(&fx, "second", &[]);
    assert_eq!(code, 0);
    let bytes = std::fs::read(fx.path("workspace/life/sources/INDEX.md")).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.starts_with(crlf), "{text:?}");
    assert!(text.ends_with(&format!("| {DATE} | a note |\r\n")), "{text:?}");
    assert!(!text.replace("\r\n", "").contains('\n'), "every line ending stays CRLF: {text:?}");
}

#[test]
fn project_files_into_that_projects_sources() {
    let fx = Fx::new();
    layer(&fx);
    fx.mkdir("workspace/projects/acme/sources");
    let input = body_file(&fx, "a.txt", b"project text\n");
    let out = fx.ok(&["source", "add", "--type", "note", "--date", DATE, "--slug", "p", "--summary", "for acme", "--body-file", &input, "--project", "acme"]);
    assert_eq!(out.replace("\r\n", "\n"), format!("src-{DATE}-01\nworkspace/projects/acme/sources/note-{DATE}-p.md\n"));
    assert!(fx.read("workspace/projects/acme/sources/INDEX.md").contains("for acme"));
    assert_eq!(files_in_sources(&fx), Vec::<String>::new(), "the life layer is untouched");
    let err = fx.refused(&["source", "add", "--type", "note", "--date", DATE, "--slug", "p", "--summary", "s", "--body-file", &input, "--project", "ghost"]);
    assert!(err.contains("workspace/projects/ghost/sources"), "{err}");
}
