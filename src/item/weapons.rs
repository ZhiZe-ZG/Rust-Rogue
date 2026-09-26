//! Weapons: wielding, throwing, and weapon initialization.
//!
//! Ported from `src/c/weapons.c` to Rust.
use crate::entity::chase::cansee;
use crate::entity::fight::fight;
use crate::game::PLAYER;
use crate::item::pack::{get_item, leave_pack};
use crate::misc::{is_current, show_floor};
use crate::rnd::rnd;
use crate::ui::output;
use crate::ui::output::{addmsg_str, endmsg, msg_str};
use glam::IVec2;

use crate::entity::player::{ObjectFlags, Thing, ThingMonster, ThingObject};
use crate::globals::weap_info;
use crate::entity::player::discard;
use crate::item::things::{dropcheck, inv_name};

const NO_WEAPON: i32 = -1;

const FLOOR: i32 = '.' as i32;
const PASSAGE: i32 = '#' as i32;
const DOOR: i32 = '+' as i32;
const WEAPON: u8 = ')' as u8;
const ARMOR: u8 = ']' as u8;

const BOW: i32 = 2;
const DAGGER: i32 = 4;
const MAXWEAPONS: usize = 9;

const ISMISL: i32 = 0o000004;
const ISMANY: i32 = 0o000010;

#[derive(Copy, Clone)]
struct InitWeap {
    iw_dam: &'static [u8],
    iw_hrl: &'static [u8],
    iw_launch: i32,
    iw_flags: i32,
}

static INIT_DAM: [InitWeap; MAXWEAPONS] = [
    InitWeap {
        iw_dam: b"2x4\0",
        iw_hrl: b"1x3\0",
        iw_launch: NO_WEAPON,
        iw_flags: 0,
    },
    InitWeap {
        iw_dam: b"3x4\0",
        iw_hrl: b"1x2\0",
        iw_launch: NO_WEAPON,
        iw_flags: 0,
    },
    InitWeap {
        iw_dam: b"1x1\0",
        iw_hrl: b"1x1\0",
        iw_launch: NO_WEAPON,
        iw_flags: 0,
    },
    InitWeap {
        iw_dam: b"1x1\0",
        iw_hrl: b"2x3\0",
        iw_launch: BOW,
        iw_flags: ISMANY | ISMISL,
    },
    InitWeap {
        iw_dam: b"1x6\0",
        iw_hrl: b"1x4\0",
        iw_launch: NO_WEAPON,
        iw_flags: ISMISL,
    },
    InitWeap {
        iw_dam: b"4x4\0",
        iw_hrl: b"1x2\0",
        iw_launch: NO_WEAPON,
        iw_flags: 0,
    },
    InitWeap {
        iw_dam: b"1x1\0",
        iw_hrl: b"1x3\0",
        iw_launch: NO_WEAPON,
        iw_flags: ISMANY | ISMISL,
    },
    InitWeap {
        iw_dam: b"1x2\0",
        iw_hrl: b"2x4\0",
        iw_launch: NO_WEAPON,
        iw_flags: ISMANY | ISMISL,
    },
    InitWeap {
        iw_dam: b"2x3\0",
        iw_hrl: b"1x6\0",
        iw_launch: NO_WEAPON,
        iw_flags: ISMISL,
    },
];

pub static mut group: i32 = 2;

static mut FALL_POS: IVec2 = IVec2 { x: 0, y: 0 };

use crate::globals::{after, has_hit, terse};


#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

#[inline]
fn hero() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
unsafe fn chat(y: i32, x: i32) -> i32 {
    crate::draw::cell_glyph(y, x) as u8 as i32
}

#[inline]
unsafe fn moat(y: i32, x: i32) -> *mut Thing {
    crate::game::monster_at(y, x)
}

#[inline]
unsafe fn copy_c_bytes(dst: &mut [u8], src: &[u8]) {
    let mut i = 0usize;
    while i + 1 < dst.len() && i < src.len() {
        dst[i] = src[i];
        if src[i] == 0 {
            return;
        }
        i += 1;
    }
    dst[dst.len() - 1] = 0;
}

/// Throws a selected weapon in the provided direction and resolves impact/fall behavior.
pub unsafe fn missile(ydelta: i32, xdelta: i32) {
    let mut obj = get_item("throw", WEAPON as i32);
    if obj.is_null() {
        return;
    }
    if dropcheck(obj) == 0 || is_current(obj) {
        return;
    }

    obj = leave_pack(obj, true as u8, false as u8);
    do_motion(obj, ydelta, xdelta);

    let o = thing_o(obj);
    if moat((*o).o_pos.y, (*o).o_pos.x).is_null()
        || hit_monster((*o).o_pos.y, (*o).o_pos.x, obj) == 0
    {
        fall(obj, true as u8);
    }
}

