fn main() {
    if let Err(error) = circuit_workbench::cli(std::env::args().skip(1).collect()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
