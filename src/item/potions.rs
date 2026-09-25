//! Potions and quaffing.
//!
//! Ported from `src/c/potions.c` to Rust.
use crate::rnd::rnd;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint, c_void};
use std::ptr;

use crate::daemon::{fuse, lengthen, start_daemon};
use crate::daemons::{come_down, land, sight, unconfuse, unsee, visuals};
use crate::draw::look;

use crate::entity::chase::see_monst;
use crate::game::MONSTER_LIST;
use crate::entity::player::{Stats, Thing, ThingMonster, ThingObject, MonsterFlags, ObjectFlags};
use crate::game::PLAYER;
use crate::globals::pot_info;
use crate::item::pack::{get_item, leave_pack};
use crate::item::rings::RingType;
use crate::item::thing_list::discard;
use crate::misc::{add_haste, add_str, call_it, check_level, chg_str, choose_str, spread};
use crate::startup::roll;
use crate::ui::output::{self, endmsg, msg_str, show_win, status};
use crate::ui::Window;
use glam::IVec2;
use std::ffi::CStr;

/// Potion and status-effect handling for the Rust FFI bridge.
/// These helpers implement the C-side potion logic so the game can call
/// them through exported C entry points.
const POTION: c_int = '!' as c_int;
const SCROLL: c_int = '?' as c_int;
const WEAPON: c_int = ')' as c_int;
const ARMOR: c_int = ']' as c_int;
const RING: c_int = '=' as c_int;
const STICK: c_int = '/' as c_int;
const AMULET: c_int = ',' as c_int;
const FOOD: c_int = ':' as c_int;
const MAGIC: c_int = '$' as c_int;
const STAIRS: c_int = '%' as c_int;
const FLOOR: c_int = '.' as c_int;
const PASSAGE: c_int = '#' as c_int;
const SPACE: c_int = ' ' as c_int;
const H_WALL: c_int = '-' as c_int;
const V_WALL: c_int = '|' as c_int;
const TRAP: c_int = '^' as c_int;

const MAXPOTIONS: usize = 14;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum PotionType {
    Confuse = 0,
    Lsd = 1,
    Poison = 2,
    Strength = 3,
    SeeInvisible = 4,
    Healing = 5,
    MonsterFind = 6,
    TrapFind = 7,
    Raise = 8,
    ExtraHealing = 9,
    Haste = 10,
    Restore = 11,
    Blind = 12,
    Levitate = 13,
}

impl PotionType {
    #[inline]
    fn from_raw(value: c_int) -> Self {
        match value {
            0 => Self::Confuse,
            1 => Self::Lsd,
            2 => Self::Poison,
            3 => Self::Strength,
            4 => Self::SeeInvisible,
            5 => Self::Healing,
            6 => Self::MonsterFind,
            7 => Self::TrapFind,
            8 => Self::Raise,
            9 => Self::ExtraHealing,
            10 => Self::Haste,
            11 => Self::Restore,
            12 => Self::Blind,
            13 => Self::Levitate,
            _ => panic!("invalid potion type: {value}"),
        }
    }

    #[inline]
    const fn index(self) -> usize {
        self as usize
    }
}

const HUHDURATION: c_int = 20;
const SEEDURATION: c_int = 850;
const HEALTIME: c_int = 30;
const BEFORE: c_int = 1;
const AFTER: c_int = 2;

#[repr(C)]
struct PACT {
    pa_flags: c_short,
    pa_daemon: *const c_void,
    pa_time: c_int,
    pa_high: *const c_char,
    pa_straight: *const c_char,
}

/// External C symbols that provide game state, UI helpers, and gameplay
/// primitives used by the potion effects.
unsafe extern "C" {
    static mut terse: c_uchar;
    static mut after: c_uchar;
    static mut seenstairs: c_uchar;
    static mut fruit: [c_char; 1024];
    static mut prbuf: [c_char; 2048];
    static mut max_stats: Stats;
    static mut e_levels: [c_int; 21];

    fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
}

/// Cast a generic thing pointer to the monster portion of the union.
#[inline]
unsafe fn thing_t(tp: *mut Thing) -> *mut ThingMonster {
    crate::entity::player::thing_t(tp)
}

