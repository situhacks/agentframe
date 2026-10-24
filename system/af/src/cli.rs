//! argparse surface and `main()`: dispatch one verb, append its outcome to the run log.

use crate::cmds::{self, AdoptArgs, ReadyArgs};
use crate::core::{Ctx, DEFAULT_DOMAIN};
use crate::doctor;
use crate::error::{last_error, reset_last_error, AfError, R};
use crate::out;
use crate::packs::marketing::PublishArgs;
use crate::pyos;
use crate::runlog;
use clap::{ArgGroup, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "af",
    about = "AgentFrame state-transition CLI",
    infer_long_args = true,
    args_override_self = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Load the voice system in one read, or lint a head against it
    Voice {
        #[command(subcommand)]
        cmd: crate::voice::VoiceCmd,
    },
    /// Mark a deliverable head ready and run the pack's readiness rules
    Ready {
        project: String,
        deliverable: String,
        /// ready a nested manifest ingredient under the row's folder
        #[arg(long)]
        artifact: Option<String>,
        #[arg(long)]
        allow_missing_exports: bool,
    },
    /// Record a published edition: URL, date, platform, media
    Publish {
        project: String,
        deliverable: String,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        posted_at: Option<String>,
        #[arg(long)]
        platform: Option<String>,
        #[arg(long, num_args = 0..)]
        media: Vec<String>,
    },
    /// Snapshot the head as a new version before any content change
    Version {
        project: String,
        deliverable: String,
        #[arg(long)]
        artifact: Option<String>,
    },
    #[command(group(ArgGroup::new("target").required(true).multiple(false).args(["artifact", "file"])))]
    /// Create a first draft (v1) and its tracker row
    Draft {
        project: String,
        deliverable: String,
        #[arg(long)]
        artifact: Option<String>,
        #[arg(long)]
        file: Option<String>,
    },
    /// Adopt an existing file as a tracked deliverable
    Adopt {
        project: String,
        deliverable: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        workstream: Option<String>,
        #[arg(long)]
        export: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Scaffold workspace/projects/<slug>/ from a domain pack
    #[command(name = "new-project")]
    NewProject {
        slug: String,
        #[arg(long, default_value = DEFAULT_DOMAIN)]
        domain: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Check state truth, size caps, links, the life layer, and button refusals
    Doctor {
        project: Option<String>,
    },
    /// The careers pipeline board and application folders
    Pipe {
        #[command(subcommand)]
        cmd: crate::pipe::PipeCmd,
    },
    /// The creator calendar board and post folders
    Studio {
        #[command(subcommand)]
        cmd: crate::studio::StudioCmd,
    },
    /// The chat registry: register, list, stop, end, bundle
    Session {
        #[command(subcommand)]
        cmd: crate::sessions::SessionCmd,
    },
    /// Open loops in workspace/life/open.md: add, close, due
    Open {
        #[command(subcommand)]
        cmd: crate::open::OpenCmd,
    },
    /// The zero-token catch-up: due loops, chats, backlog, boards
    Brief {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        write: bool,
    },
    /// Close a project: move it to completed/ and print the harvest checklist
    Close {
        slug: String,
        #[arg(long)]
        force: bool,
    },
    /// Build, refresh, or evaluate the retrieval index
    Index {
        #[command(subcommand)]
        cmd: crate::index::IndexCmd,
    },
    /// Search the vault (hybrid keyword and embedding ranking)
    Search {
        query: String,
        #[arg(short = 'n', long, default_value = "8", value_parser = crate::index::py_int, allow_negative_numbers = true)]
        limit: i64,
        #[arg(long)]
        json: bool,
    },
    /// Run a Claude Code hook: session-start, stop, session-end, notification, pre-tool-use,
    /// post-tool-use (payload on stdin)
    Hook {
        event: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
        rest: Vec<String>,
    },
}

/// The vault root: `AF_ROOT`, else the nearest ancestor of the working directory that holds
/// `AGENTS.md` and `system/`; else the working directory. The binary is installed on PATH,
/// so its own location says nothing about the vault.
pub fn discover_root() -> String {
    if let Ok(r) = std::env::var("AF_ROOT") {
        if !r.is_empty() {
            return pyos::abspath(&r);
        }
    }
    let is_root = |p: &std::path::Path| p.join("AGENTS.md").is_file() && p.join("system").is_dir();
    let cwd = std::env::current_dir().ok();
    if let Some(c) = &cwd {
        for a in c.ancestors() {
            if is_root(a) {
                return pyos::abspath(&a.to_string_lossy());
            }
        }
    }
    pyos::abspath(&cwd.map(|c| c.to_string_lossy().into_owned()).unwrap_or_else(|| ".".into()))
}

/// Parse and dispatch; Ok(code) is a normal return (0) or an explicit `sys.exit(code)`.
pub fn run(ctx: &Ctx, argv: &[String]) -> R<i32> {
    let mut full = vec!["af".to_string()];
    full.extend(argv.iter().cloned());
    let cli = match Cli::try_parse_from(&full) {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            if e.use_stderr() {
                return Err(AfError::Usage(e.to_string()));
            }
            // --help: argparse prints to stdout and exits 0.
            return Ok(0);
        }
    };
    match cli.cmd {
        Cmd::Voice { cmd } => crate::voice::run(ctx, cmd),
        Cmd::Ready { project, deliverable, artifact, allow_missing_exports } => {
            let a = ReadyArgs {
                project: &project,
                deliverable: &deliverable,
                artifact: artifact.as_deref(),
                allow_missing_exports,
            };
            cmds::cmd_ready(ctx, &a).map(|_| 0)
        }
        Cmd::Publish { project, deliverable, url, posted_at, platform, media } => {
            let a = PublishArgs {
                deliverable: &deliverable,
                url: url.as_deref(),
                posted_at: posted_at.as_deref(),
                platform: platform.as_deref(),
                media: &media,
            };
            cmds::cmd_publish(ctx, &a, &project).map(|_| 0)
        }
        Cmd::Version { project, deliverable, artifact } => {
            cmds::cmd_version(ctx, &project, &deliverable, artifact.as_deref()).map(|_| 0)
        }
        Cmd::Draft { project, deliverable, artifact, file } => {
            cmds::cmd_draft(ctx, &project, &deliverable, artifact.as_deref(), file.as_deref()).map(|_| 0)
        }
        Cmd::Adopt { project, deliverable, file, workstream, export, notes } => {
            let a = AdoptArgs {
                project: &project,
                deliverable: &deliverable,
                file: &file,
                workstream: workstream.as_deref(),
                export: export.as_deref(),
                notes: notes.as_deref(),
            };
            cmds::cmd_adopt(ctx, &a).map(|_| 0)
        }
        Cmd::NewProject { slug, domain, name } => cmds::cmd_new_project(ctx, &slug, &domain, name.as_deref()).map(|_| 0),
        Cmd::Doctor { project } => doctor::cmd_doctor(ctx, project.as_deref()),
        Cmd::Pipe { cmd } => crate::pipe::dispatch(ctx, cmd).map(|_| 0),
        Cmd::Studio { cmd } => crate::studio::dispatch(ctx, cmd).map(|_| 0),
        Cmd::Session { cmd } => crate::sessions::run(ctx, cmd),
        Cmd::Open { cmd } => crate::open::run(ctx, cmd),
        Cmd::Brief { json, write } => crate::brief::cmd_brief(ctx, json, write),
        Cmd::Close { slug, force } => crate::close::cmd_close(ctx, &slug, force),
        Cmd::Index { cmd } => crate::index::cmd_index(&ctx.root, &cmd),
        Cmd::Search { query, limit, json } => crate::index::cmd_search(&ctx.root, &query, limit, json),
        // `af hook` is intercepted before the parser; the variant exists for --help.
        Cmd::Hook { .. } => Ok(0),
    }
}

fn panic_message(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic".to_string()
    }
}

