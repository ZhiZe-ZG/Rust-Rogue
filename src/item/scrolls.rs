//! Scrolls and reading them.
//!
//! Ported from `src/c/scrolls.c` to Rust.
use crate::rnd::rnd;

use crate::config::GameConfig;
use crate::draw::{look, map_cell_reveal};
use crate::entity::monsters::{new_monster_id, randmonster};
use crate::entity::player::{MonsterFlags, ObjectFlags, Thing};
use crate::game;
use crate::game::globals::{scr_info, weap_info};
use crate::game::MONSTER_LIST;
use crate::game::PLAYER;
use crate::init::pick_color;
use crate::item::arena::{ThingId, OBJECTS};
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::pack::{get_item_id, leave_pack_id};
use crate::misc::{aggravate, call_it, choose_str};
use crate::ui::output::{addmsg_str, endmsg, msg_str, show_win};
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

use crate::game::globals::{no_command, terse};

#[inline]
fn hero() -> IVec2 {
    crate::game::PLAYER.pos()
}

#[inline]
fn proom() -> Option<usize> {
    crate::game::PLAYER.room()
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    crate::game::PLAYER.has_flag(flag)
}

// Map reveal now lives in `crate::draw::map_cell_reveal`, operating directly
// on the `CURRENT_LEVEL` tile map and flag grids.

