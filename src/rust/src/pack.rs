use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint};

use crate::draw::chat_at as draw_chat;
use crate::list::{_detach, discard, new_item};
use crate::player::{CRoom, CThing};

const TRUE: c_uchar = 1;
const FALSE: c_uchar = 0;
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
const S_SCARE: c_int = 10;
const ESCAPE: c_int = 27;
const ISFOUND: c_int = 0o0000020;
const ISGONE: c_short = 0o0000002;
const ISLEVIT: c_short = 0o0000010;
const ISDARK: c_short = 0o0000001;

unsafe extern "C" {
    static mut after: c_uchar;
    static mut again: c_uchar;
    static mut amulet: c_uchar;
    static mut inpack: c_int;
    static mut last_comm: c_char;
    static mut l_last_comm: c_char;
    static mut last_dir: c_char;
    static mut l_last_dir: c_char;
    static mut last_pick: *mut CThing;
    static mut l_last_pick: *mut CThing;
    static mut lvl_obj: *mut CThing;
    static mut mlist: *mut CThing;
    static mut move_on: c_uchar;
    static mut msg_esc: c_uchar;
    static mut mpos: c_int;
    static mut n_objs: c_int;
    static mut pack_used: [c_uchar; 26];
    static mut player: CThing;
    static mut purse: c_int;
    static mut terse: c_uchar;

    fn add_line(fmt: *mut c_char, arg: *mut c_char) -> c_char;
    fn addmsg(fmt: *const c_char, ...);
    fn endmsg() -> c_int;
    fn find_obj(y: c_int, x: c_int) -> *mut CThing;
    fn inv_name(obj: *mut CThing, drop: c_uchar) -> *mut c_char;
    fn msg(fmt: *const c_char, ...);
    fn mvaddch(y: c_int, x: c_int, ch: c_uint) -> c_int;
    fn readchar() -> c_int;
    fn show_floor() -> c_uchar;
    fn unctrl(ch: c_int) -> *mut c_char;
}

unsafe fn thing_t(tp: *mut CThing) -> *mut crate::player::CThingMonster {
    tp as *mut crate::player::CThingMonster
}

unsafe fn thing_o(tp: *mut CThing) -> *mut crate::player::CThingObject {
    tp as *mut crate::player::CThingObject
}

unsafe fn next_item(item: *mut CThing) -> *mut CThing {
    (*thing_t(item)).l_next
}

unsafe fn prev_item(item: *mut CThing) -> *mut CThing {
    (*thing_t(item)).l_prev
}

unsafe fn detach_list(head: *mut *mut CThing, item: *mut CThing) {
    let list = std::mem::transmute::<*mut *mut CThing, *mut *mut crate::list::CThing>(head);
    let ptr = std::mem::transmute::<*mut CThing, *mut crate::list::CThing>(item);
    crate::list::_detach(list, ptr);
}

unsafe fn discard_item(item: *mut CThing) {
    let ptr = std::mem::transmute::<*mut CThing, *mut crate::list::CThing>(item);
    crate::list::discard(ptr);
}

unsafe fn alloc_item() -> *mut CThing {
    let ptr = crate::list::new_item();
    std::mem::transmute::<*mut crate::list::CThing, *mut CThing>(ptr)
}

unsafe fn room_flags(rp: *mut CRoom) -> c_short {
    if rp.is_null() { 0 } else { (*rp).r_flags }
}

unsafe fn pack_head() -> *mut CThing {
    (*thing_t(&raw mut player)).t_pack
}

unsafe fn set_pack_head(value: *mut CThing) {
    (*thing_t(&raw mut player)).t_pack = value;
}

unsafe fn hero_coord() -> crate::player::CCoord {
    (*thing_t(&raw mut player)).t_pos
}

unsafe fn proom() -> *mut CRoom {
    (*thing_t(&raw mut player)).t_room
}

unsafe fn player_has(flag: c_short) -> bool {
    ((*thing_t(&raw mut player)).t_flags & flag) != 0
}

