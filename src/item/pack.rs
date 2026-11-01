//! Pack and inventory management.
//!
//! Ported from `src/c/pack.c` to Rust.
//!
//! The player's pack is stored as a `Vec` of arena handles ([`ThingId`]) on the
//! hero [`crate::entity::player::ThingMonster`]; this module threads the stable
//! raw handles produced by the item arena through the legacy order/stack merge
//! algorithm of the C original.
//!
//! Remaining state pointers must remain valid and writable throughout each call. Callers
//! must not retain exclusive references to their storage across UI/daemon
//! reentry. Pack counts use relaxed atomics; letter bitmap locks are held only
//! for immediate bitmap access, never across gameplay helpers.
//! Repeat command/direction/selection slots use shared `CommandValue` cells;
//! their locks are released by each `get`/`set` before invoking helpers.

use crate::command::Command;
use crate::direction::Direction;
use crate::dungeon::Dungeon;
use crate::entity::player::{MonsterFlags, ObjectFlags};
use crate::game::globals::ObjInfo;
use crate::item::arena::{with_object, with_object_mut, ThingId};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::scrolls::ScrollType;
use crate::item::things::{add_line, inv_name_id};
use crate::misc::show_floor;
use crate::ui::input::readchar;
use crate::ui::output::{addmsg_str, endmsg, msg_str};
use glam::IVec2;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

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

/// Unlink `id` from the current level's floor-item list.
unsafe fn detach_floor(id: ThingId, dungeon: &Dungeon) {
    dungeon.with_level_mut(|level| level.remove_item(id));
}

/// Discard the object `id` from the item arena.
unsafe fn discard_item(id: ThingId, dungeon: &Dungeon) {
    let _ = dungeon.objects.remove(id);
}

/// The player's pack as arena handles, head first.
pub unsafe fn pack_ptrs(dungeon: &Dungeon) -> Vec<ThingId> {
    dungeon.player.pack()
}

unsafe fn hero_coord(dungeon: &Dungeon) -> IVec2 {
    dungeon.player.pos()
}

unsafe fn proom(dungeon: &Dungeon) -> Option<usize> {
    dungeon.player.room()
}

unsafe fn player_has(flag: MonsterFlags, dungeon: &Dungeon) -> bool {
    dungeon.player.has_flag(flag)
}

unsafe fn floor_char_for_room(dungeon: &Dungeon) -> u8 {
    if dungeon.with_level(|level| level.room_gone(proom(dungeon))) {
        PASSAGE
    } else if show_floor() {
        FLOOR
    } else {
        b' ' as u8
    }
}

/// Find the floor object at the hero's cell without consulting the singleton.
fn floor_item(dungeon: &Dungeon) -> Option<ThingId> {
    let pos = dungeon.player.pos();
    let items = dungeon.with_level(|level| level.items.clone());
    items.into_iter().find(|&id| {
        with_object(&dungeon.objects, id, |o| o.o_pos == pos).unwrap_or(false)
    })
}

