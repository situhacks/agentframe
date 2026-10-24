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

/// Verbs af.py owns that this port does not carry yet.
pub const NOT_PORTED: [&str; 3] = ["index", "search", "voice"];

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
    Ready {
        project: String,
        deliverable: String,
        /// ready a nested manifest ingredient under the row's folder
        #[arg(long)]
        artifact: Option<String>,
        #[arg(long)]
        allow_missing_exports: bool,
    },
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
    Version {
        project: String,
        deliverable: String,
        #[arg(long)]
        artifact: Option<String>,
    },
    #[command(group(ArgGroup::new("target").required(true).multiple(false).args(["artifact", "file"])))]
    Draft {
        project: String,
        deliverable: String,
        #[arg(long)]
        artifact: Option<String>,
        #[arg(long)]
        file: Option<String>,
    },
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
    #[command(name = "new-project")]
    NewProject {
        slug: String,
        #[arg(long, default_value = DEFAULT_DOMAIN)]
        domain: String,
        #[arg(long)]
        name: Option<String>,
    },
    Doctor {
        project: Option<String>,
    },
    Pipe {
        #[command(subcommand)]
        cmd: crate::pipe::PipeCmd,
    },
    Studio {
        #[command(subcommand)]
        cmd: crate::studio::StudioCmd,
    },
}

/// The repo root: `AF_ROOT`, else the nearest ancestor of the binary, then of the working
/// directory, that holds `AGENTS.md` and `system/`; else the working directory. af.py
/// derives ROOT from its own location (`system/af.py` -> repo root); the binary cannot,
/// because it lives under `system/af/target/`.
pub fn discover_root() -> String {
    if let Ok(r) = std::env::var("AF_ROOT") {
        if !r.is_empty() {
            return pyos::abspath(&r);
        }
    }
    let is_root = |p: &std::path::Path| p.join("AGENTS.md").is_file() && p.join("system").is_dir();
    let mut starts = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            starts.push(d.to_path_buf());
        }
    }
    let cwd = std::env::current_dir().ok();
    if let Some(c) = &cwd {
        starts.push(c.clone());
    }
    for s in starts {
        for a in s.ancestors() {
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
    if let Some(first) = argv.first() {
        if NOT_PORTED.contains(&first.as_str()) {
            out::err_line(&format!(
                "af: '{first}' is not ported to the Rust binary yet; use python system/af.py {first} ..."
            ));
            return 1;
        }
    }
    let ctx = Ctx::new(&discover_root());
    main_with(&ctx, argv, &run)
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
