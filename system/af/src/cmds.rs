//! The state-transition verbs: ready, publish, version, draft, adopt, new-project.

use crate::core::{
    self, append_activity, head_of, load_pack, now_iso, pack_exportables, project_dir, project_domain, read,
    require_markdown_head, resolve_deliverable_target, safe_project_rel, state_doc, today, touch_lifecycle,
    version_target, versions_in, write, Ctx,
};
use crate::die;
use crate::error::R;
use crate::fm::{self, get_scalar, has_field, join_fm, row_add, row_get, row_set, row_span, rx, set_scalar, split_fm};
use crate::outln;
use crate::packs::{self, careers, marketing, Rules};
use crate::pyos::{self, join};

fn nonempty(v: Option<&str>) -> Option<&str> {
    v.filter(|s| !s.is_empty())
}

// ------------------------------------------------------------------ ready

pub struct ReadyArgs<'a> {
    pub project: &'a str,
    pub deliverable: &'a str,
    pub artifact: Option<&'a str>,
    pub allow_missing_exports: bool,
}

pub fn cmd_ready(ctx: &Ctx, args: &ReadyArgs) -> R<()> {
    let cdir = project_dir(ctx, args.project)?;
    let sdoc = state_doc(&cdir);
    let (mut cfm, cbody) = split_fm(&read(&join(&cdir, sdoc))?, sdoc)?;

    let (slug, rel) = match nonempty(args.artifact) {
        Some(a) => (None, version_target(&cdir, &cfm, args.deliverable, Some(a))?.0),
        None => resolve_deliverable_target(&cfm, args.deliverable)?,
    };

    let dpath = join(&cdir, &rel);
    if !pyos::isfile(&dpath) {
        die!("deliverable file not found: {rel}");
    }
    require_markdown_head(&rel)?;
    head_of(&dpath)?;

    let (dfm, dbody) = split_fm(&read(&dpath)?, &rel)?;
    let gate = core::export_gate_issues(&cdir, &rel, &dfm, &pack_exportables(ctx, &cfm)?);
    let mut override_note = String::new();
    if !gate.is_empty() {
        if !args.allow_missing_exports {
            let folder = pyos::dirname(&rel).replace('\\', "/");
            let folder = if folder.is_empty() { ".".to_string() } else { folder };
            die!(
                "{rel}: exportable deliverable failed the exports gate ({}). Land the approved finals under {folder}/media/ and record each path in exports[] frontmatter, then rerun. Override (rare): --allow-missing-exports - doctor flags the row until exports land.",
                gate.join("; ")
            );
        }
        override_note = format!("READY WITHOUT EXPORTS (override): {}", gate.join("; "));
    }
    let dfm = set_scalar(&dfm, "status", "ready", &rel)?;
    let dfm = set_scalar(&dfm, "last_updated", &today(), &rel)?;

    let mut notes: Vec<String> = Vec::new();
    let pack_dir = load_pack(ctx, project_domain(&cfm).as_deref())?.1;
    match packs::load_rules(ctx, pack_dir.as_deref())? {
        Some(Rules::Marketing) => {
            let (c, n) = marketing::on_ready(&cdir, &dpath, &rel, cfm)?;
            cfm = c;
            notes = n;
        }
        Some(Rules::Careers) => {
            let (c, n) = careers::on_ready(&cdir, &dpath, &rel, cfm)?;
            cfm = c;
            notes = n;
        }
        None => {}
    }
    if !override_note.is_empty() {
        notes.push(override_note);
    }

    write(&dpath, &join_fm(&dfm, &dbody))?;
    if let Some(s) = &slug {
        cfm = row_set(&cfm, s, "status", "ready")?;
        cfm = row_set(&cfm, s, "last_updated", &today())?;
    }
    let cfm = touch_lifecycle(&cfm)?;
    write(&join(&cdir, sdoc), &join_fm(&cfm, &cbody))?;
    let label = slug.clone().unwrap_or_else(|| pyos::basename(&rel));
    let tail = if notes.is_empty() { String::new() } else { format!("; {}", notes.join("; ")) };
    append_activity(&cdir, &format!("ready: {label} ready; artifact={rel}{tail}"))?;

    let paren = if notes.is_empty() { String::new() } else { format!(" ({})", notes.join("; ")) };
    outln!("af ready: {rel} -> ready{paren}");
    outln!("\nJudgment checklist (agent + operator):");
    outln!("  [ ] Template readiness criteria verified (the deliverable's template)");
    outln!("  [ ] Voice clean pass run on operator-voiced prose (voice/README.md step 3)");
    outln!("  [ ] Voice was loaded for this deliverable's drafting (confirm if session resumed)");
    outln!("  [ ] User-voiced? Run voice-harvest now (.claude/skills/voice-harvest/SKILL.md;");
    outln!("      gate: library/process/ready-event.md) - work to do, not a box to tick");
    outln!("  [ ] Remaining follow-ups surfaced (feedback, export, publish)");
    Ok(())
}

