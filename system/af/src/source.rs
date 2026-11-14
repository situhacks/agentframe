//! `af source add`: files one source into a layer's `sources/` (the life layer, or a project's).
//! The id, the file and its INDEX row are made under one lock on that layer's `INDEX.md`, so two
//! callers never take one id. The shapes are `library/schema/source-record.md`.

use crate::core::Ctx;
use crate::error::R;
use crate::fm::{self, rx};
use crate::life::{locked, vpath, TextFile};
use crate::pyos;
use crate::{die, outln};
use clap::{Args, Subcommand};

const TYPES: [&str; 6] = ["order", "receipt", "email", "transcript", "document", "note"];
const DEFAULT_ORIGIN: &str = "filed with af source add";

#[derive(Subcommand, Debug)]
pub enum SourceCmd {
    /// File one source: the next id, the file and its INDEX row, under one lock
    Add(AddArgs),
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// order, receipt, email, transcript, document or note
    #[arg(long = "type", value_parser = TYPES)]
    type_: String,
    /// The day the source was made or received (YYYY-MM-DD)
    #[arg(long)]
    date: String,
    /// Lower-case letters, digits and hyphens
    #[arg(long)]
    slug: String,
    /// One line for the INDEX row (no `|`)
    #[arg(long)]
    summary: String,
    /// Where it came from, in a few words
    #[arg(long)]
    origin: Option<String>,
    /// Mark the record private (frontmatter only: a file copied with --file has none, so this does nothing to it)
    #[arg(long)]
    private: bool,
    /// A text file: its content becomes the body, byte for byte, under the record's frontmatter
    #[arg(long)]
    body_file: Option<String>,
    /// Any file, copied unchanged with no frontmatter (not for an order)
    #[arg(long)]
    file: Option<String>,
    /// Remove the original after a verified copy (with --file)
    #[arg(long = "move")]
    move_: bool,
    /// File into this project's sources/ instead of the life layer's
    #[arg(long)]
    project: Option<String>,
}

pub fn run(ctx: &Ctx, cmd: SourceCmd) -> R<i32> {
    match cmd {
        SourceCmd::Add(a) => cmd_add(ctx, &a),
    }
}

/// Everything refused before the lock is taken.
fn check_args(a: &AddArgs) -> R<()> {
    if !rx(r"\A[a-z0-9-]+\z").is_match(&a.slug) {
        die!("--slug {:?} must be lower-case letters, digits and hyphens", a.slug);
    }
    if !rx(r"\A\d{4}-\d{2}-\d{2}\z").is_match(&a.date) || chrono::NaiveDate::parse_from_str(&a.date, "%Y-%m-%d").is_err() {
        die!("--date {:?} is not a real date as YYYY-MM-DD", a.date);
    }
    let summary = a.summary.trim();
    if summary.is_empty() {
        die!("--summary may not be empty");
    }
    if summary.contains(['|', '\n', '\r']) {
        die!("--summary must be one line with no `|`: it is a table cell");
    }
    if a.origin.as_deref().is_some_and(|o| o.trim().is_empty() || o.contains(['\n', '\r'])) {
        die!("--origin must be one non-empty line");
    }
    match (&a.body_file, &a.file) {
        (Some(_), Some(_)) => die!("give --body-file or --file, not both"),
        (None, None) => die!("give --body-file (a text input) or --file (any file, copied as it is)"),
        _ => {}
    }
    if a.move_ && a.file.is_none() {
        die!("--move goes with --file");
    }
    if a.file.is_some() && a.type_ == "order" {
        die!("an order is text: file it with --body-file");
    }
    if let Some(f) = &a.file {
        match file_extension(f) {
            Some(_) => {}
            None => die!("--file {f:?} needs an extension of 1 to 10 letters or digits, nothing else"),
        }
    }
    if let Some(p) = &a.project {
        if !fm::valid_slug(p) {
            die!("--project {p:?} is not a project slug");
        }
    }
    Ok(())
}

/// One more than the highest `src-<date>-NN` anywhere under the layer's `sources/`: INDEX.md, the
/// frontmatter `id` of every `.md` file, and the `order-<date>-NN-` in an order's file name.
fn next_number(sources: &str, date: &str) -> R<u64> {
    let date = regex::escape(date);
    let id = rx(&format!(r"src-{date}-(\d+)"));
    let order_name = rx(&format!(r"\Aorder-{date}-(\d+)-"));
    let fm_id = rx(&format!(r"\Asrc-{date}-(\d+)\z"));
    let mut highest = 0u64;
    let mut note = |digits: &str| {
        if let Ok(n) = digits.parse::<u64>() {
            highest = highest.max(n);
        }
    };
    pyos::walk(sources, &mut |dirpath, _, filenames| {
        for f in filenames {
            if let Some(c) = order_name.captures(f) {
                note(&c[1]);
            }
            if !f.ends_with(".md") {
                continue;
            }
            let bytes = std::fs::read(pyos::join(dirpath, f)).unwrap_or_default();
            let text = String::from_utf8_lossy(&bytes);
            let text = text.trim_start_matches('\u{feff}');
            if f == "INDEX.md" && dirpath == sources {
                id.captures_iter(text).for_each(|c| note(&c[1]));
            } else if let Some((front, _)) = fm::split_fm_optional(text) {
                if let Some(c) = fm::get_scalar(&front, "id").and_then(|v| fm_id.captures(&v).map(|c| c[1].to_string())) {
                    note(&c);
                }
            }
        }
    });
    match highest.checked_add(1) {
        Some(n) => Ok(n),
        None => die!("the highest id for {date} is already the largest number there is; nothing was written"),
    }
}

