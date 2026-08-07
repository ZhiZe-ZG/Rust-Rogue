use std::os::raw::c_char;

pub const RIP_ART: &[&str] = &[
    "                       __________\n",
    "                      /          \\\n",
    "                     /    REST    \\\n",
    "                    /      IN      \\\n",
    "                   /     PEACE      \\\n",
    "                  /                  \\\n",
    "                  |                  |\n",
    "                  |                  |\n",
    "                  |   killed by a    |\n",
    "                  |                  |\n",
    "                  |       1980       |\n",
    "                 *|     *  *  *      | *\n",
    "         ________)/\\\\_//(\\/(/\\)/\\//\\/|_)_______\n",
];

/// Returns the Rust-owned tombstone artwork used by the death screen.
pub fn rip_art() -> &'static [&'static str] {
    RIP_ART
}

/// Returns the number of lines in the Rust-backed RIP artwork.
#[no_mangle]
pub extern "C" fn rogue_rip_count() -> usize {
    RIP_ART.len()
}

/// Returns a pointer to a specific RIP artwork line for C FFI callers.
#[no_mangle]
pub extern "C" fn rogue_rip_line(index: usize) -> *const c_char {
    RIP_ART[index].as_ptr() as *const c_char
}
