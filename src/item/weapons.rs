//! Weapons: wielding, throwing, and weapon initialization.
//!
//! Ported from `src/c/weapons.c` to Rust.
//!
//! Item access is pointer-free: weapons (and the transient bolts/flames fired
//! by wands) are addressed through the item arena's [`ThingId`] handles and
//! mutated through scoped `with_object`/`with_object_mut` access rather than
//! raw `*mut Thing` dereferences. Reentrant process flags are passed explicitly
//! as raw pointers, never borrowed across combat or UI callbacks.
use crate::command::Command;
use crate::direction::Direction;
use crate::dungeon::Dungeon;
use crate::entity::chase::cansee;
use crate::entity::fight::fight;
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::{get_item_id, leave_pack_id};
use crate::misc::{is_current_id, show_floor};
use crate::rnd::Random;
use crate::ui::output;
use crate::ui::output::{addmsg_str, endmsg, msg_str};
use glam::IVec2;

use crate::entity::player::ObjectFlags;
use crate::game::globals::ObjInfo;
use crate::item::arena::{ThingArena, ThingId};
use crate::item::things::{dropcheck_id, inv_name_id};

const NO_WEAPON: i32 = -1;

const FLOOR: i32 = '.' as i32;
const PASSAGE: i32 = '#' as i32;
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

