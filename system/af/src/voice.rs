//! `af voice bundle` and `af voice lint`.

pub mod bundle;
pub mod lint;

use crate::core::{self, Ctx};
use crate::die;
use crate::error::{AfError, R};
use crate::fm::{self, get_scalar, row_get};
use crate::out;
use crate::pyjson;
use crate::pyos::{self, join, join_all};
use clap::{Parser, Subcommand};

#[derive(Subcommand, Debug)]
pub enum VoiceCmd {
    /// write the voice system to one file for a single read
    Bundle {
        project: Option<String>,
        deliverable: Option<String>,
        #[arg(long, value_parser = ["formal", "informal"])]
        register: Option<String>,
        /// task context, repeatable: long-form, short-form, email, builder-pov, market-signal, slide, cover
        #[arg(long, action = clap::ArgAction::Append)]
        context: Vec<String>,
        #[arg(long, action = clap::ArgAction::Append, value_parser = ["formal", "informal"])]
        borrow: Vec<String>,
        /// a file in voice/templates/, e.g. substack-essay.md
        #[arg(long)]
        template: Option<String>,
        /// core: rules + one exemplar for a short write or a copyedit pass; full: everything. Default from the context
        #[arg(long, value_parser = ["core", "full"])]
        tier: Option<String>,
    },
    /// AgentFrame voice lint (zero-budget rules and the hyphen alarm)
    Lint(LintArgs),
}

#[derive(clap::Args, Debug)]
pub struct LintArgs {
    /// the head to lint
    file: Option<String>,
    /// resolve the head through the project tracker instead of a path
    #[arg(long)]
    project: Option<String>,
    /// tracker row slug or project-relative path (with --project)
    #[arg(long)]
    deliverable: Option<String>,
    /// 'auto' (sibling v{N-1}), 'none', or a file
    #[arg(long, default_value = "auto")]
    prev: String,
    /// machine-readable result
    #[arg(long)]
    json: bool,
}

