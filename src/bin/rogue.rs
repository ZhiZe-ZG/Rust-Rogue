//! Native Rust entry point for Rogue.
//!
//! Collects `std::env::args()` and hands them to the game entry point so the
//! game can be built and run with `cargo` alone — no C source, headers, or
//! autotools.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match unsafe { rogue_rust::startup::rogue_main(&args) } {
        Ok(code) => code,
        Err(error) => {
            eprintln!("rogue: {error}");
            1
        }
    };
    std::process::exit(code);
}