/// Cast a generic thing pointer to the object portion of the union.
#[inline]
unsafe fn thing_o(tp: *mut Thing) -> *mut ThingObject {
    crate::entity::player::thing_o(tp)
}

#[inline]
unsafe fn hero() -> IVec2 {
    (*thing_t(crate::game::player_ptr())).t_pos
}

#[inline]
unsafe fn player_has(flag: MonsterFlags) -> bool {
    (*thing_t(crate::game::player_ptr())).t_flags.contains(flag)
}

#[inline]
unsafe fn thing_has(tp: *mut Thing, flag: MonsterFlags) -> bool {
    (*thing_t(tp)).t_flags.contains(flag)
}

#[inline]
unsafe fn ring_is(ring: *mut Thing, ring_type: RingType) -> bool {
    !ring.is_null() && RingType::from_raw((*thing_o(ring)).o_which) == Some(ring_type)
}

#[inline]
unsafe fn next_thing(tp: *mut Thing) -> *mut Thing {
    crate::entity::player::thing_next(tp)
}

#[inline]
unsafe fn moat(y: c_int, x: c_int) -> *mut Thing {
    crate::game::monster_at(y, x)
}

#[inline]
unsafe fn is_magic_local(obj: *mut Thing) -> bool {
    match (*thing_o(obj)).o_type {
        ARMOR => (*thing_o(obj)).o_flags.contains(ObjectFlags::PROT) || (*thing_o(obj)).o_arm != 0,
        WEAPON => (*thing_o(obj)).o_hplus != 0 || (*thing_o(obj)).o_dplus != 0,
        POTION | SCROLL | STICK | RING | AMULET => true,
        _ => false,
    }
}

/// Shared implementation for potion effects that need the normal fuse/flag
/// setup and knowledge tracking used by the C version.
unsafe fn do_pot_impl(potion: PotionType, knowit: bool) {
    let (flags, daemon, base_time, high_msg, straight_msg) = {
        let taste_ptr = (&raw mut prbuf) as *mut [c_char; 2048] as *mut c_char as *const c_char;
        match potion {
            PotionType::Confuse => (
                MonsterFlags::HUH,
                unconfuse as *const c_void,
                HUHDURATION,
                c"what a tripy feeling!".as_ptr(),
                c"wait, what's going on here. Huh? What? Who?".as_ptr(),
            ),
            PotionType::Lsd => (
                MonsterFlags::HALU,
                come_down as *const c_void,
                SEEDURATION,
                c"Oh, wow!  Everything seems so cosmic!".as_ptr(),
                c"Oh, wow!  Everything seems so cosmic!".as_ptr(),
            ),
            PotionType::SeeInvisible => (
                MonsterFlags::CANSEE,
                unsee as *const c_void,
                SEEDURATION,
                taste_ptr,
                taste_ptr,
            ),
            PotionType::Blind => (
                MonsterFlags::BLIND,
                sight as *const c_void,
                SEEDURATION,
                c"oh, bummer!  Everything is dark!  Help!".as_ptr(),
                c"a cloak of darkness falls around you".as_ptr(),
            ),
            PotionType::Levitate => (
                MonsterFlags::LEVIT,
                land as *const c_void,
                HEALTIME,
                c"oh, wow!  You're floating in the air!".as_ptr(),
                c"you start to float in the air".as_ptr(),
            ),
            _ => (
                MonsterFlags::NONE,
                ptr::null(),
                0,
                ptr::null(),
                ptr::null(),
            ),
        }
    };

    (*pot_info.as_mut_ptr().add(potion.index())).oi_know = knowit;

    if flags.is_empty() || daemon.is_null() {
        return;
    }

    let t = spread(base_time);
    if !player_has(flags) {
        (*thing_t(crate::game::player_ptr())).t_flags.insert(flags);
        fuse(daemon, 0, t, AFTER);
        look(false as c_uchar);
    } else {
        lengthen(daemon, t);
    }
    msg_str(&CStr::from_ptr(choose_str(high_msg, straight_msg)).to_string_lossy());
}

