//! Portable Rogue save-state code, now serialized as RON.
//!
//! This module used to be a byte-for-byte port of the legacy C `state.c`
//! binary serializer, complete with raw pointers, endian-swapping primitives
//! and `RSID_*` markers. It has been rewritten to store the whole game state
//! as [RON] (Rust Object Notation), a human-readable self-describing format.
//!
//! The engine's live state is still held in process-wide safe owners
//! ([`crate::game::PLAYER`], [`crate::game::MONSTER_LIST`],
//! [`crate::game::MONSTER_MAP`], [`crate::game::CURRENT_LEVEL`], the item
//! arena and the many `static mut` globals). This module only *snapshots* that
//! state into plain value types, and rebuilds it on restore. Pointers between
//! things (intrusive list links, chase targets, equipment slots) are encoded as
//! indices/ids so the on-disk form contains no addresses.
//!
//! [RON]: https://github.com/ron-rs/ron

use glam::IVec2;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

use crate::daemon::CDelayedAction;
use crate::entity::monsters::MonsterType;
use crate::entity::player::{MonsterFlags, Stats, Thing, ThingObject};
use crate::game::{MonsterId, MONSTER_LIST, MONSTER_MAP, PLAYER};
use crate::item::arena::{new_item_id, ThingId, OBJECTS};
use crate::level::{LevelFlags, Passage, PassageLinks, RoomGraph};
use crate::structure::{Room, Structure};

/// Number of generated scroll names persisted (legacy `MAXSCROLLS`).
const MAXSCROLLS: usize = 18;

// ─── Snapshot value types ────────────────────────────────────────────────────

/// A chase destination, encoded as an index instead of a raw pointer.
///
/// Mirrors the legacy `(listid, index)` scheme used by `state.c`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DestRef {
    /// No chase target.
    None,
    /// Chasing the hero (position read live from [`crate::game::PLAYER`]).
    Hero,
    /// Chasing the monster at this index in the monster list.
    Monster(usize),
    /// Chasing the floor item at this index in the level's item list.
    Object(usize),
    /// Heading for the gold stash of this room index.
    RoomGold(usize),
}

/// A monster/actor [`Thing`] without any pointers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonsterSnapshot {
    pub t_pos: IVec2,
    pub t_turn: bool,
    pub t_type: Option<MonsterType>,
    pub t_disguise: u8,
    pub t_oldch: u8,
    pub t_dest: DestRef,
    pub t_flags: MonsterFlags,
    pub t_stats: Stats,
    pub t_room: Option<usize>,
    /// Items carried in the monster's pack, head first.
    pub t_pack: Vec<ThingObject>,
    /// Deferred chase-target index (resolved after every monster exists).
    #[serde(default)]
    pub t_reserved: i32,
}

/// The player's equipped items, encoded as indices into the player's pack.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EquipmentSnapshot {
    pub armor: Option<usize>,
    pub left_ring: Option<usize>,
    pub right_ring: Option<usize>,
    pub weapon: Option<usize>,
    pub last_pick: Option<usize>,
    pub l_last_pick: Option<usize>,
}

/// The mutable parts of the current dungeon level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelSnapshot {
    pub depth: i32,
    pub stairs: IVec2,
    pub rooms: Vec<Room>,
    pub room_graph: RoomGraph,
    pub passages: Vec<Passage>,
    pub map: Structure,
    pub flags: LevelFlags,
    pub passage_links: Vec<PassageLinks>,
    /// Floor items, head first.
    pub items: Vec<ThingObject>,
    /// Per-cell monster occupancy as `(y, x, monster_index)`.
    pub monster_cells: Vec<(usize, usize, usize)>,
    /// The stable per-room gold positions ([`crate::game::ROOM_GOLD`]).
    pub room_gold: Vec<IVec2>,
}

/// The mutable `oi_guess`/`oi_know` state of one object-info table entry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ObjInfoState {
    pub guess: Option<String>,
    pub know: bool,
}

