//! Pack and inventory management.
//!
//! Ported from `src/c/pack.c` to Rust.
//!
//! The player's pack is stored as a `Vec` of arena handles ([`ThingId`]) on the
//! hero [`crate::entity::player::ThingMonster`]; this module threads the stable
//! raw handles produced by the item arena through the legacy order/stack merge
//! algorithm of the C original.

use crate::entity::player::{MonsterFlags, ObjectFlags};
use crate::dungeon::DUNGEON;
use crate::item::arena::{with_object, with_object_mut, ThingId, OBJECTS};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::scrolls::ScrollType;
use crate::item::things::{add_line, inv_name_id};
use crate::misc::{find_obj_id, show_floor};
use crate::ui::input::readchar;
use crate::ui::output::{addmsg_str, endmsg, msg_str};
use glam::IVec2;

const MAXPACK: i32 = 23;
const PASSAGE: u8 = b'#' as u8;
const FLOOR: u8 = b'.' as u8;
const GOLD: u8 = b'*' as u8;
const POTION: i32 = b'!' as i32;
const SCROLL: i32 = b'?' as i32;
const FOOD: i32 = b':' as i32;
const WEAPON: i32 = b')' as i32;
const ARMOR: i32 = b']' as i32;
const AMULET: i32 = b',' as i32;
const RING: i32 = b'=' as i32;
const STICK: i32 = b'/' as i32;
const CALLABLE: i32 = -1;
const R_OR_S: i32 = -2;
const ESCAPE: i32 = 27;

use crate::game::globals::{
    after, again, amulet, inpack, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir,
    last_pick, move_on, mpos, msg_esc, n_objs, pack_used, purse, terse,
};

/// Unlink `id` from the current level's floor-item list.
unsafe fn detach_floor(id: ThingId) {
    crate::game::with_current_level_mut(|level| level.remove_item(id));
}

/// Discard the object `id` from the item arena.
unsafe fn discard_item(id: ThingId) {
    let _ = OBJECTS.remove(id);
}

/// The player's pack as arena handles, head first.
pub unsafe fn pack_ptrs() -> Vec<ThingId> {
    crate::game::PLAYER.pack()
}

unsafe fn hero_coord() -> IVec2 {
    crate::game::PLAYER.pos()
}

unsafe fn proom() -> Option<usize> {
    crate::game::PLAYER.room()
}

unsafe fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

unsafe fn floor_char_for_room() -> u8 {
    if crate::game::room_gone(proom()) {
        PASSAGE
    } else if show_floor() {
        FLOOR
    } else {
        b' ' as u8
    }
}

