#[allow(dead_code)]
#[path = "../../../keyboard_keyboard/code/src/constants.rs"]
mod constants;
mod noise;
#[allow(dead_code)]
#[path = "../../../keyboard_keyboard/code/src/switch.rs"]
mod switch;
#[allow(dead_code)]
#[path = "../../../keyboard_keyboard/code/src/velocity.rs"]
mod velocity;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("noise-study") => noise::cli(&args[1..]),
        Some("noise-clock") => noise::clock_cli(&args[1..]),
        Some("power-kernel") => noise::power_fault_cli(&args[1..]),
        Some("hall-kernel") => noise::hall_fault_cli(&args[1..]),
        _ => circuit_workbench::cli(args),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