/// The extension of a `--file` for the target name, lower-cased: 1 to 10 ASCII letters or digits. Anything
/// else (none, a `)` that breaks the INDEX link, a `:` that names an NTFS stream) is None.
fn file_extension(path: &str) -> Option<String> {
    let ext = std::path::Path::new(path).extension()?.to_str()?;
    (!ext.is_empty() && ext.len() <= 10 && ext.chars().all(|c| c.is_ascii_alphanumeric())).then(|| ext.to_ascii_lowercase())
}

/// A frontmatter scalar: plain when it is plainly safe, else a double-quoted string.
fn scalar(v: &str) -> String {
    if rx(r"\A[A-Za-z0-9][A-Za-z0-9 ._/@()-]*\z").is_match(v) && !v.ends_with(' ') {
        v.to_string()
    } else {
        serde_json::to_string(v).unwrap_or_else(|_| format!("\"{v}\""))
    }
}

fn io_err(e: &std::io::Error, path: &str) -> crate::error::AfError {
    pyos::os_error(e, path)
}

/// Create `path` with create-new and write `data`; a name that exists is refused and nothing is written.
fn create_new(path: &str, data: &[u8]) -> R<()> {
    use std::io::Write;
    let mut f = match std::fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => die!("{path} already exists; nothing was written"),
        Err(e) => return Err(io_err(&e, path)),
    };
    if let Err(e) = f.write_all(data).and_then(|_| f.flush()) {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(io_err(&e, path));
    }
    Ok(())
}

fn cmd_add(ctx: &Ctx, a: &AddArgs) -> R<i32> {
    check_args(a)?;
    let sources = match &a.project {
        Some(p) => pyos::join_all(&[&ctx.projects, p, "sources"]),
        None => vpath(ctx, crate::life::SOURCES_REL),
    };
    if !pyos::isdir(&sources) {
        die!("{} does not exist; there is no layer to file into", ctx.rel(&sources)?);
    }
    // Read what the file holds before anything is taken, so a bad input costs no id.
    let (input, data) = match (&a.body_file, &a.file) {
        (Some(p), _) | (_, Some(p)) => {
            let bytes = std::fs::read(p).map_err(|e| io_err(&e, p))?;
            (p.clone(), bytes)
        }
        _ => unreachable!("check_args needs one of the two"),
    };
    let body = a.body_file.is_some();
    if body && std::str::from_utf8(&data).is_err() {
        die!("{input} is not UTF-8 text; file it with --file");
    }
    let index = pyos::join(&sources, "INDEX.md");
    let (id, rel) = locked(&index, || file_under_lock(a, &sources, &index, body, &data))?;
    outln!("{id}");
    outln!("{}", ctx.rel(&pyos::join(&sources, &rel))?);
    if a.move_ {
        // The source is filed; a failed delete leaves the original, which is a warning and not a failure.
        if let Err(e) = std::fs::remove_file(&input) {
            crate::out::err_line(&format!("af: warning: {id} is filed, but the original {input} could not be removed: {e}"));
        }
    }
    Ok(0)
}

/// From choosing the id to the INDEX row, with the lock held. Returns the id and the file's path
/// relative to `sources/`.
fn file_under_lock(a: &AddArgs, sources: &str, index: &str, body: bool, data: &[u8]) -> R<(String, String)> {
    let n = next_number(sources, &a.date)?;
    let id = format!("src-{}-{n:02}", a.date);
    let rel = if body {
        match a.type_.as_str() {
            "order" => format!("orders/order-{}-{n:02}-{}.md", a.date, a.slug),
            t => format!("{t}-{}-{}.md", a.date, a.slug),
        }
    } else {
        let input = a.file.as_deref().unwrap_or_default();
        let ext = file_extension(input).map(|e| format!(".{e}")).unwrap_or_default();
        format!("{}-{}-{}{ext}", a.type_, a.date, a.slug)
    };
    let target = pyos::join(sources, &rel);
    if body {
        pyos::makedirs(&pyos::dirname(&target))?;
        let mut out = format!("---\nid: {id}\ntype: {}\ndate: {}\norigin: {}\nverbatim: true\n", a.type_, a.date, scalar(a.origin.as_deref().unwrap_or(DEFAULT_ORIGIN).trim()));
        if a.private {
            out.push_str("private: true\n");
        }
        out.push_str("---\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(data);
        create_new(&target, &bytes)?;
    } else {
        create_new(&target, data)?;
        if std::fs::read(&target).map(|copy| copy != data).unwrap_or(true) {
            let _ = std::fs::remove_file(&target);
            die!("the copy at {target} does not match its original; nothing was kept");
        }
    }
    let row = format!("| `{id}` | [`{rel}`]({rel}) | {} | {} |", a.date, a.summary.trim());
    if let Err(e) = append_row(index, &row) {
        let _ = std::fs::remove_file(&target);
        return Err(e);
    }
    Ok((id, rel))
}

/// The row at the end of INDEX.md in the file's own line endings; a missing INDEX.md gets its header.
fn append_row(index: &str, row: &str) -> R<()> {
    let mut file = TextFile::load(index)?.unwrap_or_else(|| {
        TextFile::new("# Source Index\n\n| ID | File | Date | Description |\n|---|---|---|---|\n")
    });
    file.push_line(row);
    file.save_atomic(index)
}