/// quaff:
/// Quaff a potion from the pack.
#[no_mangle]
pub unsafe extern "C" fn quaff() {
    let obj = get_item(c"quaff".as_ptr(), POTION);
    let mut tp: *mut Thing;
    let mut mp: *mut Thing;
    let discardit;
    let mut show = false;
    let trip = player_has(MonsterFlags::HALU);

    if obj.is_null() {
        return;
    }
    if (*thing_o(obj)).o_type != POTION {
        if terse == 0 {
            msg_str("yuk! Why would you want to drink that?");
        } else {
            msg_str("that's undrinkable");
        }
        return;
    }
    if obj == PLAYER.weapon() {
        PLAYER.set_weapon(ptr::null_mut());
    }

    discardit = (*thing_o(obj)).o_count == 1;
    leave_pack(obj, false as c_uchar, false as c_uchar);

    let potion = PotionType::from_raw((*thing_o(obj)).o_which);
    match potion {
        PotionType::Confuse => do_pot_impl(
            PotionType::Confuse,
            if trip {
                false
            } else {
                true
            },
        ),
        PotionType::Poison => {
            (*pot_info.as_mut_ptr().add(PotionType::Poison.index())).oi_know = true;
            if ring_is(PLAYER.left_ring(), RingType::SustainStrength)
                || ring_is(PLAYER.right_ring(), RingType::SustainStrength)
            {
                msg_str("you feel momentarily sick");
            } else {
                chg_str(-(rnd(3) + 1));
                msg_str("you feel very sick now");
                come_down();
            }
        }
        PotionType::Healing => {
            let stats = thing_t(crate::game::player_ptr());
            (*pot_info.as_mut_ptr().add(PotionType::Healing.index())).oi_know = true;
            (*stats).t_stats.hit_points += roll((*stats).t_stats.level, 4);
            if (*stats).t_stats.hit_points > (*stats).t_stats.max_hit_points {
                (*stats).t_stats.max_hit_points += 1;
                (*stats).t_stats.hit_points = (*stats).t_stats.max_hit_points;
            }
            sight();
            msg_str("you begin to feel better");
        }
        PotionType::Strength => {
            (*pot_info.as_mut_ptr().add(PotionType::Strength.index())).oi_know = true;
            chg_str(1);
            msg_str("you feel stronger, now.  What bulging muscles!");
        }
        PotionType::MonsterFind => {
            (*thing_t(crate::game::player_ptr()))
                .t_flags
                .insert(MonsterFlags::SEEMONST);
            fuse(
                turn_see as *const c_void,
                true as c_uchar as c_int,
                HUHDURATION,
                AFTER,
            );
            if turn_see(false as c_uchar) == 0 {
                msg_str(&format!(
                    "you have a {} feeling for a moment, then it passes",
                    CStr::from_ptr(choose_str(c"normal".as_ptr(), c"strange".as_ptr()))
                        .to_string_lossy()
                ));
            }
        }
        PotionType::TrapFind => {
            let head = crate::game::with_current_level(|level| level.items.head());
            if !head.is_null() {
                let window = Window::Stdscr;
                output::clear_window(window);
                tp = head;
                while !tp.is_null() {
                    if is_magic_local(tp) {
                        show = true;
                        output::move_window_cursor(
                            window,
                            IVec2::new((*thing_o(tp)).o_pos.x, (*thing_o(tp)).o_pos.y),
                        );
                        output::write_window_glyph(window, (MAGIC as u8) as char);
                        (*pot_info.as_mut_ptr().add(PotionType::TrapFind.index())).oi_know =
                            true;
                    }
                    tp = next_thing(tp);
                }
                for id in MONSTER_LIST.ids() {
                    if let Some(mp) = MONSTER_LIST.handle(id) {
                        tp = crate::entity::player::thing_pack(mp);
                        while !tp.is_null() {
                            if is_magic_local(tp) {
                                show = true;
                                output::move_window_cursor(
                                    window,
                                    IVec2::new((*thing_t(mp)).t_pos.x, (*thing_t(mp)).t_pos.y),
                                );
                                output::write_window_glyph(window, (MAGIC as u8) as char);
                            }
                            tp = next_thing(tp);
                        }
                    }
                }
            }
            if show {
                (*pot_info.as_mut_ptr().add(PotionType::TrapFind.index())).oi_know =
                    true;
                show_win("You sense the presence of magic on this level.--More--");
            } else {
                msg_str(&format!(
                    "you have a {} feeling for a moment, then it passes",
                    CStr::from_ptr(choose_str(c"normal".as_ptr(), c"strange".as_ptr()))
                        .to_string_lossy()
                ));
            }
        }
        PotionType::Lsd => {
            if !trip {
                if player_has(MonsterFlags::SEEMONST) {
                    turn_see(false as c_uchar);
                }
                start_daemon(visuals as *const c_void, 0, BEFORE);
                seenstairs = seen_stairs();
            }
            do_pot_impl(PotionType::Lsd, true);
        }
        PotionType::SeeInvisible => {
            let _ = snprintf(
                (&raw mut prbuf) as *mut [c_char; 2048] as *mut c_char,
                prbuf.len(),
                c"this potion tastes like %s juice".as_ptr(),
                fruit.as_ptr(),
            );
            show = player_has(MonsterFlags::CANSEE);
            do_pot_impl(PotionType::SeeInvisible, false);
            if !show {
                invis_on();
            }
            sight();
        }
        PotionType::Raise => {
            (*pot_info.as_mut_ptr().add(PotionType::Raise.index())).oi_know = true;
            msg_str("you suddenly feel much more skillful");
            raise_level();
        }
        PotionType::ExtraHealing => {
            let stats = thing_t(crate::game::player_ptr());
            (*pot_info.as_mut_ptr().add(PotionType::ExtraHealing.index())).oi_know =
                true;
            (*stats).t_stats.hit_points += roll((*stats).t_stats.level, 8);
            if (*stats).t_stats.hit_points > (*stats).t_stats.max_hit_points {
                if (*stats).t_stats.hit_points > (*stats).t_stats.max_hit_points + (*stats).t_stats.level + 1 {
                    (*stats).t_stats.max_hit_points += 1;
                }
                (*stats).t_stats.max_hit_points += 1;
                (*stats).t_stats.hit_points = (*stats).t_stats.max_hit_points;
            }
            sight();
            come_down();
            msg_str("you begin to feel much better");
        }
        PotionType::Haste => {
            (*pot_info.as_mut_ptr().add(PotionType::Haste.index())).oi_know = true;
            after = false as c_uchar;
            if add_haste(true) {
                msg_str("you feel yourself moving much faster");
            }
        }
        PotionType::Restore => {
            let stats = thing_t(crate::game::player_ptr());
            if ring_is(PLAYER.left_ring(), RingType::AddStrength) {
                add_str(
                    &mut (*stats).t_stats.strength,
                    -(*thing_o(PLAYER.left_ring())).o_arm,
                );
            }
            if ring_is(PLAYER.right_ring(), RingType::AddStrength) {
                add_str(
                    &mut (*stats).t_stats.strength,
                    -(*thing_o(PLAYER.right_ring())).o_arm,
                );
            }
            if (*stats).t_stats.strength < max_stats.strength {
                (*stats).t_stats.strength = max_stats.strength;
            }
            if ring_is(PLAYER.left_ring(), RingType::AddStrength) {
                add_str(
                    &mut (*stats).t_stats.strength,
                    (*thing_o(PLAYER.left_ring())).o_arm,
                );
            }
            if ring_is(PLAYER.right_ring(), RingType::AddStrength) {
                add_str(
                    &mut (*stats).t_stats.strength,
                    (*thing_o(PLAYER.right_ring())).o_arm,
                );
            }
            msg_str("hey, this tastes great.  It make you feel warm all over");
        }
        PotionType::Blind => do_pot_impl(PotionType::Blind, true),
        PotionType::Levitate => do_pot_impl(PotionType::Levitate, true),
        _ => {
            msg_str("what an odd tasting potion!");
            return;
        }
    }

    status();
    call_it(&mut pot_info[(*thing_o(obj)).o_which as usize]);
    if discardit {
        discard(obj);
    }
}