// ------------------------------------------------------------------ publish

pub fn print_harvest_prompt() {
    outln!("  [ ] User-voiced? Run voice-harvest and deliverable-harvest now");
    outln!("      (.claude/skills/voice-harvest/SKILL.md, .claude/skills/deliverable-harvest/SKILL.md);");
    outln!("      corpus promotion is eligible from this publish. Not a box to tick: run them.");
}

pub fn cmd_publish(ctx: &Ctx, args: &marketing::PublishArgs, project: &str) -> R<()> {
    let cdir = project_dir(ctx, project)?;
    let sdoc = state_doc(&cdir);
    let cpath = join(&cdir, sdoc);
    let (cfm, cbody) = split_fm(&read(&cpath)?, sdoc)?;
    let pack_dir = load_pack(ctx, project_domain(&cfm).as_deref())?.1;
    let rules = packs::load_rules(ctx, pack_dir.as_deref())?;
    if rules == Some(Rules::Marketing) && marketing::handles_publish(&cdir, args)? {
        return marketing::publish(&cdir, args);
    }

    let (slug, rel) = resolve_deliverable_target(&cfm, args.deliverable)?;
    let dpath = join(&cdir, &rel);
    if !pyos::isfile(&dpath) {
        die!("deliverable file not found: {rel}");
    }
    require_markdown_head(&rel)?;
    let (dfm, dbody) = split_fm(&read(&dpath)?, &rel)?;
    let status = get_scalar(&dfm, "status");
    if status.as_deref() != Some("ready") {
        let found = match &status {
            Some(s) if !s.is_empty() => s.clone(),
            _ => "missing".to_string(),
        };
        die!("{rel}: publish requires status ready (found {found}); run af ready first");
    }
    let url = nonempty(args.url);
    let mut dfm = dfm;
    if let Some(u) = url {
        dfm = fm::upsert_scalar(&dfm, "published_url", &fm::yaml_quote(Some(u)), None)?;
    }
    let dfm = set_scalar(&dfm, "status", "published", &rel)?;
    let dfm = set_scalar(&dfm, "last_updated", &today(), &rel)?;
    write(&dpath, &join_fm(&dfm, &dbody))?;
    let mut cfm = cfm;
    if let Some(s) = &slug {
        cfm = row_set(&cfm, s, "status", "published")?;
        cfm = row_set(&cfm, s, "last_updated", &today())?;
    }
    let cfm = touch_lifecycle(&cfm)?;
    write(&cpath, &join_fm(&cfm, &cbody))?;
    let label = slug.clone().unwrap_or_else(|| pyos::basename(&rel));
    let detail = url.map(|u| format!("; url={u}")).unwrap_or_default();
    append_activity(&cdir, &format!("publish: {label} published; artifact={rel}{detail}"))?;
    let paren = url.map(|u| format!(" ({u})")).unwrap_or_default();
    outln!("af publish: {rel} -> published{paren}");
    outln!("\nJudgment checklist (agent + operator):");
    print_harvest_prompt();
    Ok(())
}

