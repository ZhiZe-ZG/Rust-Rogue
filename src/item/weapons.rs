//! Weapons: wielding, throwing, and weapon initialization.
//!
//! Ported from `src/c/weapons.c` to Rust.
//!
//! This module is pointer-free: weapons (and the transient bolts/flames fired
//! by wands) are addressed through the item arena's [`ThingId`] handles and
//! mutated through scoped `with_object`/`with_object_mut` access rather than
//! raw `*mut Thing` dereferences.
use crate::entity::chase::cansee;
use crate::entity::fight::fight;
use crate::game::PLAYER;
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::{get_item_id, leave_pack_id};
use crate::misc::{is_current_id, show_floor};
use crate::rnd::rnd;
use crate::ui::output;
use crate::ui::output::{addmsg_str, endmsg, msg_str};
use glam::IVec2;

use crate::entity::player::ObjectFlags;
use crate::game::globals::weap_info;
use crate::item::arena::{ThingId, OBJECTS};
use crate::item::things::{dropcheck_id, inv_name_id};

const NO_WEAPON: i32 = -1;

const FLOOR: i32 = '.' as i32;
const PASSAGE: i32 = '#' as i32;
const DOOR: i32 = '+' as i32;
const WEAPON: u8 = ')' as u8;

const BOW: i32 = 2;
const DAGGER: i32 = 4;
const MAXWEAPONS: usize = 9;

/// A weapon's initialisation template entry.
#[derive(Copy, Clone)]
struct InitWeap {
    iw_dam: &'static [u8],
    iw_hrl: &'static [u8],
    iw_launch: i32,
    /// The object flags applied to a freshly initialised weapon of this kind.
    iw_flags: ObjectFlags,
}

static INIT_DAM: [InitWeap; MAXWEAPONS] = [
    InitWeap {
        iw_dam: b"2x4\0",
        iw_hrl: b"1x3\0",
        iw_launch: NO_WEAPON,
        iw_flags: ObjectFlags::NONE,
    },
    InitWeap {
        iw_dam: b"3x4\0",
        iw_hrl: b"1x2\0",
        iw_launch: NO_WEAPON,
        iw_flags: ObjectFlags::NONE,
    },
    InitWeap {
        iw_dam: b"1x1\0",
        iw_hrl: b"1x1\0",
        iw_launch: NO_WEAPON,
        iw_flags: ObjectFlags::NONE,
    },
    InitWeap {
        iw_dam: b"1x1\0",
        iw_hrl: b"2x3\0",
        iw_launch: BOW,
        iw_flags: ObjectFlags::MANY.union(ObjectFlags::MISL),
    },
    InitWeap {
        iw_dam: b"1x6\0",
        iw_hrl: b"1x4\0",
        iw_launch: NO_WEAPON,
        iw_flags: ObjectFlags::MISL,
    },
    InitWeap {
        iw_dam: b"4x4\0",
        iw_hrl: b"1x2\0",
        iw_launch: NO_WEAPON,
        iw_flags: ObjectFlags::NONE,
    },
    InitWeap {
        iw_dam: b"1x1\0",
        iw_hrl: b"1x3\0",
        iw_launch: NO_WEAPON,
        iw_flags: ObjectFlags::MANY.union(ObjectFlags::MISL),
    },
    InitWeap {
        iw_dam: b"1x2\0",
        iw_hrl: b"2x4\0",
        iw_launch: NO_WEAPON,
        iw_flags: ObjectFlags::MANY.union(ObjectFlags::MISL),
    },
    InitWeap {
        iw_dam: b"2x3\0",
        iw_hrl: b"1x6\0",
        iw_launch: NO_WEAPON,
        iw_flags: ObjectFlags::MISL,
    },
];

use crate::game::globals::{after, group, has_hit, terse};

#[inline]
fn hero() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
unsafe fn chat(y: i32, x: i32) -> i32 {
    crate::draw::cell_glyph(y, x) as u8 as i32
}

/// Copy a legacy NUL-terminated damage spec into a fixed buffer, zero-filling.
#[inline]
fn set_damage(dst: &mut [u8; 8], src: &[u8]) {
    dst.fill(0);
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
}

/// Throws a selected weapon in the provided direction and resolves impact/fall behavior.
pub unsafe fn missile(ydelta: i32, xdelta: i32) {
    let Some(id) = get_item_id("throw", ItemFilter::Category(ItemType::WEAPON)) else {
        return;
    };
    if !dropcheck_id(id) || is_current_id(id) {
        return;
    }

    let Some(obj) = leave_pack_id(id, true, false) else {
        return;
    };
    do_motion(obj, ydelta, xdelta);

    let pos = OBJECTS.with_object(obj, |o| o.o_pos).unwrap_or(IVec2::ZERO);
    if !crate::game::monster_here(pos.y, pos.x) || hit_monster(pos.y, pos.x, obj) == 0 {
        fall(obj, true);
    }
}

