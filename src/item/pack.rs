//! Pack and inventory management.
//!
//! Ported from `src/c/pack.c` to Rust.
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uchar};

use crate::entity::player::{MonsterFlags, ObjectFlags, Thing};
use crate::game::MONSTER_LIST;
use crate::item::scrolls::ScrollType;
use crate::entity::player::{detach, discard};
use crate::item::arena::new_item;
use crate::item::things::{add_line, inv_name};
use crate::misc::{find_obj, show_floor};
use crate::ui::input::readchar;
use crate::ui::output;
use crate::ui::output::{addmsg_str, endmsg, msg_str};
use glam::IVec2;

const MAXPACK: c_int = 23;
const MAXSTR: usize = 1024;
const PASSAGE: c_char = b'#' as c_char;
const FLOOR: c_char = b'.' as c_char;
const GOLD: c_char = b'*' as c_char;
const POTION: c_int = b'!' as c_int;
const SCROLL: c_int = b'?' as c_int;
const FOOD: c_int = b':' as c_int;
const WEAPON: c_int = b')' as c_int;
const ARMOR: c_int = b']' as c_int;
const AMULET: c_int = b',' as c_int;
const RING: c_int = b'=' as c_int;
const STICK: c_int = b'/' as c_int;
const CALLABLE: c_int = -1;
const R_OR_S: c_int = -2;
const ESCAPE: c_int = 27;

unsafe extern "C" {
    static mut after: c_uchar;
    static mut again: c_uchar;
    static mut amulet: c_uchar;
    static mut inpack: c_int;
    static mut last_comm: c_char;
    static mut l_last_comm: c_char;
    static mut last_dir: c_char;
    static mut l_last_dir: c_char;
    static mut last_pick: *mut Thing;
    static mut l_last_pick: *mut Thing;
    static mut move_on: c_uchar;
    static mut msg_esc: bool;
    static mut mpos: c_int;
    static mut n_objs: c_int;
    static mut pack_used: [c_uchar; 26];
    static mut purse: c_int;
    static mut terse: c_uchar;

}

unsafe fn thing_t(tp: *mut Thing) -> *mut crate::entity::player::ThingMonster {
    crate::entity::player::thing_t(tp)
}

unsafe fn thing_o(tp: *mut Thing) -> *mut crate::entity::player::ThingObject {
    crate::entity::player::thing_o(tp)
}

unsafe fn next_item(item: *mut Thing) -> *mut Thing {
    crate::entity::player::thing_next(item)
}

unsafe fn detach_list(head: *mut *mut Thing, item: *mut Thing) {
    detach(head, item);
}

unsafe fn prev_item(item: *mut Thing) -> *mut Thing {
    crate::entity::player::thing_prev(item)
}

unsafe fn discard_item(item: *mut Thing) {
    discard(item);
}

unsafe fn alloc_item() -> *mut Thing {
    new_item()
}

unsafe fn pack_head() -> *mut Thing {
    crate::game::PLAYER.pack()
}