unsafe fn chat_at(y: c_int, x: c_int) -> c_char {
    draw_chat(y, x)
}

unsafe fn floor_char_for_room() -> c_char {
    if room_flags(proom()) & ISGONE != 0 {
        PASSAGE
    } else if show_floor() != 0 {
        FLOOR
    } else {
        b' ' as c_char
    }
}

#[no_mangle]
pub unsafe extern "C" fn add_pack(obj: *mut CThing, silent: c_uchar) {
    let mut item = obj;
    let mut from_floor = FALSE;
    let mut op: *mut CThing;
    let mut lp: *mut CThing;

    if item.is_null() {
        item = find_obj(hero_coord().y, hero_coord().x);
        if item.is_null() {
            return;
        }
        from_floor = TRUE;
    }

    if (*thing_o(item)).o_type == SCROLL as c_int && (*thing_o(item)).o_which == S_SCARE
        && ((*thing_o(item)).o_flags & ISFOUND) != 0
    {
        detach_list(&raw mut lvl_obj, item);
        // The object is removed from `lvl_obj`, so the terrain glyph shows
        // automatically via draw.
        mvaddch(hero_coord().y, hero_coord().x, floor_char_for_room() as c_uint);
        discard_item(item);
        msg(c"the scroll turns to dust as you pick it up".as_ptr());
        return;
    }

    if pack_head().is_null() {
        set_pack_head(item);
        (*thing_o(item)).o_packch = pack_char();
        inpack += 1;
    } else {
        lp = std::ptr::null_mut();
        op = pack_head();
        while !op.is_null() {
            if (*thing_o(op)).o_type != (*thing_o(item)).o_type {
                lp = op;
            } else {
                while (*thing_o(op)).o_type == (*thing_o(item)).o_type && (*thing_o(op)).o_which != (*thing_o(item)).o_which {
                    lp = op;
                    if next_item(op).is_null() {
                        break;
                    }
                    op = next_item(op);
                }
                if (*thing_o(op)).o_type == (*thing_o(item)).o_type && (*thing_o(op)).o_which == (*thing_o(item)).o_which {
                    if ((*thing_o(op)).o_type == FOOD as c_int || (*thing_o(op)).o_type == POTION as c_int || (*thing_o(op)).o_type == SCROLL as c_int) {
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
                        while (*thing_o(op)).o_type == (*thing_o(item)).o_type && (*thing_o(op)).o_which == (*thing_o(item)).o_which && (*thing_o(op)).o_group != (*thing_o(item)).o_group {
                            lp = op;
                            if next_item(op).is_null() {
                                break;
                            }
                            op = next_item(op);
                        }
                        if (*thing_o(op)).o_type == (*thing_o(item)).o_type && (*thing_o(op)).o_which == (*thing_o(item)).o_which && (*thing_o(op)).o_group == (*thing_o(item)).o_group {
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
            (*thing_o(item)).o_packch = pack_char();
            (*thing_t(item)).l_next = next_item(lp);
            (*thing_t(item)).l_prev = lp;
            if !next_item(lp).is_null() {
                (*thing_t(next_item(lp))).l_prev = item;
            }
            (*thing_t(lp)).l_next = item;
        }
    }

    (*thing_o(item)).o_flags |= ISFOUND as c_int;

    op = mlist;
    while !op.is_null() {
        if (*thing_t(op)).t_dest == &raw mut (*thing_o(item)).o_pos {
            (*thing_t(op)).t_dest = &raw mut (*thing_t(&raw mut player)).t_pos;
        }
        op = next_item(op);
    }

    if (*thing_o(item)).o_type == AMULET as c_int {
        amulet = TRUE;
    }

    if silent == 0 {
        if terse == 0 {
            addmsg(c"you now have ".as_ptr());
        }
        msg(c"%s (%c)".as_ptr(), inv_name(item, if terse == 0 { 0 } else { 1 }), (*thing_o(item)).o_packch as c_uint);
    }
}

#[no_mangle]
pub unsafe extern "C" fn pack_room(from_floor: c_uchar, obj: *mut CThing) -> c_uchar {
    if inpack + 1 > MAXPACK {
        if terse == 0 {
            addmsg(c"there's ".as_ptr());
        }
        addmsg(c"no room".as_ptr());
        if terse == 0 {
            addmsg(c" in your pack".as_ptr());
        }
        endmsg();
        if from_floor != 0 {
            move_msg(obj);
        }
        inpack = MAXPACK;
        return FALSE;
    }

    if from_floor != 0 {
        detach_list(&raw mut lvl_obj, obj);
        // The object is removed from `lvl_obj`, so the terrain glyph shows
        // automatically via draw.
        mvaddch(hero_coord().y, hero_coord().x, floor_char_for_room() as c_uint);
    }

    inpack += 1;
    TRUE
}

#[no_mangle]
pub unsafe extern "C" fn leave_pack(obj: *mut CThing, newobj: c_uchar, all: c_uchar) -> *mut CThing {
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
            std::ptr::copy_nonoverlapping(obj, nobj, 1);
            (*thing_t(nobj)).l_next = std::ptr::null_mut();
            (*thing_t(nobj)).l_prev = std::ptr::null_mut();
            (*thing_o(nobj)).o_count = 1;
        }
    } else {
        last_pick = std::ptr::null_mut();
        pack_used[(*thing_o(obj)).o_packch as usize - 'a' as usize] = FALSE;
        {
            let head = &raw mut (*thing_t(&raw mut player)).t_pack as *mut *mut CThing;
            detach_list(head, obj);
        }
    }
    nobj
}

#[no_mangle]
pub unsafe extern "C" fn pack_char() -> c_char {
    for i in 0..pack_used.len() {
        if pack_used[i] == 0 {
            pack_used[i] = TRUE;
            return (b'a' + i as u8) as c_char;
        }
    }
    b'a' as c_char
}

#[no_mangle]
pub unsafe extern "C" fn inventory(list: *mut CThing, type_: c_int) -> c_uchar {
    let mut cur = list;
    n_objs = 0;

    while !cur.is_null() {
        if type_ != 0
            && type_ != (*thing_o(cur)).o_type
            && !(type_ == CALLABLE && (*thing_o(cur)).o_type != FOOD && (*thing_o(cur)).o_type != AMULET)
            && !(type_ == R_OR_S && ((*thing_o(cur)).o_type == RING || (*thing_o(cur)).o_type == STICK))
        {
            cur = next_item(cur);
            continue;
        }

        n_objs += 1;
        msg_esc = TRUE;
        let mut inv_temp = [0 as c_char; MAXSTR];
        if (*thing_o(cur)).o_packch == 0 {
            std::ptr::copy_nonoverlapping(c"%s".as_ptr(), inv_temp.as_mut_ptr(), 3);
        } else {
            let format = [(*thing_o(cur)).o_packch, b')' as c_char, b' ' as c_char, b'%' as c_char, b's' as c_char, 0];
            std::ptr::copy_nonoverlapping(format.as_ptr(), inv_temp.as_mut_ptr(), format.len());
        }
        if add_line(inv_temp.as_mut_ptr(), inv_name(cur, FALSE)) == ESCAPE as c_char {
            msg_esc = FALSE;
            msg(c"".as_ptr());
            return TRUE;
        }
        msg_esc = FALSE;
        cur = next_item(cur);
    }

    if n_objs == 0 {
        if terse != 0 {
            msg(if type_ == 0 { c"empty handed".as_ptr() } else { c"nothing appropriate".as_ptr() });
        } else {
            msg(if type_ == 0 { c"you are empty handed".as_ptr() } else { c"you don't have anything appropriate".as_ptr() });
        }
        return FALSE;
    }

    TRUE
}

#[no_mangle]
pub unsafe extern "C" fn pick_up(ch: c_char) {
    let obj = find_obj(hero_coord().y, hero_coord().x);
    if player_has(ISLEVIT) {
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
                detach_list(&raw mut lvl_obj, obj);
                discard_item(obj);
                if !proom().is_null() {
                    (*proom()).r_goldval = 0;
                }
            }
            ARMOR | POTION | FOOD | WEAPON | SCROLL | AMULET | RING | STICK => {
                add_pack(std::ptr::null_mut(), FALSE);
            }
            _ => {}
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_item(purpose: *const c_char, type_: c_int) -> *mut CThing {
    let mut ch: c_int;

    if pack_head().is_null() {
        msg(c"you aren't carrying anything".as_ptr());
        return std::ptr::null_mut();
    }

    if again != 0 {
        if !last_pick.is_null() {
            return last_pick;
        }
        msg(c"you ran out".as_ptr());
        return std::ptr::null_mut();
    }

    loop {
        if terse == 0 {
            addmsg(c"which object do you want to ".as_ptr());
        }
        addmsg(purpose);
        if terse != 0 {
            addmsg(c" what".as_ptr());
        }
        msg(c"? (* for list): ".as_ptr());
        ch = readchar();
        mpos = 0;
        if ch == ESCAPE {
            reset_last();
            after = FALSE;
            msg(c"".as_ptr());
            return std::ptr::null_mut();
        }
        n_objs = 1;
        if ch == '*' as c_int {
            mpos = 0;
            if inventory(pack_head(), type_) == 0 {
                after = FALSE;
                return std::ptr::null_mut();
            }
            continue;
        }
        let mut obj = pack_head();
        while !obj.is_null() {
            if (*thing_o(obj)).o_packch == ch as c_char {
                return obj;
            }
            obj = next_item(obj);
        }
        msg(c"'%s' is not a valid item".as_ptr(), unctrl(ch));
    }
}

#[no_mangle]
pub unsafe extern "C" fn money(value: c_int) {
    purse += value;
    // The gold object was discarded, so the terrain glyph shows via draw.
    mvaddch(hero_coord().y, hero_coord().x, floor_char_for_room() as c_uint);
    if value > 0 {
        if terse == 0 {
            addmsg(c"you found ".as_ptr());
        }
        msg(c"%d gold pieces".as_ptr(), value);
    }
}

#[no_mangle]
pub unsafe extern "C" fn floor_ch() -> c_char {
    floor_char_for_room()
}

#[no_mangle]
pub unsafe extern "C" fn floor_at() -> c_char {
    let ch = chat_at(hero_coord().y, hero_coord().x);
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
pub unsafe extern "C" fn move_msg(obj: *mut CThing) {
    if terse == 0 {
        addmsg(c"you ".as_ptr());
    }
    msg(c"moved onto %s".as_ptr(), inv_name(obj, TRUE));
}

#[no_mangle]
pub unsafe extern "C" fn picky_inven() {
    if pack_head().is_null() {
        msg(c"you aren't carrying anything".as_ptr());
    } else if next_item(pack_head()).is_null() {
        msg(c"a) %s".as_ptr(), inv_name(pack_head(), FALSE));
    } else {
        msg(if terse != 0 { c"item: ".as_ptr() } else { c"which item do you wish to inventory: ".as_ptr() });
        mpos = 0;
        let mch = readchar() as c_char;
        if mch as c_int == ESCAPE {
            msg(c"".as_ptr());
            return;
        }
        let mut obj = pack_head();
        while !obj.is_null() {
            if mch == (*thing_o(obj)).o_packch {
                msg(c"%c) %s".as_ptr(), mch as c_int, inv_name(obj, FALSE));
                return;
            }
            obj = next_item(obj);
        }
        msg(c"'%s' not in pack".as_ptr(), unctrl(mch as c_int));
    }
}

#[no_mangle]
pub unsafe extern "C" fn pick_up_char(ch: c_char) {
    pick_up(ch);
}