/// Pointer-free core of `add_pack`: `item` is `None` when picking up the
/// object on the hero's floor cell.
pub unsafe fn add_pack_id(
    mut item: Option<ThingId>,
    silent: bool,
    dungeon: &Dungeon,
    inpack: &AtomicI32,
    pack_used: &Mutex<[u8; 26]>,
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    let mut from_floor = false;

    if item.is_none() {
        item = floor_item(dungeon);
        if item.is_none() {
            return;
        }
        from_floor = true;
    }
    let mut item_id = item.expect("item resolved above");

    let is_scare_dust = with_object(&dungeon.objects, item_id, |o| {
        matches!(o.o_type, ItemType::Scroll(ScrollType::Scare))
            && o.o_flags.contains(ObjectFlags::FOUND)
    })
    .unwrap_or(false);
    if is_scare_dust {
        detach_floor(item_id, dungeon);
        // The object is removed from the floor list, so the terrain glyph
        // shows automatically via draw.
        crate::draw::write_cell_glyph(hero_coord(dungeon), floor_char_for_room(dungeon) as char);
        discard_item(item_id, dungeon);
        msg_str("the scroll turns to dust as you pick it up");
        return;
    }

    let mut pack = dungeon.player.pack();

    if pack.is_empty() {
        if !pack_room_id(from_floor, item_id, dungeon, inpack, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) {
            return;
        }
        with_object_mut(&dungeon.objects, item_id, |o| o.o_packch = pack_char(pack_used));
        pack.push(item_id);
    } else {
        let item_type = with_object(&dungeon.objects, item_id, |o| o.o_type).unwrap_or(ItemType::None);
        let item_which = with_object(&dungeon.objects, item_id, |o| o.o_which).unwrap_or(0);
        let item_group = with_object(&dungeon.objects, item_id, |o| o.o_group).unwrap_or(0);
        let n = pack.len();

        // Walk the pack exactly as the C list traversal did, tracking the
        // position after which to insert (`lp`) or the stack we merge into.
        let mut lp: Option<usize> = None;
        let mut op = 0usize;
        let mut merged = false;
        let cat_of = |id: ThingId| with_object(&dungeon.objects, id, |o| o.o_type).unwrap_or(ItemType::None);
        let which_of = |id: ThingId| with_object(&dungeon.objects, id, |o| o.o_which).unwrap_or(0);
        let group_of = |id: ThingId| with_object(&dungeon.objects, id, |o| o.o_group).unwrap_or(0);
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
                    if !pack_room_id(from_floor, item_id, dungeon, inpack, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) {
                        return;
                    }
                    with_object_mut(&dungeon.objects, opp, |o| o.o_count += 1);
                    discard_item(item_id, dungeon);
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
                            let item_count = with_object(&dungeon.objects, item_id, |x| x.o_count).unwrap_or(0);
                            with_object_mut(&dungeon.objects, o, |x| x.o_count += item_count);
                            inpack.fetch_sub(1, Ordering::Relaxed);
                            if !pack_room_id(from_floor, item_id, dungeon, inpack, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) {
                                return;
                            }
                            with_object_mut(&dungeon.objects, o, |x| x.o_count += 1);
                            discard_item(item_id, dungeon);
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
                    if !pack_room_id(from_floor, item_id, dungeon, inpack, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) {
                        return;
                    }
                    with_object_mut(&dungeon.objects, item_id, |o| o.o_packch = pack_char(pack_used));
                    pack.insert(pos + 1, item_id);
                }
            }
        }
    }

    with_object_mut(&dungeon.objects, item_id, |o| o.o_flags.insert(ObjectFlags::FOUND));
    dungeon.player.set_pack(pack);

    let item_dest = crate::entity::player::DestRef::Object(item_id);
    for id in dungeon.monster_list.ids() {
        dungeon.monster_list.with_mut(id, |thing| {
            if let crate::entity::player::Thing::Monster { data } = thing {
                if data.t_dest == item_dest {
                    data.t_dest = crate::entity::player::DestRef::Hero;
                }
            }
        });
    }

    if matches!(with_object(&dungeon.objects, item_id, |o| o.o_type), Some(ItemType::Amulet)) {
        dungeon.set_amulet(true);
    }

    if !silent {
        if terse == 0 {
            addmsg_str("you now have ");
        }
        let packch = with_object(&dungeon.objects, item_id, |o| o.o_packch).unwrap_or(0);
        msg_str(&format!(
            "{} ({})",
            inv_name_id(item_id, terse == 0, dungeon, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info),
            packch as char,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::player::{Thing, ThingObject};

    #[test]
    fn floor_access_uses_only_the_supplied_dungeon() {
        let dungeon = Dungeon::empty();
        let other = Dungeon::empty();
        let item = dungeon.objects.insert(Thing::object(ThingObject {
            o_type: ItemType::Food,
            o_pos: dungeon.player.pos(),
            ..ThingObject::default()
        }));
        dungeon.with_level_mut(|level| level.add_item(item));

        unsafe {
            assert_eq!(floor_at(&dungeon), b':');
            assert_eq!(floor_at(&other), b' ');
        }
    }

    #[test]
    fn pack_letters_only_update_the_supplied_bitmap() {
        let used = Mutex::new([0; 26]);
        let other = Mutex::new([0; 26]);
        unsafe {
            assert_eq!(pack_char(&other), b'a');
            for letter in b'a'..=b'z' {
                assert_eq!(pack_char(&used), letter);
            }
            assert_eq!(*used.lock().unwrap(), [1; 26]);
            // Preserve the legacy fallback when no free letter remains.
            assert_eq!(pack_char(&used), b'a');
        }
        let other = other.lock().unwrap();
        assert_eq!(other[0], 1);
        assert_eq!(&other[1..], &[0; 25]);
    }

    #[test]
    fn pack_letters_recover_a_poisoned_bitmap() {
        let used = Mutex::new([0; 26]);
        let _ = std::panic::catch_unwind(|| {
            let mut bitmap = used.lock().unwrap();
            bitmap[0] = 1;
            panic!("poison bitmap");
        });
        assert!(used.is_poisoned());
        unsafe {
            assert_eq!(pack_char(&used), b'b');
        }

        let dungeon = Dungeon::empty();
        let id = dungeon.objects.insert(Thing::object(ThingObject {
            o_count: 1,
            o_packch: b'b',
            ..ThingObject::default()
        }));
        dungeon.player.set_pack(vec![id]);
        let inpack = AtomicI32::new(1);
        let last_pick = crate::command_dispatch::CommandValue::new(Some(id));
        unsafe {
            assert_eq!(leave_pack_id(id, false, false, &dungeon, &inpack, &last_pick, &used), Some(id));
        }
        assert_eq!(inpack.load(Ordering::Relaxed), 0);
        // Both bitmap helpers release their guards before returning.
        let bitmap = used.try_lock().unwrap_or_else(|error| match error {
            std::sync::TryLockError::Poisoned(poisoned) => poisoned.into_inner(),
            std::sync::TryLockError::WouldBlock => panic!("bitmap lock is still held"),
        });
        assert_eq!(bitmap[0], 1);
        assert_eq!(bitmap[1], 0);
    }

    #[test]
    fn leaving_invalid_pack_letters_does_not_change_the_bitmap() {
        let dungeon = Dungeon::empty();
        let inpack = AtomicI32::new(1);
        let used = Mutex::new([1; 26]);
        let last_pick = crate::command_dispatch::CommandValue::new(None);
        for packch in [0, b'a' - 1, b'z' + 1, u8::MAX] {
            let id = dungeon.objects.insert(Thing::object(ThingObject {
                o_count: 1,
                o_packch: packch,
                ..ThingObject::default()
            }));
            dungeon.player.set_pack(vec![id]);
            inpack.store(1, Ordering::Relaxed);
            unsafe {
                assert_eq!(leave_pack_id(id, false, false, &dungeon, &inpack, &last_pick, &used), Some(id));
            }
            assert_eq!(inpack.load(Ordering::Relaxed), 0);
            assert_eq!(*used.try_lock().unwrap(), [1; 26]);
            assert!(dungeon.player.pack().is_empty());
        }
    }

    #[test]
    fn grouped_stacks_keep_one_capacity_slot_at_the_limit() {
        let dungeon = Dungeon::empty();
        let stack = dungeon.objects.insert(Thing::object(ThingObject {
            o_type: ItemType::Weapon(0),
            o_count: 3,
            o_group: 7,
            o_packch: b'a',
            ..ThingObject::default()
        }));
        let incoming = dungeon.objects.insert(Thing::object(ThingObject {
            o_type: ItemType::Weapon(0),
            o_count: 2,
            o_group: 7,
            ..ThingObject::default()
        }));
        dungeon.player.set_pack(vec![stack]);
        let inpack = AtomicI32::new(MAXPACK);
        let used = Mutex::new([0; 26]);
        used.lock().unwrap()[0] = 1;
        let last_pick = crate::command_dispatch::CommandValue::new(None);
        unsafe {
            add_pack_id(Some(incoming), true, &dungeon, &inpack, &used, 0, 1, &[], &[], &[], &[], &[], &[]);
            assert_eq!(inpack.load(Ordering::Relaxed), MAXPACK);
            assert_eq!(dungeon.player.pack(), vec![stack]);
            assert!(!dungeon.objects.contains(incoming));
            // Preserve the legacy grouped merge's extra increment exactly.
            assert_eq!(dungeon.objects.with_object(stack, |o| o.o_count), Some(6));

            let split = leave_pack_id(stack, true, false, &dungeon, &inpack, &last_pick, &used).unwrap();
            assert_ne!(split, stack);
            assert_eq!(inpack.load(Ordering::Relaxed), MAXPACK);
            assert_eq!(dungeon.objects.with_object(stack, |o| o.o_count), Some(5));
            assert_eq!(dungeon.objects.with_object(split, |o| o.o_count), Some(1));
            assert_eq!(used.lock().unwrap()[0], 1);

            assert_eq!(leave_pack_id(stack, false, true, &dungeon, &inpack, &last_pick, &used), Some(stack));
        }
        assert_eq!(inpack.load(Ordering::Relaxed), MAXPACK - 1);
        assert_eq!(*used.try_lock().unwrap(), [0; 26]);
        assert!(dungeon.player.pack().is_empty());
    }

    #[test]
    fn reset_last_restores_each_supplied_repeat_slot() {
        let dungeon = Dungeon::empty();
        let id = dungeon.objects.new_item_id();
        let command = crate::command_dispatch::CommandValue::new(Command::Inventory);
        let direction = crate::command_dispatch::CommandValue::new(Direction::None);
        let pick = crate::command_dispatch::CommandValue::new(None);
        let other_command = crate::command_dispatch::CommandValue::new(Command::Inventory);
        let other_direction = crate::command_dispatch::CommandValue::new(Direction::None);
        let other_pick = crate::command_dispatch::CommandValue::new(None::<ThingId>);
        let saved_command = Command::Drop;
        let saved_direction = Direction::North;
        unsafe {
            reset_last(saved_command, saved_direction, Some(id), &command, &direction, &pick);
        }
        assert_eq!(command.get(), saved_command);
        assert_eq!(direction.get(), saved_direction);
        assert_eq!(pick.get(), Some(id));
        assert_eq!(other_command.get(), Command::Inventory);
        assert_eq!(other_direction.get(), Direction::None);
        assert_eq!(other_pick.get(), None);

        unsafe {
            reset_last(Command::Inventory, Direction::None, None, &command, &direction, &pick);
        }
        assert_eq!(command.get(), Command::Inventory);
        assert_eq!(direction.get(), Direction::None);
        assert_eq!(pick.get(), None);
    }

    #[test]
    fn leaving_a_stack_threads_pack_count_selection_and_letters() {
        let dungeon = Dungeon::empty();
        let other = Dungeon::empty();
        let id = dungeon.objects.insert(Thing::object(ThingObject {
            o_type: ItemType::Food,
            o_count: 2,
            o_packch: b'c',
            ..ThingObject::default()
        }));
        dungeon.player.set_pack(vec![id]);
        let inpack = AtomicI32::new(2);
        let last_pick = crate::command_dispatch::CommandValue::new(None);
        let pack_used = Mutex::new([0; 26]);
        pack_used.lock().unwrap()[2] = 1;

        unsafe {
            let split = leave_pack_id(id, true, false, &dungeon, &inpack, &last_pick, &pack_used).unwrap();
            assert_ne!(split, id);
            assert_eq!(dungeon.objects.with_object(split, |o| o.o_count), Some(1));
            assert_eq!(dungeon.objects.with_object(id, |o| o.o_count), Some(1));
            assert_eq!(dungeon.player.pack(), vec![id]);
            assert_eq!(inpack.load(Ordering::Relaxed), 1);
            assert_eq!(last_pick.get(), Some(id));
            assert_eq!(pack_used.lock().unwrap()[2], 1);

            assert_eq!(leave_pack_id(id, false, false, &dungeon, &inpack, &last_pick, &pack_used), Some(id));
            assert_eq!(inpack.load(Ordering::Relaxed), 0);
            assert_eq!(last_pick.get(), None);
            assert_eq!(pack_used.lock().unwrap()[2], 0);
            assert!(dungeon.player.pack().is_empty());
            assert!(other.objects.is_empty());
            assert!(other.player.pack().is_empty());

            dungeon.objects.remove(id);
            assert_eq!(leave_pack_id(id, false, false, &dungeon, &inpack, &last_pick, &pack_used), None);
            assert_eq!(inpack.load(Ordering::Relaxed), 0);
        }
    }

    #[test]
    fn pickup_transfers_floor_items_and_merges_only_distinct_objects() {
        unsafe {
            let dungeon = Dungeon::empty();
            let inpack = AtomicI32::new(0);
            let pack_used = Mutex::new([0; 26]);
            // Silent, successful pickups never consult the naming tables.
            let pos = dungeon.player.pos();
            let food = dungeon.objects.insert(Thing::object(ThingObject {
                o_type: ItemType::Food,
                o_count: 1,
                o_pos: pos,
                ..ThingObject::default()
            }));
            dungeon.with_level_mut(|level| level.add_item(food));
            add_pack_id(None, true, &dungeon, &inpack, &pack_used, 0, 1, &[], &[], &[], &[], &[], &[]);
            assert_eq!(dungeon.player.pack(), vec![food]);
            assert_eq!(floor_item(&dungeon), None);
            assert_eq!(inpack.load(Ordering::Relaxed), 1);
            assert!(dungeon.objects
                .with_object(food, |obj| obj.o_flags.contains(ObjectFlags::FOUND))
                .unwrap());

            add_pack_id(None, true, &dungeon, &inpack, &pack_used, 0, 1, &[], &[], &[], &[], &[], &[]);
            assert_eq!(dungeon.objects.with_object(food, |obj| obj.o_count), Some(1));
            assert_eq!(dungeon.player.pack(), vec![food]);

            let weapon = dungeon.objects.insert(Thing::object(ThingObject {
                o_type: ItemType::Weapon(0),
                o_count: 1,
                o_pos: pos,
                ..ThingObject::default()
            }));
            dungeon.with_level_mut(|level| level.add_item(weapon));
            add_pack_id(None, true, &dungeon, &inpack, &pack_used, 0, 1, &[], &[], &[], &[], &[], &[]);
            assert_eq!(dungeon.player.pack(), vec![food, weapon]);
            assert_eq!(floor_item(&dungeon), None);
            assert_eq!(inpack.load(Ordering::Relaxed), 2);
            assert_eq!(dungeon.objects.with_object(food, |obj| obj.o_packch), Some(b'a'));
            assert_eq!(dungeon.objects.with_object(weapon, |obj| obj.o_packch), Some(b'b'));

            let more_food = dungeon.objects.insert(Thing::object(ThingObject {
                o_type: ItemType::Food,
                o_count: 1,
                o_pos: pos,
                ..ThingObject::default()
            }));
            dungeon.with_level_mut(|level| level.add_item(more_food));
            add_pack_id(None, true, &dungeon, &inpack, &pack_used, 0, 1, &[], &[], &[], &[], &[], &[]);
            assert_eq!(dungeon.player.pack(), vec![food, weapon]);
            assert_eq!(floor_item(&dungeon), None);
            assert_eq!(dungeon.objects.with_object(food, |obj| obj.o_count), Some(2));
            assert!(!dungeon.objects.contains(more_food));
            assert_eq!(inpack.load(Ordering::Relaxed), 3);
        }
    }
}

/// Pointer-free core of `pack_room`: returns whether the object fits.
pub unsafe fn pack_room_id(
    from_floor: bool,
    id: ThingId,
    dungeon: &Dungeon,
    inpack: &AtomicI32,
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) -> bool {
    if inpack.load(Ordering::Relaxed) + 1 > MAXPACK {
        if terse == 0 {
            addmsg_str("there's ");
        }
        addmsg_str("no room");
        if terse == 0 {
            addmsg_str(" in your pack");
        }
        endmsg();
        if from_floor {
            move_msg_id(id, dungeon, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
        }
        inpack.store(MAXPACK, Ordering::Relaxed);
        return false;
    }

    if from_floor {
        detach_floor(id, dungeon);
        // The object is removed from the floor list, so the terrain glyph
        // shows automatically via draw.
        crate::draw::write_cell_glyph(hero_coord(dungeon), floor_char_for_room(dungeon) as char);
    }

    inpack.fetch_add(1, Ordering::Relaxed);
    true
}

/// Pointer-free version of `leave_pack`: works on an arena [`ThingId`].
///
/// Splits `id` out of the player's pack (or one item off its stack) and returns
/// the handle to use for the departed object: the original `id` when the whole
/// stack leaves the pack, or a freshly allocated copy carrying one item when a
/// stack of more than one is being split (`newobj`).
pub unsafe fn leave_pack_id(
    id: ThingId,
    newobj: bool,
    all: bool,
    dungeon: &Dungeon,
    inpack: &AtomicI32,
    last_pick: &crate::command_dispatch::CommandValue<Option<ThingId>>,
    pack_used: &Mutex<[u8; 26]>,
) -> Option<ThingId> {
    let (count, group, packch) =
        crate::item::arena::with_object(&dungeon.objects, id, |o| (o.o_count, o.o_group, o.o_packch))?;

    inpack.fetch_sub(1, Ordering::Relaxed);
    if count > 1 && !all {
        last_pick.set(Some(id));
        dungeon.objects.with_object_mut(id, |o| o.o_count -= 1);
        if group != 0 {
            inpack.fetch_add(1, Ordering::Relaxed);
        }
        if newobj {
            let cloned = dungeon.objects.with(id, |thing| thing.clone())?;
            let copy = dungeon.objects.insert(cloned);
            dungeon.objects.with_object_mut(copy, |o| o.o_count = 1);
            Some(copy)
        } else {
            Some(id)
        }
    } else {
        last_pick.set(None);
        if (b'a'..=b'z').contains(&packch) {
            let mut used = pack_used.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            used[(packch - b'a') as usize] = 0;
        }
        dungeon.player.remove_from_pack(id);
        Some(id)
    }
}

pub unsafe fn pack_char(pack_used: &Mutex<[u8; 26]>) -> u8 {
    let mut used = pack_used.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    for i in 0..26 {
        if used[i] == 0 {
            used[i] = 1;
            return (b'a' + i as u8) as u8;
        }
    }
    b'a' as u8
}

pub unsafe fn inventory(
    items: &[ThingId],
    filter: ItemFilter,
    dungeon: &Dungeon,
    msg_esc: &std::sync::atomic::AtomicU8,
    n_objs: *mut i32,
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) -> u8 {
    *n_objs = 0;
    let any = filter == ItemFilter::Any;

    for &id in items {
        let Some((otyp, packch)) = with_object(&dungeon.objects, id, |o| (o.o_type, o.o_packch)) else {
            continue;
        };
        if !filter.matches(otyp) {
            continue;
        }

        *n_objs += 1;
        msg_esc.store(1, std::sync::atomic::Ordering::Relaxed);
        let format = if packch == 0 {
            "%s".to_string()
        } else {
            format!("{}) %s", packch as char)
        };
        let _ = add_line(&format, &inv_name_id(id, false, dungeon, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info));
        msg_esc.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    if *n_objs == 0 {
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

pub unsafe fn pick_up(
    ch: u8,
    dungeon: &Dungeon,
    inpack: &AtomicI32,
    move_on: u8,
    pack_used: &Mutex<[u8; 26]>,
    purse: &std::sync::atomic::AtomicI32,
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    let obj = floor_item(dungeon);
    if player_has(MonsterFlags::LEVIT, dungeon) {
        return;
    }
    if move_on != 0 {
        if let Some(id) = obj {
            move_msg_id(id, dungeon, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
        }
    } else {
        match ch as i32 {
            x if x == GOLD as i32 => {
                let Some(id) = obj else {
                    return;
                };
                money(with_object(&dungeon.objects, id, |o| o.o_arm).unwrap_or(0), dungeon, purse, terse);
                detach_floor(id, dungeon);
                discard_item(id, dungeon);
                if proom(dungeon).is_some() {
                    dungeon.with_level_mut(|level| level.set_room_goldval(proom(dungeon), 0));
                }
            }
            ARMOR | POTION | FOOD | WEAPON | SCROLL | AMULET | RING | STICK => {
                add_pack_id(None, false, dungeon, inpack, pack_used, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
            }
            _ => {}
        }
    }
}

/// Pointer-free version of `get_item`: resolves the player's selection to an
/// arena [`ThingId`] (or `None` when the player cancels / carries nothing).
pub unsafe fn get_item_id(
    purpose: &str,
    filter: ItemFilter,
    dungeon: &Dungeon,
    after: *mut u8,
    again: u8,
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: &crate::command_dispatch::CommandValue<Command>,
    last_dir: &crate::command_dispatch::CommandValue<Direction>,
    last_pick: &crate::command_dispatch::CommandValue<Option<ThingId>>,
    mpos: &std::sync::atomic::AtomicI32,
    msg_esc: &std::sync::atomic::AtomicU8,
    n_objs: *mut i32,
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) -> Option<ThingId> {
    let mut ch: i32;

    if dungeon.player.pack().is_empty() {
        msg_str("you aren't carrying anything");
        return None;
    }

    if again != 0 {
        if let Some(id) = last_pick.get() {
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
        mpos.store(0, std::sync::atomic::Ordering::Relaxed);
        if ch == ESCAPE {
            reset_last(l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick);
            *after = false as u8;
            msg_str("");
            return None;
        }
        *n_objs = 1;
        if ch == '*' as i32 {
            mpos.store(0, std::sync::atomic::Ordering::Relaxed);
            if inventory(&pack_ptrs(dungeon), filter, dungeon, msg_esc, n_objs, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info) == 0 {
                *after = false as u8;
                return None;
            }
            continue;
        }
        for id in dungeon.player.pack() {
            if crate::item::arena::with_object(&dungeon.objects, id, |o| o.o_packch) == Some(ch as u8) {
                return Some(id);
            }
        }
        msg_str(&format!(
            "'{}' is not a valid item",
            crate::ui::output::format_key(ch as u8)
        ));
    }
}

/// Adds gold atomically before drawing or invoking reentrant message helpers.
pub unsafe fn money(value: i32, dungeon: &Dungeon, purse: &std::sync::atomic::AtomicI32, terse: u8) {
    purse.fetch_add(value, std::sync::atomic::Ordering::Relaxed);
    // The gold object was discarded, so the terrain glyph shows via draw.
    crate::draw::write_cell_glyph(hero_coord(dungeon), floor_char_for_room(dungeon) as char);
    if value > 0 {
        if terse == 0 {
            addmsg_str("you found ");
        }
        msg_str(&format!("{} gold pieces", value));
    }
}

pub unsafe fn floor_ch(dungeon: &Dungeon) -> u8 {
    floor_char_for_room(dungeon)
}

pub unsafe fn floor_at(dungeon: &Dungeon) -> u8 {
    let ch = crate::entity::player::cell_glyph(hero_coord(dungeon).y, hero_coord(dungeon).x, dungeon);
    if ch == FLOOR {
        floor_char_for_room(dungeon)
    } else {
        ch
    }
}

pub unsafe fn reset_last(
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: &crate::command_dispatch::CommandValue<Command>,
    last_dir: &crate::command_dispatch::CommandValue<Direction>,
    last_pick: &crate::command_dispatch::CommandValue<Option<ThingId>>,
) {
    last_comm.set(l_last_comm);
    last_dir.set(l_last_dir);
    last_pick.set(l_last_pick);
}

/// Pointer-free core of `move_msg`: works on an arena handle.
pub unsafe fn move_msg_id(
    id: ThingId,
    dungeon: &Dungeon,
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    if terse == 0 {
        addmsg_str("you ");
    }
    msg_str(&format!("moved onto {}", inv_name_id(id, true, dungeon, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info)));
}

pub unsafe fn picky_inven(
    dungeon: &Dungeon,
    mpos: &std::sync::atomic::AtomicI32,
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    let pack = dungeon.player.pack();
    if pack.is_empty() {
        msg_str("you aren't carrying anything");
    } else if pack.len() == 1 {
        msg_str(&format!("a) {}", inv_name_id(pack[0], false, dungeon, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info)));
    } else {
        msg_str(if terse != 0 {
            "item: "
        } else {
            "which item do you wish to inventory: "
        });
        mpos.store(0, std::sync::atomic::Ordering::Relaxed);
        let mch = readchar() as u8;
        if mch as i32 == ESCAPE {
            msg_str("");
            return;
        }
        for id in &pack {
            if with_object(&dungeon.objects, *id, |o| o.o_packch) == Some(mch) {
                msg_str(&format!("{}) {}", mch as char, inv_name_id(*id, false, dungeon, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info)));
                return;
            }
        }
        msg_str(&format!(
            "'{}' not in pack",
            crate::ui::output::format_key(mch)
        ));
    }
}

unsafe fn pick_up_char(
    ch: u8,
    dungeon: &Dungeon,
    inpack: &AtomicI32,
    move_on: u8,
    pack_used: &Mutex<[u8; 26]>,
    purse: &std::sync::atomic::AtomicI32,
    terse: u8,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    scr_info: &[ObjInfo],
    weap_info: &[ObjInfo],
    ws_info: &[ObjInfo],
) {
    pick_up(ch, dungeon, inpack, move_on, pack_used, purse, terse, inv_describe, arm_info, pot_info, ring_info, scr_info, weap_info, ws_info);
}
