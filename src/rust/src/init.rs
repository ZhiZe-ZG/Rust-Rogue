use crate::rnd::rnd;
/*
 * Global variable initialization.
 *
 * Ported from init.c to Rust.
 *
 * Rogue: Exploring the Dungeons of Doom
 * Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
 * All rights reserved.
 *
 * See the file LICENSE.TXT for full copyright and licensing information.
 */

use std::os::raw::{c_char, c_int, c_uchar, c_void};

use crate::player::{CStats, CThing, CThingMonster, CThingObject};

// ─── Constants ───────────────────────────────────────────────────────────────

const TRUE:  c_uchar = 1;
const FALSE: c_uchar = 0;

const MAXSTR:     usize = 1024;
const MAXNAME:    usize = 40;

const MAXPOTIONS: usize = 14;
const MAXSCROLLS: usize = 18;
const MAXRINGS:   usize = 14;
const MAXSTICKS:  usize = 14;
const NUMTHINGS:  usize = 7;
const MAXWEAPONS: usize = 9;
const MAXARMORS:  usize = 8;

const HUNGERTIME: c_int = 1300;

// Item types
const FOOD:   c_int = b':' as c_int;
const ARMOR:  c_int = b']' as c_int;
const WEAPON: c_int = b')' as c_int;

// Armor / weapon indices
const RING_MAIL: c_int = 1;
const MACE:      c_int = 0;
const BOW:       c_int = 2;
const ARROW:     c_int = 3;

// Object flags
const ISKNOW: c_int   = 0o000200;

// Player flags
const ISHALU: c_short = 0o004000;

use std::os::raw::c_short;

// ─── Data types ──────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CObjInfo {
    pub oi_name:  *mut c_char,
    pub oi_prob:  c_int,
    pub oi_worth: c_int,
    pub oi_guess: *mut c_char,
    pub oi_know:  c_uchar,
}

/// Matches the C `STONE` typedef used for ring stone names and values.
#[repr(C)]
pub struct CStone {
    pub st_name:  *const c_char,
    pub st_value: c_int,
}

// Safety: CStone only carries const string pointers (string literals) that are
// never mutated, so cross-thread sharing is fine.
unsafe impl Sync for CStone {}

// ─── Exported global data arrays ─────────────────────────────────────────────

const NCOLORS: usize = 27;
const NSTONES: usize = 26;
const NWOOD:   usize = 33;
const NMETAL:  usize = 22;

/// Potion colours.  Exported as `char *rainbow[]` for C.
#[no_mangle]
pub static mut rainbow: [*mut c_char; NCOLORS] = [
    b"amber\0"       .as_ptr() as *mut c_char,
    b"aquamarine\0"  .as_ptr() as *mut c_char,
    b"black\0"       .as_ptr() as *mut c_char,
    b"blue\0"        .as_ptr() as *mut c_char,
    b"brown\0"       .as_ptr() as *mut c_char,
    b"clear\0"       .as_ptr() as *mut c_char,
    b"crimson\0"     .as_ptr() as *mut c_char,
    b"cyan\0"        .as_ptr() as *mut c_char,
    b"ecru\0"        .as_ptr() as *mut c_char,
    b"gold\0"        .as_ptr() as *mut c_char,
    b"green\0"       .as_ptr() as *mut c_char,
    b"grey\0"        .as_ptr() as *mut c_char,
    b"magenta\0"     .as_ptr() as *mut c_char,
    b"orange\0"      .as_ptr() as *mut c_char,
    b"pink\0"        .as_ptr() as *mut c_char,
    b"plaid\0"       .as_ptr() as *mut c_char,
    b"purple\0"      .as_ptr() as *mut c_char,
    b"red\0"         .as_ptr() as *mut c_char,
    b"silver\0"      .as_ptr() as *mut c_char,
    b"tan\0"         .as_ptr() as *mut c_char,
    b"tangerine\0"   .as_ptr() as *mut c_char,
    b"topaz\0"       .as_ptr() as *mut c_char,
    b"turquoise\0"   .as_ptr() as *mut c_char,
    b"vermilion\0"   .as_ptr() as *mut c_char,
    b"violet\0"      .as_ptr() as *mut c_char,
    b"white\0"       .as_ptr() as *mut c_char,
    b"yellow\0"      .as_ptr() as *mut c_char,
];

/// Count of entries in `rainbow`.  Exported as `int cNCOLORS` for C.
#[no_mangle]
pub static mut cNCOLORS: c_int = NCOLORS as c_int;