const INIT_DAM: [InitWeap; MAXWEAPONS] = [
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

#[inline]
fn hero(dungeon: &Dungeon) -> IVec2 {
    dungeon.player.pos()
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
///
/// # Safety
/// `has_hit` must point to a valid, writable hit-message flag. It is passed as a
/// raw pointer because combat may update the same flag before the item falls.
pub unsafe fn missile(
    ydelta: i32,
    xdelta: i32,
    has_hit: *mut u8,
    random: &mut Random,
    dungeon: &Dungeon,
    terse: u8,
    weap_info: &[ObjInfo],
    after: *mut u8,
    again: u8,
    inpack: *mut i32,
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: *mut Command,
    last_dir: *mut Direction,
    last_pick: *mut Option<ThingId>,
    mpos: *mut i32,
    msg_esc: *mut u8,
    n_objs: *mut i32,
    pack_used: *mut [u8; 26],
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    let Some(id) = get_item_id("throw", ItemFilter::Category(ItemType::WEAPON), dungeon, after, again, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, mpos, msg_esc, n_objs, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) else {
        return;
    };
    if !dropcheck_id(id, random, dungeon) || is_current_id(id) {
        return;
    }

    let Some(obj) = leave_pack_id(id, true, false, dungeon, inpack, last_pick, pack_used) else {
        return;
    };
    do_motion(obj, ydelta, xdelta, dungeon, terse);

    let pos = dungeon.objects.with_object(obj, |o| o.o_pos).unwrap_or(IVec2::ZERO);
    if dungeon.monster_map.at(pos.y as usize, pos.x as usize).is_none()
        || hit_monster(pos.y, pos.x, obj, random) == 0
    {
        fall(obj, true, has_hit, random, dungeon, weap_info);
    }
}

/// Animates projectile movement until it hits blocking terrain or a door.
pub unsafe fn do_motion(id: ThingId, ydelta: i32, xdelta: i32, dungeon: &Dungeon, terse: u8) -> IVec2 {
    let mut pos = hero(dungeon);
    let o_type = dungeon.objects
        .with_object(id, |o| o.o_type)
        .unwrap_or(ItemType::None);

    loop {
        let h = hero(dungeon);
        if (pos.x != h.x || pos.y != h.y) && cansee(pos.y, pos.x) != 0 && terse == 0 {
            let mut ch = chat(pos.y, pos.x);
            if ch == FLOOR && !show_floor() {
                ch = ' ' as i32;
            }
            crate::draw::write_cell_glyph(pos, ch as u8 as char);
        }

        pos.y += ydelta;
        pos.x += xdelta;

        let walkable = dungeon.monster_map.at(pos.y as usize, pos.x as usize).is_none()
            && dungeon.with_level(|level| {
                level.tile_at(pos.y as usize, pos.x as usize).is_walkable()
                    && !level.is_door_at(pos.y as usize, pos.x as usize)
            });
        if walkable {
            if cansee(pos.y, pos.x) != 0 && terse == 0 {
                crate::draw::write_cell_glyph(pos, crate::draw::item_glyph(o_type));
                output::refresh();
            }
            continue;
        }
        break;
    }

    dungeon.objects.with_object_mut(id, |o| o.o_pos = pos);
    pos
}

/// Drops an item near its current position or discards it if no floor slot is available.
///
/// # Safety
/// `has_hit` must remain valid and writable throughout the call. Do not retain
/// an exclusive reference to it across visibility or message callbacks.
pub unsafe fn fall(
    id: ThingId,
    pr: bool,
    has_hit: *mut u8,
    random: &mut Random,
    dungeon: &Dungeon,
    weap_info: &[ObjInfo],
) {
    let Some(pos) = dungeon.objects.with_object(id, |o| o.o_pos) else {
        return;
    };

    if let Some(newpos) = fallpos(pos, random, dungeon) {
        // Objects render from the `lvl_obj` list; no glyph write needed.
        dungeon.objects.with_object_mut(id, |o| o.o_pos = newpos);

        if cansee(newpos.y, newpos.x) != 0 {
            let glyph = dungeon.objects
                .with_object(id, |o| crate::draw::item_glyph(o.o_type))
                .unwrap_or(')');
            if let Some(mid) = dungeon.monster_map.at(newpos.y as usize, newpos.x as usize) {
                dungeon.monster_list.with_mut(mid, |t| {
                    if let crate::entity::player::Thing::Monster { data } = t {
                        data.t_oldch = glyph as u8;
                    }
                });
            } else {
                crate::draw::write_cell_glyph(newpos, glyph);
            }
        }

        dungeon.with_level_mut(|level| level.add_item(id));
        return;
    }

    if pr {
        if *has_hit != 0 {
            endmsg();
            *has_hit = 0;
        }
        let which = dungeon.objects.with_object(id, |o| o.o_which).unwrap_or(0);
        msg_str(&format!(
            "the {} vanishes as it hits the ground",
            weap_info[which as usize].oi_name
        ));
    }

    let _ = dungeon.objects.remove(id);
}

/// Initializes a weapon object with baseline damage, flags, and stack counts.
///
/// # Safety
/// `group` must point to a writable stack-group counter for the entire call.
pub unsafe fn init_weapon(
    id: ThingId,
    which: i32,
    random: &mut Random,
    dungeon: &Dungeon,
    group: *mut i32,
) {
    init_weapon_in(&dungeon.objects, id, which, random, group);
}

/// Initialize a weapon in the supplied arena using the shared template.
///
/// # Safety
/// `group` must point to a writable stack-group counter. No exclusive reference
/// to that storage may overlap reentrant access; only immediate raw reads and
/// writes are performed here.
pub(crate) unsafe fn init_weapon_in(
    objects: &ThingArena,
    id: ThingId,
    which: i32,
    random: &mut Random,
    group: *mut i32,
) {
    let Some(iwp) = INIT_DAM.get(which as usize).copied() else {
        return;
    };

    let many = iwp.iw_flags.contains(ObjectFlags::MANY);
    let (count, grp) = if which == DAGGER {
        (random.rnd(4) + 2, *group)
    } else if many {
        (random.rnd(8) + 8, *group)
    } else {
        (1, 0)
    };
    if which == DAGGER || many {
        *group += 1;
    }

    objects.with_object_mut(id, |o| {
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
pub unsafe fn hit_monster(y: i32, x: i32, id: ThingId, random: &mut Random) -> i32 {
    let mp = IVec2 { x, y };
    fight(mp, Some(id), true as u8, random)
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
///
/// # Safety
/// `after` must remain writable, without an overlapping exclusive reference
/// across reentrant item/UI calls.
pub unsafe fn wield(
    random: &mut Random,
    dungeon: &Dungeon,
    terse: u8,
    after: *mut u8,
    again: u8,
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: *mut Command,
    last_dir: *mut Direction,
    last_pick: *mut Option<ThingId>,
    mpos: *mut i32,
    msg_esc: *mut u8,
    n_objs: *mut i32,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    // Track the previously wielded weapon by arena handle (pointer-free).
    let oweapon = dungeon.player.equipment().weapon_id();
    if let Some(id) = oweapon {
        if !dropcheck_id(id, random, dungeon) {
            dungeon.player.set_weapon_id(oweapon);
            return;
        }
    }
    dungeon.player.set_weapon_id(oweapon);

    let Some(obj) = get_item_id("wield", ItemFilter::Category(ItemType::WEAPON), dungeon, after, again, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, mpos, msg_esc, n_objs, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) else {
        *after = 0;
        return;
    };

    let is_armor = dungeon.objects
        .with_object(obj, |o| matches!(o.o_type, ItemType::Armor(_)))
        .unwrap_or(false);
    if is_armor {
        msg_str("you can't wield armor");
        *after = 0;
        return;
    }
    if is_current_id(obj) {
        *after = 0;
        return;
    }

    let sp = inv_name_id(obj, true, dungeon, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
    dungeon.player.set_weapon_id(Some(obj));
    if terse == 0 {
        addmsg_str("you are now ");
    }
    let packch = dungeon.objects.with_object(obj, |o| o.o_packch).unwrap_or(0);
    msg_str(&format!("wielding {} ({})", sp, packch as char));
}

/// Chooses a nearby floor/passage cell to drop an item into, returning it (or
/// `None` when every neighbour is occupied or blocked).
pub unsafe fn fallpos(pos: IVec2, random: &mut Random, dungeon: &Dungeon) -> Option<IVec2> {
    let mut cnt = 0;
    let mut newpos = None;
    for y in (pos.y - 1)..=(pos.y + 1) {
        for x in (pos.x - 1)..=(pos.x + 1) {
            let h = hero(dungeon);
            if y == h.y && x == h.x {
                continue;
            }
            let ch = chat(y, x);
            if ch == FLOOR || ch == PASSAGE {
                cnt += 1;
                if random.rnd(cnt) == 0 {
                    newpos = Some(IVec2 { x, y });
                }
            }
        }
    }
    newpos
}

#[cfg(test)]
mod tests {
    use super::{hero, num, set_damage, WEAPON};
    use crate::dungeon::Dungeon;

    #[test]
    fn enchantment_numbers_preserve_signs_and_weapon_pairs() {
        assert_eq!(num(2, -3, WEAPON), "+2,-3");
        assert_eq!(num(0, 0, WEAPON), "+0,+0");
        assert_eq!(num(-2, 9, 0), "-2");
    }

    #[test]
    fn damage_copy_clears_old_bytes_and_truncates_long_specs() {
        let mut damage = [b'x'; 8];
        set_damage(&mut damage, b"2x4\0");
        assert_eq!(&damage, b"2x4\0\0\0\0\0");
        set_damage(&mut damage, b"123456789");
        assert_eq!(&damage, b"12345678");
        set_damage(&mut damage, b"");
        assert_eq!(damage, [0; 8]);
    }

    #[test]
    fn hero_reads_the_supplied_dungeon() {
        let dungeon = Dungeon::empty();
        assert_eq!(hero(&dungeon), dungeon.player.pos());
    }

}
