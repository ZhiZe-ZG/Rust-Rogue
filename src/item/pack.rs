//! Pack and inventory management.
//!
//! Ported from `src/c/pack.c` to Rust.

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

const MAXPACK: i32 = 23;
const MAXSTR: usize = 1024;
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

use crate::globals::{after, again, amulet, inpack, l_last_comm, l_last_dir, l_last_pick, last_comm, last_dir, last_pick, move_on, mpos, msg_esc, n_objs, pack_used, purse, terse};


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

unsafe fn floor_char_for_room() -> u8 {
    if crate::game::room_gone(proom()) {
        PASSAGE
    } else if show_floor() {
        FLOOR
    } else {
        b' ' as u8
    }
}

pub unsafe fn add_pack(obj: *mut Thing, silent: u8) {
    let mut item = obj;
    let mut from_floor = false as u8;
    let mut op: *mut Thing;
    let mut lp: *mut Thing;

    if item.is_null() {
        item = find_obj(hero_coord().y, hero_coord().x);
        if item.is_null() {
            return;
        }
        from_floor = true as u8;
    }

    if (*thing_o(item)).o_type == SCROLL as i32
        && (*thing_o(item)).o_which == ScrollType::Scare as i32
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
                    if ((*thing_o(op)).o_type == FOOD as i32
                        || (*thing_o(op)).o_type == POTION as i32
                        || (*thing_o(op)).o_type == SCROLL as i32)
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

    if (*thing_o(item)).o_type == AMULET as i32 {
        amulet = true as u8;
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

pub unsafe fn pack_room(from_floor: u8, obj: *mut Thing) -> u8 {
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
        return false as u8;
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
    true as u8
}

pub unsafe fn leave_pack(obj: *mut Thing, newobj: u8, all: u8) -> *mut Thing {
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
        pack_used[(*thing_o(obj)).o_packch as usize - 'a' as usize] = false as u8;
        crate::entity::player::detach_pack_from_player(obj);
    }
    nobj
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

pub unsafe fn inventory(list: *mut Thing, type_: i32) -> u8 {
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
        msg_esc = 1;
        let format = if (*thing_o(cur)).o_packch == 0 {
            "%s".to_string()
        } else {
            format!("{}) %s", (*thing_o(cur)).o_packch as char)
        };
        let _ = add_line(&format, &inv_name(cur, false as u8));
        msg_esc = 0;
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
        return false as u8;
    }

    true as u8
}

pub unsafe fn pick_up(ch: u8) {
    let obj = find_obj(hero_coord().y, hero_coord().x);
    if player_has(MonsterFlags::LEVIT) {
        return;
    }
    if move_on != 0 {
        if !obj.is_null() {
            move_msg(obj);
        }
    } else {
        match ch as i32 {
            x if x == GOLD as i32 => {
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
                add_pack(std::ptr::null_mut(), false as u8);
            }
            _ => {}
        }
    }
}

pub unsafe fn get_item(purpose: &str, type_: i32) -> *mut Thing {
    let mut ch: i32;

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
            return std::ptr::null_mut();
        }
        n_objs = 1;
        if ch == '*' as i32 {
            mpos = 0;
            if inventory(pack_head(), type_) == 0 {
                after = false as u8;
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

pub unsafe fn money(value: i32) {
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

pub unsafe fn move_msg(obj: *mut Thing) {
    if terse == 0 {
        addmsg_str("you ");
    }
    msg_str(&format!("moved onto {}", inv_name(obj, true as u8)));
}

pub unsafe fn picky_inven() {
    if pack_head().is_null() {
        msg_str("you aren't carrying anything");
    } else if next_item(pack_head()).is_null() {
        msg_str(&format!("a) {}", inv_name(pack_head(), false as u8)));
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
        let mut obj = pack_head();
        while !obj.is_null() {
            if mch as u8 == (*thing_o(obj)).o_packch {
                msg_str(&format!(
                    "{}) {}",
                    mch as u8 as char,
                    inv_name(obj, false as u8)
                ));
                return;
            }
            obj = next_item(obj);
        }
        msg_str(&format!("'{}' not in pack", output::format_key(mch as u8)));
    }
}

unsafe fn pick_up_char(ch: u8) {
    pick_up(ch);
}
