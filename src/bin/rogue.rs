//! Native Rust entry point for Rogue.
//!
//! Collects `std::env::args()` and hands them to the game entry point so the
//! game can be built and run with `cargo` alone — no C source, headers, or
//! autotools.

use clap::Parser;
use rogue_rust::command_line::CommandLineParameter;

fn main() {
    let parameter = match CommandLineParameter::try_parse_from(std::env::args()) {
        Ok(command_line) => command_line,
        Err(error) => {
            let exit_code = error.exit_code();
            let _ = error.print();
            std::process::exit(exit_code);
        }
    };
    unsafe { rogue_rust::startup::rogue_main(parameter) }
}
