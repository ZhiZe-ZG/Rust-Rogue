use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::os::raw::c_int;
use std::sync::{Mutex, OnceLock};

static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();

// Lazily initialize and return the shared RNG state used by the legacy C API.
fn rng_state() -> &'static Mutex<StdRng> {
    RNG.get_or_init(|| Mutex::new(StdRng::from_seed([0x5eu8; 32])))
}

#[no_mangle]
// Match Rogue's legacy rnd(range) contract while sourcing values from Rust.
pub extern "C" fn rnd(range: c_int) -> c_int {
    if range == 0 {
        return 0;
    }

    let magnitude = range.abs();
    let mut guard = rng_state().lock().unwrap();
    let value = guard.gen_range(0..magnitude);

    if range < 0 {
        -value
    } else {
        value
    }
}

#[no_mangle]
// Reset the shared RNG so C startup code can preserve deterministic seeds.
pub extern "C" fn set_seed(seed: c_int) {
    let mut guard = rng_state().lock().unwrap();
    *guard = StdRng::seed_from_u64(seed as u64);
}