// ------------------------------------------------------------------ version

pub fn cmd_version(ctx: &Ctx, project: &str, deliverable: &str, artifact: Option<&str>) -> R<()> {
    let cdir = project_dir(ctx, project)?;
    let sdoc = state_doc(&cdir);
    let cpath = join(&cdir, sdoc);
    let (cfm, cbody) = split_fm(&read(&cpath)?, sdoc)?;
    let (rel, move_pointer) = version_target(&cdir, &cfm, deliverable, artifact)?;
    let dpath = join(&cdir, &rel);
    let (name, n) = match head_of(&dpath)? {
        Some(h) => h,
        None => {
            return Err(crate::error::crash(
                crate::error::CrashKind::Other,
                "TypeError: cannot unpack non-iterable NoneType object",
            ))
        }
    };

    if !move_pointer && row_get(&cfm, deliverable, "status").as_deref() == Some("published") {
        die!(
            "tracker row '{deliverable}' points at a published unversioned assembly. Published records are immutable; create a new tracked edition before revising its ingredients."
        );
    }

    let new_rel = join(&pyos::dirname(&rel), &format!("{name}-v{}.md", n + 1)).replace('\\', "/");
    let new_path = join(&cdir, &new_rel);
    if pyos::exists(&new_path) {
        die!("destination already exists: {new_rel}");
    }

    let (dfm, dbody) = split_fm(&read(&dpath)?, &rel)?;
    let source_status = get_scalar(&dfm, "status");
    let dfm = set_scalar(&dfm, "status", "drafting", &new_rel)?;
    let dfm = set_scalar(&dfm, "last_updated", &today(), &new_rel)?;
    write(&new_path, &join_fm(&dfm, &dbody))?;

    let mut cfm = cfm;
    if move_pointer {
        cfm = row_set(&cfm, deliverable, "file", &new_rel)?;
    } else {
        let parent_rel = row_get(&cfm, deliverable, "file").unwrap_or_default();
        let parent_path = join(&cdir, &parent_rel);
        if pyos::isfile(&parent_path) {
            let (pfm, pbody) = split_fm(&read(&parent_path)?, &parent_rel)?;
            let pfm = set_scalar(&pfm, "status", "drafting", &parent_rel)?;
            let pfm = set_scalar(&pfm, "last_updated", &today(), &parent_rel)?;
            write(&parent_path, &join_fm(&pfm, &pbody))?;
        }
    }
    cfm = row_set(&cfm, deliverable, "status", "drafting")?;
    cfm = row_set(&cfm, deliverable, "last_updated", &today())?;
    let cfm = touch_lifecycle(&cfm)?;
    write(&cpath, &join_fm(&cfm, &cbody))?;

    let label = nonempty(artifact).unwrap_or(deliverable);
    let source_note = match source_status.as_deref() {
        Some(s @ ("ready" | "published")) => format!("; source_status={s}"),
        _ => String::new(),
    };
    append_activity(&cdir, &format!("artifact_versioned: {label} v{n} -> v{}; {new_rel}{source_note}", n + 1))?;

    let pointer_note = if move_pointer { "tracker pointer moved" } else { "parent tracker pointer unchanged" };
    outln!("af version: {rel} -> {new_rel} (head; {pointer_note}; prior version untouched as the snapshot)");
    outln!("\nJudgment (stays with the agent):");
    outln!("  - {new_rel} already contains v{n}'s full content — write the change as targeted");
    outln!("    edits to that copy rather than retyping; a full-file rewrite is right only when");
    outln!("    the change is genuinely whole-body.");
    outln!("  - Every content change to a head versions first, copyedits included (deliverable-versioning.md).");
    outln!("  - The operator's feedback on this draft is narration: it goes in the new head's changes_from_v{n}.");
    Ok(())
}

