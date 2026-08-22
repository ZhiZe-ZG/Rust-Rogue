use std::os::raw::c_int;

use crate::rnd::rnd;

use super::rooms::{build_room_model, Room};
use glam::IVec2;

const ISGONE: i16 = 0o000002;
const ISMAZE: i16 = 0o000004;
const ISDARK: i16 = 0o000001;
const NUMCOLS: i32 = 80;
const NUMLINES: i32 = 24;
pub(crate) const MAXROOMS: usize = 9;
const MAX_ROOM_TRIES: usize = 100;

#[derive(Copy, Clone)]
pub(crate) struct RoomState {
    pub(crate) pos: IVec2,
    pub(crate) size: IVec2,
    pub(crate) gold: IVec2,
    pub(crate) goldval: i32,
    pub(crate) flags: i16,
    pub(crate) nexits: i32,
}

impl RoomState {
    pub(crate) fn is_gone(&self) -> bool {
        (self.flags & ISGONE) != 0
    }

    pub(crate) fn is_maze(&self) -> bool {
        (self.flags & ISMAZE) != 0
    }
}

pub(crate) struct GeneratedRooms {
    pub(crate) room_states: [RoomState; MAXROOMS],
    pub(crate) room_models: [Option<Room>; MAXROOMS],
}

pub(crate) fn generate_room_grid_and_rooms(
    room_states: [RoomState; MAXROOMS],
    bsze: IVec2,
    depth: i32,
) -> GeneratedRooms {
    let room_states = determine_room_layouts(room_states, bsze, depth);
    let room_models = std::array::from_fn(|i| build_room_model_from_state(&room_states[i]));

    GeneratedRooms {
        room_states,
        room_models,
    }
}

fn rnd_room_from_state(room_states: &[RoomState; MAXROOMS]) -> usize {
    loop {
        let rm = rnd(MAXROOMS as c_int) as usize;
        if !room_states[rm].is_gone() {
            return rm;
        }
    }
}

fn determine_room_layouts(
    mut room_states: [RoomState; MAXROOMS],
    bsze: IVec2,
    depth: i32,
) -> [RoomState; MAXROOMS] {
    // Reset per-room state before generating the level layout.
    for room in &mut room_states {
        room.goldval = 0;
        room.nexits = 0;
        room.flags = 0;
    }

    let left_out = rnd(4);
    // Randomly mark a few rooms as removed for this level.
    for _ in 0..left_out {
        let room_idx = rnd_room_from_state(&room_states);
        room_states[room_idx].flags |= ISGONE;
    }

    // Compute geometry, sizes, and flags for every room slot.
    for i in 0..MAXROOMS {
        let room = &mut room_states[i];
        let top = IVec2::new((i as i32 % 3) * bsze.x + 1, (i as i32 / 3) * bsze.y);

        if room.is_gone() {
            // Keep rerolling until the off-map placeholder position is valid.
            loop {
                room.pos.x = top.x + rnd(bsze.x - 2) + 1;
                room.pos.y = top.y + rnd(bsze.y - 2) + 1;
                room.size = IVec2::new(-NUMCOLS, -NUMLINES);
                if room.pos.y > 0 && room.pos.y < NUMLINES - 1 {
                    break;
                }
            }
            continue;
        }

        if rnd(10) < depth - 1 {
            room.flags |= ISDARK;
            if rnd(15) == 0 {
                room.flags = ISMAZE;
            }
        }

        if room.is_maze() {
            room.size.x = bsze.x - 1;
            room.size.y = bsze.y - 1;
            room.pos.x = top.x;
            if room.pos.x == 1 {
                room.pos.x = 0;
            }
            room.pos.y = top.y;
            if room.pos.y == 0 {
                room.pos.y += 1;
                room.size.y -= 1;
            }
        } else {
            let mut placed = false;
            for _ in 0..MAX_ROOM_TRIES {
                room.size.x = rnd(bsze.x - 4) + 4;
                room.size.y = rnd(bsze.y - 4) + 4;
                room.pos.x = top.x + rnd(bsze.x - room.size.x);
                room.pos.y = top.y + rnd(bsze.y - room.size.y);
                if room.pos.y != 0 {
                    placed = true;
                    break;
                }
            }

            if !placed {
                room.flags |= ISGONE;
            }
        }
    }

    room_states
}

fn build_room_model_from_state(room: &RoomState) -> Option<Room> {
    if room.is_gone() {
        return None;
    }

    build_room_model(room.pos, room.size, room.is_maze())
}
