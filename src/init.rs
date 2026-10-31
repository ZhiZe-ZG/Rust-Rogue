//! Global variable initialization.
//!
//! Ported from `src/c/init.c` to Rust.
//!
//! Rogue: Exploring the Dungeons of Doom
//! Copyright (C) 1980-1983, 1985, 1999 Michael Toy, Ken Arnold and Glenn Wichman
//! All rights reserved.
//!
//! See the file LICENSE.TXT for full copyright and licensing information.

use crate::game::globals::{
    arm_info, pot_info, ring_info, scr_info, things, weap_info, ws_info, ObjInfo,
};
use crate::dungeon::DUNGEON;
use crate::rnd::{Random, RNG};
use std::sync::atomic::Ordering;

use crate::entity::player::{MonsterFlags, ObjectFlags, Thing, ThingMonster, ThingObject};
use crate::item::arena::new_item_id;
use crate::item::item_type::ItemType;
use crate::item::pack::add_pack_id;
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

const HUNGERTIME: i32 = 1300;

// Armor / weapon indices
const RING_MAIL: i32 = 1;
const MACE: i32 = 0;
const BOW: i32 = 2;
const ARROW: i32 = 3;

/// Ring stone names and values (legacy `STONE` table).
pub struct Stone {
    pub st_name: &'static str,
    pub st_value: i32,
}

// ─── Exported global data arrays ─────────────────────────────────────────────

const NSTONES: usize = 26;
const NWOOD: usize = 33;
const NMETAL: usize = 22;

/// Ring-stone table.  Exported as `STONE stones[]` for legacy consumers.
pub static stones: [Stone; NSTONES] = [
    Stone {
        st_name: "agate",
        st_value: 25,
    },
    Stone {
        st_name: "alexandrite",
        st_value: 40,
    },
    Stone {
        st_name: "amethyst",
        st_value: 50,
    },
    Stone {
        st_name: "carnelian",
        st_value: 40,
    },
    Stone {
        st_name: "diamond",
        st_value: 300,
    },
    Stone {
        st_name: "emerald",
        st_value: 300,
    },
    Stone {
        st_name: "germanium",
        st_value: 225,
    },
    Stone {
        st_name: "granite",
        st_value: 5,
    },
    Stone {
        st_name: "garnet",
        st_value: 50,
    },
    Stone {
        st_name: "jade",
        st_value: 150,
    },
    Stone {
        st_name: "kryptonite",
        st_value: 300,
    },
    Stone {
        st_name: "lapis lazuli",
        st_value: 50,
    },
    Stone {
        st_name: "moonstone",
        st_value: 50,
    },
    Stone {
        st_name: "obsidian",
        st_value: 15,
    },
    Stone {
        st_name: "onyx",
        st_value: 60,
    },
    Stone {
        st_name: "opal",
        st_value: 200,
    },
    Stone {
        st_name: "pearl",
        st_value: 220,
    },
    Stone {
        st_name: "peridot",
        st_value: 63,
    },
    Stone {
        st_name: "ruby",
        st_value: 350,
    },
    Stone {
        st_name: "sapphire",
        st_value: 285,
    },
    Stone {
        st_name: "stibotantalite",
        st_value: 200,
    },
    Stone {
        st_name: "tiger eye",
        st_value: 50,
    },
    Stone {
        st_name: "topaz",
        st_value: 60,
    },
    Stone {
        st_name: "turquoise",
        st_value: 70,
    },
    Stone {
        st_name: "taaffeite",
        st_value: 300,
    },
    Stone {
        st_name: "zircon",
        st_value: 80,
    },
];

/// Count of entries in `stones`.  Exported as `int cNSTONES` for C.
pub const C_NSTONES: i32 = NSTONES as i32;

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
pub const C_NWOOD: i32 = NWOOD as i32;

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
pub const C_NMETAL: i32 = NMETAL as i32;

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

// ─── Extern C globals ────────────────────────────────────────────────────────

use crate::game::globals::{a_class, inv_describe, terse};

// ─── Exported functions ───────────────────────────────────────────────────────

