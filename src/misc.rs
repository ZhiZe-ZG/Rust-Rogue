//! Miscellaneous game routines.
//!
//! Ported from `src/c/misc.c` to Rust: gold, hunger, the floor map, and other
//! helpers shared across the game loop.
use crate::config::GameConfig;
use crate::daemon::{extinguish, fuse, Daemon};
use crate::entity::chase::runto;
use crate::game::PLAYER;
use crate::game::globals::ObjInfo;
use crate::item::item_type::{ItemFilter, ItemType};
use crate::item::arena::OBJECTS;
use crate::item::pack::{get_item_id, leave_pack_id, reset_last};
use crate::rnd::rnd;
use crate::ui::input::readchar;
use crate::ui::output::{addmsg_str, msg_str};
use glam::IVec2;

use crate::entity::player::{MonsterFlags, Thing};
use crate::game::MONSTER_LIST;
use crate::startup::roll;

const PASSAGE: u8 = b'#' as u8;
const DOOR: u8 = b'+' as u8;
const FLOOR: u8 = b'.' as u8;
const TRAP: u8 = b'^' as u8;
const STAIRS: u8 = b'%' as u8;
const GOLD: u8 = b'*' as u8;
const POTION: u8 = b'!' as u8;
const SCROLL: u8 = b'?' as u8;
const MAGIC: u8 = b'$' as u8;
const FOOD: u8 = b':' as u8;
const WEAPON: u8 = b')' as u8;
const ARMOR: u8 = b']' as u8;
const AMULET: u8 = b',' as u8;
const RING: u8 = b'=' as u8;
const STICK: u8 = b'/' as u8;

const F_PASS: u8 = 0x80u8 as u8;
const MAXSTR: usize = 1024;
const HUNGERTIME: i32 = 1300;
const STOMACHSIZE: i32 = 2000;
const AFTER: i32 = 2;
const ESCAPE: i32 = 27;
const NORM: i32 = 0;
const F_SEEN: u8 = 0x40;

use crate::game::globals::{after, again, amulet, delta, dir_ch, door_stop, e_levels, firstmove, food_left, hungry_state, jump, last_dir, max_stats, mpos, no_command, no_move, oldpos, passgo, runch, running, see_floor, seenstairs, terse};


#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    PLAYER.has_flag(flag)
}

#[inline]
#[allow(dead_code)]
fn first_is_vowel(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    matches!(
        bytes[0],
        b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U'
    )
}

/// show_floor:
/// Returns whether the floor of the player's room should be displayed.
pub unsafe fn show_floor() -> bool {
    let player_room = PLAYER.room();
    if crate::game::room_dark(player_room)
        && !crate::game::room_gone(player_room)
        && !player_has(MonsterFlags::BLIND)
    {
        return see_floor != 0;
    }
    true
}

/// The floor object at `(y, x)`, as an arena handle.
pub fn find_obj_id(y: i32, x: i32) -> Option<crate::item::arena::ThingId> {
    for id in crate::game::item_ids() {
        let hit = crate::item::arena::with_object(id, |data| {
            data.o_pos.y == y && data.o_pos.x == x
        })
        .unwrap_or(false);
        if hit {
            return Some(id);
        }
    }
    None
}

pub unsafe fn eat() {
    let Some(obj) = get_item_id("eat", ItemFilter::Category(ItemType::Food)) else {
        return;
    };
    if !matches!(OBJECTS.with_object(obj, |o| o.o_type), Some(ItemType::Food)) {
        if terse == 0 {
            msg_str("ugh, you would get ill if you ate that");
        } else {
            msg_str("that's Inedible!");
        }
        return;
    }

    if food_left < 0 {
        food_left = 0;
    }
    food_left += HUNGERTIME - 200 + rnd(400);
    if food_left > STOMACHSIZE {
        food_left = STOMACHSIZE;
    }
    hungry_state = 0;
    if PLAYER.equipment().weapon_id() == Some(obj) {
        PLAYER.set_weapon_id(None);
    }
    let which = OBJECTS.with_object(obj, |o| o.o_which).unwrap_or(0);
    if which == 1 {
        msg_str(&format!("my, that was a yummy {}", crate::game::globals::fruit()));
    } else if rnd(100) > 70 {
        PLAYER.with_stats_mut(|stats| stats.experience += 1);
        msg_str("bummer, this food tastes awful");
    } else {
        msg_str("yum, that tasted good");
    }
    leave_pack_id(obj, false, false);
}

