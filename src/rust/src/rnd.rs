use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::{Mutex, OnceLock};

static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();

// Lazily initialize and return the shared RNG state.
fn rng_state() -> &'static Mutex<StdRng> {
    RNG.get_or_init(|| Mutex::new(StdRng::from_seed([0x5eu8; 32])))
}

/// Return a pseudo-random integer in `[0, range)`, matching the legacy
/// Rogue `rnd(range)` contract.  `range == 0` yields `0`.
pub fn rnd(range: i32) -> i32 {
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

/// Reset the shared RNG so startup code can preserve deterministic seeds.
pub fn set_seed(seed: i32) {
    let mut guard = rng_state().lock().unwrap();
    *guard = StdRng::seed_from_u64(seed as u64);
}