/// Roll up the starting player: give food, armor, weapons, and arrows.
pub unsafe fn init_player() {
    DUNGEON.player.set_stats(DUNGEON.player.max_stats());
    DUNGEON.player.food_left.store(HUNGERTIME, Ordering::Relaxed);

    // Item callbacks must not retain borrows of mutable global metadata.
    let arm_names = (&*(&raw const arm_info)).clone();
    let pot_names = (&*(&raw const pot_info)).clone();
    let ring_names = (&*(&raw const ring_info)).clone();
    let scr_names = (&*(&raw const scr_info)).clone();
    let weap_names = (&*(&raw const weap_info)).clone();
    let ws_names = (&*(&raw const ws_info)).clone();

    // Give her some food
    let food = new_item_id(&DUNGEON.objects);
    crate::dungeon::DUNGEON.objects.with_object_mut(food, |o| {
        o.o_type = ItemType::Food;
        o.o_count = 1;
    });
    add_pack_id(Some(food), true, &DUNGEON, &DUNGEON.player.inpack, &DUNGEON.player.pack_used,
        terse, inv_describe, &arm_names, &pot_names, &ring_names, &scr_names,
        &weap_names, &ws_names);

    // A suit of ring-mail armor
    let armor = new_item_id(&DUNGEON.objects);
    crate::dungeon::DUNGEON.objects.with_object_mut(armor, |o| {
        o.o_type = ItemType::Armor(RING_MAIL);
        o.o_which = RING_MAIL;
        o.o_arm = a_class[RING_MAIL as usize] - 1;
        o.o_flags.insert(ObjectFlags::KNOW);
        o.o_count = 1;
    });
    DUNGEON.player.set_armor_id(Some(armor));
    add_pack_id(Some(armor), true, &DUNGEON, &DUNGEON.player.inpack, &DUNGEON.player.pack_used,
        terse, inv_describe, &arm_names, &pot_names, &ring_names, &scr_names,
        &weap_names, &ws_names);

    // A +1 mace
    let mace_id = new_item_id(&DUNGEON.objects);
    init_weapon(mace_id, MACE, &mut RNG.lock().unwrap(), &DUNGEON, &DUNGEON.group);
    crate::dungeon::DUNGEON.objects.with_object_mut(mace_id, |o| {
        o.o_hplus = 1;
        o.o_dplus = 1;
        o.o_flags.insert(ObjectFlags::KNOW);
    });
    add_pack_id(Some(mace_id), true, &DUNGEON, &DUNGEON.player.inpack, &DUNGEON.player.pack_used,
        terse, inv_describe, &arm_names, &pot_names, &ring_names, &scr_names,
        &weap_names, &ws_names);
    DUNGEON.player.set_weapon_id(Some(mace_id));

    // A +1 bow
    let bow_id = new_item_id(&DUNGEON.objects);
    init_weapon(bow_id, BOW, &mut RNG.lock().unwrap(), &DUNGEON, &DUNGEON.group);
    crate::dungeon::DUNGEON.objects.with_object_mut(bow_id, |o| {
        o.o_hplus = 1;
        o.o_flags.insert(ObjectFlags::KNOW);
    });
    add_pack_id(Some(bow_id), true, &DUNGEON, &DUNGEON.player.inpack, &DUNGEON.player.pack_used,
        terse, inv_describe, &arm_names, &pot_names, &ring_names, &scr_names,
        &weap_names, &ws_names);

    // Arrows
    let arrow_id = new_item_id(&DUNGEON.objects);
    init_weapon(arrow_id, ARROW, &mut RNG.lock().unwrap(), &DUNGEON, &DUNGEON.group);
    crate::dungeon::DUNGEON.objects.with_object_mut(arrow_id, |o| {
        o.o_count = {
            let mut random = RNG.lock().unwrap();
            random.rnd(15)
        } + 25;
        o.o_flags.insert(ObjectFlags::KNOW);
    });
    add_pack_id(Some(arrow_id), true, &DUNGEON, &DUNGEON.player.inpack, &DUNGEON.player.pack_used,
        terse, inv_describe, &arm_names, &pot_names, &ring_names, &scr_names,
        &weap_names, &ws_names);
}

/// Assign a random colour from [`crate::colors::POTION_COLORS`] to each potion.
pub unsafe fn init_colors() {
    // Local `used[]` scratch array (mirrors the C-side `static bool used[]`,
    // which was reset at the start of every init routine anyway; the Rust local
    // is zero-initialised for free).
    let mut used = [0u8; crate::colors::POTION_COLOR_COUNT];
    let mut colors = [""; MAXPOTIONS];
    for i in 0..MAXPOTIONS {
        let j = loop {
            let j = {
                let mut random = RNG.lock().unwrap();
                random.rnd(crate::colors::POTION_COLOR_COUNT as i32)
            } as usize;
            if used[j] == 0 {
                break j;
            }
        };
        used[j] = 1;
        colors[i] = crate::colors::POTION_COLORS[j];
    }
    DUNGEON.set_p_colors(colors);
}