// ------------------------------------------------------------------ draft

pub fn cmd_draft(ctx: &Ctx, project: &str, deliverable: &str, artifact: Option<&str>, file: Option<&str>) -> R<()> {
    let cdir = project_dir(ctx, project)?;
    let sdoc = state_doc(&cdir);
    let cpath = join(&cdir, sdoc);
    let (cfm, cbody) = split_fm(&read(&cpath)?, sdoc)?;
    let row_exists = row_span(&cfm, deliverable).is_some();
    let artifact = nonempty(artifact);

    let mut notes: Vec<String> = Vec::new();
    let adopt_hint = format!(
        "a file that already exists registers with af adopt {project} {deliverable} --file <project-relative path>; the next head comes from af version"
    );
    let (new_rel, new_path, move_pointer) = if let Some(art) = artifact {
        if !row_exists {
            die!(
                "tracker row '{deliverable}' not found — nested artifacts require a parent row. Start it with af draft {project} {deliverable} --file <folder>/<first-ingredient>-v1.md: that creates the row (a domain hook may add the assembly record and point the row at it), and --artifact works for the rest. Never hand-write the row."
            );
        }
        if !fm::valid_slug(art) {
            die!("artifact name must contain only letters, numbers, underscores, and hyphens");
        }
        let parent_rel = match row_get(&cfm, deliverable, "file") {
            Some(p) if !p.is_empty() => p,
            _ => die!("tracker row '{deliverable}' has no parent file/folder pointer"),
        };
        let parent_rel = parent_rel.replace('\\', "/");
        let folder_rel = if parent_rel.to_lowercase().ends_with(".md") {
            pyos::dirname(&parent_rel)
        } else {
            parent_rel.trim_end_matches('/').to_string()
        };
        let folder_rel = if folder_rel.is_empty() { ".".to_string() } else { folder_rel };
        let (new_rel, new_path) = safe_project_rel(&cdir, Some(&format!("{folder_rel}/{art}-v1.md")))?;
        if !versions_in(&pyos::dirname(&new_path), art).is_empty() {
            die!("artifact '{art}' already has a version chain under {folder_rel}");
        }
        (new_rel, new_path, false)
    } else {
        let (new_rel, new_path) = safe_project_rel(&cdir, file)?;
        let base = pyos::basename(&new_rel);
        let stem = match rx(r"\A(.+)-v1\.md\z").captures(&base) {
            Some(c) => c[1].to_string(),
            None => die!(
                "--file must name a canonical -v1.md first-draft path (<slug>-v1.md): draft creates the stub and you write into it. A file that already exists registers with af adopt."
            ),
        };
        if let Some(current) = row_get(&cfm, deliverable, "file") {
            if !current.is_empty() && pyos::exists(&join(&cdir, &current)) {
                die!(
                    "tracker row '{deliverable}' already has an existing artifact: {current}; run af version {project} {deliverable} for the next head"
                );
            }
        }
        if !versions_in(&pyos::dirname(&new_path), &stem).is_empty() {
            let d = pyos::dirname(&new_rel);
            let d = if d.is_empty() { ".".to_string() } else { d };
            die!("deliverable '{stem}' already has a version chain under {d}; {adopt_hint}");
        }
        (new_rel, new_path, true)
    };

    if pyos::exists(&new_path) {
        if artifact.is_none() {
            die!("destination already exists: {new_rel}; {adopt_hint}");
        }
        die!("destination already exists: {new_rel}");
    }

    pyos::makedirs(&pyos::dirname(&new_path))?;
    write(&new_path, &format!("---\nstatus: drafting\nlast_updated: {}\n---\n\n", today()))?;

    let mut cfm = cfm;
    if !row_exists {
        cfm = row_add(
            &cfm,
            deliverable,
            &[("status", "drafting".to_string()), ("file", new_rel.clone()), ("last_updated", today())],
        )?;
    }

    let pack_dir = load_pack(ctx, project_domain(&cfm).as_deref())?.1;
    if packs::load_rules(ctx, pack_dir.as_deref())? == Some(Rules::Marketing) {
        let (c, n) = marketing::on_draft(&cdir, &new_path, &new_rel, cfm, deliverable)?;
        cfm = c;
        notes = n;
    }

    if move_pointer {
        cfm = row_set(&cfm, deliverable, "file", &new_rel)?;
    }
    cfm = row_set(&cfm, deliverable, "status", "drafting")?;
    cfm = row_set(&cfm, deliverable, "last_updated", &today())?;
    let cfm = touch_lifecycle(&cfm)?;
    write(&cpath, &join_fm(&cfm, &cbody))?;

    let label = artifact.unwrap_or(deliverable);
    append_activity(&cdir, &format!("artifact_drafted: {label} created; {new_rel}"))?;

    let pointer_note = if move_pointer { "tracker pointer moved" } else { "parent tracker pointer preserved" };
    let tail = if notes.is_empty() { String::new() } else { format!("; {}", notes.join("; ")) };
    outln!("af draft: created {new_rel} ({pointer_note}{tail})");
    outln!("\nJudgment (stays with the agent):");
    outln!("  - Load the resolved deliverable template before writing content.");
    outln!("  - Add any template-specific frontmatter fields before drafting; this command");
    outln!("    creates only the shared status/last_updated container.");
    Ok(())
}