/// The full serialized game state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameSnapshot {
    // ── boolean/char flags ──────────────────────────────────────────────
    pub after: bool,
    pub again: bool,
    pub noscore: i32,
    pub seenstairs: bool,
    pub amulet: bool,
    pub door_stop: bool,
    pub fight_flush: bool,
    pub firstmove: bool,
    pub got_ltc: bool,
    pub has_hit: bool,
    pub in_shell: bool,
    pub inv_describe: bool,
    pub jump: bool,
    pub kamikaze: bool,
    pub lower_msg: bool,
    pub move_on: bool,
    pub msg_esc: bool,
    pub passgo: bool,
    pub playing: bool,
    pub q_comm: bool,
    pub running: bool,
    pub save_msg: bool,
    pub see_floor: bool,
    pub stat_msg: bool,
    pub terse: bool,
    pub to_death: bool,
    pub tombstone: bool,
    pub wizard: i32,
    pub pack_used: [u8; 26],
    pub dir_ch: u8,
    pub runch: u8,
    pub take: u8,

    // ── strings / string tables ─────────────────────────────────────────
    pub file_name: String,
    pub huh: String,
    pub prbuf: String,
    pub release: String,
    pub whoami: String,
    pub fruit: String,
    pub home: String,
    pub scroll_names: Vec<String>,
    pub inv_t_names: Vec<String>,
    pub trap_names: Vec<String>,
    pub p_colors: Vec<String>,
    pub r_stones: Vec<String>,
    pub ws_type: Vec<String>,
    pub ws_made: Vec<String>,

    // ── numeric globals ─────────────────────────────────────────────────
    pub orig_dsusp: i32,
    pub l_last_comm: u8,
    pub l_last_dir: u8,
    pub last_comm: u8,
    pub last_dir: u8,
    pub n_objs: i32,
    pub ntraps: i32,
    pub hungry_state: i32,
    pub inpack: i32,
    pub inv_type: i32,
    pub max_level: i32,
    pub mpos: i32,
    pub no_food: i32,
    pub a_class: Vec<i32>,
    pub count: i32,
    pub food_left: i32,
    pub lastscore: i32,
    pub no_command: i32,
    pub no_move: i32,
    pub purse: i32,
    pub quiet: i32,
    pub vf_hit: i32,
    pub dnum: i32,
    pub seed: i32,
    pub e_levels: Vec<i32>,
    pub delta: IVec2,
    pub oldpos: IVec2,

    // ── tables that gameplay mutates in place ───────────────────────────
    pub monster_stats: Vec<Stats>,
    pub arm_info: Vec<ObjInfoState>,
    pub pot_info: Vec<ObjInfoState>,
    pub ring_info: Vec<ObjInfoState>,
    pub scr_info: Vec<ObjInfoState>,
    pub weap_info: Vec<ObjInfoState>,
    pub ws_info: Vec<ObjInfoState>,
    pub things: Vec<ObjInfoState>,

    // ── delayed actions ─────────────────────────────────────────────────
    pub daemons: Vec<CDelayedAction>,

    // ── player, level and misc game state ───────────────────────────────
    pub player: MonsterSnapshot,
    /// The live monster list, in traversal order.
    pub monsters: Vec<MonsterSnapshot>,
    pub player_pack: Vec<ThingObject>,
    pub equipment: EquipmentSnapshot,
    pub level: LevelSnapshot,
    pub max_stats: Stats,
    pub oldrp: Option<usize>,
    pub between: i32,
    pub group: i32,
    pub nh: IVec2,
}

// ─── Pointer-free helpers ────────────────────────────────────────────────────

/// Collect the objects behind a list of pack handles into `Vec<ThingObject>`,
/// head first.
fn collect_objects(ids: &[ThingId]) -> Vec<ThingObject> {
    ids.iter()
        .filter_map(|&id| {
            OBJECTS
                .with(id, |t| match t {
                    Thing::Object { data, .. } => Some(data.clone()),
                    Thing::Monster { .. } => None,
                })
                .flatten()
        })
        .collect()
}

/// Allocate objects for `items` (head = index 0) and return their handles.
fn build_object_list(items: &[ThingObject]) -> Vec<ThingId> {
    items
        .iter()
        .map(|data| {
            let id = new_item_id();
            let _ = OBJECTS.with_mut(id, |t| {
                if let Thing::Object { data: slot, .. } = t {
                    *slot = data.clone();
                }
            });
            id
        })
        .collect()
}