/// Ring-stone table.  Exported as `STONE stones[]` for C.
#[no_mangle]
pub static stones: [CStone; NSTONES] = [
    CStone { st_name: b"agate\0"          .as_ptr() as *const c_char, st_value:  25 },
    CStone { st_name: b"alexandrite\0"    .as_ptr() as *const c_char, st_value:  40 },
    CStone { st_name: b"amethyst\0"       .as_ptr() as *const c_char, st_value:  50 },
    CStone { st_name: b"carnelian\0"      .as_ptr() as *const c_char, st_value:  40 },
    CStone { st_name: b"diamond\0"        .as_ptr() as *const c_char, st_value: 300 },
    CStone { st_name: b"emerald\0"        .as_ptr() as *const c_char, st_value: 300 },
    CStone { st_name: b"germanium\0"      .as_ptr() as *const c_char, st_value: 225 },
    CStone { st_name: b"granite\0"        .as_ptr() as *const c_char, st_value:   5 },
    CStone { st_name: b"garnet\0"         .as_ptr() as *const c_char, st_value:  50 },
    CStone { st_name: b"jade\0"           .as_ptr() as *const c_char, st_value: 150 },
    CStone { st_name: b"kryptonite\0"     .as_ptr() as *const c_char, st_value: 300 },
    CStone { st_name: b"lapis lazuli\0"   .as_ptr() as *const c_char, st_value:  50 },
    CStone { st_name: b"moonstone\0"      .as_ptr() as *const c_char, st_value:  50 },
    CStone { st_name: b"obsidian\0"       .as_ptr() as *const c_char, st_value:  15 },
    CStone { st_name: b"onyx\0"           .as_ptr() as *const c_char, st_value:  60 },
    CStone { st_name: b"opal\0"           .as_ptr() as *const c_char, st_value: 200 },
    CStone { st_name: b"pearl\0"          .as_ptr() as *const c_char, st_value: 220 },
    CStone { st_name: b"peridot\0"        .as_ptr() as *const c_char, st_value:  63 },
    CStone { st_name: b"ruby\0"           .as_ptr() as *const c_char, st_value: 350 },
    CStone { st_name: b"sapphire\0"       .as_ptr() as *const c_char, st_value: 285 },
    CStone { st_name: b"stibotantalite\0" .as_ptr() as *const c_char, st_value: 200 },
    CStone { st_name: b"tiger eye\0"      .as_ptr() as *const c_char, st_value:  50 },
    CStone { st_name: b"topaz\0"          .as_ptr() as *const c_char, st_value:  60 },
    CStone { st_name: b"turquoise\0"      .as_ptr() as *const c_char, st_value:  70 },
    CStone { st_name: b"taaffeite\0"      .as_ptr() as *const c_char, st_value: 300 },
    CStone { st_name: b"zircon\0"         .as_ptr() as *const c_char, st_value:  80 },
];

/// Count of entries in `stones`.  Exported as `int cNSTONES` for C.
#[no_mangle]
pub static mut cNSTONES: c_int = NSTONES as c_int;

/// Wand / staff wood materials.  Exported as `char *wood[]` for C.
#[no_mangle]
pub static mut wood: [*mut c_char; NWOOD] = [
    b"avocado wood\0"   .as_ptr() as *mut c_char,
    b"balsa\0"          .as_ptr() as *mut c_char,
    b"bamboo\0"         .as_ptr() as *mut c_char,
    b"banyan\0"         .as_ptr() as *mut c_char,
    b"birch\0"          .as_ptr() as *mut c_char,
    b"cedar\0"          .as_ptr() as *mut c_char,
    b"cherry\0"         .as_ptr() as *mut c_char,
    b"cinnibar\0"       .as_ptr() as *mut c_char,
    b"cypress\0"        .as_ptr() as *mut c_char,
    b"dogwood\0"        .as_ptr() as *mut c_char,
    b"driftwood\0"      .as_ptr() as *mut c_char,
    b"ebony\0"          .as_ptr() as *mut c_char,
    b"elm\0"            .as_ptr() as *mut c_char,
    b"eucalyptus\0"     .as_ptr() as *mut c_char,
    b"fall\0"           .as_ptr() as *mut c_char,
    b"hemlock\0"        .as_ptr() as *mut c_char,
    b"holly\0"          .as_ptr() as *mut c_char,
    b"ironwood\0"       .as_ptr() as *mut c_char,
    b"kukui wood\0"     .as_ptr() as *mut c_char,
    b"mahogany\0"       .as_ptr() as *mut c_char,
    b"manzanita\0"      .as_ptr() as *mut c_char,
    b"maple\0"          .as_ptr() as *mut c_char,
    b"oaken\0"          .as_ptr() as *mut c_char,
    b"persimmon wood\0" .as_ptr() as *mut c_char,
    b"pecan\0"          .as_ptr() as *mut c_char,
    b"pine\0"           .as_ptr() as *mut c_char,
    b"poplar\0"         .as_ptr() as *mut c_char,
    b"redwood\0"        .as_ptr() as *mut c_char,
    b"rosewood\0"       .as_ptr() as *mut c_char,
    b"spruce\0"         .as_ptr() as *mut c_char,
    b"teak\0"           .as_ptr() as *mut c_char,
    b"walnut\0"         .as_ptr() as *mut c_char,
    b"zebrawood\0"      .as_ptr() as *mut c_char,
];