/// Animates projectile movement until it hits blocking terrain or a door.
pub unsafe fn do_motion(obj: *mut Thing, ydelta: i32, xdelta: i32) {
    let o = thing_o(obj);
    (*o).o_pos = hero();

    loop {
        let h = hero();
        if ((*o).o_pos.x != h.x || (*o).o_pos.y != h.y)
            && cansee((*o).o_pos.y, (*o).o_pos.x) != 0
            && terse == 0
        {
            let mut ch = chat((*o).o_pos.y, (*o).o_pos.x);
            if ch == FLOOR && !show_floor() {
                ch = ' ' as i32;
            }
            output::write_glyph_at(IVec2::new((*o).o_pos.x, (*o).o_pos.y), (ch as u8) as char);
        }

        (*o).o_pos.y += ydelta;
        (*o).o_pos.x += xdelta;

        if crate::game::cell_is_walkable((*o).o_pos.y, (*o).o_pos.x)
            && !crate::game::is_door_at((*o).o_pos.y, (*o).o_pos.x)
        {
            if cansee((*o).o_pos.y, (*o).o_pos.x) != 0 && terse == 0 {
                output::write_glyph_at(
                    IVec2::new((*o).o_pos.x, (*o).o_pos.y),
                    ((*o).o_type as u8) as char,
                );
                output::refresh();
            }
            continue;
        }
        break;
    }
}

/// Drops an item near its current position or discards it if no floor slot is available.
pub unsafe fn fall(obj: *mut Thing, pr: u8) {
    if fallpos(&mut (*thing_o(obj)).o_pos, &raw mut FALL_POS) != 0 {
        // Objects render from the `lvl_obj` list; no glyph write needed.
        (*thing_o(obj)).o_pos = FALL_POS;

        if cansee(FALL_POS.y, FALL_POS.x) != 0 {
            let m = moat(FALL_POS.y, FALL_POS.x);
            if !m.is_null() {
                (*thing_t(m)).t_oldch = (*thing_o(obj)).o_type as u8;
            } else {
                output::write_glyph_at(
                    IVec2::new(FALL_POS.x, FALL_POS.y),
                    ((*thing_o(obj)).o_type as u8) as char,
                );
            }
        }

        crate::game::with_current_level_mut(|level| level.items.attach(obj));
        return;
    }

    if pr != 0 {
        if has_hit != 0 {
            endmsg();
            has_hit = 0;
        }
        msg_str(&format!(
            "the {} vanishes as it hits the ground",
            weap_info[(*thing_o(obj)).o_which as usize].oi_name
        ));
    }

    discard(obj);
}

/// Initializes a weapon object with baseline damage, flags, and stack counts.
pub unsafe fn init_weapon(weap: *mut Thing, which: i32) {
    let o = thing_o(weap);
    (*o).o_type = WEAPON as i32;
    (*o).o_which = which;

    let iwp = INIT_DAM[which as usize];
    copy_c_bytes(&mut (*o).o_damage, iwp.iw_dam);
    copy_c_bytes(&mut (*o).o_hurldmg, iwp.iw_hrl);
    (*o).o_launch = iwp.iw_launch;
    (*o).o_flags = ObjectFlags::from_bits(iwp.iw_flags);
    (*o).o_hplus = 0;
    (*o).o_dplus = 0;

    if which == DAGGER {
        (*o).o_count = rnd(4) + 2;
        (*o).o_group = group;
        group += 1;
    } else if (*o).o_flags.contains(ObjectFlags::MANY) {
        (*o).o_count = rnd(8) + 8;
        (*o).o_group = group;
        group += 1;
    } else {
        (*o).o_count = 1;
        (*o).o_group = 0;
    }
}

/// Resolves thrown-weapon combat against the target tile.
pub unsafe fn hit_monster(y: i32, x: i32, obj: *mut Thing) -> i32 {
    let mut mp = IVec2 { x, y };
    fight(&mut mp, obj, true as u8)
}

/// Formats signed enchantment numbers for armor and weapons.
pub fn num(n1: i32, n2: i32, obj_type: u8) -> String {
    if obj_type == WEAPON {
        format!("{:+},{:+}", n1, n2)
    } else {
        format!("{:+}", n1)
    }
}

/// Equips a selected weapon after validating curses and item type constraints.
pub unsafe fn wield() {
    let oweapon = PLAYER.weapon();
    if dropcheck(PLAYER.weapon()) == 0 {
        PLAYER.set_weapon(oweapon);
        return;
    }
    PLAYER.set_weapon(oweapon);

    let obj = get_item("wield", WEAPON as i32);
    if obj.is_null() {
        after = 0;
        return;
    }

    if (*thing_o(obj)).o_type == ARMOR as i32 {
        msg_str("you can't wield armor");
        after = 0;
        return;
    }
    if is_current(obj) {
        after = 0;
        return;
    }

    let sp = inv_name(obj, true as u8);
    PLAYER.set_weapon(obj);
    if terse == 0 {
        addmsg_str("you are now ");
    }
    msg_str(&format!(
        "wielding {} ({})",
        sp,
        (*thing_o(obj)).o_packch as char,
    ));
}

/// Chooses a nearby floor/passage location to drop an item and returns whether one was found.
pub unsafe fn fallpos(pos: *mut IVec2, newpos: *mut IVec2) -> u8 {
    let mut cnt = 0;
    for y in ((*pos).y - 1)..=((*pos).y + 1) {
        for x in ((*pos).x - 1)..=((*pos).x + 1) {
            let h = hero();
            if y == h.y && x == h.x {
                continue;
            }
            let ch = chat(y, x);
            if ch == FLOOR || ch == PASSAGE {
                cnt += 1;
                if rnd(cnt) == 0 {
                    (*newpos).y = y;
                    (*newpos).x = x;
                }
            }
        }
    }
    if cnt != 0 {
        true as u8
    } else {
        false as u8
    }
}
