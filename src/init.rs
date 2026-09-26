//! Global variable initialization.
//!
//! Ported from `src/c/init.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use crate::game::PLAYER;
use crate::globals::{
    arm_info, pot_info, ring_info, scr_info, things, weap_info, ws_info, CObjInfo,
};
use crate::rnd::rnd;

use std::os::raw::{c_int, c_uchar};

use crate::entity::player::{MonsterFlags, ObjectFlags, Stats, Thing, ThingMonster, ThingObject};
use crate::item::pack::add_pack;
use crate::item::arena::new_item;
use crate::item::weapons::init_weapon;

// ─── Constants ───────────────────────────────────────────────────────────────

const MAXSTR: usize = 1024;
const MAXNAME: usize = 40;

const MAXPOTIONS: usize = 14;
const MAXSCROLLS: usize = 18;
const MAXRINGS: usize = 14;
const MAXSTICKS: usize = 14;
const NUMTHINGS: usize = 7;
const MAXWEAPONS: usize = 9;
const MAXARMORS: usize = 8;

const HUNGERTIME: c_int = 1300;

// Item types
const FOOD: c_int = b':' as c_int;
const ARMOR: c_int = b']' as c_int;
const WEAPON: c_int = b')' as c_int;

// Armor / weapon indices
const RING_MAIL: c_int = 1;
const MACE: c_int = 0;
const BOW: c_int = 2;
const ARROW: c_int = 3;

/// Matches the C `STONE` typedef used for ring stone names and values.
#[repr(C)]
pub struct CStone {
    pub st_name: &'static str,
    pub st_value: c_int,
}

// Safety: CStone only carries `&'static str` references (string literals) that
// are never mutated, so cross-thread sharing is fine.
unsafe impl Sync for CStone {}

// ─── Exported global data arrays ─────────────────────────────────────────────

const NSTONES: usize = 26;
const NWOOD: usize = 33;
const NMETAL: usize = 22;

/// Ring-stone table.  Exported as `STONE stones[]` for legacy consumers.
#[no_mangle]
pub static stones: [CStone; NSTONES] = [
    CStone { st_name: "agate", st_value: 25 },
    CStone { st_name: "alexandrite", st_value: 40 },
    CStone { st_name: "amethyst", st_value: 50 },
    CStone { st_name: "carnelian", st_value: 40 },
    CStone { st_name: "diamond", st_value: 300 },
    CStone { st_name: "emerald", st_value: 300 },
    CStone { st_name: "germanium", st_value: 225 },
    CStone { st_name: "granite", st_value: 5 },
    CStone { st_name: "garnet", st_value: 50 },
    CStone { st_name: "jade", st_value: 150 },
    CStone { st_name: "kryptonite", st_value: 300 },
    CStone { st_name: "lapis lazuli", st_value: 50 },
    CStone { st_name: "moonstone", st_value: 50 },
    CStone { st_name: "obsidian", st_value: 15 },
    CStone { st_name: "onyx", st_value: 60 },
    CStone { st_name: "opal", st_value: 200 },
    CStone { st_name: "pearl", st_value: 220 },
    CStone { st_name: "peridot", st_value: 63 },
    CStone { st_name: "ruby", st_value: 350 },
    CStone { st_name: "sapphire", st_value: 285 },
    CStone { st_name: "stibotantalite", st_value: 200 },
    CStone { st_name: "tiger eye", st_value: 50 },
    CStone { st_name: "topaz", st_value: 60 },
    CStone { st_name: "turquoise", st_value: 70 },
    CStone { st_name: "taaffeite", st_value: 300 },
    CStone { st_name: "zircon", st_value: 80 },
];

/// Count of entries in `stones`.  Exported as `int cNSTONES` for C.
#[no_mangle]
pub static mut cNSTONES: c_int = NSTONES as c_int;

/// Wand / staff wood materials.  Exported as `char *wood[]` for C.
pub static wood: [&'static str; NWOOD] = [
    "avocado wood",
    "balsa",
    "bamboo",
    "banyan",
    "birch",
    "cedar",
    "cherry",
    "cinnibar",
    "cypress",
    "dogwood",
    "driftwood",
    "ebony",
    "elm",
    "eucalyptus",
    "fall",
    "hemlock",
    "holly",
    "ironwood",
    "kukui wood",
    "mahogany",
    "manzanita",
    "maple",
    "oaken",
    "persimmon wood",
    "pecan",
    "pine",
    "poplar",
    "redwood",
    "rosewood",
    "spruce",
    "teak",
    "walnut",
    "zebrawood",
];