unsafe fn set_pack_head(value: *mut Thing) {
    crate::game::PLAYER.set_pack(value);
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

unsafe fn floor_char_for_room() -> c_char {
    if crate::game::room_gone(proom()) {
        PASSAGE
    } else if show_floor() {
        FLOOR
    } else {
        b' ' as c_char
    }
}

#[no_mangle]
pub unsafe extern "C" fn add_pack(obj: *mut Thing, silent: c_uchar) {
    let mut item = obj;
    let mut from_floor = false as c_uchar;
    let mut op: *mut Thing;
    let mut lp: *mut Thing;

    if item.is_null() {
        item = find_obj(hero_coord().y, hero_coord().x);
        if item.is_null() {
            return;
        }
        from_floor = true as c_uchar;
    }

    if (*thing_o(item)).o_type == SCROLL as c_int
        && (*thing_o(item)).o_which == ScrollType::Scare as c_int
        && (*thing_o(item)).o_flags.contains(ObjectFlags::FOUND)
    {
        crate::game::with_current_level_mut(|level| level.items.detach(item));
        // The object is removed from `lvl_obj`, so the terrain glyph shows
        // automatically via draw.
        output::write_glyph_at(
            IVec2::new(hero_coord().x, hero_coord().y),
            (floor_char_for_room() as u8) as char,
        );
        discard_item(item);
        msg_str("the scroll turns to dust as you pick it up");
        return;
    }

    if pack_head().is_null() {
        set_pack_head(item);
        (*thing_o(item)).o_packch = pack_char() as u8;
        inpack += 1;
    } else {
        lp = std::ptr::null_mut();
        op = pack_head();
        while !op.is_null() {
            if (*thing_o(op)).o_type != (*thing_o(item)).o_type {
                lp = op;
            } else {
                while (*thing_o(op)).o_type == (*thing_o(item)).o_type
                    && (*thing_o(op)).o_which != (*thing_o(item)).o_which
                {
                    lp = op;
                    if next_item(op).is_null() {
                        break;
                    }
                    op = next_item(op);
                }
                if (*thing_o(op)).o_type == (*thing_o(item)).o_type
                    && (*thing_o(op)).o_which == (*thing_o(item)).o_which
                {
                    if ((*thing_o(op)).o_type == FOOD as c_int
                        || (*thing_o(op)).o_type == POTION as c_int
                        || (*thing_o(op)).o_type == SCROLL as c_int)
                    {
                        if pack_room(from_floor, item) == 0 {
                            return;
                        }
                        (*thing_o(op)).o_count += 1;
                        discard_item(item);
                        item = op;
                        lp = std::ptr::null_mut();
                        break;
                    }
                    if (*thing_o(item)).o_group != 0 {
                        lp = op;
                        while (*thing_o(op)).o_type == (*thing_o(item)).o_type
                            && (*thing_o(op)).o_which == (*thing_o(item)).o_which
                            && (*thing_o(op)).o_group != (*thing_o(item)).o_group
                        {
                            lp = op;
                            if next_item(op).is_null() {
                                break;
                            }
                            op = next_item(op);
                        }
                        if (*thing_o(op)).o_type == (*thing_o(item)).o_type
                            && (*thing_o(op)).o_which == (*thing_o(item)).o_which
                            && (*thing_o(op)).o_group == (*thing_o(item)).o_group
                        {
                            (*thing_o(op)).o_count += (*thing_o(item)).o_count;
                            inpack -= 1;
                            if pack_room(from_floor, item) == 0 {
                                return;
                            }
                            (*thing_o(op)).o_count += 1;
                            discard_item(item);
                            item = op;
                            lp = std::ptr::null_mut();
                            break;
                        }
                    } else {
                        lp = op;
                    }
                }
                break;
            }
            op = next_item(op);
        }

        if !lp.is_null() {
            if pack_room(from_floor, item) == 0 {
                return;
            }
            (*thing_o(item)).o_packch = pack_char() as u8;
            crate::entity::player::set_thing_next(item, next_item(lp));
            crate::entity::player::set_thing_prev(item, lp);
            if !next_item(lp).is_null() {
                crate::entity::player::set_thing_prev(next_item(lp), item);
            }
            crate::entity::player::set_thing_next(lp, item);
        }
    }

    (*thing_o(item)).o_flags.insert(ObjectFlags::FOUND);

    for id in MONSTER_LIST.ids() {
        if let Some(op) = MONSTER_LIST.handle(id) {
            if crate::entity::player::thing_dest(op) == &raw mut (*thing_o(item)).o_pos {
                let mut hero_pos = crate::game::PLAYER.pos();
                crate::entity::player::set_thing_dest(op, &raw mut hero_pos);
            }
        }
    }

    if (*thing_o(item)).o_type == AMULET as c_int {
        amulet = true as c_uchar;
    }

    if silent == 0 {
        if terse == 0 {
            addmsg_str("you now have ");
        }
        msg_str(&format!(
            "{} ({})",
            inv_name(item, if terse == 0 { 0 } else { 1 }),
            (*thing_o(item)).o_packch as char,
        ));
    }
}

#[no_mangle]
pub unsafe extern "C" fn pack_room(from_floor: c_uchar, obj: *mut Thing) -> c_uchar {
    if inpack + 1 > MAXPACK {
        if terse == 0 {
            addmsg_str("there's ");
        }
        addmsg_str("no room");
        if terse == 0 {
            addmsg_str(" in your pack");
        }
        endmsg();
        if from_floor != 0 {
            move_msg(obj);
        }
        inpack = MAXPACK;
        return false as c_uchar;
    }

    if from_floor != 0 {
        crate::game::with_current_level_mut(|level| level.items.detach(obj));
        // The object is removed from `lvl_obj`, so the terrain glyph shows
        // automatically via draw.
        output::write_glyph_at(
            IVec2::new(hero_coord().x, hero_coord().y),
            (floor_char_for_room() as u8) as char,
        );
    }

    inpack += 1;
    true as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn leave_pack(obj: *mut Thing, newobj: c_uchar, all: c_uchar) -> *mut Thing {
    let mut nobj = obj;

    inpack -= 1;
    if (*thing_o(obj)).o_count > 1 && all == 0 {
        last_pick = obj;
        (*thing_o(obj)).o_count -= 1;
        if (*thing_o(obj)).o_group != 0 {
            inpack += 1;
        }
        if newobj != 0 {
            nobj = alloc_item();
            *nobj = (*obj).clone();
            crate::entity::player::set_thing_next(nobj, std::ptr::null_mut());
            crate::entity::player::set_thing_prev(nobj, std::ptr::null_mut());
            (*thing_o(nobj)).o_count = 1;
        }
    } else {
        last_pick = std::ptr::null_mut();
        pack_used[(*thing_o(obj)).o_packch as usize - 'a' as usize] = false as c_uchar;
        crate::entity::player::detach_pack_from_player(obj);
    }
    nobj
}

#[no_mangle]
pub unsafe extern "C" fn pack_char() -> c_char {
    // `pack_used` is a 26-entry array (one slot per letter); index it directly so
    // no shared reference to the mutable static is created.
    for i in 0..26 {
        if pack_used[i] == 0 {
            pack_used[i] = true as c_uchar;
            return (b'a' + i as u8) as c_char;
        }
    }
    b'a' as c_char
}

#[no_mangle]
pub unsafe extern "C" fn inventory(list: *mut Thing, type_: c_int) -> c_uchar {
    let mut cur = list;
    n_objs = 0;

    while !cur.is_null() {
        if type_ != 0
            && type_ != (*thing_o(cur)).o_type
            && !(type_ == CALLABLE
                && (*thing_o(cur)).o_type != FOOD
                && (*thing_o(cur)).o_type != AMULET)
            && !(type_ == R_OR_S
                && ((*thing_o(cur)).o_type == RING || (*thing_o(cur)).o_type == STICK))
        {
            cur = next_item(cur);
            continue;
        }

        n_objs += 1;
        msg_esc = true;
        let format = if (*thing_o(cur)).o_packch == 0 {
            "%s".to_string()
        } else {
            format!("{}) %s", (*thing_o(cur)).o_packch as char)
        };
        let _ = add_line(&format, &inv_name(cur, false as c_uchar));
        msg_esc = false;
        cur = next_item(cur);
    }

    if n_objs == 0 {
        if terse != 0 {
            msg_str(if type_ == 0 {
                "empty handed"
            } else {
                "nothing appropriate"
            });
        } else {
            msg_str(if type_ == 0 {
                "you are empty handed"
            } else {
                "you don't have anything appropriate"
            });
        }
        return false as c_uchar;
    }

    true as c_uchar
}

#[no_mangle]
pub unsafe extern "C" fn pick_up(ch: c_char) {
    let obj = find_obj(hero_coord().y, hero_coord().x);
    if player_has(MonsterFlags::LEVIT) {
        return;
    }
    if move_on != 0 {
        if !obj.is_null() {
            move_msg(obj);
        }
    } else {
        match ch as c_int {
            x if x == GOLD as c_int => {
                if obj.is_null() {
                    return;
                }
                money((*thing_o(obj)).o_arm);
                crate::game::with_current_level_mut(|level| level.items.detach(obj));
                discard_item(obj);
                if proom().is_some() {
                    crate::game::set_room_goldval(proom(), 0);
                }
            }
            ARMOR | POTION | FOOD | WEAPON | SCROLL | AMULET | RING | STICK => {
                add_pack(std::ptr::null_mut(), false as c_uchar);
            }
            _ => {}
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_item(purpose: *const c_char, type_: c_int) -> *mut Thing {
    let mut ch: c_int;

    if pack_head().is_null() {
        msg_str("you aren't carrying anything");
        return std::ptr::null_mut();
    }

    if again != 0 {
        if !last_pick.is_null() {
            return last_pick;
        }
        msg_str("you ran out");
        return std::ptr::null_mut();
    }

    loop {
        if terse == 0 {
            addmsg_str("which object do you want to ");
        }
        addmsg_str(&CStr::from_ptr(purpose).to_string_lossy());
        if terse != 0 {
            addmsg_str(" what");
        }
        msg_str("? (* for list): ");
        ch = readchar();
        mpos = 0;
        if ch == ESCAPE {
            reset_last();
            after = false as c_uchar;
            msg_str("");
            return std::ptr::null_mut();
        }
        n_objs = 1;
        if ch == '*' as c_int {
            mpos = 0;
            if inventory(pack_head(), type_) == 0 {
                after = false as c_uchar;
                return std::ptr::null_mut();
            }
            continue;
        }
        let mut obj = pack_head();
        while !obj.is_null() {
            if (*thing_o(obj)).o_packch == ch as u8 {
                return obj;
            }
            obj = next_item(obj);
        }
        msg_str(&format!(
            "'{}' is not a valid item",
            output::format_key(ch as u8)
        ));
    }
}

#[no_mangle]
pub unsafe extern "C" fn money(value: c_int) {
    purse += value;
    // The gold object was discarded, so the terrain glyph shows via draw.
    output::write_glyph_at(
        IVec2::new(hero_coord().x, hero_coord().y),
        (floor_char_for_room() as u8) as char,
    );
    if value > 0 {
        if terse == 0 {
            addmsg_str("you found ");
        }
        msg_str(&format!("{} gold pieces", value));
    }
}

#[no_mangle]
pub unsafe extern "C" fn floor_ch() -> c_char {
    floor_char_for_room()
}

#[no_mangle]
pub unsafe extern "C" fn floor_at() -> c_char {
    let ch = crate::draw::cell_glyph(hero_coord().y, hero_coord().x);
    if ch == FLOOR {
        floor_char_for_room()
    } else {
        ch
    }
}

#[no_mangle]
pub unsafe extern "C" fn reset_last() {
    last_comm = l_last_comm;
    last_dir = l_last_dir;
    last_pick = l_last_pick;
}

#[no_mangle]
pub unsafe extern "C" fn move_msg(obj: *mut Thing) {
    if terse == 0 {
        addmsg_str("you ");
    }
    msg_str(&format!("moved onto {}", inv_name(obj, true as c_uchar)));
}

#[no_mangle]
pub unsafe extern "C" fn picky_inven() {
    if pack_head().is_null() {
        msg_str("you aren't carrying anything");
    } else if next_item(pack_head()).is_null() {
        msg_str(&format!("a) {}", inv_name(pack_head(), false as c_uchar)));
    } else {
        msg_str(if terse != 0 {
            "item: "
        } else {
            "which item do you wish to inventory: "
        });
        mpos = 0;
        let mch = readchar() as c_char;
        if mch as c_int == ESCAPE {
            msg_str("");
            return;
        }
        let mut obj = pack_head();
        while !obj.is_null() {
            if mch as u8 == (*thing_o(obj)).o_packch {
                msg_str(&format!(
                    "{}) {}",
                    mch as u8 as char,
                    inv_name(obj, false as c_uchar)
                ));
                return;
            }
            obj = next_item(obj);
        }
        msg_str(&format!("'{}' not in pack", output::format_key(mch as u8)));
    }
}

unsafe fn pick_up_char(ch: c_char) {
    pick_up(ch);
}