/// read_scroll:
/// Read a scroll from the pack and apply its effect.
pub unsafe fn read_scroll() {
    let Some(obj) = get_item_id("read", ItemFilter::Category(ItemType::SCROLL)) else {
        return;
    };

    if !matches!(
        OBJECTS.with_object(obj, |o| o.o_type),
        Some(ItemType::Scroll(_))
    ) {
        if terse == 0 {
            msg_str("there is nothing on it to read");
        } else {
            msg_str("nothing to read");
        }
        return;
    }

    if PLAYER.equipment().weapon_id() == Some(obj) {
        PLAYER.set_weapon_id(None);
    }

    let (o_count, o_which) = OBJECTS
        .with_object(obj, |o| (o.o_count, o.o_which))
        .unwrap_or((0, 0));
    let discardit = o_count == 1;
    leave_pack_id(obj, false, false);
    let orig_obj = obj;

    let scroll_type = ScrollType::from_raw(o_which);
    match scroll_type {
        ScrollType::Confuse => {
            crate::game::PLAYER.add_flag(MonsterFlags::CANHUH);
            msg_str(&format!("your hands begin to glow {}", pick_color("red")));
        }
        ScrollType::Armor => {
            // Enchant the worn armor (pointer-free mutation via the arena).
            let enchanted = PLAYER
                .with_armor_mut(|o| {
                    o.o_arm -= 1;
                    o.o_flags.remove(ObjectFlags::CURSED);
                })
                .is_some();
            if enchanted {
                msg_str(&format!(
                    "your armor glows {} for a moment",
                    pick_color("silver")
                ));
            }
        }
        ScrollType::Hold => {
            let mut ch: u8 = 0;
            let h = hero();
            for x in (h.x - 2)..=(h.x + 2) {
                if !(0..GameConfig::SCREEN_COLS).contains(&x) {
                    continue;
                }
                for y in (h.y - 2)..=(h.y + 2) {
                    if y < 0 || y > (GameConfig::SCREEN_LINES - 1) {
                        continue;
                    }
                    if let Some(mid) = game::monster_id_at(y, x) {
                        let running_held = game::MONSTER_LIST
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
                scr_info[ScrollType::Hold.index()].oi_know = true;
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
        ScrollType::Sleep => {
            scr_info[ScrollType::Sleep.index()].oi_know = true;
            no_command += rnd(SLEEPTIME) + 4;
            crate::game::PLAYER.remove_flag(MonsterFlags::RUN);
            msg_str("you fall asleep");
        }
        ScrollType::CreateMonster => {
            let mut i = 0;
            let mut mp = IVec2 { y: 0, x: 0 };
            let h = hero();
            for y in (h.y - 1)..=(h.y + 1) {
                for x in (h.x - 1)..=(h.x + 1) {
                    if y == h.y && x == h.x {
                        continue;
                    }
                    if !crate::game::cell_is_walkable(y, x) {
                        continue;
                    }
                    let is_scare = crate::misc::find_obj_id(y, x)
                        .and_then(|id| crate::item::arena::with_object(id, |data| data.o_type))
                        .is_some_and(|t| matches!(t, ItemType::Scroll(ScrollType::Scare)));
                    if is_scare {
                        continue;
                    }
                    i += 1;
                    if rnd(i) == 0 {
                        mp.y = y;
                        mp.x = x;
                    }
                }
            }

            if i == 0 {
                msg_str("you hear a faint cry of anguish in the distance");
            } else {
                let id = MONSTER_LIST.spawn_actor();
                new_monster_id(id, randmonster(false), mp);
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
            scr_info[o_which as usize].oi_know = true;
            msg_str(&format!(
                "this scroll is an {} scroll",
                scr_info[o_which as usize].oi_name
            ));
            whatis(true as u8, id_filter[o_which as usize]);
        }
        ScrollType::Map => {
            scr_info[ScrollType::Map.index()].oi_know = true;
            msg_str("oh, now this scroll has a map on it");

            for y in 1..(GameConfig::SCREEN_LINES - 1) {
                for x in 0..GameConfig::SCREEN_COLS {
                    let ch = map_cell_reveal(y, x);
                    if ch != SPACE {
                        let has_monster = game::monster_id_at(y, x);
                        if let Some(mid) = has_monster {
                            game::MONSTER_LIST.with_mut(mid, |t| {
                                if let Thing::Monster { data } = t {
                                    data.t_oldch = ch as u8;
                                }
                            });
                        }
                        if has_monster.is_none() || !player_has(MonsterFlags::SEEMONST) {
                            crate::draw::write_cell_glyph(IVec2::new(x, y), ch as u8 as char);
                        }
                    }
                }
            }
        }
        ScrollType::FindFood => {
            let mut found = false as u8;
            crate::ui::terminal::clear();
            for id in crate::game::item_ids() {
                let info = crate::item::arena::with_object(id, |data| (data.o_type, data.o_pos));
                if let Some((otype, opos)) = info {
                    if matches!(otype, ItemType::Food) {
                        found = true as u8;
                        crate::ui::terminal::move_cursor(IVec2::new(opos.x, opos.y));
                        crate::draw::write_cell_glyph(opos, FOOD as u8 as char);
                    }
                }
            }
            if found != 0 {
                scr_info[ScrollType::FindFood.index()].oi_know = true;
                show_win("Your nose tingles and you smell food.--More--");
            } else {
                msg_str("your nose tingles");
            }
        }
        ScrollType::Teleport => {
            let cur_room = proom();
            teleport();
            if cur_room != proom() {
                scr_info[ScrollType::Teleport.index()].oi_know = true;
            }
        }
        ScrollType::Enchant => {
            // Enchant the wielded weapon; only a real weapon qualifies.
            let weapon_name = PLAYER
                .with_weapon_mut(|o| {
                    if !matches!(o.o_type, ItemType::Weapon(_)) {
                        return None;
                    }
                    o.o_flags.remove(ObjectFlags::CURSED);
                    if rnd(2) == 0 {
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
                    pick_color("blue")
                )),
                None => msg_str("you feel a strange sense of loss"),
            };
        }
        ScrollType::Scare => {
            msg_str("you hear maniacal laughter in the distance");
        }
        ScrollType::RemoveCurse => {
            // Lift the curse from every equipped object (pointer-free).
            PLAYER.with_armor_mut(|o| o.o_flags.remove(ObjectFlags::CURSED));
            PLAYER.with_weapon_mut(|o| o.o_flags.remove(ObjectFlags::CURSED));
            PLAYER.with_ring_mut(0, |o| o.o_flags.remove(ObjectFlags::CURSED));
            PLAYER.with_ring_mut(1, |o| o.o_flags.remove(ObjectFlags::CURSED));
            msg_str(choose_str(
                "you feel in touch with the Universal Onenes",
                "you feel as if somebody is watching over you",
            ));
        }
        ScrollType::Aggravate => {
            aggravate();
            msg_str("you hear a high pitched humming noise");
        }
        ScrollType::Protect => {
            // Mark the worn armor as protected (pointer-free).
            let protected = PLAYER
                .with_armor_mut(|o| o.o_flags.insert(ObjectFlags::PROT))
                .is_some();
            if protected {
                msg_str(&format!(
                    "your armor is covered by a shimmering {} shield",
                    pick_color("gold")
                ));
            } else {
                msg_str("you feel a strange sense of loss");
            }
        }
        _ => {}
    }

    let _ = orig_obj;
    look(true as u8);
    call_it(&mut scr_info[o_which as usize]);
    if discardit {
        let _ = OBJECTS.remove(obj);
    }
}

/// Uncurse an item.
pub unsafe fn uncurse_id(id: ThingId) {
    OBJECTS.with_object_mut(id, |o| o.o_flags.remove(ObjectFlags::CURSED));
}
