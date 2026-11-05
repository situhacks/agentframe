//! `af place` and `af person`: the mechanics of `workspace/life/places/` and `people/` pages
//! (library/schema/place.md, library/schema/person-profile.md), and the doctor's check of
//! their frontmatter. The buttons create pages, keep slugs unique, and raise and close the
//! verdict loop; the page's prose stays with the agent.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::fm::{fm_list_values, get_scalar, rx, split_fm_optional};
use crate::life::{self, vpath, TextFile};
use crate::open;
use crate::outln;
use crate::pyos;
use crate::tags::{self, Page, Registry, Resolved, PAGE_DIRS};
use chrono::NaiveDate;
use clap::Subcommand;
use std::collections::BTreeSet;

pub const KINDS: [&str; 10] = ["restaurant", "bar", "club", "cafe", "venue", "shop", "gym", "park", "service", "area"];
pub const STATUSES: [&str; 6] = ["candidate", "want", "been", "regular", "avoid", "closed"];
/// Statuses a verdict leaves alone unless `--status` says otherwise.
const STICKY_STATUSES: [&str; 3] = ["regular", "avoid", "closed"];
pub const VERDICTS: [&str; 3] = ["yes", "no", "maybe"];
const STALE_DAYS: i64 = 183;
const GROUPS_DIR: &str = "workspace/life/people/groups";
const SECTIONS: [&str; 6] = ["What it is", "Practical", "Vibe and music", "Reviews found", "His take", "Visits"];
const PERSON_TYPES: [&str; 2] = ["person", "group"];