/// Pointer-free core of `add_pack`: `item` is `None` when picking up the
/// object on the hero's floor cell.
pub unsafe fn add_pack_id(mut item: Option<ThingId>, silent: bool) {
    let mut from_floor = false;

    if item.is_none() {
        item = find_obj_id(hero_coord().y, hero_coord().x);
        if item.is_none() {
            return;
        }
        from_floor = true;
    }
    let mut item_id = item.expect("item resolved above");

    let is_scare_dust = with_object(item_id, |o| {
        matches!(o.o_type, ItemType::Scroll(ScrollType::Scare))
            && o.o_flags.contains(ObjectFlags::FOUND)
    })
    .unwrap_or(false);
    if is_scare_dust {
        detach_floor(item_id);
        // The object is removed from the floor list, so the terrain glyph
        // shows automatically via draw.
        crate::draw::write_cell_glyph(hero_coord(), floor_char_for_room() as char);
        discard_item(item_id);
        msg_str("the scroll turns to dust as you pick it up");
        return;
    }

    let mut pack = crate::game::PLAYER.pack();

    if pack.is_empty() {
        if !pack_room_id(from_floor, item_id) {
            return;
        }
        with_object_mut(item_id, |o| o.o_packch = pack_char());
        pack.push(item_id);
    } else {
        let item_type = with_object(item_id, |o| o.o_type).unwrap_or(ItemType::None);
        let item_which = with_object(item_id, |o| o.o_which).unwrap_or(0);
        let item_group = with_object(item_id, |o| o.o_group).unwrap_or(0);
        let n = pack.len();

        // Walk the pack exactly as the C list traversal did, tracking the
        // position after which to insert (`lp`) or the stack we merge into.
        let mut lp: Option<usize> = None;
        let mut op = 0usize;
        let mut merged = false;
        let cat_of = |id: ThingId| with_object(id, |o| o.o_type).unwrap_or(ItemType::None);
        let which_of = |id: ThingId| with_object(id, |o| o.o_which).unwrap_or(0);
        let group_of = |id: ThingId| with_object(id, |o| o.o_group).unwrap_or(0);
        'outer: while op < n {
            if !cat_of(pack[op]).same_category(item_type) {
                lp = Some(op);
                op += 1;
                continue;
            }
            // Same category: advance while the `o_which` differs.
            loop {
                if cat_of(pack[op]).same_category(item_type) && which_of(pack[op]) != item_which {
                    lp = Some(op);
                    if op + 1 >= n {
                        op = n;
                        break;
                    }
                    op += 1;
                } else {
                    break;
                }
            }
            if op < n
                && cat_of(pack[op]).same_category(item_type)
                && which_of(pack[op]) == item_which
            {
                let opp = pack[op];
                if matches!(
                    item_type,
                    ItemType::Food | ItemType::Potion(_) | ItemType::Scroll(_)
                ) {
                    if !pack_room_id(from_floor, item_id) {
                        return;
                    }
                    with_object_mut(opp, |o| o.o_count += 1);
                    discard_item(item_id);
                    item_id = opp;
                    lp = None;
                    merged = true;
                    break 'outer;
                }
                if item_group != 0 {
                    lp = Some(op);
                    loop {
                        let o = pack[op];
                        if cat_of(o).same_category(item_type)
                            && which_of(o) == item_which
                            && group_of(o) != item_group
                        {
                            lp = Some(op);
                            if op + 1 >= n {
                                op = n;
                                break;
                            }
                            op += 1;
                        } else {
                            break;
                        }
                    }
                    if op < n {
                        let o = pack[op];
                        if cat_of(o).same_category(item_type)
                            && which_of(o) == item_which
                            && group_of(o) == item_group
                        {
                            let item_count = with_object(item_id, |x| x.o_count).unwrap_or(0);
                            with_object_mut(o, |x| x.o_count += item_count);
                            inpack -= 1;
                            if !pack_room_id(from_floor, item_id) {
                                return;
                            }
                            with_object_mut(o, |x| x.o_count += 1);
                            discard_item(item_id);
                            item_id = o;
                            lp = None;
                            merged = true;
                            break 'outer;
                        }
                        lp = Some(op);
                    }
                } else {
                    lp = Some(op);
                }
            }
            break;
        }

        if !merged {
            if let Some(pos) = lp {
                if pos + 1 <= pack.len() {
                    if !pack_room_id(from_floor, item_id) {
                        return;
                    }
                    with_object_mut(item_id, |o| o.o_packch = pack_char());
                    pack.insert(pos + 1, item_id);
                }
            }
        }
    }

    with_object_mut(item_id, |o| o.o_flags.insert(ObjectFlags::FOUND));
    crate::game::PLAYER.set_pack(pack);

    let item_dest = crate::entity::player::DestRef::Object(item_id);
    for id in DUNGEON.monster_list.ids() {
        if crate::entity::player::monster_dest(id) == item_dest {
            crate::entity::player::set_monster_dest_hero(id);
        }
    }

    if matches!(with_object(item_id, |o| o.o_type), Some(ItemType::Amulet)) {
        amulet = true as u8;
    }

    if !silent {
        if terse == 0 {
            addmsg_str("you now have ");
        }
        let packch = with_object(item_id, |o| o.o_packch).unwrap_or(0);
        msg_str(&format!(
            "{} ({})",
            inv_name_id(item_id, terse == 0),
            packch as char,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::player::{Thing, ThingObject};
    use crate::game::{with_current_level_mut, PLAYER};
    use crate::item::arena::OBJECTS;
    use crate::misc::find_obj_id;

    #[test]
    fn pickup_transfers_floor_items_and_merges_only_distinct_objects() {
        unsafe {
            let old_pack = PLAYER.pack();
            let old_inpack = inpack;
            let old_pack_used = pack_used;
            let old_items = with_current_level_mut(|level| std::mem::take(&mut level.items));
            PLAYER.set_pack(Vec::new());
            inpack = 0;
            pack_used = [0; 26];

            let pos = PLAYER.pos();
            let food = OBJECTS.insert(Thing::object(ThingObject {
                o_type: ItemType::Food,
                o_count: 1,
                o_pos: pos,
                ..ThingObject::default()
            }));
            with_current_level_mut(|level| level.add_item(food));
            add_pack_id(None, true);
            assert_eq!(PLAYER.pack(), vec![food]);
            assert_eq!(find_obj_id(pos.y, pos.x), None);
            assert_eq!({ inpack }, 1);
            assert!(OBJECTS
                .with_object(food, |obj| obj.o_flags.contains(ObjectFlags::FOUND))
                .unwrap());

            add_pack_id(None, true);
            assert_eq!(OBJECTS.with_object(food, |obj| obj.o_count), Some(1));
            assert_eq!(PLAYER.pack(), vec![food]);

            let weapon = OBJECTS.insert(Thing::object(ThingObject {
                o_type: ItemType::Weapon(0),
                o_count: 1,
                o_pos: pos,
                ..ThingObject::default()
            }));
            with_current_level_mut(|level| level.add_item(weapon));
            add_pack_id(None, true);
            assert_eq!(PLAYER.pack(), vec![food, weapon]);
            assert_eq!(find_obj_id(pos.y, pos.x), None);
            assert_eq!({ inpack }, 2);
            assert_eq!(OBJECTS.with_object(food, |obj| obj.o_packch), Some(b'a'));
            assert_eq!(OBJECTS.with_object(weapon, |obj| obj.o_packch), Some(b'b'));

            let more_food = OBJECTS.insert(Thing::object(ThingObject {
                o_type: ItemType::Food,
                o_count: 1,
                o_pos: pos,
                ..ThingObject::default()
            }));
            with_current_level_mut(|level| level.add_item(more_food));
            add_pack_id(None, true);
            assert_eq!(PLAYER.pack(), vec![food, weapon]);
            assert_eq!(find_obj_id(pos.y, pos.x), None);
            assert_eq!(OBJECTS.with_object(food, |obj| obj.o_count), Some(2));
            assert!(!OBJECTS.contains(more_food));
            assert_eq!({ inpack }, 3);

            OBJECTS.remove(food);
            OBJECTS.remove(weapon);
            PLAYER.set_pack(old_pack);
            inpack = old_inpack;
            pack_used = old_pack_used;
            with_current_level_mut(|level| level.items = old_items);
        }
    }
}

/// Pointer-free core of `pack_room`: returns whether the object fits.
pub unsafe fn pack_room_id(from_floor: bool, id: ThingId) -> bool {
    if inpack + 1 > MAXPACK {
        if terse == 0 {
            addmsg_str("there's ");
        }
        addmsg_str("no room");
        if terse == 0 {
            addmsg_str(" in your pack");
        }
        endmsg();
        if from_floor {
            move_msg_id(id);
        }
        inpack = MAXPACK;
        return false;
    }

    if from_floor {
        detach_floor(id);
        // The object is removed from the floor list, so the terrain glyph
        // shows automatically via draw.
        crate::draw::write_cell_glyph(hero_coord(), floor_char_for_room() as char);
    }

    inpack += 1;
    true
}

/// Pointer-free version of `leave_pack`: works on an arena [`ThingId`].
///
/// Splits `id` out of the player's pack (or one item off its stack) and returns
/// the handle to use for the departed object: the original `id` when the whole
/// stack leaves the pack, or a freshly allocated copy carrying one item when a
/// stack of more than one is being split (`newobj`).
pub unsafe fn leave_pack_id(id: ThingId, newobj: bool, all: bool) -> Option<ThingId> {
    let (count, group, packch) =
        crate::item::arena::with_object(id, |o| (o.o_count, o.o_group, o.o_packch))?;

    inpack -= 1;
    if count > 1 && !all {
        last_pick = Some(id);
        crate::item::arena::OBJECTS.with_object_mut(id, |o| o.o_count -= 1);
        if group != 0 {
            inpack += 1;
        }
        if newobj {
            let cloned = crate::item::arena::OBJECTS.with(id, |thing| thing.clone())?;
            let copy = crate::item::arena::OBJECTS.insert(cloned);
            crate::item::arena::OBJECTS.with_object_mut(copy, |o| o.o_count = 1);
            Some(copy)
        } else {
            Some(id)
        }
    } else {
        last_pick = None;
        if (packch as usize) >= b'a' as usize {
            pack_used[packch as usize - 'a' as usize] = false as u8;
        }
        crate::game::PLAYER.remove_from_pack(id);
        Some(id)
    }
}

pub unsafe fn pack_char() -> u8 {
    // `pack_used` is a 26-entry array (one slot per letter); index it directly so
    // no shared reference to the mutable static is created.
    for i in 0..26 {
        if pack_used[i] == 0 {
            pack_used[i] = true as u8;
            return (b'a' + i as u8) as u8;
        }
    }
    b'a' as u8
}

pub unsafe fn inventory(items: &[ThingId], filter: ItemFilter) -> u8 {
    n_objs = 0;
    let any = filter == ItemFilter::Any;

    for &id in items {
        let Some((otyp, packch)) = with_object(id, |o| (o.o_type, o.o_packch)) else {
            continue;
        };
        if !filter.matches(otyp) {
            continue;
        }

        n_objs += 1;
        msg_esc = 1;
        let format = if packch == 0 {
            "%s".to_string()
        } else {
            format!("{}) %s", packch as char)
        };
        let _ = add_line(&format, &inv_name_id(id, false));
        msg_esc = 0;
    }

    if n_objs == 0 {
        if terse != 0 {
            msg_str(if any {
                "empty handed"
            } else {
                "nothing appropriate"
            });
        } else {
            msg_str(if any {
                "you are empty handed"
            } else {
                "you don't have anything appropriate"
            });
        }
        return false as u8;
    }

    true as u8
}

pub unsafe fn pick_up(ch: u8) {
    let obj = find_obj_id(hero_coord().y, hero_coord().x);
    if player_has(MonsterFlags::LEVIT) {
        return;
    }
    if move_on != 0 {
        if let Some(id) = obj {
            move_msg_id(id);
        }
    } else {
        match ch as i32 {
            x if x == GOLD as i32 => {
                let Some(id) = obj else {
                    return;
                };
                money(with_object(id, |o| o.o_arm).unwrap_or(0));
                detach_floor(id);
                discard_item(id);
                if proom().is_some() {
                    crate::game::set_room_goldval(proom(), 0);
                }
            }
            ARMOR | POTION | FOOD | WEAPON | SCROLL | AMULET | RING | STICK => {
                add_pack_id(None, false);
            }
            _ => {}
        }
    }
}

/// Pointer-free version of `get_item`: resolves the player's selection to an
/// arena [`ThingId`] (or `None` when the player cancels / carries nothing).
pub unsafe fn get_item_id(purpose: &str, filter: ItemFilter) -> Option<ThingId> {
    let mut ch: i32;

    if crate::game::PLAYER.pack().is_empty() {
        msg_str("you aren't carrying anything");
        return None;
    }

    if again != 0 {
        if let Some(id) = last_pick {
            return Some(id);
        }
        msg_str("you ran out");
        return None;
    }

    loop {
        if terse == 0 {
            addmsg_str("which object do you want to ");
        }
        addmsg_str(purpose);
        if terse != 0 {
            addmsg_str(" what");
        }
        msg_str("? (* for list): ");
        ch = readchar();
        mpos = 0;
        if ch == ESCAPE {
            reset_last();
            after = false as u8;
            msg_str("");
            return None;
        }
        n_objs = 1;
        if ch == '*' as i32 {
            mpos = 0;
            if inventory(&pack_ptrs(), filter) == 0 {
                after = false as u8;
                return None;
            }
            continue;
        }
        for id in crate::game::PLAYER.pack() {
            if crate::item::arena::with_object(id, |o| o.o_packch) == Some(ch as u8) {
                return Some(id);
            }
        }
        msg_str(&format!(
            "'{}' is not a valid item",
            crate::ui::output::format_key(ch as u8)
        ));
    }
}

pub unsafe fn money(value: i32) {
    purse += value;
    // The gold object was discarded, so the terrain glyph shows via draw.
    crate::draw::write_cell_glyph(hero_coord(), floor_char_for_room() as char);
    if value > 0 {
        if terse == 0 {
            addmsg_str("you found ");
        }
        msg_str(&format!("{} gold pieces", value));
    }
}

pub unsafe fn floor_ch() -> u8 {
    floor_char_for_room()
}

pub unsafe fn floor_at() -> u8 {
    let ch = crate::draw::cell_glyph(hero_coord().y, hero_coord().x);
    if ch == FLOOR {
        floor_char_for_room()
    } else {
        ch
    }
}

pub unsafe fn reset_last() {
    last_comm = l_last_comm;
    last_dir = l_last_dir;
    last_pick = l_last_pick;
}

/// Pointer-free core of `move_msg`: works on an arena handle.
pub unsafe fn move_msg_id(id: ThingId) {
    if terse == 0 {
        addmsg_str("you ");
    }
    msg_str(&format!("moved onto {}", inv_name_id(id, true)));
}

pub unsafe fn picky_inven() {
    let pack = crate::game::PLAYER.pack();
    if pack.is_empty() {
        msg_str("you aren't carrying anything");
    } else if pack.len() == 1 {
        msg_str(&format!("a) {}", inv_name_id(pack[0], false)));
    } else {
        msg_str(if terse != 0 {
            "item: "
        } else {
            "which item do you wish to inventory: "
        });
        mpos = 0;
        let mch = readchar() as u8;
        if mch as i32 == ESCAPE {
            msg_str("");
            return;
        }
        for id in &pack {
            if with_object(*id, |o| o.o_packch) == Some(mch) {
                msg_str(&format!("{}) {}", mch as char, inv_name_id(*id, false)));
                return;
            }
        }
        msg_str(&format!(
            "'{}' not in pack",
            crate::ui::output::format_key(mch)
        ));
    }
}

unsafe fn pick_up_char(ch: u8) {
    pick_up(ch);
}