/// Animates projectile movement until it hits blocking terrain or a door.
pub unsafe fn do_motion(id: ThingId, ydelta: i32, xdelta: i32) -> IVec2 {
    let mut pos = hero();
    let o_type = OBJECTS
        .with_object(id, |o| o.o_type)
        .unwrap_or(ItemType::None);

    loop {
        let h = hero();
        if (pos.x != h.x || pos.y != h.y) && cansee(pos.y, pos.x) != 0 && terse == 0 {
            let mut ch = chat(pos.y, pos.x);
            if ch == FLOOR && !show_floor() {
                ch = ' ' as i32;
            }
            crate::draw::write_cell_glyph(pos, ch as u8 as char);
        }

        pos.y += ydelta;
        pos.x += xdelta;

        if crate::game::cell_is_walkable(pos.y, pos.x) && !crate::game::is_door_at(pos.y, pos.x) {
            if cansee(pos.y, pos.x) != 0 && terse == 0 {
                crate::draw::write_cell_glyph(pos, crate::draw::item_glyph(o_type));
                output::refresh();
            }
            continue;
        }
        break;
    }

    OBJECTS.with_object_mut(id, |o| o.o_pos = pos);
    pos
}

/// Drops an item near its current position or discards it if no floor slot is available.
pub unsafe fn fall(id: ThingId, pr: bool) {
    let Some(pos) = OBJECTS.with_object(id, |o| o.o_pos) else {
        return;
    };

    if let Some(newpos) = fallpos(pos) {
        // Objects render from the `lvl_obj` list; no glyph write needed.
        OBJECTS.with_object_mut(id, |o| o.o_pos = newpos);

        if cansee(newpos.y, newpos.x) != 0 {
            let glyph = OBJECTS
                .with_object(id, |o| crate::draw::item_glyph(o.o_type))
                .unwrap_or(')');
            if let Some(mid) = crate::game::monster_id_at(newpos.y, newpos.x) {
                crate::game::DUNGEON.monster_list.with_mut(mid, |t| {
                    if let crate::entity::player::Thing::Monster { data } = t {
                        data.t_oldch = glyph as u8;
                    }
                });
            } else {
                crate::draw::write_cell_glyph(newpos, glyph);
            }
        }

        crate::game::with_current_level_mut(|level| level.add_item(id));
        return;
    }

    if pr {
        if has_hit != 0 {
            endmsg();
            has_hit = 0;
        }
        let which = OBJECTS.with_object(id, |o| o.o_which).unwrap_or(0);
        msg_str(&format!(
            "the {} vanishes as it hits the ground",
            weap_info[which as usize].oi_name
        ));
    }

    let _ = OBJECTS.remove(id);
}

/// Initializes a weapon object with baseline damage, flags, and stack counts.
pub unsafe fn init_weapon(id: ThingId, which: i32) {
    let Some(iwp) = INIT_DAM.get(which as usize).copied() else {
        return;
    };

    let many = iwp.iw_flags.contains(ObjectFlags::MANY);
    let (count, grp) = if which == DAGGER {
        (rnd(4) + 2, group)
    } else if many {
        (rnd(8) + 8, group)
    } else {
        (1, 0)
    };
    if which == DAGGER || many {
        group += 1;
    }

    OBJECTS.with_object_mut(id, |o| {
        o.o_type = ItemType::Weapon(which);
        o.o_which = which;
        set_damage(&mut o.o_damage, iwp.iw_dam);
        set_damage(&mut o.o_hurldmg, iwp.iw_hrl);
        o.o_launch = iwp.iw_launch;
        o.o_flags = iwp.iw_flags;
        o.o_hplus = 0;
        o.o_dplus = 0;
        o.o_count = count;
        o.o_group = grp;
    });
}

/// Resolves thrown-weapon combat against the target tile.
pub unsafe fn hit_monster(y: i32, x: i32, id: ThingId) -> i32 {
    let mp = IVec2 { x, y };
    fight(mp, Some(id), true as u8)
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
    // Track the previously wielded weapon by arena handle (pointer-free).
    let oweapon = PLAYER.equipment().weapon_id();
    if let Some(id) = oweapon {
        if !dropcheck_id(id) {
            PLAYER.set_weapon_id(oweapon);
            return;
        }
    }
    PLAYER.set_weapon_id(oweapon);

    let Some(obj) = get_item_id("wield", ItemFilter::Category(ItemType::WEAPON)) else {
        after = 0;
        return;
    };

    let is_armor = OBJECTS
        .with_object(obj, |o| matches!(o.o_type, ItemType::Armor(_)))
        .unwrap_or(false);
    if is_armor {
        msg_str("you can't wield armor");
        after = 0;
        return;
    }
    if is_current_id(obj) {
        after = 0;
        return;
    }

    let sp = inv_name_id(obj, true);
    PLAYER.set_weapon_id(Some(obj));
    if terse == 0 {
        addmsg_str("you are now ");
    }
    let packch = OBJECTS.with_object(obj, |o| o.o_packch).unwrap_or(0);
    msg_str(&format!("wielding {} ({})", sp, packch as char));
}

/// Chooses a nearby floor/passage cell to drop an item into, returning it (or
/// `None` when every neighbour is occupied or blocked).
pub unsafe fn fallpos(pos: IVec2) -> Option<IVec2> {
    let mut cnt = 0;
    let mut newpos = None;
    for y in (pos.y - 1)..=(pos.y + 1) {
        for x in (pos.x - 1)..=(pos.x + 1) {
            let h = hero();
            if y == h.y && x == h.x {
                continue;
            }
            let ch = chat(y, x);
            if ch == FLOOR || ch == PASSAGE {
                cnt += 1;
                if rnd(cnt) == 0 {
                    newpos = Some(IVec2 { x, y });
                }
            }
        }
    }
    newpos
}