// ------------------------------------------------------------------ adopt

pub struct AdoptArgs<'a> {
    pub project: &'a str,
    pub deliverable: &'a str,
    pub file: &'a str,
    pub workstream: Option<&'a str>,
    pub export: Option<&'a str>,
    pub notes: Option<&'a str>,
}

pub fn cmd_adopt(ctx: &Ctx, args: &AdoptArgs) -> R<()> {
    let cdir = project_dir(ctx, args.project)?;
    let sdoc = state_doc(&cdir);
    let cpath = join(&cdir, sdoc);
    let (cfm, cbody) = split_fm(&read(&cpath)?, sdoc)?;
    let (rel, target) = safe_project_rel(&cdir, Some(args.file))?;
    if !rel.to_lowercase().ends_with(".md") || !pyos::isfile(&target) {
        die!("--file must point at an existing project-relative Markdown artifact");
    }
    let (afm, abody) = match fm::split_fm_optional(&read(&target)?) {
        Some(p) => p,
        None => die!(
            "{rel} has no frontmatter block (adopt registers a drafting head: at least 'status: drafting' between --- fences)"
        ),
    };
    if get_scalar(&afm, "status").as_deref() != Some("drafting") {
        die!(
            "{rel}: status must be drafting before adoption (a superseded or ready file is a snapshot, not a head to register)"
        );
    }
    let mut notes: Vec<String> = Vec::new();
    if !has_field(&afm, "last_updated") {
        let afm = fm::upsert_scalar(&afm, "last_updated", &today(), None)?;
        write(&target, &join_fm(&afm, &abody))?;
        notes.push("last_updated stamped on the artifact".to_string());
    }

    let mut fields: Vec<(&str, String)> =
        vec![("status", "drafting".to_string()), ("file", rel.clone()), ("last_updated", today())];
    for (key, value) in [("workstream", args.workstream), ("export", args.export), ("notes", args.notes)] {
        if let Some(v) = nonempty(value) {
            fields.push((key, fm::yaml_quote(Some(v))));
        }
    }

    let mut cfm = cfm;
    let action = if row_span(&cfm, args.deliverable).is_some() {
        let current = row_get(&cfm, args.deliverable, "file");
        if let Some(cur) = &current {
            if !cur.is_empty() && cur != "null" && *cur != rel && pyos::exists(&join(&cdir, cur)) {
                die!("tracker row '{}' already has an existing artifact: {cur}", args.deliverable);
            }
        }
        for (key, value) in &fields {
            cfm = row_set(&cfm, args.deliverable, key, value)?;
        }
        "updated"
    } else {
        cfm = row_add(&cfm, args.deliverable, &fields)?;
        "created"
    };

    let cfm = touch_lifecycle(&cfm)?;
    write(&cpath, &join_fm(&cfm, &cbody))?;
    append_activity(&cdir, &format!("deliverable_adopted: {} -> {rel}", args.deliverable))?;
    let paren = if notes.is_empty() { String::new() } else { format!(" ({})", notes.join("; ")) };
    outln!("af adopt: {action} row '{}' -> {rel}{paren}", args.deliverable);
    Ok(())
}

