#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--selftest") {
        std::process::exit(portablehub_lib::selftest::run(&args));
    }
    if args.iter().any(|a| a == "--discover" || a == "--import") {
        std::process::exit(portablehub_lib::cli::run(&args));
    }
    portablehub_lib::run();
}