/// is_magic:
/// Returns true if an object radiates magic.
#[no_mangle]
pub unsafe extern "C" fn is_magic(obj: *mut Thing) -> c_uchar {
    if obj.is_null() {
        return 0;
    }
    if is_magic_local(obj) {
        1
    } else {
        0
    }
}

/// invis_on:
/// Turn on the ability to see invisible.
#[no_mangle]
pub unsafe extern "C" fn invis_on() {
    (*thing_t(crate::game::player_ptr()))
        .t_flags
        .insert(MonsterFlags::CANSEE);
    for id in MONSTER_LIST.ids() {
        if let Some(mp) = MONSTER_LIST.handle(id) {
            if thing_has(mp, MonsterFlags::INVIS)
                && see_monst(mp) != 0
                && !player_has(MonsterFlags::HALU)
            {
                output::write_glyph_at(
                    IVec2::new((*thing_t(mp)).t_pos.x, (*thing_t(mp)).t_pos.y),
                    ((*thing_t(mp)).t_disguise as u8) as char,
                );
            }
        }
    }
}

/// turn_see:
/// Put on or off seeing monsters on this level.
#[no_mangle]
pub unsafe extern "C" fn turn_see(turn_off: c_uchar) -> c_uchar {
    let mut add_new = 0;

    for id in MONSTER_LIST.ids() {
        if let Some(mp) = MONSTER_LIST.handle(id) {
            output::move_cursor(IVec2::new((*thing_t(mp)).t_pos.x, (*thing_t(mp)).t_pos.y));
            let can_see = see_monst(mp) != 0;
            if turn_off != 0 {
                if !can_see {
                    output::write_glyph(((*thing_t(mp)).t_oldch as u8) as char);
                }
            } else {
                if !can_see {
                    output::set_standout(true);
                }
                if !player_has(MonsterFlags::HALU) {
                    output::write_glyph(((*thing_t(mp)).t_type as u8) as char);
                } else {
                    output::write_glyph((rnd(26) as u8 + b'A') as char);
                }
                if !can_see {
                    output::set_standout(false);
                    add_new += 1;
                }
            }
        }
    }

    if turn_off != 0 {
        (*thing_t(crate::game::player_ptr()))
            .t_flags
            .remove(MonsterFlags::SEEMONST);
    } else {
        (*thing_t(crate::game::player_ptr()))
            .t_flags
            .insert(MonsterFlags::SEEMONST);
    }

    if add_new != 0 {
        1
    } else {
        0
    }
}