/// Count of entries in `wood`.  Exported as `int cNWOOD` for C.
pub static mut cNWOOD: c_int = NWOOD as c_int;

/// Wand metal materials.  Exported as `char *metal[]` for C.
pub static metal: [&'static str; NMETAL] = [
    "aluminum",
    "beryllium",
    "bone",
    "brass",
    "bronze",
    "copper",
    "electrum",
    "gold",
    "iron",
    "lead",
    "magnesium",
    "mercury",
    "nickel",
    "pewter",
    "platinum",
    "steel",
    "silver",
    "silicon",
    "tin",
    "titanium",
    "tungsten",
    "zinc",
];

/// Count of entries in `metal`.  Exported as `int cNMETAL` for C.
pub static mut cNMETAL: c_int = NMETAL as c_int;

// ─── Private static data ─────────────────────────────────────────────────────

/// Syllables used to generate scroll names.
const SYLLS: &[&str] = &[
    "a", "ab", "ag", "aks", "ala", "an", "app", "arg", "arze", "ash", "bek", "bie", "bit", "bjor",
    "blu", "bot", "bu", "byt", "comp", "con", "cos", "cre", "dalf", "dan", "den", "do", "e", "eep",
    "el", "eng", "er", "ere", "erk", "esh", "evs", "fa", "fid", "fri", "fu", "gan", "gar", "glen",
    "gop", "gre", "ha", "hyd", "i", "ing", "ip", "ish", "it", "ite", "iv", "jo", "kho", "kli",
    "klis", "la", "lech", "mar", "me", "mi", "mic", "mik", "mon", "mung", "mur", "nej", "nelg",
    "nep", "ner", "nes", "nes", "nih", "nin", "o", "od", "ood", "org", "orn", "ox", "oxy", "pay",
    "ple", "plu", "po", "pot", "prok", "re", "rea", "rhov", "ri", "ro", "rog", "rok", "rol", "sa",
    "san", "sat", "sef", "seh", "shu", "ski", "sna", "sne", "snik", "sno", "so", "sol", "sri",
    "sta", "sun", "ta", "tab", "tem", "ther", "ti", "tox", "trol", "tue", "turs", "u", "ulk", "um",
    "un", "uni", "ur", "val", "viv", "vly", "vom", "wah", "wed", "werg", "wex", "whon", "wun",
    "xo", "y", "yot", "yu", "zant", "zeb", "zim", "zok", "zon", "zum",
];

// Size = max(potion colours 27, stones 26, wood 33) = 33.
/// Shared boolean scratch array used by init_colors, init_stones,
/// and init_materials (mirrors the C-side `static bool used[]`).
static mut USED: [c_uchar; 33] = [0; 33];

// ─── Extern C globals ────────────────────────────────────────────────────────

use crate::globals::{a_class, food_left, max_stats};


// ─── Private helpers ─────────────────────────────────────────────────────────

#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

// ─── Exported functions ───────────────────────────────────────────────────────

/// Roll up the starting player: give food, armor, weapons, and arrows.
pub unsafe fn init_player() {
    crate::game::PLAYER.set_stats(max_stats);
    food_left = HUNGERTIME;

    // Give her some food
    let obj = new_item();
    (*thing_o(obj)).o_type = FOOD;
    (*thing_o(obj)).o_count = 1;
    add_pack(obj, true as c_uchar);

    // A suit of ring-mail armor
    let obj = new_item();
    (*thing_o(obj)).o_type = ARMOR;
    (*thing_o(obj)).o_which = RING_MAIL;
    (*thing_o(obj)).o_arm = a_class[RING_MAIL as usize] - 1;
    (*thing_o(obj)).o_flags.insert(ObjectFlags::KNOW);
    (*thing_o(obj)).o_count = 1;
    PLAYER.set_armor(obj);
    add_pack(obj, true as c_uchar);

    // A +1 mace
    let obj = new_item();
    init_weapon(obj, MACE);
    (*thing_o(obj)).o_hplus = 1;
    (*thing_o(obj)).o_dplus = 1;
    (*thing_o(obj)).o_flags.insert(ObjectFlags::KNOW);
    add_pack(obj, true as c_uchar);
    PLAYER.set_weapon(obj);

    // A +1 bow
    let obj = new_item();
    init_weapon(obj, BOW);
    (*thing_o(obj)).o_hplus = 1;
    (*thing_o(obj)).o_flags.insert(ObjectFlags::KNOW);
    add_pack(obj, true as c_uchar);

    // Arrows
    let obj = new_item();
    init_weapon(obj, ARROW);
    (*thing_o(obj)).o_count = rnd(15) + 25;
    (*thing_o(obj)).o_flags.insert(ObjectFlags::KNOW);
    add_pack(obj, true as c_uchar);
}