// ------------------------------------------------------------------ new-project

pub const TOPOLOGY_SURFACES: [(&str, &str); 2] = [
    ("pipeline", "workspace/pipeline/ (use 'af pipe save' / 'af pipe start')"),
    ("studio", "workspace/studio/ (use 'af studio new')"),
];

pub fn cmd_new_project(ctx: &Ctx, slug: &str, domain: &str, name: Option<&str>) -> R<()> {
    if !rx(r"\A[a-z0-9][a-z0-9-]*\n?\z").is_match(slug) {
        die!("slug must be folder-safe lowercase kebab-case");
    }
    let (desc, pack_dir) = load_pack(ctx, Some(domain))?;
    let desc = match desc {
        Some(d) if !d.is_empty() => d,
        _ => die!("no domain pack at library/domains/{domain}/ (pack.md missing) — author the pack first"),
    };
    if let Some(topology) = get_scalar(&desc, "topology") {
        if !topology.is_empty() && topology != "null" {
            let hint = TOPOLOGY_SURFACES
                .iter()
                .find(|(k, _)| *k == topology)
                .map(|(_, v)| v.to_string())
                .unwrap_or_else(|| format!("its own {topology} surface"));
            die!("domain '{domain}' is {topology}-topology — its work lives under {hint}, not workspace/projects/");
        }
    }
    let pack_dir = pack_dir.unwrap_or_default();
    let skel_path = join(&pack_dir, "skeleton.md");
    if !pyos::isfile(&skel_path) {
        die!("domain '{domain}' ships no skeleton.md");
    }
    let cdir = join(&ctx.projects, slug);
    if pyos::exists(&cdir) {
        die!("{cdir} already exists");
    }
    pyos::makedirs(&cdir)?;
    for sub in ["sources", "knowledge"] {
        pyos::makedirs(&join(&cdir, sub))?;
    }
    for sub in ["people", "meetings", "_archive"] {
        pyos::makedirs(&pyos::join_all(&[&cdir, "knowledge", sub]))?;
    }
    write(
        &pyos::join_all(&[&cdir, "sources", "INDEX.md"]),
        "# Source Index\n\n| ID | File | Date | Description |\n|---|---|---|---|\n",
    )?;
    let name = match nonempty(name) {
        Some(n) => n.to_string(),
        None => pyos::py_title(&slug.replace('-', " ")),
    };
    let text = core::py_format(
        &read(&skel_path)?,
        &[("name", &name), ("slug", slug), ("date", &today()), ("domain", domain), ("ts", &now_iso())],
    )?;
    write(&join(&cdir, "project.md"), &text)?;
    append_activity(&cdir, &format!("project_started: {name} scaffolded ({domain})"))?;

    outln!("af new-project: workspace/projects/{slug}/ scaffolded ({domain})");
    outln!("\nJudgment (stays with the agent):");
    outln!("  - Load library/process/project-flow.md and run its kickoff: propose the plan");
    outln!("    (objective, phases if any, runway, parked) for the operator to narrow.");
    Ok(())
}