/// seen_stairs:
/// Return true if the player has seen the stairs.
#[no_mangle]
pub unsafe extern "C" fn seen_stairs() -> c_uchar {
    let tp: *mut Thing;
    let stairs = crate::game::stairs();

    output::move_cursor(IVec2::new(stairs.x, stairs.y));
    if output::glyph_at_cursor() as c_int == STAIRS {
        return 1;
    }
    if hero().x == stairs.x && hero().y == stairs.y {
        return 1;
    }

    tp = moat(stairs.y, stairs.x);
    if !tp.is_null() {
        if see_monst(tp) != 0 && thing_has(tp, MonsterFlags::RUN) {
            return 1;
        }
        if player_has(MonsterFlags::SEEMONST) && (*thing_t(tp)).t_oldch as c_int == STAIRS {
            return 1;
        }
    }

    0
}

/// raise_level:
/// The player just magically went up a level.
#[no_mangle]
pub unsafe extern "C" fn raise_level() {
    (*thing_t(crate::game::player_ptr())).t_stats.experience =
        e_levels[(*thing_t(crate::game::player_ptr())).t_stats.level as usize - 1] + 1;
    check_level();
}

/// do_pot:
/// Do a potion with the standard fuse/flag setup.
unsafe fn do_pot(type_id: c_int, knowit: bool) {
    do_pot_impl(PotionType::from_raw(type_id), knowit);
}