/// Count of entries in `wood`.  Exported as `int cNWOOD` for C.
#[no_mangle]
pub static mut cNWOOD: c_int = NWOOD as c_int;

/// Wand metal materials.  Exported as `char *metal[]` for C.
#[no_mangle]
pub static mut metal: [*mut c_char; NMETAL] = [
    b"aluminum\0"  .as_ptr() as *mut c_char,
    b"beryllium\0" .as_ptr() as *mut c_char,
    b"bone\0"      .as_ptr() as *mut c_char,
    b"brass\0"     .as_ptr() as *mut c_char,
    b"bronze\0"    .as_ptr() as *mut c_char,
    b"copper\0"    .as_ptr() as *mut c_char,
    b"electrum\0"  .as_ptr() as *mut c_char,
    b"gold\0"      .as_ptr() as *mut c_char,
    b"iron\0"      .as_ptr() as *mut c_char,
    b"lead\0"      .as_ptr() as *mut c_char,
    b"magnesium\0" .as_ptr() as *mut c_char,
    b"mercury\0"   .as_ptr() as *mut c_char,
    b"nickel\0"    .as_ptr() as *mut c_char,
    b"pewter\0"    .as_ptr() as *mut c_char,
    b"platinum\0"  .as_ptr() as *mut c_char,
    b"steel\0"     .as_ptr() as *mut c_char,
    b"silver\0"    .as_ptr() as *mut c_char,
    b"silicon\0"   .as_ptr() as *mut c_char,
    b"tin\0"       .as_ptr() as *mut c_char,
    b"titanium\0"  .as_ptr() as *mut c_char,
    b"tungsten\0"  .as_ptr() as *mut c_char,
    b"zinc\0"      .as_ptr() as *mut c_char,
];

/// Count of entries in `metal`.  Exported as `int cNMETAL` for C.
#[no_mangle]
pub static mut cNMETAL: c_int = NMETAL as c_int;

// ─── Private static data ─────────────────────────────────────────────────────

/// Syllables used to generate scroll names.
const SYLLS: &[&str] = &[
    "a", "ab", "ag", "aks", "ala", "an", "app", "arg", "arze", "ash",
    "bek", "bie", "bit", "bjor", "blu", "bot", "bu", "byt", "comp", "con",
    "cos", "cre", "dalf", "dan", "den", "do", "e", "eep", "el", "eng", "er",
    "ere", "erk", "esh", "evs", "fa", "fid", "fri", "fu", "gan", "gar",
    "glen", "gop", "gre", "ha", "hyd", "i", "ing", "ip", "ish", "it", "ite",
    "iv", "jo", "kho", "kli", "klis", "la", "lech", "mar", "me", "mi", "mic",
    "mik", "mon", "mung", "mur", "nej", "nelg", "nep", "ner", "nes", "nes", "nih",
    "nin", "o", "od", "ood", "org", "orn", "ox", "oxy", "pay", "ple", "plu",
    "po", "pot", "prok", "re", "rea", "rhov", "ri", "ro", "rog", "rok", "rol",
    "sa", "san", "sat", "sef", "seh", "shu", "ski", "sna", "sne", "snik", "sno",
    "so", "sol", "sri", "sta", "sun", "ta", "tab", "tem", "ther", "ti", "tox",
    "trol", "tue", "turs", "u", "ulk", "um", "un", "uni", "ur", "val", "viv",
    "vly", "vom", "wah", "wed", "werg", "wex", "whon", "wun", "xo", "y", "yot",
    "yu", "zant", "zeb", "zim", "zok", "zon", "zum",
];