pub unsafe fn check_level() {
    let experience = PLAYER.stats().experience;
    let mut i: i32 = 0;
    while e_levels[i as usize] != 0 {
        if e_levels[i as usize] > experience {
            break;
        }
        i += 1;
    }
    i += 1;
    let olevel = PLAYER.level();
    PLAYER.with_stats_mut(|stats| stats.level = i);
    if i > olevel {
        let add = roll(i - olevel, 10);
        PLAYER.with_stats_mut(|stats| {
            stats.max_hit_points += add;
            stats.hit_points += add;
        });
        msg_str(&format!("welcome to level {}", i));
    }
}

pub unsafe fn chg_str(amt: i32) {
    if amt == 0 {
        return;
    }
    let mut new_strength = PLAYER.stats().strength as i32 + amt;
    if new_strength < 3 {
        new_strength = 3;
    } else if new_strength > 31 {
        new_strength = 31;
    }
    PLAYER.with_stats_mut(|stats| stats.strength = new_strength as u32);
    let mut comp = PLAYER.stats().strength;

    // Subtract each worn ring's strength bonus (pointer-free read of `o_arm`).
    for hand in 0..2usize {
        if let Some(bonus) = PLAYER.equipment().ring_arm(hand) {
            let reduced = comp as i32 - bonus;
            comp = if reduced < 3 { 3 } else { reduced as u32 };
        }
    }
    if comp > max_stats.strength {
        max_stats.strength = comp;
    }
}

pub fn add_str(sp: &mut u32, amt: i32) {
    let newv = (*sp).wrapping_add(amt as u32);
    if newv < 3 {
        *sp = 3;
    } else if newv > 31 {
        *sp = 31;
    } else {
        *sp = newv;
    }
}

pub unsafe fn add_haste(potion: bool) -> bool {
    if player_has(MonsterFlags::HASTE) {
        no_command += rnd(8);
        PLAYER.remove_flag(MonsterFlags::RUN | MonsterFlags::HASTE);
        extinguish(Daemon::Nohaste);
        msg_str("you faint from exhaustion");
        return false;
    }

    PLAYER.add_flag(MonsterFlags::HASTE);
    if potion {
        fuse(Daemon::Nohaste, 0, rnd(4) + 4, AFTER);
    }
    true
}

pub unsafe fn aggravate() {
    for id in MONSTER_LIST.ids() {
        if let Some(pos) = MONSTER_LIST
            .with(id, |t| match t {
                Thing::Monster { data } => Some(data.t_pos),
                Thing::Object { .. } => None,
            })
            .flatten()
        {
            runto(pos);
        }
    }
}

/// Whether the object `id` is currently equipped (worn/wielded), announcing so
/// when it is.
pub fn is_current_id(id: crate::item::arena::ThingId) -> bool {
    let eq = PLAYER.equipment();
    if eq.armor_id() == Some(id)
        || eq.weapon_id() == Some(id)
        || eq.left_ring_id() == Some(id)
        || eq.right_ring_id() == Some(id)
    {
        unsafe {
            if terse == 0 {
                addmsg_str("That's already ");
            }
            msg_str("in use");
        }
        return true;
    }
    false
}

