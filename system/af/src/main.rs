fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(afcore::cli::main_entry(argv));
}