// MAX3(NCOLORS=27, NSTONES=26, NWOOD=33) = 33
/// Shared boolean scratch array used by init_colors, init_stones,
/// and init_materials (mirrors the C-side `static bool used[]`).
static mut USED: [c_uchar; 33] = [FALSE; 33];

// ─── Extern C globals ────────────────────────────────────────────────────────

unsafe extern "C" {
    static mut player:     CThing;
    static mut max_stats:  CStats;
    static mut food_left:  c_int;
    static mut cur_armor:  *mut CThing;
    static mut cur_weapon: *mut CThing;
    static mut a_class:    [c_int; 26];

    // Per-item colour / material / name assignments (in extern.c)
    static mut p_colors:   [*mut c_char; MAXPOTIONS];
    static mut r_stones:   [*mut c_char; MAXRINGS];
    static mut s_names:    [*mut c_char; MAXSCROLLS];
    static mut ws_made:    [*mut c_char; MAXSTICKS];
    static mut ws_type:    [*mut c_char; MAXSTICKS];

    // Scratch string buffer
    static mut prbuf:      [c_char; MAXSTR];

    // Item-info probability tables
    static mut ring_info:  [CObjInfo; MAXRINGS];
    static mut things:     [CObjInfo; NUMTHINGS];
    static mut pot_info:   [CObjInfo; MAXPOTIONS];
    static mut scr_info:   [CObjInfo; MAXSCROLLS];
    static mut ws_info:    [CObjInfo; MAXSTICKS];
    static mut weap_info:  [CObjInfo; MAXWEAPONS + 1];
    static mut arm_info:   [CObjInfo; MAXARMORS];
}

// ─── Extern C functions ──────────────────────────────────────────────────────

unsafe extern "C" {
    fn add_pack(obj: *mut CThing, silent: c_uchar);
    fn init_weapon(obj: *mut CThing, which: c_int);
    fn malloc(size: usize) -> *mut c_void;
    fn new_item() -> *mut CThing;
    fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char;
    fn strlen(s: *const c_char) -> usize;
}

// ─── Private helpers ─────────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut CThing) -> *mut CThingMonster {
    tp as *mut CThingMonster
}

#[inline]
unsafe fn thing_o(tp: *mut CThing) -> *mut CThingObject {
    tp as *mut CThingObject
}

// ─── Exported functions ───────────────────────────────────────────────────────

/// Roll up the starting player: give food, armor, weapons, and arrows.
#[no_mangle]
pub unsafe extern "C" fn init_player() {
    (*thing_t(&raw mut player)).t_stats = max_stats;
    food_left = HUNGERTIME;

    // Give her some food
    let obj = new_item();
    (*thing_o(obj)).o_type  = FOOD;
    (*thing_o(obj)).o_count = 1;
    add_pack(obj, TRUE);

    // A suit of ring-mail armor
    let obj = new_item();
    (*thing_o(obj)).o_type  = ARMOR;
    (*thing_o(obj)).o_which = RING_MAIL;
    (*thing_o(obj)).o_arm   = a_class[RING_MAIL as usize] - 1;
    (*thing_o(obj)).o_flags |= ISKNOW;
    (*thing_o(obj)).o_count = 1;
    cur_armor = obj;
    add_pack(obj, TRUE);

    // A +1 mace
    let obj = new_item();
    init_weapon(obj, MACE);
    (*thing_o(obj)).o_hplus = 1;
    (*thing_o(obj)).o_dplus = 1;
    (*thing_o(obj)).o_flags |= ISKNOW;
    add_pack(obj, TRUE);
    cur_weapon = obj;

    // A +1 bow
    let obj = new_item();
    init_weapon(obj, BOW);
    (*thing_o(obj)).o_hplus = 1;
    (*thing_o(obj)).o_flags |= ISKNOW;
    add_pack(obj, TRUE);

    // Arrows
    let obj = new_item();
    init_weapon(obj, ARROW);
    (*thing_o(obj)).o_count = rnd(15) + 25;
    (*thing_o(obj)).o_flags |= ISKNOW;
    add_pack(obj, TRUE);
}

/// Assign a random colour from `rainbow` to each potion.
#[no_mangle]
pub unsafe extern "C" fn init_colors() {
    for i in 0..NCOLORS {
        USED[i] = FALSE;
    }
    for i in 0..MAXPOTIONS {
        let j = loop {
            let j = rnd(NCOLORS as c_int) as usize;
            if USED[j] == FALSE {
                break j;
            }
        };
        USED[j] = TRUE;
        p_colors[i] = rainbow[j];
    }
}

