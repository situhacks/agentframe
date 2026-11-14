/// The verbs run on a thread with a large stack. In a debug build even an unknown verb needs about 950 to 1024 KB
/// of the main thread's 1 MB on Windows, because clap builds the whole command tree, so a few more locals
/// anywhere on a verb's path overflow it.
const STACK_BYTES: usize = 64 * 1024 * 1024;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(afcore::cli::run_on_stack(STACK_BYTES, argv, afcore::cli::main_entry));
}