#[derive(Subcommand, Debug)]
pub enum PlaceCmd {
    /// Create a place page in the schema's shape; refuses a duplicate slug
    Add {
        slug: String,
        #[arg(long)]
        name: String,
        /// restaurant, bar, club, cafe, venue, shop, gym, park, service, area
        #[arg(long)]
        kind: String,
        /// slug of a kind: area page (an area page itself takes none)
        #[arg(long)]
        area: Option<String>,
        #[arg(long)]
        address: Option<String>,
        #[arg(long)]
        transit: Option<String>,
        /// candidate, want, been, regular, avoid, closed
        #[arg(long, default_value = "candidate")]
        status: String,
        /// a registry tag id or alt label (repeat)
        #[arg(long)]
        tag: Vec<String>,
    },
    /// Add a visit row under Visits and raise its verdict loop for the day after
    Visit {
        slug: String,
        /// YYYY-MM-DD
        #[arg(long)]
        date: String,
        #[arg(long)]
        what: String,
        /// page slugs of who is coming, comma separated
        #[arg(long, value_delimiter = ',')]
        with: Vec<String>,
        /// source id or chat name for the loop
        #[arg(long)]
        from: Option<String>,
    },
    /// Record his verdict on a visit: the row, His take, the status, and the loop's close
    Verdict {
        slug: String,
        /// the visit's YYYY-MM-DD
        #[arg(long)]
        date: String,
        /// yes, no, or maybe (go back?)
        #[arg(long)]
        verdict: String,
        /// his words, near-verbatim
        #[arg(long)]
        words: String,
        /// the place's new status; defaults to been unless it is already regular
        #[arg(long)]
        status: Option<String>,
    },
    /// List places by exact field
    List {
        #[arg(long)]
        area: Option<String>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        status: Option<String>,
        /// only places whose checked date is over six months old or missing
        #[arg(long)]
        stale: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum PersonCmd {
    /// Create a person page, or with --group a group page; refuses a duplicate slug
    Add {
        slug: String,
        #[arg(long)]
        name: String,
        /// a friend group: the slug starts with group- and the page lands under people/groups/
        #[arg(long)]
        group: bool,
        /// where and when he met them (a person), or how the group formed
        #[arg(long)]
        met: Option<String>,
        /// a registry tag id or alt label (repeat; people facets only)
        #[arg(long)]
        tag: Vec<String>,
    },
}

pub fn run_place(ctx: &Ctx, cmd: PlaceCmd) -> R<i32> {
    match cmd {
        PlaceCmd::Add { slug, name, kind, area, address, transit, status, tag } => {
            cmd_place_add(ctx, &slug, &name, &kind, area.as_deref(), address.as_deref(), transit.as_deref(), &status, &tag)
        }
        PlaceCmd::Visit { slug, date, what, with, from } => cmd_visit(ctx, &slug, &date, &what, &with, from.as_deref()),
        PlaceCmd::Verdict { slug, date, verdict, words, status } => {
            cmd_verdict(ctx, &slug, &date, &verdict, &words, status.as_deref())
        }
        PlaceCmd::List { area, kind, tag, status, stale } => {
            cmd_list(ctx, area.as_deref(), kind.as_deref(), tag.as_deref(), status.as_deref(), stale)
        }
    }
}

pub fn run_person(ctx: &Ctx, cmd: PersonCmd) -> R<i32> {
    match cmd {
        PersonCmd::Add { slug, name, group, met, tag } => cmd_person_add(ctx, &slug, &name, group, met.as_deref(), &tag),
    }
}

// ------------------------------------------------------------------ pages

fn stem(rel: &str) -> String {
    pyos::splitext(&pyos::basename(rel)).0
}

fn find_page(pages: &[Page], slug: &str) -> Option<Page> {
    pages.iter().find(|p| stem(&p.rel) == slug).cloned()
}

fn check_slug(slug: &str) -> R<()> {
    if !rx(r"\A[a-z0-9]+(?:-[a-z0-9]+)*\z").is_match(slug) {
        die!("'{slug}' is not a kebab-case slug; derive it from the name, for example ramen-example");
    }
    Ok(())
}

fn refuse_duplicate(pages: &[Page], slug: &str) -> R<()> {
    if let Some(p) = find_page(pages, slug) {
        die!("[[{slug}]] already exists at {}; slugs are unique across places/ and people/. Open that page, or pick another slug", p.rel);
    }
    Ok(())
}

fn place_page(ctx: &Ctx, slug: &str) -> R<Page> {
    let pages = tags::scan_pages(ctx)?;
    match find_page(&pages, slug) {
        Some(p) if p.side == "places" => Ok(p),
        Some(p) => die!("[[{slug}]] is {}, not a place page", p.rel),
        None => die!("no place page for '{slug}'; create it with af place add {slug} --name \"<name>\" --kind <kind>"),
    }
}

fn resolve_tags(ctx: &Ctx, wanted: &[String], side: &str) -> R<Vec<String>> {
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let reg = Registry::require(ctx)?;
    let mut out = Vec::new();
    for w in wanted {
        let t = match reg.resolve(w, None) {
            Resolved::Use(t, _) => t,
            Resolved::Deprecated(t) => die!("tag '{}' is deprecated; pick a live tag (af tag match {w})", t.id),
            Resolved::None => {
                die!("no tag matches '{w}'; af tag propose {} --facet <f> --scope \"<one line>\" first", tags::normalize(w))
            }
        };
        if let Some(f) = reg.facet(&t.facet) {
            if f.applies != "both" && f.applies != side {
                die!("tag '{}' is in facet '{}', a {} facet; this is a {side} page", t.id, f.name, f.applies);
            }
        }
        if !out.contains(&t.id) {
            out.push(t.id.clone());
        }
    }
    Ok(out)
}

fn yaml_list(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

fn parse_date(v: &str, flag: &str) -> R<NaiveDate> {
    match NaiveDate::parse_from_str(v.trim(), "%Y-%m-%d") {
        Ok(d) => Ok(d),
        Err(_) => die!("{flag} takes YYYY-MM-DD, got '{v}'"),
    }
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

// ------------------------------------------------------------------ af place add

#[allow(clippy::too_many_arguments)]
fn cmd_place_add(
    ctx: &Ctx,
    slug: &str,
    name: &str,
    kind: &str,
    area: Option<&str>,
    address: Option<&str>,
    transit: Option<&str>,
    status: &str,
    tag: &[String],
) -> R<i32> {
    check_slug(slug)?;
    if !KINDS.contains(&kind) {
        die!("kind '{kind}' is not one of {}", KINDS.join(", "));
    }
    if !STATUSES.contains(&status) {
        die!("status '{status}' is not one of {}", STATUSES.join(", "));
    }
    let pages = tags::scan_pages(ctx)?;
    refuse_duplicate(&pages, slug)?;
    if kind != "area" {
        let a = match area {
            Some(a) if !a.trim().is_empty() => a,
            _ => die!("--area names a kind: area page; a place needs one (create it with af place add <slug> --kind area)"),
        };
        match find_page(&pages, a) {
            Some(p) if p.is_area() => {}
            _ => die!("--area '{a}' does not resolve to a kind: area page under {}", PAGE_DIRS[0]),
        }
    }
    let tag_ids = resolve_tags(ctx, tag, "places")?;
    let address = life::one_line(address.unwrap_or(""));
    let transit = life::one_line(transit.unwrap_or(""));
    let area = area.unwrap_or("").trim().to_string();
    let today = crate::core::today();

    let mut same_address = Vec::new();
    if !address.is_empty() {
        for p in pages.iter().filter(|p| p.side == "places") {
            if let Some(tf) = TextFile::load(&p.path)? {
                let other = split_fm_optional(&tf.text).and_then(|(fm, _)| get_scalar(&fm, "address"));
                if other.is_some_and(|o| o.eq_ignore_ascii_case(&address)) {
                    same_address.push(stem(&p.rel));
                }
            }
        }
    }

    let mut card: Vec<String> = vec![title_case(kind)];
    if !tag_ids.is_empty() {
        card.push(tag_ids.join(", "));
    }
    if !area.is_empty() {
        card.push(format!("[[{area}]]"));
    }
    for part in [&address, &transit] {
        if !part.is_empty() {
            card.push(part.clone());
        }
    }
    card.push(status.to_string());

    let mut text = format!(
        "---\nprivate: true\nkind: {kind}\narea: {area}\naddress: {address}\ntransit: {transit}\ntags: {}\nstatus: {status}\nchecked: {today}\n---\n# {}\n{}\n",
        yaml_list(&tag_ids),
        life::one_line(name),
        card.join(" · ")
    );
    for s in SECTIONS {
        text.push_str(&format!("\n## {s}\n"));
    }
    let rel = format!("{}/{slug}.md", PAGE_DIRS[0]);
    let path = vpath(ctx, &rel);
    pyos::makedirs(&pyos::dirname(&path))?;
    crate::core::write(&path, &text)?;
    outln!("af place add: {rel} ({kind}, {status})");
    if !area.is_empty() {
        outln!("  next: add the one-line entry under ## Places on [[{area}]]");
    }
    for other in same_address {
        outln!("  note: [[{other}]] has the same address; the same address under a new name is the same file");
    }
    Ok(0)
}

// ------------------------------------------------------------------ af place visit

fn section_end(lines: &[String], heading: &str) -> Option<usize> {
    let start = lines.iter().position(|l| l.trim() == format!("## {heading}"))?;
    let next = lines.iter().enumerate().skip(start + 1).find(|(_, l)| l.starts_with("## ")).map(|(i, _)| i);
    let mut end = next.unwrap_or(lines.len());
    while end > start + 1 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    Some(end)
}

fn insert_in_section(file: &mut TextFile, heading: &str, line: &str) -> R<()> {
    let mut lines = file.lines();
    match section_end(&lines, heading) {
        Some(at) => lines.insert(at, line.to_string()),
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!("## {heading}"));
            lines.push(line.to_string());
        }
    }
    file.set_lines(&lines);
    Ok(())
}

fn page_name(file: &TextFile, slug: &str) -> String {
    file.lines()
        .iter()
        .find_map(|l| l.strip_prefix("# ").map(|t| t.trim().to_string()))
        .unwrap_or_else(|| slug.to_string())
}

fn verdict_loop_text(name: &str, date: &str, what: &str) -> String {
    format!("verdict: {name}, {date} {what}")
}

fn cmd_visit(ctx: &Ctx, slug: &str, date: &str, what: &str, with: &[String], from: Option<&str>) -> R<i32> {
    let page = place_page(ctx, slug)?;
    let day = parse_date(date, "--date")?;
    let what = life::one_line(what);
    if what.is_empty() {
        die!("--what says what the visit is: dinner for 4, a show");
    }
    let pages = tags::scan_pages(ctx)?;
    let mut links = Vec::new();
    for w in with.iter().map(|w| w.trim()).filter(|w| !w.is_empty()) {
        if find_page(&pages, w).is_none() {
            die!("--with '{w}' has no page under places/ or people/; create it with af person add {w} --name \"<name>\"");
        }
        links.push(format!("[[{w}]]"));
    }
    let date = day.format("%Y-%m-%d").to_string();
    let mut file = match TextFile::load(&page.path)? {
        Some(f) => f,
        None => die!("{}: no such page", page.rel),
    };
    if visit_line(&file, &date).is_some() {
        die!("{} already has a visit row for {date}; record his answer with af place verdict {slug} --date {date}", page.rel);
    }
    let with_part = if links.is_empty() { String::new() } else { format!(" · with: {}", links.join(", ")) };
    insert_in_section(&mut file, "Visits", &format!("- {date} · {what}{with_part} · verdict: pending"))?;
    let name = page_name(&file, slug);
    file.save(&page.path)?;
    let nudge = (day + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let loop_text = verdict_loop_text(&name, &date, &what);
    open::cmd_add(ctx, &loop_text, Some(&nudge), None, from, None)?;
    outln!("af place visit: {} row added for {date}", page.rel);
    Ok(0)
}

// ------------------------------------------------------------------ af place verdict

fn visit_line(file: &TextFile, date: &str) -> Option<usize> {
    let lines = file.lines();
    let start = lines.iter().position(|l| l.trim() == "## Visits")?;
    lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .take_while(|(_, l)| !l.starts_with("## "))
        .find(|(_, l)| l.starts_with(&format!("- {date} ·")))
        .map(|(i, _)| i)
}

/// A verdict row may already hold his words or an earlier answer: those stay, the new one follows.
fn next_verdict(prior: &str, verdict: &str) -> String {
    if prior.is_empty() || prior == "pending" {
        verdict.to_string()
    } else if prior == verdict || prior.ends_with(&format!("; {verdict}")) {
        prior.to_string()
    } else {
        format!("{prior}; {verdict}")
    }
}

fn cmd_verdict(ctx: &Ctx, slug: &str, date: &str, verdict: &str, words: &str, status: Option<&str>) -> R<i32> {
    let page = place_page(ctx, slug)?;
    let date = parse_date(date, "--date")?.format("%Y-%m-%d").to_string();
    if !VERDICTS.contains(&verdict) {
        die!("--verdict is yes, no, or maybe (go back?), got '{verdict}'; his words go in --words");
    }
    let words = life::one_line(words);
    if words.is_empty() {
        die!("--words holds his words, near-verbatim");
    }
    if let Some(s) = status {
        if !STATUSES.contains(&s) {
            die!("status '{s}' is not one of {}", STATUSES.join(", "));
        }
    }
    let mut file = match TextFile::load(&page.path)? {
        Some(f) => f,
        None => die!("{}: no such page", page.rel),
    };
    let at = match visit_line(&file, &date) {
        Some(i) => i,
        None => die!("{} has no visit row for {date}; add it with af place visit {slug} --date {date} --what \"<what>\"", page.rel),
    };
    let mut lines = file.lines();
    let row = lines[at].clone();
    let (body, prior) = match row.rfind(" · verdict:") {
        Some(i) => (&row[..i], row[i + " · verdict:".len()..].trim()),
        None => (row.as_str(), ""),
    };
    lines[at] = format!("{body} · verdict: {}", next_verdict(prior, verdict));
    file.set_lines(&lines);
    insert_in_section(&mut file, "His take", &format!("- {date} · {words}"))?;
    let (fm, _) = match split_fm_optional(&file.text) {
        Some(p) => p,
        None => die!("{} has no frontmatter; give it the shape in library/schema/place.md", page.rel),
    };
    let current = get_scalar(&fm, "status").unwrap_or_default();
    let new_status = status.map(str::to_string).unwrap_or_else(|| if STICKY_STATUSES.contains(&current.as_str()) { current.clone() } else { "been".to_string() });
    let mut lines = file.lines();
    for l in lines.iter_mut() {
        if l.starts_with("status:") {
            *l = format!("status: {new_status}");
            break;
        }
    }
    file.set_lines(&lines);
    let name = page_name(&file, slug);
    file.save(&page.path)?;

    let what = body.split(" · ").nth(1).unwrap_or("").trim().to_string();
    let prefix = verdict_loop_text(&name, &date, &what);
    let doc = open::OpenDoc::load(ctx)?;
    let has_loop = doc.is_some_and(|d| d.loops.iter().any(|l| l.text.starts_with(&prefix)));
    if has_loop {
        open::cmd_close(ctx, &prefix, verdict, false)?;
    }
    outln!("af place verdict: {} {date} -> {verdict}, status {new_status}", page.rel);
    if !has_loop {
        outln!("  note: no open verdict loop matched '{prefix}'");
    }
    outln!("  next: tell him how the place matched what he asked for, and note what it taught under the city's taste page");
    Ok(0)
}

// ------------------------------------------------------------------ af place list

fn cmd_list(
    ctx: &Ctx,
    area: Option<&str>,
    kind: Option<&str>,
    tag: Option<&str>,
    status: Option<&str>,
    stale: bool,
) -> R<i32> {
    let today = life::now().date();
    let tag_id = match tag {
        Some(t) => match Registry::load(ctx)? {
            Some(reg) => match reg.resolve(t, None) {
                Resolved::Use(found, _) => Some(found.id.clone()),
                _ => Some(t.to_string()),
            },
            None => Some(t.to_string()),
        },
        None => None,
    };
    let mut count = 0usize;
    for p in tags::scan_pages(ctx)?.into_iter().filter(|p| p.side == "places") {
        let tf = match TextFile::load(&p.path)? {
            Some(f) => f,
            None => continue,
        };
        let fm = split_fm_optional(&tf.text).map(|(f, _)| f).unwrap_or_default();
        let field = |k: &str| get_scalar(&fm, k).unwrap_or_default();
        if area.is_some_and(|a| field("area") != a)
            || kind.is_some_and(|k| field("kind") != k)
            || status.is_some_and(|s| field("status") != s)
            || tag_id.as_ref().is_some_and(|t| !p.tags.contains(t))
        {
            continue;
        }
        let checked = field("checked");
        let old = NaiveDate::parse_from_str(&checked, "%Y-%m-%d").map(|d| (today - d).num_days() > STALE_DAYS).unwrap_or(true);
        if stale && !old {
            continue;
        }
        outln!(
            "{} · {} · {} · {} · checked {}",
            stem(&p.rel),
            field("kind"),
            field("area"),
            field("status"),
            if checked.is_empty() { "never".to_string() } else { checked.clone() }
        );
        count += 1;
    }
    outln!("{count} place(s)");
    Ok(0)
}

// ------------------------------------------------------------------ af person add

fn cmd_person_add(ctx: &Ctx, slug: &str, name: &str, group: bool, met: Option<&str>, tag: &[String]) -> R<i32> {
    check_slug(slug)?;
    if group && !slug.starts_with("group-") {
        die!("a group's slug starts with group-: group-{slug}");
    }
    let pages = tags::scan_pages(ctx)?;
    refuse_duplicate(&pages, slug)?;
    let tag_ids = resolve_tags(ctx, tag, "people")?;
    let name = life::one_line(name);
    let met = life::one_line(met.unwrap_or(""));
    let (rel, text) = if group {
        (
            format!("{GROUPS_DIR}/{slug}.md"),
            format!("---\nprivate: true\nname: {name}\ntype: group\nformed: {met}\nmembers: []\n---\n# {name}\n\n## Members\n\n## Habits\n\n## Forming\n"),
        )
    } else {
        (
            format!("{}/{slug}.md", PAGE_DIRS[1]),
            format!("---\nprivate: true\nname: {name}\ntype: person\nmet: {met}\ntags: {}\n---\n# {name}\n", yaml_list(&tag_ids)),
        )
    };
    let path = vpath(ctx, &rel);
    pyos::makedirs(&pyos::dirname(&path))?;
    crate::core::write(&path, &text)?;
    outln!("af person add: {rel}");
    Ok(0)
}

// ------------------------------------------------------------------ doctor

/// The frontmatter of every place and person page: (issues, notes). A bad value is an issue
/// that names the vocabulary; `[[slug]]` resolution is `tags::dangling_link_issues`.
pub fn doctor_checks(ctx: &Ctx) -> R<(Vec<String>, Vec<String>)> {
    let mut issues = Vec::new();
    let notes = Vec::new();
    let pages = tags::scan_pages(ctx)?;
    let areas: BTreeSet<String> = pages.iter().filter(|p| p.is_area()).map(|p| stem(&p.rel)).collect();
    for p in &pages {
        if pyos::basename(&p.rel).eq_ignore_ascii_case("README.md") {
            continue;
        }
        let tf = match TextFile::load(&p.path)? {
            Some(f) => f,
            None => continue,
        };
        let fm = match split_fm_optional(&tf.text) {
            Some((f, _)) => f,
            None => {
                issues.push(format!("{}: no frontmatter; give it the shape in library/schema/{}", p.rel, schema_of(p)));
                continue;
            }
        };
        if get_scalar(&fm, "private").as_deref() != Some("true") {
            issues.push(format!("{}: missing private: true (library/schema/{})", p.rel, schema_of(p)));
        }
        if p.side == "places" {
            let kind = get_scalar(&fm, "kind").unwrap_or_default();
            if !KINDS.contains(&kind.as_str()) {
                issues.push(format!("{}: kind '{kind}' is not one of {}", p.rel, KINDS.join(", ")));
            }
            let status = get_scalar(&fm, "status").unwrap_or_default();
            if !STATUSES.contains(&status.as_str()) {
                issues.push(format!("{}: status '{status}' is not one of {}", p.rel, STATUSES.join(", ")));
            }
            let area = get_scalar(&fm, "area").unwrap_or_default();
            if kind != "area" && !areas.contains(&area) {
                issues.push(format!("{}: area '{area}' does not resolve to a kind: area page", p.rel));
            }
        } else {
            let kind = get_scalar(&fm, "type").unwrap_or_default();
            if !PERSON_TYPES.contains(&kind.as_str()) {
                issues.push(format!("{}: type '{kind}' is not person or group (library/schema/person-profile.md)", p.rel));
            }
            if kind == "group" {
                for m in fm_list_values(&fm, "members") {
                    if !pages.iter().any(|o| stem(&o.rel) == m) {
                        issues.push(format!("{}: member '{m}' has no page under people/", p.rel));
                    }
                }
            }
        }
    }
    Ok((issues, notes))
}

fn schema_of(p: &Page) -> &'static str {
    if p.side == "places" {
        "place.md"
    } else {
        "person-profile.md"
    }
}