/// `af voice ...` dispatched from the button parser (bundle writes a run-log row).
pub fn run(ctx: &Ctx, cmd: VoiceCmd) -> R<i32> {
    match cmd {
        VoiceCmd::Bundle { project, deliverable, register, context, borrow, template, tier } => {
            cmd_bundle(ctx, project, deliverable, register, context, borrow, template, tier).map(|_| 0)
        }
        VoiceCmd::Lint(a) => Ok(cmd_lint(ctx, &a)),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_bundle(
    ctx: &Ctx,
    project: Option<String>,
    deliverable: Option<String>,
    register: Option<String>,
    contexts: Vec<String>,
    borrow: Vec<String>,
    template: Option<String>,
    tier: Option<String>,
) -> R<()> {
    let mut head = None;
    let mut register = register;
    let mut borrow = borrow;
    let mut contexts = contexts;
    let mut template = template;
    if project.is_some() || deliverable.is_some() {
        let (Some(project), Some(deliverable)) = (project.as_deref(), deliverable.as_deref()) else {
            die!("af voice bundle: give both project and deliverable, or --register");
        };
        let cdir = core::project_dir(ctx, project)?;
        let sdoc = core::state_doc(&cdir);
        let (cfm, _) = fm::split_fm(&core::read(&join(&cdir, sdoc))?, sdoc)?;
        let (_, rel) = core::resolve_deliverable_target(&cfm, deliverable)?;
        let h = join(&cdir, &rel);
        if !pyos::isfile(&h) {
            die!("af voice bundle: head not found: {rel}");
        }
        let (base, head_borrow, dtype) = bundle::recipe_from_head(&h)?;
        if register.as_deref().map_or(true, str::is_empty) {
            register = base;
        }
        if borrow.is_empty() {
            borrow = head_borrow;
        }
        if template.as_deref().map_or(true, str::is_empty) {
            template = bundle::type_template(dtype.as_deref().unwrap_or("")).map(|s| s.to_string());
        }
        if contexts.is_empty() {
            if let Some(c) = dtype.as_deref().and_then(bundle::type_context) {
                contexts = vec![c.to_string()];
            }
        }
        head = Some(h);
    }
    let Some(register) = register.filter(|r| !r.is_empty()) else {
        die!("af voice bundle: the head declares no voice recipe (voice.base_register or register); pass --register");
    };
    let recipe = bundle::Recipe { register, contexts, borrow, template };
    let voice_dir = bundle::voice_dir(&ctx.root);
    let out_dir = bundle::out_dir(&ctx.root);
    let place = bundle::Place { root: &ctx.root, voice_dir: &voice_dir, out_dir: &out_dir, stamp: None };
    let b = bundle::build(&place, &recipe, head.as_deref(), tier.as_deref())?;
    out::line(&bundle::format_report(&ctx.root, &b)?);
    Ok(())
}

// ---------------------------------------------------------------- voice lint

/// The tracked head for a project row or project-relative path, with af's refusals
/// silenced (voice_lint.resolve_head): Ok(None) where af would refuse, Err on a crash.
pub fn resolve_head_quiet(ctx: &Ctx, project: &str, deliverable: &str) -> R<Option<String>> {
    let Some(cdir) = project_dir_quiet(ctx, project)? else { return Ok(None) };
    let sdoc = core::state_doc(&cdir);
    let Some((cfm, _)) = fm::split_fm_optional(&core::read(&join(&cdir, sdoc))?) else { return Ok(None) };
    let norm = deliverable.replace('\\', "/");
    let rel = if !norm.contains('/') {
        match row_get(&cfm, deliverable, "file") {
            Some(f) if !f.is_empty() => f,
            _ => return Ok(None),
        }
    } else {
        norm
    };
    let head = join(&cdir, &rel);
    Ok(if pyos::isfile(&head) { Some(head) } else { None })
}

/// `af.project_dir` with its refusal as Ok(None).
fn project_dir_quiet(ctx: &Ctx, arg: &str) -> R<Option<String>> {
    let completed = join(&ctx.projects, "completed");
    for base in [&ctx.projects, &completed] {
        let d = join(base, arg);
        if pyos::isfile(&join(&d, "project.md")) {
            return Ok(Some(d));
        }
    }
    let d = join_all(&[&ctx.pipeline, "applications", arg]);
    if pyos::isfile(&join(&d, "application.md")) {
        return Ok(Some(d));
    }
    let d = join_all(&[&ctx.studio, "posts", arg]);
    if pyos::isfile(&join(&d, "post.md")) {
        return Ok(Some(d));
    }
    for base in [&ctx.projects, &completed] {
        if !pyos::isdir(base) {
            continue;
        }
        for name in pyos::sorted_listdir(base)? {
            let sp = join_all(&[base, &name, "project.md"]);
            if pyos::isfile(&sp) {
                let Some((sfm, _)) = fm::split_fm_optional(&core::read(&sp)?) else { return Ok(None) };
                if get_scalar(&sfm, "slug").as_deref() == Some(arg) {
                    return Ok(Some(join(base, &name)));
                }
            }
        }
    }
    Ok(None)
}

/// `af voice lint`; returns the exit code.
pub fn cmd_lint(ctx: &Ctx, a: &LintArgs) -> i32 {
    let path = if let Some(project) = &a.project {
        let Some(deliverable) = &a.deliverable else {
            out::err_line("af voice lint: error: --deliverable is required with --project");
            return 2;
        };
        match resolve_head_quiet(ctx, project, deliverable) {
            Ok(Some(p)) => p,
            Ok(None) => {
                out::err_line(&format!("voice lint: could not resolve {project} / {deliverable}"));
                return 2;
            }
            Err(e) => return crashed(&e),
        }
    } else if let Some(f) = &a.file {
        f.clone()
    } else {
        out::err_line("af voice lint: error: give a file, or --project with --deliverable");
        return 2;
    };
    let prev = match a.prev.as_str() {
        "auto" => lint::Prev::Auto,
        "none" => lint::Prev::None,
        p => lint::Prev::Path(p),
    };
    let result = match lint::lint_file(&path, prev) {
        Ok(r) => r,
        Err(e) => return crashed(&e),
    };
    if a.json {
        out::line(&pyjson::dumps_indent1(&lint::to_pyval(&result)));
    } else {
        out::line(&lint::format_report(&result, &ctx.root));
    }
    if result.hard.is_empty() {
        0
    } else {
        1
    }
}

/// An uncaught Python exception: the traceback's last line on stderr, exit 1.
fn crashed(e: &AfError) -> i32 {
    out::err_line(&format!("af: CRASH: {e}"));
    1
}

#[derive(Parser, Debug)]
#[command(name = "af voice lint", infer_long_args = true, args_override_self = true, disable_version_flag = true)]
struct LintCli {
    #[command(flatten)]
    args: LintArgs,
}

/// `af voice lint ...` runs outside the button wrapper: it writes no run-log
/// row, and a hard finding is a lint result, not a button refusal.
pub fn intercept(argv: &[String]) -> Option<i32> {
    if argv.first().map(String::as_str) != Some("voice") || argv.get(1).map(String::as_str) != Some("lint") {
        return None;
    }
    let mut full = vec!["af voice lint".to_string()];
    full.extend(argv[2..].iter().cloned());
    let cli = match LintCli::try_parse_from(&full) {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return Some(if e.use_stderr() { 2 } else { 0 });
        }
    };
    let ctx = Ctx::new(&crate::cli::discover_root());
    Some(cmd_lint(&ctx, &cli.args))
}

// ---------------------------------------------------------------- sha1

/// SHA-1 hex digest (hashlib.sha1(...).hexdigest()).
pub fn sha1_hex(data: &[u8]) -> String {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