/// Assign a random colour from [`crate::colors::POTION_COLORS`] to each potion.
pub unsafe fn init_colors() {
    for i in 0..crate::colors::POTION_COLOR_COUNT {
        USED[i] = 0;
    }
    for i in 0..MAXPOTIONS {
        let j = loop {
            let j = rnd(crate::colors::POTION_COLOR_COUNT as c_int) as usize;
            if USED[j] == 0 {
                break j;
            }
        };
        USED[j] = 1;
        crate::globals::p_colors[i] = crate::colors::POTION_COLORS[j];
    }
}

/// Generate random pronounceable names for each scroll.
///
/// Builds each name as an owned Rust [`String`] and stores it in
/// [`crate::globals::SCROLL_NAMES`], preserving the syllable, word-count, and
/// `MAXNAME` length limits of the original C routine.
pub unsafe fn init_names() {
    crate::globals::set_scroll_names(MAXSCROLLS, |_| {
        let mut name = String::new();
        let mut nwords = rnd(3) + 2;
        while nwords > 0 {
            nwords -= 1;
            let mut nsyl = rnd(3) + 1;
            while nsyl > 0 {
                nsyl -= 1;
                let syllable = SYLLS[rnd(SYLLS.len() as c_int) as usize];
                if name.len() + syllable.len() > MAXNAME {
                    break;
                }
                name.push_str(syllable);
            }
            name.push(' ');
        }
        // Back up over the trailing space.
        name.pop();
        name
    });
}

/// Assign a random stone setting to each ring type.
pub unsafe fn init_stones() {
    for i in 0..NSTONES {
        USED[i] = 0;
    }
    for i in 0..MAXRINGS {
        let j = loop {
            let j = rnd(NSTONES as c_int) as usize;
            if USED[j] == 0 {
                break j;
            }
        };
        USED[j] = 1;
        crate::globals::r_stones[i] = stones[j].st_name;
        ring_info[i].oi_worth += stones[j].st_value;
    }
}

/// Assign random wood / metal materials to wands and staves.
pub unsafe fn init_materials() {
    for i in 0..NWOOD {
        USED[i] = 0;
    }
    let mut metused: [c_uchar; NMETAL] = [0; NMETAL];
    for i in 0..MAXSTICKS {
        loop {
            if rnd(2) == 0 {
                let j = rnd(NMETAL as c_int) as usize;
                if metused[j] == 0 {
                    crate::globals::ws_type[i] = "wand";
                    crate::globals::ws_made[i] = metal[j];
                    metused[j] = 1;
                    break;
                }
            } else {
                let j = rnd(NWOOD as c_int) as usize;
                if USED[j] == 0 {
                    crate::globals::ws_type[i] = "staff";
                    crate::globals::ws_made[i] = wood[j];
                    USED[j] = 1;
                    break;
                }
            }
        }
    }
}

/// Accumulate cumulative probabilities for one item-info table.
///
/// Mirrors the C `sumprobs(struct obj_info *info, int bound)`.
pub unsafe fn sumprobs(info: *mut CObjInfo, bound: c_int) {
    let endp = info.add(bound as usize);
    let mut p = info.add(1);
    while p < endp {
        (*p).oi_prob += (*p.sub(1)).oi_prob;
        p = p.add(1);
    }
}

/// Initialize cumulative probabilities for all item types.
pub unsafe fn init_probs() {
    sumprobs(std::ptr::addr_of_mut!(things).cast(), NUMTHINGS as c_int);
    sumprobs(std::ptr::addr_of_mut!(pot_info).cast(), MAXPOTIONS as c_int);
    sumprobs(std::ptr::addr_of_mut!(scr_info).cast(), MAXSCROLLS as c_int);
    sumprobs(std::ptr::addr_of_mut!(ring_info).cast(), MAXRINGS as c_int);
    sumprobs(std::ptr::addr_of_mut!(ws_info).cast(), MAXSTICKS as c_int);
    sumprobs(
        std::ptr::addr_of_mut!(weap_info).cast(),
        MAXWEAPONS as c_int,
    );
    sumprobs(std::ptr::addr_of_mut!(arm_info).cast(), MAXARMORS as c_int);
}

/// Return a random colour if the player is hallucinating, otherwise
/// return the supplied colour unchanged.
pub unsafe fn pick_color(col: &'static str) -> &'static str {
    if crate::game::PLAYER.has_flag(MonsterFlags::HALU) {
        crate::colors::random_color()
    } else {
        col
    }
}