/// Index of the object handle `target` within the pack `pack`, or `None`.
fn list_index_of_id(pack: &[ThingId], target: Option<ThingId>) -> Option<usize> {
    let target = target?;
    pack.iter().position(|&id| id == target)
}

/// Snapshot the fields of an actor (monster or the hero) into a
/// pointer-free [`MonsterSnapshot`], resolving any live [`DestRef`] handle into
/// the index-based, address-free form. `level_items` is the live floor-item
/// handle list, head first (used to resolve object chase targets).
fn snapshot_actor(
    pos: IVec2,
    turn: bool,
    typ: Option<MonsterType>,
    disguise: u8,
    oldch: u8,
    dest_ref: crate::entity::player::DestRef,
    flags: MonsterFlags,
    stats: Stats,
    room: Option<usize>,
    pack: &[ThingId],
    reserved: i32,
    level_items: &[ThingId],
) -> MonsterSnapshot {
    let dest = match dest_ref {
        crate::entity::player::DestRef::None => DestRef::None,
        crate::entity::player::DestRef::Hero => DestRef::Hero,
        crate::entity::player::DestRef::Monster(id) => {
            DestRef::Monster(MONSTER_LIST.position(id).unwrap_or(usize::MAX))
        }
        crate::entity::player::DestRef::Object(id) => {
            DestRef::Object(level_items.iter().position(|&o| o == id).unwrap_or(usize::MAX))
        }
        crate::entity::player::DestRef::RoomGold(r) => DestRef::RoomGold(r),
    };

    MonsterSnapshot {
        t_pos: pos,
        t_turn: turn,
        t_type: typ,
        t_disguise: disguise,
        t_oldch: oldch,
        t_dest: dest,
        t_flags: flags,
        t_stats: stats,
        t_room: room,
        t_pack: collect_objects(pack),
        t_reserved: reserved,
    }
}

/// Snapshot a live monster `id` into a pointer-free [`MonsterSnapshot`].
fn snapshot_monster(id: MonsterId, level_items: &[ThingId]) -> Option<MonsterSnapshot> {
    MONSTER_LIST
        .with(id, |t| match t {
            Thing::Monster { data } => Some(snapshot_actor(
                data.t_pos,
                data.t_turn,
                data.t_type,
                data.t_disguise,
                data.t_oldch,
                data.t_dest,
                data.t_flags,
                data.t_stats,
                data.t_room,
                &data.t_pack,
                data.t_reserved,
                level_items,
            )),
            Thing::Object { .. } => None,
        })
        .flatten()
}

// ─── Save ────────────────────────────────────────────────────────────────────