pub unsafe fn get_dir() -> u8 {
    let mut gotit: bool;
    let mut last_delt: IVec2 = IVec2 { x: 0, y: 0 };

    if again != 0 && last_dir != crate::direction::Direction::None {
        delta.y = last_delt.y;
        delta.x = last_delt.x;
        dir_ch = last_dir;
    } else {
        if terse == 0 {
            msg_str("which direction? ");
        }
        loop {
            gotit = true;
            let key = readchar() as u8;
            dir_ch = match crate::direction::Direction::from_byte(key) {
                Some(crate::direction::Direction::West) => {
                    delta.y = 0;
                    delta.x = -1;
                    crate::direction::Direction::West
                }
                Some(crate::direction::Direction::South) => {
                    delta.y = 1;
                    delta.x = 0;
                    crate::direction::Direction::South
                }
                Some(crate::direction::Direction::North) => {
                    delta.y = -1;
                    delta.x = 0;
                    crate::direction::Direction::North
                }
                Some(crate::direction::Direction::East) => {
                    delta.y = 0;
                    delta.x = 1;
                    crate::direction::Direction::East
                }
                Some(crate::direction::Direction::NorthWest) => {
                    delta.y = -1;
                    delta.x = -1;
                    crate::direction::Direction::NorthWest
                }
                Some(crate::direction::Direction::NorthEast) => {
                    delta.y = -1;
                    delta.x = 1;
                    crate::direction::Direction::NorthEast
                }
                Some(crate::direction::Direction::SouthWest) => {
                    delta.y = 1;
                    delta.x = -1;
                    crate::direction::Direction::SouthWest
                }
                Some(crate::direction::Direction::SouthEast) => {
                    delta.y = 1;
                    delta.x = 1;
                    crate::direction::Direction::SouthEast
                }
                _ if key as i32 == ESCAPE => {
                    last_dir = crate::direction::Direction::None;
                    reset_last();
                    return false as u8;
                }
                _ => {
                    mpos = 0;
                    msg_str("which direction? ");
                    gotit = false;
                    crate::direction::Direction::None
                }
            };
            if gotit {
                break;
            }
        }
        last_dir = dir_ch;
        last_delt.y = delta.y;
        last_delt.x = delta.x;
    }

    if player_has(MonsterFlags::HUH) && rnd(5) == 0 {
        loop {
            delta.y = rnd(3) - 1;
            delta.x = rnd(3) - 1;
            if !(delta.y == 0 && delta.x == 0) {
                break;
            }
        }
    }
    mpos = 0;
    true as u8
}

pub unsafe fn sign(nm: i32) -> i32 {
    if nm < 0 {
        -1
    } else if nm > 0 {
        1
    } else {
        0
    }
}

pub unsafe fn spread(nm: i32) -> i32 {
    nm - nm / 20 + rnd(nm / 10)
}

pub unsafe fn call_it(info: &mut ObjInfo) {
    if info.oi_know {
        info.oi_guess = None;
    } else if info.oi_guess.is_none() {
        if terse != 0 {
            msg_str("call it: ");
        } else {
            msg_str("what do you want to call it? ");
        }
        if let Some(text) = crate::options::read_line("") {
            info.oi_guess = Some(text);
        }
    }
}

pub unsafe fn rnd_thing() -> u8 {
    let thing_list = [
        POTION, SCROLL, RING, STICK, FOOD, WEAPON, ARMOR, STAIRS, GOLD, AMULET,
    ];
    let idx = if crate::game::current_depth() >= GameConfig::AMULET_LEVEL {
        rnd(thing_list.len() as i32)
    } else {
        rnd((thing_list.len() - 1) as i32)
    };
    thing_list[idx as usize]
}

/// Return `ts` when the player is hallucinating, otherwise `ns`.
pub fn choose_str(ts: &'static str, ns: &'static str) -> &'static str {
    if player_has(MonsterFlags::HALU) {
        ts
    } else {
        ns
    }
}

/// Returns a newline string borrowed for the given argument.
#[allow(dead_code)]
fn vowelstr(s: &str) -> &'static str {
    if matches!(
        s.as_bytes().first(),
        Some(b'a' | b'A' | b'e' | b'E' | b'i' | b'I' | b'o' | b'O' | b'u' | b'U')
    ) {
        "n"
    } else {
        ""
    }
}
