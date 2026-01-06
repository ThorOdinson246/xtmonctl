fn main() {
    if let Err(error) = xtmonctl::cli::run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