/// Generate random pronounceable names for each scroll.
#[no_mangle]
pub unsafe extern "C" fn init_names() {
    for i in 0..MAXSCROLLS {
        let prbuf_base: *mut c_char = (&raw mut prbuf) as *mut c_char;
        let mut cp: *mut c_char = prbuf_base;
        let mut nwords = rnd(3) + 2;
        while nwords > 0 {
            nwords -= 1;
            let mut nsyl = rnd(3) + 1;
            while nsyl > 0 {
                nsyl -= 1;
                let sp = SYLLS[rnd(SYLLS.len() as c_int) as usize];
                let sp_ptr = sp.as_ptr() as *const c_char;
                if (cp as *const c_char).add(strlen(sp_ptr))
                    > (prbuf_base as *const c_char).add(MAXNAME)
                {
                    break;
                }
                let mut p = sp_ptr;
                while unsafe { *p != 0 } {
                    unsafe { *cp = *p; }
                    cp = cp.add(1);
                    p = p.add(1);
                }
            }
            *cp = b' ' as c_char;
            cp = cp.add(1);
        }
        // Back up over the trailing space and NUL-terminate
        cp = cp.sub(1);
        *cp = 0;
        let len = strlen(prbuf_base as *const c_char);
        s_names[i] = malloc(len + 1) as *mut c_char;
        strcpy(s_names[i], prbuf_base as *const c_char);
    }
}

/// Assign a random stone setting to each ring type.
#[no_mangle]
pub unsafe extern "C" fn init_stones() {
    for i in 0..NSTONES {
        USED[i] = FALSE;
    }
    for i in 0..MAXRINGS {
        let j = loop {
            let j = rnd(NSTONES as c_int) as usize;
            if USED[j] == FALSE {
                break j;
            }
        };
        USED[j] = TRUE;
        r_stones[i] = stones[j].st_name as *mut c_char;
        ring_info[i].oi_worth += stones[j].st_value;
    }
}

/// Assign random wood / metal materials to wands and staves.
#[no_mangle]
pub unsafe extern "C" fn init_materials() {
    for i in 0..NWOOD {
        USED[i] = FALSE;
    }
    let mut metused: [c_uchar; NMETAL] = [FALSE; NMETAL];
    for i in 0..MAXSTICKS {
        loop {
            if rnd(2) == 0 {
                let j = rnd(NMETAL as c_int) as usize;
                if metused[j] == FALSE {
                    ws_type[i] = b"wand\0".as_ptr() as *mut c_char;
                    ws_made[i] = metal[j];
                    metused[j] = TRUE;
                    break;
                }
            } else {
                let j = rnd(NWOOD as c_int) as usize;
                if USED[j] == FALSE {
                    ws_type[i] = b"staff\0".as_ptr() as *mut c_char;
                    ws_made[i] = wood[j];
                    USED[j] = TRUE;
                    break;
                }
            }
        }
    }
}

/// Accumulate cumulative probabilities for one item-info table.
///
/// Mirrors the C `sumprobs(struct obj_info *info, int bound)`.
#[no_mangle]
pub unsafe extern "C" fn sumprobs(info: *mut CObjInfo, bound: c_int) {
    let endp = info.add(bound as usize);
    let mut p = info.add(1);
    while p < endp {
        (*p).oi_prob += (*p.sub(1)).oi_prob;
        p = p.add(1);
    }
}

/// Initialize cumulative probabilities for all item types.
#[no_mangle]
pub unsafe extern "C" fn init_probs() {
    sumprobs(things.as_mut_ptr(),    NUMTHINGS  as c_int);
    sumprobs(pot_info.as_mut_ptr(),  MAXPOTIONS as c_int);
    sumprobs(scr_info.as_mut_ptr(),  MAXSCROLLS as c_int);
    sumprobs(ring_info.as_mut_ptr(), MAXRINGS   as c_int);
    sumprobs(ws_info.as_mut_ptr(),   MAXSTICKS  as c_int);
    sumprobs(weap_info.as_mut_ptr(), MAXWEAPONS as c_int);
    sumprobs(arm_info.as_mut_ptr(),  MAXARMORS  as c_int);
}

/// Return a random colour if the player is hallucinating, otherwise
/// return the supplied colour unchanged.
#[no_mangle]
pub unsafe extern "C" fn pick_color(col: *mut c_char) -> *mut c_char {
    if (*thing_t(&raw mut player)).t_flags & ISHALU != 0 {
        rainbow[rnd(NCOLORS as c_int) as usize]
    } else {
        col
    }
}
