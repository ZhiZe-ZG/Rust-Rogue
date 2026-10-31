//! Scrolls and reading them.
//!
//! Ported from `src/c/scrolls.c` to Rust.
use crate::command::Command;
use crate::direction::Direction;
use crate::rnd::Random;

use crate::config::GameConfig;
use crate::draw::{look, map_cell_reveal};
use crate::dungeon::Dungeon;
use crate::entity::monsters::{new_monster_id, randmonster};
use crate::entity::player::{MonsterFlags, ObjectFlags, Thing};
use crate::game::globals::{MonsterInfo, ObjInfo};
use crate::init::pick_color;
use crate::item::arena::ThingId;
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::{get_item_id, leave_pack_id};
use crate::misc::{aggravate, call_it, choose_str};
use crate::ui::output::{addmsg_str, endmsg, msg_str, show_win};
use crate::ui::terminal::UiState;
use crate::wizard::{teleport, whatis};
use glam::IVec2;

const SLEEPTIME: i32 = 5;

const DOOR: i32 = '+' as i32;
const FLOOR: i32 = '.' as i32;
const PASSAGE: i32 = '#' as i32;
const TRAP: i32 = '^' as i32;
const STAIRS: i32 = '%' as i32;
const H_WALL: i32 = '-' as i32;
const V_WALL: i32 = '|' as i32;
const SPACE: i32 = ' ' as i32;
const FOOD: i32 = ':' as i32;

const F_PASS: u8 = 0x80u8 as u8;
const F_SEEN: u8 = 0x40u8 as u8;
const F_REAL: u8 = 0x10;

const MAXSCROLLS: usize = 18;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ScrollType {
    Confuse = 0,
    Map = 1,
    Hold = 2,
    Sleep = 3,
    Armor = 4,
    IdentifyPotion = 5,
    IdentifyScroll = 6,
    IdentifyWeapon = 7,
    IdentifyArmor = 8,
    IdentifyRingOrStick = 9,
    Scare = 10,
    FindFood = 11,
    Teleport = 12,
    Enchant = 13,
    CreateMonster = 14,
    RemoveCurse = 15,
    Aggravate = 16,
    Protect = 17,
}

impl ScrollType {
    /// Number of scroll kinds.
    pub const COUNT: usize = 18;

    #[inline]
    pub fn from_raw(value: i32) -> Self {
        Self::try_from_raw(value).unwrap_or_else(|| panic!("invalid scroll type: {value}"))
    }