/// Generate random pronounceable names for each scroll.
///
/// Builds each name as an owned Rust [`String`] and stores it in
/// [`crate::dungeon::Dungeon`], preserving the syllable, word-count, and
/// `MAXNAME` length limits of the original C routine.
pub unsafe fn init_names() {
    let names = (0..MAXSCROLLS).map(|_| {
        let mut name = String::new();
        let mut nwords = {
            let mut random = RNG.lock().unwrap();
            random.rnd(3)
        } + 2;
        while nwords > 0 {
            nwords -= 1;
            let mut nsyl = {
                let mut random = RNG.lock().unwrap();
                random.rnd(3)
            } + 1;
            while nsyl > 0 {
                nsyl -= 1;
                let syllable = SYLLS[{
                    let mut random = RNG.lock().unwrap();
                    random.rnd(SYLLS.len() as i32)
                } as usize];
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
    }).collect();
    crate::dungeon::DUNGEON.set_scroll_names(names);
}

/// Assign a random stone setting to each ring type.
pub unsafe fn init_stones() {
    let mut used = [0u8; NSTONES];
    let mut names = [""; MAXRINGS];
    for i in 0..MAXRINGS {
        let j = loop {
            let j = {
                let mut random = RNG.lock().unwrap();
                random.rnd(NSTONES as i32)
            } as usize;
            if used[j] == 0 {
                break j;
            }
        };
        used[j] = 1;
        names[i] = stones[j].st_name;
        ring_info[i].oi_worth += stones[j].st_value;
    }
    DUNGEON.set_r_stones(names);
}

/// Assign random wood / metal materials to wands and staves.
pub unsafe fn init_materials() {
    let mut used = [0u8; NWOOD];
    let mut metused: [u8; NMETAL] = [0; NMETAL];
    let mut kinds = [""; MAXSTICKS];
    let mut materials = [""; MAXSTICKS];
    for i in 0..MAXSTICKS {
        loop {
            if {
                let mut random = RNG.lock().unwrap();
                random.rnd(2)
            } == 0
            {
                let j = {
                    let mut random = RNG.lock().unwrap();
                    random.rnd(NMETAL as i32)
                } as usize;
                if metused[j] == 0 {
                    kinds[i] = "wand";
                    materials[i] = metal[j];
                    metused[j] = 1;
                    break;
                }
            } else {
                let j = {
                    let mut random = RNG.lock().unwrap();
                    random.rnd(NWOOD as i32)
                } as usize;
                if used[j] == 0 {
                    kinds[i] = "staff";
                    materials[i] = wood[j];
                    used[j] = 1;
                    break;
                }
            }
        }
    }
    DUNGEON.set_ws_type(kinds);
    DUNGEON.set_ws_made(materials);
}

/// Accumulate cumulative probabilities for one item-info table.
///
/// Mirrors the C `sumprobs(struct obj_info *info, int bound)`.
pub unsafe fn sumprobs(info: &mut [ObjInfo], bound: usize) {
    let mut i = 1usize;
    while i < bound {
        info[i].oi_prob += info[i - 1].oi_prob;
        i += 1;
    }
}

/// Initialize cumulative probabilities for all item types.
pub unsafe fn init_probs() {
    sumprobs(&mut things[..], NUMTHINGS);
    sumprobs(&mut pot_info[..], MAXPOTIONS);
    sumprobs(&mut scr_info[..], MAXSCROLLS);
    sumprobs(&mut ring_info[..], MAXRINGS);
    sumprobs(&mut ws_info[..], MAXSTICKS);
    sumprobs(&mut weap_info[..], MAXWEAPONS);
    sumprobs(&mut arm_info[..], MAXARMORS);
}

/// Return a random colour if the player is hallucinating, otherwise
/// return the supplied colour unchanged.
pub unsafe fn pick_color(col: &'static str, random: &mut Random) -> &'static str {
    if DUNGEON.player.has_flag(MonsterFlags::HALU) {
        crate::colors::random_color(random)
    } else {
        col
    }
}
