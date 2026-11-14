/// The verbs run on a thread with a large stack: a debug build's frames for `af doctor` and `af brief` come
/// close to the main thread's 1 MB on Windows, and a few more locals anywhere on their path overflow it.
const STACK_BYTES: usize = 64 * 1024 * 1024;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(move || afcore::cli::main_entry(argv))
        .ok()
        .and_then(|t| t.join().ok())
        .unwrap_or(1);
    std::process::exit(code);
}