    /// Build a scroll kind from its legacy index (`None` when out of range).
    #[inline]
    pub fn try_from_raw(value: i32) -> Option<Self> {
        Some(match value {
            0 => Self::Confuse,
            1 => Self::Map,
            2 => Self::Hold,
            3 => Self::Sleep,
            4 => Self::Armor,
            5 => Self::IdentifyPotion,
            6 => Self::IdentifyScroll,
            7 => Self::IdentifyWeapon,
            8 => Self::IdentifyArmor,
            9 => Self::IdentifyRingOrStick,
            10 => Self::Scare,
            11 => Self::FindFood,
            12 => Self::Teleport,
            13 => Self::Enchant,
            14 => Self::CreateMonster,
            15 => Self::RemoveCurse,
            16 => Self::Aggravate,
            17 => Self::Protect,
            _ => return None,
        })
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[inline]
fn hero(dungeon: &Dungeon) -> IVec2 {
    dungeon.player.pos()
}

#[inline]
fn proom(dungeon: &Dungeon) -> Option<usize> {
    dungeon.player.room()
}

#[inline]
fn player_has(flag: MonsterFlags, dungeon: &Dungeon) -> bool {
    dungeon.player.has_flag(flag)
}

// Map reveal now lives in `crate::draw::map_cell_reveal`, operating directly
// on the level's tile map and flag grids.

/// read_scroll:
/// Read a scroll from the pack and apply its effect.
///
/// # Safety
/// `scr_info` and `no_command` must remain valid throughout the call. The
/// metadata slices must cover their item kinds. Raw pointers avoid retaining
/// exclusive state borrows across helpers such as `whatis` and `call_it`.
pub(crate) unsafe fn read_scroll(
    random: &mut Random,
    dungeon: &Dungeon,
    ui: &UiState,
    scr_info: *mut [ObjInfo],
    weap_info: &[ObjInfo],
    no_command: *mut i32,
    terse: u8,
    after: *mut u8,
    again: u8,
    inpack: &std::sync::atomic::AtomicI32,
    l_last_comm: Command,
    l_last_dir: Direction,
    l_last_pick: Option<ThingId>,
    last_comm: *mut Command,
    last_dir: *mut Direction,
    last_pick: *mut Option<ThingId>,
    mpos: &std::sync::atomic::AtomicI32,
    msg_esc: &std::sync::atomic::AtomicU8,
    n_objs: *mut i32,
    pack_used: &std::sync::Mutex<[u8; 26]>,
    inv_describe: u8,
    arm_info: &[ObjInfo],
    pot_info: &[ObjInfo],
    ring_info: &[ObjInfo],
    ws_info: &[ObjInfo],
    monsters: *const [MonsterInfo; 26],
) {
    // Do not hold a shared borrow of the mutable scroll table across input.
    let scr_names = (&*scr_info).to_vec();
    let Some(obj) = get_item_id("read", ItemFilter::Category(ItemType::SCROLL), dungeon, after, again, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, mpos, msg_esc, n_objs, terse, inv_describe, arm_info, pot_info, ring_info, &scr_names, weap_info, ws_info) else {
        return;
    };

    if !matches!(
        dungeon.objects.with_object(obj, |o| o.o_type),
        Some(ItemType::Scroll(_))
    ) {
        if terse == 0 {
            msg_str("there is nothing on it to read");
        } else {
            msg_str("nothing to read");
        }
        return;
    }

    if dungeon.player.equipment().weapon_id() == Some(obj) {
        dungeon.player.set_weapon_id(None);
    }

    let (o_count, o_which) = dungeon.objects
        .with_object(obj, |o| (o.o_count, o.o_which))
        .unwrap_or((0, 0));
    let discardit = o_count == 1;
    leave_pack_id(obj, false, false, dungeon, inpack, last_pick, pack_used);
    let orig_obj = obj;

    let scroll_type = ScrollType::from_raw(o_which);
    match scroll_type {
        ScrollType::Confuse => {
            dungeon.player.add_flag(MonsterFlags::CANHUH);
            msg_str(&format!("your hands begin to glow {}", pick_color("red", random)));
        }
        ScrollType::Armor => {
            // Enchant the worn armor (pointer-free mutation via the arena).
            let enchanted = dungeon.player
                .with_armor_mut(|o| {
                    o.o_arm -= 1;
                    o.o_flags.remove(ObjectFlags::CURSED);
                })
                .is_some();
            if enchanted {
                msg_str(&format!(
                    "your armor glows {} for a moment",
                    pick_color("silver", random)
                ));
            }
        }
        ScrollType::Hold => {
            let mut ch: u8 = 0;
            let h = hero(dungeon);
            for x in (h.x - 2)..=(h.x + 2) {
                if !(0..GameConfig::SCREEN_COLS).contains(&x) {
                    continue;
                }
                for y in (h.y - 2)..=(h.y + 2) {
                    if y < 0 || y > (GameConfig::SCREEN_LINES - 1) {
                        continue;
                    }
                    if let Some(mid) = dungeon.monster_map.at(y as usize, x as usize) {
                        let running_held = dungeon.monster_list
                            .with_mut(mid, |t| {
                                if let Thing::Monster { data } = t {
                                    if data.t_flags.contains(MonsterFlags::RUN) {
                                        data.t_flags.remove(MonsterFlags::RUN);
                                        data.t_flags.insert(MonsterFlags::HELD);
                                        return true;
                                    }
                                }
                                false
                            })
                            .unwrap_or(false);
                        if running_held {
                            ch += 1;
                        }
                    }
                }
            }

            if ch != 0 {
                addmsg_str("the monster");
                if ch > 1 {
                    addmsg_str("s around you");
                }
                addmsg_str(" freeze");
                if ch == 1 {
                    addmsg_str("s");
                }
                endmsg();
                (&mut *scr_info)[ScrollType::Hold.index()].oi_know = true;
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
        ScrollType::Sleep => {
            (&mut *scr_info)[ScrollType::Sleep.index()].oi_know = true;
            *no_command += random.rnd(SLEEPTIME) + 4;
            dungeon.player.remove_flag(MonsterFlags::RUN);
            msg_str("you fall asleep");
        }
        ScrollType::CreateMonster => {
            let mut i = 0;
            let mut mp = IVec2 { y: 0, x: 0 };
            let h = hero(dungeon);
            for y in (h.y - 1)..=(h.y + 1) {
                for x in (h.x - 1)..=(h.x + 1) {
                    if y == h.y && x == h.x {
                        continue;
                    }
                    if dungeon.monster_map.at(y as usize, x as usize).is_some()
                        || !dungeon.with_level(|level| level.tile_at(y as usize, x as usize).is_walkable())
                    {
                        continue;
                    }
                    let floor_object = dungeon.with_level(|level| {
                        level.items.iter().copied().find(|&id| {
                            dungeon.objects.with_object(id, |object| object.o_pos == IVec2::new(x, y))
                                .unwrap_or(false)
                        })
                    });
                    let is_scare = floor_object
                        .and_then(|id| crate::item::arena::with_object(&dungeon.objects, id, |data| data.o_type))
                        .is_some_and(|t| matches!(t, ItemType::Scroll(ScrollType::Scare)));
                    if is_scare {
                        continue;
                    }
                    i += 1;
                    if random.rnd(i) == 0
                    {
                        mp.y = y;
                        mp.x = x;
                    }
                }
            }

            if i == 0 {
                msg_str("you hear a faint cry of anguish in the distance");
            } else {
                let id = dungeon.monster_list.spawn_actor();
                new_monster_id(id, randmonster(false, random, dungeon), mp,
                    random, dungeon, monsters);
            }
        }
        ScrollType::IdentifyPotion
        | ScrollType::IdentifyScroll
        | ScrollType::IdentifyWeapon
        | ScrollType::IdentifyArmor
        | ScrollType::IdentifyRingOrStick => {
            let id_filter: [ItemFilter; ScrollType::IdentifyRingOrStick.index() + 1] = [
                ItemFilter::Any,
                ItemFilter::Any,
                ItemFilter::Any,
                ItemFilter::Any,
                ItemFilter::Any,
                ItemFilter::Category(ItemType::POTION),
                ItemFilter::Category(ItemType::SCROLL),
                ItemFilter::Category(ItemType::WEAPON),
                ItemFilter::Category(ItemType::ARMOR),
                ItemFilter::RingOrStick,
            ];
            (&mut *scr_info)[o_which as usize].oi_know = true;
            msg_str(&format!(
                "this scroll is an {} scroll",
                (&*scr_info)[o_which as usize].oi_name
            ));
            whatis(true as u8, id_filter[o_which as usize]);
        }
        ScrollType::Map => {
            (&mut *scr_info)[ScrollType::Map.index()].oi_know = true;
            msg_str("oh, now this scroll has a map on it");

            for y in 1..(GameConfig::SCREEN_LINES - 1) {
                for x in 0..GameConfig::SCREEN_COLS {
                    let ch = map_cell_reveal(y, x);
                    if ch != SPACE {
                        let has_monster = dungeon.monster_map.at(y as usize, x as usize);
                        if let Some(mid) = has_monster {
                            dungeon.monster_list.with_mut(mid, |t| {
                                if let Thing::Monster { data } = t {
                                    data.t_oldch = ch as u8;
                                }
                            });
                        }
                        if has_monster.is_none() || !player_has(MonsterFlags::SEEMONST, dungeon) {
                            crate::draw::write_cell_glyph(IVec2::new(x, y), ch as u8 as char);
                        }
                    }
                }
            }
        }
        ScrollType::FindFood => {
            let mut found = false as u8;
            ui.clear();
            let floor = dungeon.with_level(|level| {
                level.items.iter().copied()
                    .filter(|&id| dungeon.objects.contains(id))
                    .collect::<Vec<_>>()
            });
            for id in floor {
                let info = crate::item::arena::with_object(&dungeon.objects, id, |data| (data.o_type, data.o_pos));
                if let Some((otype, opos)) = info {
                    if matches!(otype, ItemType::Food) {
                        found = true as u8;
                        ui.move_cursor(IVec2::new(opos.x, opos.y));
                        crate::draw::write_cell_glyph(opos, FOOD as u8 as char);
                    }
                }
            }
            if found != 0 {
                (&mut *scr_info)[ScrollType::FindFood.index()].oi_know = true;
                show_win("Your nose tingles and you smell food.--More--");
            } else {
                msg_str("your nose tingles");
            }
        }
        ScrollType::Teleport => {
            let cur_room = proom(dungeon);
            teleport(random);
            if cur_room != proom(dungeon) {
                (&mut *scr_info)[ScrollType::Teleport.index()].oi_know = true;
            }
        }
        ScrollType::Enchant => {
            // Enchant the wielded weapon; only a real weapon qualifies.
            let weapon_name = dungeon.player
                .with_weapon_mut(|o| {
                    if !matches!(o.o_type, ItemType::Weapon(_)) {
                        return None;
                    }
                    o.o_flags.remove(ObjectFlags::CURSED);
                    if random.rnd(2) == 0
                    {
                        o.o_hplus += 1;
                    } else {
                        o.o_dplus += 1;
                    }
                    Some(weap_info[o.o_which as usize].oi_name)
                })
                .flatten();
            match weapon_name {
                Some(name) => msg_str(&format!(
                    "your {} glows {} for a moment",
                    name,
                    pick_color("blue", random)
                )),
                None => msg_str("you feel a strange sense of loss"),
            };
        }
        ScrollType::Scare => {
            msg_str("you hear maniacal laughter in the distance");
        }
        ScrollType::RemoveCurse => {
            // Lift the curse from every equipped object (pointer-free).
            dungeon.player.with_armor_mut(|o| o.o_flags.remove(ObjectFlags::CURSED));
            dungeon.player.with_weapon_mut(|o| o.o_flags.remove(ObjectFlags::CURSED));
            dungeon.player.with_ring_mut(0, |o| o.o_flags.remove(ObjectFlags::CURSED));
            dungeon.player.with_ring_mut(1, |o| o.o_flags.remove(ObjectFlags::CURSED));
            msg_str(choose_str(
                "you feel in touch with the Universal Onenes",
                "you feel as if somebody is watching over you",
            ));
        }
        ScrollType::Aggravate => {
            aggravate(random);
            msg_str("you hear a high pitched humming noise");
        }
        ScrollType::Protect => {
            // Mark the worn armor as protected (pointer-free).
            let protected = dungeon.player
                .with_armor_mut(|o| o.o_flags.insert(ObjectFlags::PROT))
                .is_some();
            if protected {
                msg_str(&format!(
                    "your armor is covered by a shimmering {} shield",
                    pick_color("gold", random)
                ));
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
    }

    let _ = orig_obj;
    look(true as u8, random);
    // The naming prompt may reenter the table. Give it a local value and
    // preserve any knowledge or guess installed during that prompt.
    let mut info = (&*scr_info)[o_which as usize].clone();
    call_it(&mut info);
    let current = &mut (&mut *scr_info)[o_which as usize];
    if current.oi_know {
        current.oi_guess = None;
    } else if current.oi_guess.is_none() {
        current.oi_guess = info.oi_guess;
    }
    if discardit {
        let _ = dungeon.objects.remove(obj);
    }
}

/// Uncurse an item.
pub unsafe fn uncurse_id(id: ThingId, dungeon: &Dungeon) {
    dungeon.objects.with_object_mut(id, |o| o.o_flags.remove(ObjectFlags::CURSED));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncurse_only_changes_the_supplied_dungeon() {
        let dungeon = Dungeon::empty();
        let other = Dungeon::empty();
        let id = dungeon.objects.new_item_id();
        let other_id = other.objects.new_item_id();
        assert_eq!(id, other_id);
        let flags = ObjectFlags::CURSED | ObjectFlags::PROT;
        dungeon.objects.with_object_mut(id, |o| o.o_flags = flags);
        other.objects.with_object_mut(other_id, |o| o.o_flags = flags);

        unsafe { uncurse_id(id, &dungeon); }

        assert_eq!(dungeon.objects.with_object(id, |o| o.o_flags), Some(ObjectFlags::PROT));
        assert_eq!(other.objects.with_object(other_id, |o| o.o_flags), Some(flags));
        dungeon.objects.remove(id);
        unsafe { uncurse_id(id, &dungeon); }
    }
}