/// `main()`: run one button and append its outcome to the run log, whatever the outcome.
/// Returns the process exit code.
pub fn main_entry(argv: Vec<String>) -> i32 {
    if let Some(code) = crate::hooks::intercept(&argv).or_else(|| crate::voice::intercept(&argv)) {
        return code;
    }
    let ctx = Ctx::new(&discover_root());
    let code = main_with(&ctx, argv.clone(), &run);
    crate::hooks::index_refresh::release_lock(&argv);
    code
}

/// `main()` against a given root, with an injectable dispatcher (tests drive the crash path
/// and the run log through it).
pub fn main_with(ctx: &Ctx, argv: Vec<String>, dispatch: &dyn Fn(&Ctx, &[String]) -> R<i32>) -> i32 {
    let quiet = argv.is_empty() || argv.iter().any(|a| a == "-h" || a == "--help");
    reset_last_error();
    let verb = runlog::verb_of(&argv);
    // A panic is the port's traceback: the default hook prints it, the run log records it.
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dispatch(ctx, &argv)));
    // (process exit code, logged exit, refusal, crash)
    let (exit, logged, refusal, crash): (i32, i32, Option<String>, Option<String>) = match outcome {
        Ok(Ok(code)) => {
            let refusal = if code != 0 { last_error().or_else(|| (code == 2).then(|| "usage".to_string())) } else { None };
            (code, code, refusal, None)
        }
        Ok(Err(AfError::Refusal(m))) => (1, 1, Some(last_error().unwrap_or(m)), None),
        Ok(Err(AfError::Usage(_))) => (2, 2, Some(last_error().unwrap_or_else(|| "usage".to_string())), None),
        Ok(Err(AfError::Crash { msg, .. })) => {
            out::err_line(&format!("af: CRASH: {msg}"));
            (1, 2, None, Some(msg))
        }
        Err(p) => {
            let msg = format!("RustPanic: {}", panic_message(p.as_ref()));
            out::err_line(&format!("af: CRASH: {msg}"));
            (1, 2, None, Some(msg))
        }
    };
    if !quiet {
        runlog::log_run(ctx, &verb, &argv, logged, refusal.as_deref(), crash.as_deref());
    }
    exit
}