/// Builds the full [`GameSnapshot`] from the live process-wide state.
unsafe fn build_snapshot() -> GameSnapshot {
    use crate::game::globals::*;

    // Player actor snapshot (without its pack, which is captured separately).
    let hero_pack = PLAYER.pack();
    let level_items = crate::game::item_ids();

    let mut player = PLAYER.with_monster(|data| {
        snapshot_actor(
            data.t_pos,
            data.t_turn,
            data.t_type,
            data.t_disguise,
            data.t_oldch,
            data.t_dest,
            data.t_flags,
            data.t_stats,
            data.t_room,
            &data.t_pack,
            data.t_reserved,
            &level_items,
        )
    });
    player.t_pack = Vec::new();

    let monster_snaps: Vec<MonsterSnapshot> = MONSTER_LIST
        .ids()
        .into_iter()
        .filter_map(|id| snapshot_monster(id, &level_items))
        .collect();

    let player_pack = collect_objects(&hero_pack);
    let eq = PLAYER.equipment();
    let equipment = EquipmentSnapshot {
        armor: list_index_of_id(&hero_pack, eq.armor_id()),
        left_ring: list_index_of_id(&hero_pack, eq.left_ring_id()),
        right_ring: list_index_of_id(&hero_pack, eq.right_ring_id()),
        weapon: list_index_of_id(&hero_pack, eq.weapon_id()),
        last_pick: list_index_of_id(&hero_pack, crate::game::globals::last_pick),
        l_last_pick: list_index_of_id(&hero_pack, crate::game::globals::l_last_pick),
    };

    // Level snapshot.
    let level = crate::game::with_current_level(|lvl| {
        let mut monster_cells = Vec::new();
        for y in 0..crate::config::GameConfig::LEVEL_HEIGHT {
            for x in 0..crate::config::GameConfig::LEVEL_WIDTH {
                if let Some(id) = MONSTER_MAP.at(y, x) {
                    if let Some(pos) = MONSTER_LIST.position(id) {
                        monster_cells.push((y, x, pos));
                    }
                }
            }
        }

        LevelSnapshot {
            depth: lvl.depth,
            stairs: lvl.stairs,
            rooms: lvl.rooms.clone(),
            room_graph: lvl.room_graph.clone(),
            passages: lvl.passages.clone(),
            map: lvl.map.clone(),
            flags: lvl.flags.clone(),
            passage_links: lvl.passage_links.clone(),
            items: level_items
                .iter()
                .filter_map(|&id| {
                    OBJECTS.with(id, |t| match t {
                        Thing::Object { data, .. } => Some(data.clone()),
                        Thing::Monster { .. } => None,
                    })
                    .flatten()
                })
                .collect(),
            monster_cells,
            room_gold: crate::game::ROOM_GOLD.to_vec(),
        }
    });

    // Monster table stats (mutated in place by combat/wizard code).
    let mut monster_stats = Vec::new();
    for m in crate::game::globals::monsters.iter() {
        monster_stats.push(m.m_stats);
    }

    let obj_states = |table: &[ObjInfo]| -> Vec<ObjInfoState> {
        table
            .iter()
            .map(|info| ObjInfoState {
                guess: info.oi_guess.clone(),
                know: info.oi_know,
            })
            .collect()
    };

    let daemons: Vec<CDelayedAction> = daemon_table().to_vec();

    GameSnapshot {
        after: after != 0,
        again: again != 0,
        noscore,
        seenstairs: seenstairs != 0,
        amulet: amulet != 0,
        door_stop: door_stop != 0,
        fight_flush: fight_flush != 0,
        firstmove: firstmove != 0,
        got_ltc: got_ltc != 0,
        has_hit: has_hit != 0,
        in_shell: in_shell != 0,
        inv_describe: inv_describe != 0,
        jump: jump != 0,
        kamikaze: kamikaze != 0,
        lower_msg: lower_msg != 0,
        move_on: move_on != 0,
        msg_esc: msg_esc != 0,
        passgo: passgo != 0,
        playing: playing != 0,
        q_comm: q_comm != 0,
        running: running != 0,
        save_msg: save_msg != 0,
        see_floor: see_floor != 0,
        stat_msg: stat_msg != 0,
        terse: terse != 0,
        to_death: to_death != 0,
        tombstone: tombstone != 0,
        wizard,
        pack_used,
        dir_ch,
        runch,
        take,

        file_name: crate::game::globals::file_name(),
        huh: crate::game::globals::huh_string(),
        prbuf: crate::game::globals::prbuf(),
        release: crate::vers::release(),
        whoami: crate::game::globals::whoami(),
        fruit: crate::game::globals::fruit(),
        home: crate::game::globals::get_home(),
        scroll_names: (0..MAXSCROLLS)
            .map(crate::game::globals::scroll_name)
            .collect(),
        inv_t_names: crate::game::globals::inv_t_names(),
        trap_names: crate::game::globals::trap_names(),
        p_colors: p_colors.iter().map(|c| (*c).to_string()).collect(),
        r_stones: r_stones.iter().map(|c| (*c).to_string()).collect(),
        ws_type: ws_type.iter().map(|c| (*c).to_string()).collect(),
        ws_made: ws_made.iter().map(|c| (*c).to_string()).collect(),

        orig_dsusp,
        l_last_comm,
        l_last_dir,
        last_comm,
        last_dir,
        n_objs,
        ntraps,
        hungry_state,
        inpack,
        inv_type,
        max_level,
        mpos,
        no_food,
        a_class: a_class.to_vec(),
        count,
        food_left,
        lastscore,
        no_command,
        no_move,
        purse,
        quiet,
        vf_hit,
        dnum,
        seed,
        e_levels: e_levels.to_vec(),
        delta,
        oldpos,

        monster_stats,
        arm_info: obj_states(&arm_info),
        pot_info: obj_states(&pot_info),
        ring_info: obj_states(&ring_info),
        scr_info: obj_states(&scr_info),
        weap_info: obj_states(&weap_info),
        ws_info: obj_states(&ws_info),
        things: obj_states(&things),

        daemons,

        player,
        monsters: monster_snaps,
        player_pack,
        equipment,
        level,
        max_stats,
        oldrp,
        between,
        group,
        nh: crate::entity::player::nh,
    }
}

