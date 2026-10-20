//! Native Rust entry point for Rogue.
//!
//! Parses `std::env::args()` with clap and hands the result to
//! [`rogue_rust::startup::rogue_main`], so the game can be built and run with
//! `cargo` alone — no C source, headers, or autotools.

use clap::Parser;
use rogue_rust::command_line_parameter::CommandLineParameter;
use rogue_rust::startup::rogue_main;

fn main() {
    let parameter = match CommandLineParameter::try_parse_from(std::env::args()) {
        Ok(command_line) => command_line,
        Err(error) => {
            let exit_code = error.exit_code();
            let _ = error.print();
            std::process::exit(exit_code);
        }
    };
    // `rogue_main` never returns: it terminates the process itself.
    unsafe { rogue_main(parameter) }
}
