//! Native Rust entry point for Rogue.
//!
//! Builds the C-style `argv`/`envp` arrays from `std::env` and hands them to
//! the legacy C-ABI `startup::rogue_main`, so the game can be built and run
//! with `cargo` alone — no C source, headers, or autotools.

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

fn main() {
    // Collect args as CStrings; keep them alive for the duration of the call.
    let args: Vec<CString> = std::env::args()
        .map(|a| CString::new(a).unwrap_or_default())
        .collect();

    let mut argv_ptrs: Vec<*mut c_char> = args.iter().map(|a| a.as_ptr() as *mut c_char).collect();
    argv_ptrs.push(std::ptr::null_mut());

    // Build a trivial envp from the process environment.
    let env: Vec<CString> = std::env::vars()
        .map(|(k, v)| CString::new(format!("{k}={v}")).unwrap_or_default())
        .collect();
    let mut envp_ptrs: Vec<*mut c_char> = env.iter().map(|e| e.as_ptr() as *mut c_char).collect();
    envp_ptrs.push(std::ptr::null_mut());

    let code = unsafe {
        rogue_rust::startup::rogue_main(
            args.len() as c_int,
            argv_ptrs.as_mut_ptr(),
            envp_ptrs.as_mut_ptr(),
        )
    };

    std::process::exit(code);
}