/// Serializes the whole game state into `out` as RON.
pub fn rs_save_file(out: &mut dyn Write) -> std::io::Result<()> {
    let snapshot = unsafe { build_snapshot() };
    let pretty = ron::ser::PrettyConfig::default();
    let text = ron::ser::to_string_pretty(&snapshot, pretty)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    out.write_all(text.as_bytes())?;
    Ok(())
}

// ─── Restore ─────────────────────────────────────────────────────────────────

/// Restores the whole game state from the RON document in `input`.
pub fn rs_restore_file(input: &mut dyn Read) -> std::io::Result<()> {
    let mut text = String::new();
    input.read_to_string(&mut text)?;
    let snapshot: GameSnapshot = ron::de::from_str(&text)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    unsafe { apply_snapshot(snapshot) };
    Ok(())
}

/// Resolve an `Option<usize>` pack index against the rebuilt pack handles,
/// returning the arena handle (or `None`).
fn resolve_id(pack: &[ThingId], index: Option<usize>) -> Option<ThingId> {
    pack.get(index?).copied()
}

/// Writes a `&'static str` table entry from a saved string, using `lookup` to
/// map back to a canonical `'static` literal (`""` when unknown).
fn resolve_static_str<'a>(value: &str, table: &'a [&'a str]) -> &'a str {
    table.iter().find(|&&c| c == value).copied().unwrap_or("")
}

/// Applies a restored [`GameSnapshot`] back into the live process-wide state.
unsafe fn apply_snapshot(s: GameSnapshot) {
    use crate::game::globals::*;

    // ── boolean/char flags ──────────────────────────────────────────────
    macro_rules! set_flag {
        ($name:ident) => {
            $name = s.$name as u8;
        };
    }
    set_flag!(after);
    set_flag!(again);
    noscore = s.noscore;
    set_flag!(seenstairs);
    set_flag!(amulet);
    set_flag!(door_stop);
    set_flag!(fight_flush);
    set_flag!(firstmove);
    set_flag!(got_ltc);
    set_flag!(has_hit);
    set_flag!(in_shell);
    set_flag!(inv_describe);
    set_flag!(jump);
    set_flag!(kamikaze);
    set_flag!(lower_msg);
    set_flag!(move_on);
    set_flag!(msg_esc);
    set_flag!(passgo);
    set_flag!(playing);
    set_flag!(q_comm);
    set_flag!(running);
    set_flag!(save_msg);
    set_flag!(see_floor);
    set_flag!(stat_msg);
    set_flag!(terse);
    set_flag!(to_death);
    set_flag!(tombstone);
    wizard = s.wizard;
    pack_used = s.pack_used;
    dir_ch = s.dir_ch;
    runch = s.runch;
    take = s.take;

    // ── strings / string tables ─────────────────────────────────────────
    crate::game::globals::set_file_name(s.file_name);
    crate::game::globals::set_huh_string(&s.huh);
    crate::game::globals::set_prbuf(s.prbuf);
    crate::vers::set_release(s.release);
    crate::game::globals::set_whoami(s.whoami);
    crate::game::globals::set_fruit(s.fruit);
    crate::game::globals::set_home(s.home);
    for (i, name) in s.scroll_names.iter().enumerate() {
        crate::game::globals::set_scroll_name(i, name.clone());
    }
    for (i, name) in s.inv_t_names.iter().enumerate() {
        crate::game::globals::set_inv_t_name(i, name.clone());
    }
    for (i, name) in s.trap_names.iter().enumerate() {
        crate::game::globals::set_trap_name(i, name.clone());
    }
    for (i, name) in s.p_colors.iter().enumerate() {
        if i < p_colors.len() {
            p_colors[i] = resolve_static_str(name, &crate::colors::POTION_COLORS);
        }
    }
    for (i, name) in s.r_stones.iter().enumerate() {
        if i < r_stones.len() {
            r_stones[i] = crate::init::stones
                .iter()
                .find(|st| st.st_name == name)
                .map(|st| st.st_name)
                .unwrap_or("");
        }
    }
    for (i, name) in s.ws_type.iter().enumerate() {
        if i < ws_type.len() {
            ws_type[i] = if name == "staff" { "staff" } else { "wand" };
        }
    }
    for (i, name) in s.ws_made.iter().enumerate() {
        if i < ws_made.len() {
            ws_made[i] = if ws_type[i] == "staff" {
                crate::init::wood
                    .iter()
                    .find(|w| **w == *name)
                    .copied()
                    .unwrap_or("")
            } else {
                crate::init::metal
                    .iter()
                    .find(|m| **m == *name)
                    .copied()
                    .unwrap_or("")
            };
        }
    }

    // ── numeric globals ─────────────────────────────────────────────────
    orig_dsusp = s.orig_dsusp;
    l_last_comm = s.l_last_comm;
    l_last_dir = s.l_last_dir;
    last_comm = s.last_comm;
    last_dir = s.last_dir;
    n_objs = s.n_objs;
    ntraps = s.ntraps;
    hungry_state = s.hungry_state;
    inpack = s.inpack;
    inv_type = s.inv_type;
    max_level = s.max_level;
    mpos = s.mpos;
    no_food = s.no_food;
    for (i, v) in s.a_class.iter().enumerate() {
        if i < a_class.len() {
            a_class[i] = *v;
        }
    }
    count = s.count;
    food_left = s.food_left;
    lastscore = s.lastscore;
    no_command = s.no_command;
    no_move = s.no_move;
    purse = s.purse;
    quiet = s.quiet;
    vf_hit = s.vf_hit;
    dnum = s.dnum;
    seed = s.seed;
    for (i, v) in s.e_levels.iter().enumerate() {
        if i < e_levels.len() {
            e_levels[i] = *v;
        }
    }
    delta = s.delta;
    oldpos = s.oldpos;

    // ── in-place tables ─────────────────────────────────────────────────
    for (i, stats) in s.monster_stats.iter().enumerate() {
        if i < monsters.len() {
            monsters[i].m_stats = *stats;
        }
    }
    let apply_obj = |table: &mut [ObjInfo], states: &[ObjInfoState]| {
        for (i, st) in states.iter().enumerate() {
            if i < table.len() {
                table[i].oi_guess = st.guess.clone();
                table[i].oi_know = st.know;
            }
        }
    };
    apply_obj(&mut arm_info, &s.arm_info);
    apply_obj(&mut pot_info, &s.pot_info);
    apply_obj(&mut ring_info, &s.ring_info);
    apply_obj(&mut scr_info, &s.scr_info);
    apply_obj(&mut weap_info, &s.weap_info);
    apply_obj(&mut ws_info, &s.ws_info);
    apply_obj(&mut things, &s.things);

    // ── daemons ─────────────────────────────────────────────────────────
    for (i, slot) in s.daemons.iter().enumerate() {
        if i < D_LIST.len() {
            D_LIST[i] = *slot;
        }
    }

    // ── level ───────────────────────────────────────────────────────────
    MONSTER_LIST.clear();
    MONSTER_MAP.clear();

    let room_gold = s.level.room_gold.clone();
    crate::game::with_current_level_mut(|lvl| {
        lvl.depth = s.level.depth;
        lvl.stairs = s.level.stairs;
        lvl.rooms = s.level.rooms.clone();
        lvl.room_graph = s.level.room_graph.clone();
        lvl.passages = s.level.passages.clone();
        lvl.map = s.level.map.clone();
        lvl.flags = s.level.flags.clone();
        lvl.passage_links = s.level.passage_links.clone();
        lvl.items = s
            .level
            .items
            .iter()
            .map(|data| {
                let item = new_item_id();
                let _ = OBJECTS.with_mut(item, |t| {
                    if let Thing::Object { data: slot, .. } = t {
                        *slot = data.clone();
                    }
                });
                item
            })
            .collect();
    });
    crate::game::set_current_depth(s.level.depth);

    // Restore the stable per-room gold coordinate array (chase-target backing).
    for (i, g) in room_gold.iter().enumerate() {
        if i < crate::game::ROOM_GOLD.len() {
            crate::game::ROOM_GOLD[i] = *g;
        }
    }

    // ── monsters ────────────────────────────────────────────────────────
    let mut monster_ids: Vec<MonsterId> = Vec::new();
    for snap in &s.monsters {
        let id = MONSTER_LIST.spawn_actor();
        MONSTER_LIST.with_mut(id, |t| {
            if let Thing::Monster { data } = t {
                data.t_pos = snap.t_pos;
                data.t_turn = snap.t_turn;
                data.t_type = snap.t_type;
                data.t_disguise = snap.t_disguise;
                data.t_oldch = snap.t_oldch;
                data.t_flags = snap.t_flags;
                data.t_stats = snap.t_stats;
                data.t_room = snap.t_room;
                data.t_dest = crate::entity::player::DestRef::None;
            }
        });
        let pack = build_object_list(&snap.t_pack);
        crate::entity::player::set_monster_pack(id, pack);
        monster_ids.push(id);
    }

    // Monster occupancy.
    for &(y, x, idx) in &s.level.monster_cells {
        if let Some(&id) = monster_ids.get(idx) {
            crate::game::set_monster_id(y as i32, x as i32, Some(id));
        }
    }

    // Resolve object, monster and room-gold chase targets to live handles now
    // that everything exists.
    let item_ids = crate::game::item_ids();
    for (i, snap) in s.monsters.iter().enumerate() {
        let id = monster_ids[i];
        let live = match snap.t_dest {
            DestRef::None => crate::entity::player::DestRef::None,
            DestRef::Hero => crate::entity::player::DestRef::Hero,
            DestRef::Object(j) => item_ids
                .get(j)
                .copied()
                .map_or(crate::entity::player::DestRef::None, crate::entity::player::DestRef::Object),
            DestRef::RoomGold(j) => crate::entity::player::DestRef::RoomGold(j),
            DestRef::Monster(j) => monster_ids
                .get(j)
                .copied()
                .map_or(crate::entity::player::DestRef::None, crate::entity::player::DestRef::Monster),
        };
        crate::entity::player::set_monster_dest(id, live);
    }

    // ── player ──────────────────────────────────────────────────────────
    PLAYER.with_monster_mut(|data| {
        data.t_pos = s.player.t_pos;
        data.t_turn = s.player.t_turn;
        data.t_type = s.player.t_type;
        data.t_disguise = s.player.t_disguise;
        data.t_oldch = s.player.t_oldch;
        data.t_flags = s.player.t_flags;
        data.t_stats = s.player.t_stats;
        data.t_room = s.player.t_room;
        data.t_dest = crate::entity::player::DestRef::None;
    });
    let player_pack = build_object_list(&s.player_pack);
    PLAYER.set_pack(player_pack.clone());
    PLAYER.set_weapon_id(None);
    PLAYER.set_armor_id(None);
    PLAYER.set_left_ring_id(None);
    PLAYER.set_right_ring_id(None);

    PLAYER.set_armor_id(resolve_id(&player_pack, s.equipment.armor));
    PLAYER.set_left_ring_id(resolve_id(&player_pack, s.equipment.left_ring));
    PLAYER.set_right_ring_id(resolve_id(&player_pack, s.equipment.right_ring));
    PLAYER.set_weapon_id(resolve_id(&player_pack, s.equipment.weapon));
    crate::game::globals::last_pick = resolve_id(&player_pack, s.equipment.last_pick);
    crate::game::globals::l_last_pick = resolve_id(&player_pack, s.equipment.l_last_pick);

    // ── misc ────────────────────────────────────────────────────────────
    max_stats = s.max_stats;
    oldrp = s.oldrp;
    between = s.between;
    group = s.group;
    crate::entity::player::nh = s.nh;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A snapshot round-trips through RON without loss.
    #[test]
    fn game_snapshot_round_trips_through_ron() {
        let snapshot = GameSnapshot {
            after: true,
            again: false,
            noscore: 3,
            seenstairs: true,
            amulet: false,
            door_stop: true,
            fight_flush: false,
            firstmove: true,
            got_ltc: false,
            has_hit: true,
            in_shell: false,
            inv_describe: true,
            jump: false,
            kamikaze: true,
            lower_msg: false,
            move_on: true,
            msg_esc: false,
            passgo: true,
            playing: true,
            q_comm: false,
            running: false,
            save_msg: true,
            see_floor: true,
            stat_msg: false,
            terse: false,
            to_death: false,
            tombstone: true,
            wizard: 0,
            pack_used: [0; 26],
            dir_ch: b'h',
            runch: 0,
            take: 0,
            file_name: "save.ron".to_owned(),
            huh: String::new(),
            prbuf: String::new(),
            release: "5.4.4".to_owned(),
            whoami: "tester".to_owned(),
            fruit: "slime-mold".to_owned(),
            home: "/tmp".to_owned(),
            scroll_names: vec!["zap".to_owned()],
            inv_t_names: vec!["Overwrite".to_owned()],
            trap_names: vec!["a trapdoor".to_owned()],
            p_colors: vec!["red".to_owned()],
            r_stones: vec!["agate".to_owned()],
            ws_type: vec!["staff".to_owned()],
            ws_made: vec!["oaken".to_owned()],
            orig_dsusp: 0,
            l_last_comm: 0,
            l_last_dir: 0,
            last_comm: 0,
            last_dir: 0,
            n_objs: 0,
            ntraps: 0,
            hungry_state: 0,
            inpack: 0,
            inv_type: 0,
            max_level: 1,
            mpos: 0,
            no_food: 0,
            a_class: vec![8, 7, 7, 6, 5, 4, 4, 3],
            count: 0,
            food_left: 1300,
            lastscore: -1,
            no_command: 0,
            no_move: 0,
            purse: 0,
            quiet: 0,
            vf_hit: 0,
            dnum: 0,
            seed: 42,
            e_levels: vec![10, 20],
            delta: IVec2::new(1, 0),
            oldpos: IVec2::new(5, 6),
            monster_stats: vec![Stats::default()],
            arm_info: vec![ObjInfoState::default()],
            pot_info: vec![ObjInfoState {
                guess: Some("fizzy".to_owned()),
                know: true,
            }],
            ring_info: vec![ObjInfoState::default()],
            scr_info: vec![ObjInfoState::default()],
            weap_info: vec![ObjInfoState::default()],
            ws_info: vec![ObjInfoState::default()],
            things: vec![ObjInfoState::default()],
            daemons: vec![CDelayedAction {
                d_type: -1,
                d_func: None,
                d_arg: 0,
                d_time: 0,
            }],
            player: MonsterSnapshot {
                t_pos: IVec2::new(3, 4),
                t_turn: false,
                t_type: None,
                t_disguise: 0,
                t_oldch: 0,
                t_dest: DestRef::None,
                t_flags: MonsterFlags::NONE,
                t_stats: Stats::default(),
                t_room: None,
                t_pack: Vec::new(),
                t_reserved: -1,
            },
            monsters: Vec::new(),
            player_pack: Vec::new(),
            equipment: EquipmentSnapshot::default(),
            level: LevelSnapshot {
                depth: 1,
                stairs: IVec2::ZERO,
                rooms: Vec::new(),
                room_graph: RoomGraph::default(),
                passages: Vec::new(),
                map: Structure::new(1, 1, crate::tile::Tile::Empty),
                flags: LevelFlags {
                    real: vec![true],
                    passage: vec![false],
                    seen: vec![false],
                    passnum: vec![0],
                },
                passage_links: Vec::new(),
                items: Vec::new(),
                monster_cells: Vec::new(),
                room_gold: Vec::new(),
            },
            max_stats: Stats::default(),
            oldrp: None,
            between: 0,
            group: 2,
            nh: IVec2::ZERO,
        };

        let text = ron::ser::to_string_pretty(&snapshot, ron::ser::PrettyConfig::default())
            .expect("serialize");
        let back: GameSnapshot = ron::de::from_str(&text).expect("deserialize");
        assert_eq!(back.seed, 42);
        assert_eq!(back.player.t_pos, IVec2::new(3, 4));
        assert_eq!(back.pot_info[0].guess.as_deref(), Some("fizzy"));
        assert!(back.pot_info[0].know);
    }
}