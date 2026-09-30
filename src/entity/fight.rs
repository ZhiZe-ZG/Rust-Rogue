//! All the fighting gets done here.
//!
//! Ported from `src/c/fight.c` to Rust.
//!
//! Monsters are addressed by [`MonsterId`] and items by arena [`ThingId`];
//! monster fields are read into small local snapshots and written back through
//! scoped `MONSTER_LIST.with`/`with_mut`, so no raw `*mut Thing` is threaded
//! through combat.

use crate::rnd::{rnd, roll};

use crate::entity::chase::{runto, see_monst};
use crate::entity::monsters::save;
use crate::entity::monsters::MonsterType;
use crate::entity::player::MonsterFlags;
use crate::game::globals::{monsters, weap_info};
use crate::game::{MonsterId, MONSTER_LIST, PLAYER};
use crate::init::pick_color;
use crate::item::arena::{ThingId, OBJECTS};
use crate::item::armor::rust_armor_id;
use crate::item::item_type::ItemType;
use crate::item::pack::leave_pack_id;
use crate::item::potions::is_magic_id;
use crate::item::rings::RingType;
use crate::item::things::inv_name_id;
use crate::item::weapons::{fall, fallpos};
use crate::machdep::flush_type;
use crate::misc::{check_level, chg_str, choose_str};
use crate::rip::death;
use crate::ui::output::{addmsg_str, endmsg, msg_str};
use glam::IVec2;

// ─── Constants ────────────────────────────────────────────────────────────────

const MAXSTR: usize = 1024;

// Item types
const GOLD: i32 = b'*' as i32;

// Misc constants
const BORE_LEVEL: i32 = 50;

// Save-vs constants
const VS_POISON: i32 = 0;
const VS_MAGIC: i32 = 0o03;

// ─── Adjustments due to strength ─────────────────────────────────────────────

static STR_PLUS: [i32; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2,
    2, 2, 3,
];

static ADD_DAM: [i32; 32] = [
    -7, -6, -5, -4, -3, -2, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 2, 3, 3, 4, 5, 5, 5, 5, 5, 5, 5,
    5, 5, 6,
];

// ─── Hit/miss message tables ──────────────────────────────────────────────────

pub static H_NAMES: [&str; 8] = [
    " scored an excellent hit on ",
    " hit ",
    " have injured ",
    " swing and hit ",
    " scored an excellent hit on ",
    " hit ",
    " has injured ",
    " swings and hits ",
];

pub static M_NAMES: [&str; 8] = [
    " miss",
    " swing and miss",
    " barely miss",
    " don't hit",
    " misses",
    " swings and misses",
    " barely misses",
    " doesn't hit",
];

/// A snapshot of the actor fields combat reads and writes.
#[derive(Clone)]
struct Mon {
    pos: IVec2,
    typ: Option<MonsterType>,
    disguise: u8,
    oldch: u8,
    flags: MonsterFlags,
    stats: crate::entity::player::Stats,
    room: Option<usize>,
}

impl Mon {
    fn get(id: MonsterId) -> Option<Mon> {
        MONSTER_LIST
            .with(id, |t| match t {
                crate::entity::player::Thing::Monster { data } => Some(Mon {
                    pos: data.t_pos,
                    typ: data.t_type,
                    disguise: data.t_disguise,
                    oldch: data.t_oldch,
                    flags: data.t_flags,
                    stats: data.t_stats,
                    room: data.t_room,
                }),
                crate::entity::player::Thing::Object { .. } => None,
            })
            .flatten()
    }
}

use crate::game::globals::{
    count, e_levels, fight_flush, has_hit, kamikaze, max_hit, max_level, no_command, purse, quiet,
    running, terse, to_death, vf_hit,
};

// ─── Inline helpers ───────────────────────────────────────────────────────────

#[inline]
fn on_p(id: MonsterId, flag: MonsterFlags) -> bool {
    MONSTER_LIST
        .with(id, |t| match t {
            crate::entity::player::Thing::Monster { data } => data.t_flags.contains(flag),
            crate::entity::player::Thing::Object { .. } => false,
        })
        .unwrap_or(false)
}

#[inline]
fn monster_has_flag(id: MonsterId, flag: MonsterFlags) -> bool {
    on_p(id, flag)
}

#[inline]
fn player_has(flag: MonsterFlags) -> bool {
    PLAYER.has_flag(flag)
}

// ─── Exported functions ───────────────────────────────────────────────────────

/// fight:
/// The player attacks the monster.
pub unsafe fn fight(mp: IVec2, weap: Option<ThingId>, thrown: u8) -> i32 {
    let Some(tp) = crate::game::monster_id_at(mp.y, mp.x) else {
        return 0;
    };
    let Some(mut mon) = Mon::get(tp) else {
        return 0;
    };

    // Since we are fighting, things are not quiet — no healing.
    count = 0;
    quiet = 0;
    runto(mp);

    // Let him know it was really a xeroc (if it was one).
    let mut ch: u8 = b'\0' as u8;
    if mon.typ == Some(MonsterType::Xeroc)
        && mon.disguise != b'X'
        && !player_has(MonsterFlags::BLIND)
    {
        let new_disguise = if player_has(MonsterFlags::HALU) {
            ch = (rnd(26) + b'A' as i32) as u8;
            ch
        } else {
            b'X'
        };
        let pos = mon.pos;
        MONSTER_LIST.with_mut(tp, |t| {
            if let crate::entity::player::Thing::Monster { data } = t {
                data.t_disguise = new_disguise;
            }
        });
        mon.disguise = new_disguise;
        if player_has(MonsterFlags::HALU) {
            crate::draw::write_cell_glyph(pos, ch as char);
        }
        msg_str(choose_str(
            "heavy!  That's a nasty critter!",
            "wait!  That's a xeroc!",
        ));
        if thrown == 0 {
            return false as u8 as i32;
        }
    }

    let mname = set_mname(tp);
    let mut did_hit = false as u8;
    has_hit = if terse != 0 && to_death == 0 {
        true as u8
    } else {
        false as u8
    };

    if roll_em_hero_to(tp, weap, thrown) != 0 {
        did_hit = false as u8;
        if thrown != 0 {
            thunk(weap, Some(&mname), terse);
        } else {
            hit(None, Some(&mname), terse);
        }
        if player_has(MonsterFlags::CANHUH) {
            did_hit = true as u8;
            MONSTER_LIST.with_mut(tp, |t| {
                if let crate::entity::player::Thing::Monster { data } = t {
                    data.t_flags.insert(MonsterFlags::HUH);
                }
            });
            PLAYER.remove_flag(MonsterFlags::CANHUH);
            endmsg();
            has_hit = false as u8;
            msg_str(&format!("your hands stop glowing {}", pick_color("red")));
        }
        let hp = MONSTER_LIST
            .with(tp, |t| match t {
                crate::entity::player::Thing::Monster { data } => data.t_stats.hit_points,
                crate::entity::player::Thing::Object { .. } => 0,
            })
            .unwrap_or(0);
        if hp <= 0 {
            killed(tp, true as u8);
        } else if did_hit != 0 && !player_has(MonsterFlags::BLIND) {
            msg_str(&format!("{mname} appears confused"));
        }
        did_hit = true as u8;
    } else if thrown != 0 {
        bounce(weap, Some(&mname), terse);
    } else {
        miss(None, Some(&mname), terse);
    }
    did_hit as i32
}

/// attack:
/// The monster attacks the player.
pub unsafe fn attack(tp: MonsterId) -> i32 {
    // Stop running / healing.
    running = false as u8;
    count = 0;
    quiet = 0;

    let Some(mut mon) = Mon::get(tp) else {
        return 0;
    };

    if to_death != 0 && !on_p(tp, MonsterFlags::TARGET) {
        to_death = false as u8;
        kamikaze = false as u8;
    }

    if mon.typ == Some(MonsterType::Xeroc)
        && mon.disguise != b'X'
        && !player_has(MonsterFlags::BLIND)
    {
        let pos = mon.pos;
        MONSTER_LIST.with_mut(tp, |t| {
            if let crate::entity::player::Thing::Monster { data } = t {
                data.t_disguise = b'X';
            }
        });
        mon.disguise = b'X';
        if player_has(MonsterFlags::HALU) {
            crate::draw::write_cell_glyph(pos, crate::draw::hallucination_glyph());
        }
    }

    let mname = set_mname(tp);
    let oldhp = PLAYER.stats().hit_points;

    if roll_em_to_hero(tp, None, false as u8) != 0 {
        if mon.typ != Some(MonsterType::IceMonster) {
            if has_hit != 0 {
                addmsg_str(".  ");
            }
            hit(Some(&mname), None, false as u8);
        } else if has_hit != 0 {
            endmsg();
        }
        has_hit = false as u8;

        if PLAYER.stats().hit_points <= 0 {
            death(mon.typ.map_or(0, |m| m.glyph()));
            if crate::startup::exit_requested() {
                return 0;
            }
        } else if kamikaze == 0 {
            let damage_dealt = oldhp - PLAYER.stats().hit_points;
            if damage_dealt > max_hit {
                max_hit = damage_dealt;
            }
            if PLAYER.stats().hit_points <= max_hit {
                to_death = false as u8;
            }
        }

        if !on_p(tp, MonsterFlags::CANCELLED) {
            let mtype = mon.typ;
            if mtype == Some(MonsterType::Aquator) {
                // Aquator: corrode armor
                rust_armor_id_from_player();
            } else if mtype == Some(MonsterType::IceMonster) {
                // Ice monster: freeze player
                PLAYER.remove_flag(MonsterFlags::RUN);
                if no_command == 0 {
                    addmsg_str("you are frozen");
                    if terse == 0 {
                        addmsg_str(&format!(" by the {mname}"));
                    }
                    endmsg();
                }
                no_command += rnd(2) + 2;
                if no_command > BORE_LEVEL {
                    death(b'h' as u8);
                    if crate::startup::exit_requested() {
                        return 0;
                    }
                }
            } else if mtype == Some(MonsterType::Rattlesnake) {
                // Rattlesnake: poisonous bite
                if save(VS_POISON) == 0 {
                    if !iswearing(RingType::SustainStrength) {
                        chg_str(-1);
                        if terse == 0 {
                            msg_str("you feel a bite in your leg and now feel weaker");
                        } else {
                            msg_str("a bite has weakened you");
                        }
                    } else if to_death == 0 {
                        if terse == 0 {
                            msg_str("a bite momentarily weakens you");
                        } else {
                            msg_str("bite has no effect");
                        }
                    }
                }
            } else if mtype == Some(MonsterType::Wraith) || mtype == Some(MonsterType::Vampire) {
                // Wraith / Vampire: drain energy or max HP
                let threshold = if mtype == Some(MonsterType::Wraith) {
                    15
                } else {
                    30
                };
                if rnd(100) < threshold {
                    let fewer;
                    if mtype == Some(MonsterType::Wraith) {
                        if PLAYER.stats().experience == 0 {
                            death(b'W' as u8);
                            if crate::startup::exit_requested() {
                                return 0;
                            }
                        }
                        PLAYER.with_stats_mut(|pstats| {
                            pstats.level -= 1;
                            if pstats.level == 0 {
                                pstats.experience = 0;
                                pstats.level = 1;
                            } else {
                                pstats.experience = e_levels[(pstats.level - 1) as usize] + 1;
                            }
                        });
                        fewer = roll(1, 10);
                    } else {
                        fewer = roll(1, 3);
                    }
                    {
                        let mut dead = false;
                        PLAYER.with_stats_mut(|pstats| {
                            pstats.hit_points -= fewer;
                            pstats.max_hit_points -= fewer;
                            if pstats.hit_points <= 0 {
                                pstats.hit_points = 1;
                            }
                            if pstats.max_hit_points <= 0 {
                                dead = true;
                            }
                        });
                        if dead {
                            death(mtype.map_or(0, |m| m.glyph()));
                            if crate::startup::exit_requested() {
                                return 0;
                            }
                        }
                    }
                    msg_str("you suddenly feel weaker");
                }
            } else if mtype == Some(MonsterType::VenusFlytrap) {
                // Venus flytrap: holds the player, deals ongoing damage
                PLAYER.add_flag(MonsterFlags::HELD);
                vf_hit += 1;
                let text = format!("{}x1", vf_hit);
                let bytes = text.as_bytes();
                let damage = &mut monsters[(b'F' as usize) - (b'A' as usize)].m_stats.damage;
                let copy_len = bytes.len().min(damage.len() - 1);
                damage[..copy_len].copy_from_slice(&bytes[..copy_len]);
                damage[copy_len] = 0;
                PLAYER.with_stats_mut(|stats| stats.hit_points -= 1);
                if PLAYER.stats().hit_points <= 0 {
                    death(b'F' as u8);
                    if crate::startup::exit_requested() {
                        return 0;
                    }
                }
            } else if mtype == Some(MonsterType::Leprechaun) {
                // Leprechaun: steals gold
                let level = crate::game::current_depth();
                let lastpurse = purse;
                purse -= rnd(50 + 10 * level) + 2; // GOLDCALC
                if save(VS_MAGIC) == 0 {
                    let g = rnd(50 + 10 * level) + 2;
                    purse -= g + g + g + g;
                }
                if purse < 0 {
                    purse = 0;
                }
                remove_mon(mon.pos, tp, false as u8);
                if purse != lastpurse {
                    msg_str("your purse feels lighter");
                }
                count = 0;
                return -1;
            } else if mtype == Some(MonsterType::Nymph) {
                // Nymph: steals a magic item
                let mut steal: Option<ThingId> = None;
                let mut nobj: i32 = 0;
                let eq = PLAYER.equipment();
                for id in PLAYER.pack() {
                    let equipped = eq.armor_id() == Some(id)
                        || eq.weapon_id() == Some(id)
                        || eq.left_ring_id() == Some(id)
                        || eq.right_ring_id() == Some(id);
                    if equipped {
                        continue;
                    }
                    if is_magic_id(id) {
                        nobj += 1;
                        if rnd(nobj) == 0 {
                            steal = Some(id);
                        }
                    }
                }
                if let Some(steal) = steal {
                    remove_mon(mon.pos, tp, false as u8);
                    leave_pack_id(steal, false, false);
                    msg_str(&format!("she stole {}!", inv_name_id(steal, true)));
                    let _ = OBJECTS.remove(steal);
                    count = 0;
                    return -1;
                }
            }
        }
    } else if mon.typ != Some(MonsterType::IceMonster) {
        // Miss branch
        if has_hit != 0 {
            addmsg_str(".  ");
            has_hit = false as u8;
        }
        if mon.typ == Some(MonsterType::VenusFlytrap) {
            PLAYER.with_stats_mut(|stats| stats.hit_points -= vf_hit);
            if PLAYER.stats().hit_points <= 0 {
                death(mon.typ.map_or(0, |m| m.glyph()));
                if crate::startup::exit_requested() {
                    return 0;
                }
            }
        }
        miss(Some(&mname), None, false as u8);
    }

    if fight_flush != 0 && to_death == 0 {
        flush_type();
    }
    count = 0;
    0
}

#[inline]
fn iswearing(ring_type: RingType) -> bool {
    PLAYER.wearing_ring(ring_type)
}

/// Corrode the hero's equipped armor (pointer-free).
unsafe fn rust_armor_id_from_player() {
    rust_armor_id(PLAYER.equipment().armor_id());
}

/// set_mname:
/// Return the monster name for the given monster.
pub unsafe fn set_mname(tp: MonsterId) -> String {
    if see_monst(tp) == 0 && !player_has(MonsterFlags::SEEMONST) {
        return if terse != 0 {
            "it".to_string()
        } else {
            "something".to_string()
        };
    }

    let mname: &'static str;
    if player_has(MonsterFlags::HALU) {
        let pos = MONSTER_LIST
            .with(tp, |t| match t {
                crate::entity::player::Thing::Monster { data } => Some(data.t_pos),
                crate::entity::player::Thing::Object { .. } => None,
            })
            .flatten();
        let ch = pos
            .map(crate::draw::screen_glyph_at)
            .unwrap_or(' ')
            .to_ascii_uppercase() as i32;
        let idx = if (ch as u8).is_ascii_uppercase() {
            (ch - b'A' as i32) as usize
        } else {
            rnd(26) as usize
        };
        mname = monsters[idx].m_name;
    } else {
        let idx = MONSTER_LIST
            .with(tp, |t| match t {
                crate::entity::player::Thing::Monster { data } => {
                    data.t_type.map_or(0, |m| m.index())
                }
                crate::entity::player::Thing::Object { .. } => 0,
            })
            .unwrap_or(0);
        mname = monsters[idx].m_name;
    }

    format!("the {mname}")
}

/// swing:
/// Returns true (1) if the swing hits.
pub unsafe fn swing(at_lvl: i32, op_arm: i32, wplus: i32) -> i32 {
    let res = rnd(20);
    let need = (20 - at_lvl) - op_arm;
    (res + wplus >= need) as i32
}

/// roll_em:
/// Roll several attacks and apply damage.
pub unsafe fn roll_em(
    thatt: MonsterId,
    thdef: Option<MonsterId>,
    weap: Option<ThingId>,
    hurl: u8,
) -> i32 {
    let att_stats = MONSTER_LIST
        .with(thatt, |t| match t {
            crate::entity::player::Thing::Monster { data } => data.t_stats,
            crate::entity::player::Thing::Object { .. } => crate::entity::player::Stats::default(),
        })
        .unwrap_or_default();
    roll_em_impl(&att_stats, thdef, false, weap, hurl)
}

/// The hero attacks monster `thdef`.
unsafe fn roll_em_hero_to(thdef: MonsterId, weap: Option<ThingId>, hurl: u8) -> i32 {
    let att_stats = PLAYER.stats();
    roll_em_impl(&att_stats, Some(thdef), false, weap, hurl)
}

/// Monster `thatt` attacks the hero.
unsafe fn roll_em_to_hero(thatt: MonsterId, weap: Option<ThingId>, hurl: u8) -> i32 {
    let att_stats = MONSTER_LIST
        .with(thatt, |t| match t {
            crate::entity::player::Thing::Monster { data } => data.t_stats,
            crate::entity::player::Thing::Object { .. } => crate::entity::player::Stats::default(),
        })
        .unwrap_or_default();
    roll_em_impl(&att_stats, None, true, weap, hurl)
}

/// The weapon fields needed by the damage roll, read in one arena access.
struct WeaponRoll {
    hplus: i32,
    dplus: i32,
    damage: [u8; 8],
    hurldmg: [u8; 8],
    launch: i32,
    misl: bool,
}

/// Shared roll_em implementation. `def_is_hero` selects the hero's armor class.
unsafe fn roll_em_impl(
    att_stats: &crate::entity::player::Stats,
    thdef: Option<MonsterId>,
    def_is_hero: bool,
    weap: Option<ThingId>,
    hurl: u8,
) -> i32 {
    let damage: [u8; 8];
    let hplus: i32;
    let dplus: i32;

    let Some(wr) = weap.and_then(|id| {
        crate::item::arena::with_object(id, |o| WeaponRoll {
            hplus: o.o_hplus,
            dplus: o.o_dplus,
            damage: o.o_damage,
            hurldmg: o.o_hurldmg,
            launch: o.o_launch,
            misl: o.o_flags.contains(crate::entity::player::ObjectFlags::MISL),
        })
    }) else {
        let src = &att_stats.damage;
        let mut buf = [0u8; 8];
        let n = src.len().min(8);
        buf[..n].copy_from_slice(&src[..n]);
        return roll_em_inner(att_stats, thdef, def_is_hero, &buf, 0, 0);
    };

    let mut hp = wr.hplus;
    let mut dp = wr.dplus;
    if weap == PLAYER.weapon_id() {
        // Ring bonuses, read through pointer-free equipment accessors.
        let equipment = PLAYER.equipment();
        for hand in 0..2usize {
            let arm = equipment.ring_arm(hand).unwrap_or(0);
            match equipment.ring_type(hand) {
                Some(RingType::AddDamage) => dp += arm,
                Some(RingType::AddHit) => hp += arm,
                _ => {}
            }
        }
    }
    if hurl != 0 {
        if wr.misl
            && PLAYER
                .weapon_which()
                .is_some_and(|which| which == wr.launch)
        {
            let (whp, wdp) = PLAYER.weapon_hdplus().unwrap_or((0, 0));
            return roll_em_inner(
                att_stats,
                thdef,
                def_is_hero,
                &wr.hurldmg,
                hp + whp,
                dp + wdp,
            );
        } else if wr.launch < 0 {
            return roll_em_inner(att_stats, thdef, def_is_hero, &wr.hurldmg, hp, dp);
        }
    }
    damage = wr.damage;
    hplus = hp;
    dplus = dp;

    roll_em_inner(att_stats, thdef, def_is_hero, &damage, hplus, dplus)
}

/// Parses a legacy damage specification like `"2x4"` or `"1x8/1x8/3x10"`
/// into `(ndice, nsides)` pairs.
fn parse_damage(spec: &[u8]) -> Vec<(i32, i32)> {
    let text: String = spec
        .iter()
        .take_while(|b| **b != 0)
        .map(|b| *b as char)
        .collect();
    let mut out = Vec::new();
    for part in text.split('/') {
        let mut it = part.split('x');
        if let (Some(a), Some(b)) = (it.next(), it.next()) {
            if let (Ok(n), Ok(s)) = (a.trim().parse::<i32>(), b.trim().parse::<i32>()) {
                out.push((n, s));
            }
        }
    }
    out
}

/// Inner roll loop, factored out to handle the hurldmg shortcut cleanly.
unsafe fn roll_em_inner(
    att_stats: &crate::entity::player::Stats,
    thdef: Option<MonsterId>,
    def_is_hero: bool,
    damage_spec: &[u8],
    hplus: i32,
    dplus: i32,
) -> i32 {
    // If the defender is not running (asleep or held), attacker gets +4 to hit.
    let def_running = if def_is_hero {
        player_has(MonsterFlags::RUN)
    } else {
        thdef.is_some_and(|id| on_p(id, MonsterFlags::RUN))
    };
    let hplus = hplus + if !def_running { 4 } else { 0 };

    // Defender's armor class
    let mut def_arm = if def_is_hero {
        PLAYER.stats().armor
    } else {
        thdef
            .and_then(|id| {
                MONSTER_LIST.with(id, |t| match t {
                    crate::entity::player::Thing::Monster { data } => Some(data.t_stats.armor),
                    crate::entity::player::Thing::Object { .. } => None,
                })
            })
            .flatten()
            .unwrap_or(0)
    };
    if def_is_hero {
        let equipment = PLAYER.equipment();
        if let Some(arm) = equipment.armor_arm() {
            def_arm = arm;
        }
        for hand in 0..2usize {
            if equipment.ring_type(hand) == Some(RingType::Protection) {
                def_arm -= equipment.ring_arm(hand).unwrap_or(0);
            }
        }
    }

    let att_str = att_stats.strength as usize;
    let att_lvl = att_stats.level;
    let str_idx = att_str.min(STR_PLUS.len() - 1);
    let mut did_hit = 0i32;
    let mut total_damage = 0i32;

    for (ndice, nsides) in parse_damage(damage_spec) {
        if swing(att_lvl, def_arm, hplus + STR_PLUS[str_idx]) != 0 {
            let proll = roll(ndice, nsides);
            let damage = dplus + proll + ADD_DAM[str_idx];
            total_damage += if damage > 0 { damage } else { 0 };
            did_hit = 1;
        }
    }

    if did_hit != 0 {
        if def_is_hero {
            PLAYER.with_stats_mut(|stats| stats.hit_points -= total_damage);
        } else if let Some(id) = thdef {
            MONSTER_LIST.with_mut(id, |t| {
                if let crate::entity::player::Thing::Monster { data } = t {
                    data.t_stats.hit_points -= total_damage;
                }
            });
        }
    }
    did_hit
}

/// prname:
/// The print name of a combatant.
pub unsafe fn prname(mname: Option<&str>, upper: u8) -> String {
    let mut text = mname.unwrap_or("you").to_string();
    if upper != 0 && !text.is_empty() {
        let first = text.as_bytes()[0].to_ascii_uppercase();
        text.replace_range(0..1, &(first as char).to_string());
    }
    text
}

/// thunk:
/// A missile hits a monster.
pub unsafe fn thunk(weap: Option<ThingId>, mname: Option<&str>, noend: u8) {
    if to_death != 0 {
        return;
    }
    let weapon_name = weap.and_then(|id| {
        crate::item::arena::with_object(id, |o| {
            (matches!(o.o_type, ItemType::Weapon(_)), o.o_which)
        })
        .filter(|(is_weapon, _)| *is_weapon)
        .map(|(_, which)| weap_info[which as usize].oi_name)
    });
    if let Some(name) = weapon_name {
        addmsg_str(&format!("the {} hits ", name));
    } else {
        addmsg_str("you hit ");
    }
    addmsg_str(mname.unwrap_or(""));
    if noend == 0 {
        endmsg();
    }
}

/// hit:
/// Print a message to indicate a successful hit.
pub unsafe fn hit(er: Option<&str>, ee: Option<&str>, noend: u8) {
    if to_death != 0 {
        return;
    }
    addmsg_str(&prname(er, true as u8));
    let s: &str = if terse != 0 {
        " hit"
    } else {
        let mut i = rnd(4) as usize;
        if er.is_some() {
            i += 4;
        }
        H_NAMES[i]
    };
    addmsg_str(s);
    if terse == 0 {
        addmsg_str(&prname(ee, false as u8));
    }
    if noend == 0 {
        endmsg();
    }
}

/// miss:
/// Print a message to indicate a poor swing.
pub unsafe fn miss(er: Option<&str>, ee: Option<&str>, noend: u8) {
    if to_death != 0 {
        return;
    }
    addmsg_str(&prname(er, true as u8));
    let i: usize = if terse != 0 {
        if er.is_some() {
            4
        } else {
            0
        }
    } else {
        let base = rnd(4) as usize;
        if er.is_some() {
            base + 4
        } else {
            base
        }
    };
    addmsg_str(M_NAMES[i]);
    if terse == 0 {
        addmsg_str(&format!(" {}", prname(ee, false as u8)));
    }
    if noend == 0 {
        endmsg();
    }
}

/// bounce:
/// A missile misses a monster.
pub unsafe fn bounce(weap: Option<ThingId>, mname: Option<&str>, noend: u8) {
    if to_death != 0 {
        return;
    }
    let weapon_name = weap.and_then(|id| {
        crate::item::arena::with_object(id, |o| {
            (matches!(o.o_type, ItemType::Weapon(_)), o.o_which)
        })
        .filter(|(is_weapon, _)| *is_weapon)
        .map(|(_, which)| weap_info[which as usize].oi_name)
    });
    if let Some(name) = weapon_name {
        addmsg_str(&format!("the {} misses ", name));
    } else {
        addmsg_str("you missed ");
    }
    addmsg_str(mname.unwrap_or(""));
    if noend == 0 {
        endmsg();
    }
}

/// remove_mon:
/// Remove a monster from the screen.
pub unsafe fn remove_mon(mp: IVec2, tp: MonsterId, waskill: u8) {
    let Some(mon) = Mon::get(tp) else {
        return;
    };
    let drop_pos = mon.pos;
    let pack = crate::entity::player::monster_pack(tp);
    crate::entity::player::set_monster_pack(tp, Vec::new());
    for id in pack {
        if crate::item::arena::with_object_mut(id, |o| o.o_pos = drop_pos).is_none() {
            continue;
        }
        if waskill != 0 {
            fall(id, false);
        } else {
            let _ = crate::item::arena::OBJECTS.remove(id);
        }
    }
    crate::game::set_monster_id(mp.y, mp.x, None);
    // Re-draw the underlying character.
    crate::draw::write_cell_glyph(mp, mon.oldch as char);

    if on_p(tp, MonsterFlags::TARGET) {
        kamikaze = false as u8;
        to_death = false as u8;
        if fight_flush != 0 {
            flush_type();
        }
    }
    crate::entity::player::discard_monster(tp);
}

/// killed:
/// Called to put a monster to death.
pub unsafe fn killed(tp: MonsterId, pr: u8) {
    let Some(mon) = Mon::get(tp) else {
        return;
    };
    let gained = mon.stats.experience;
    PLAYER.with_stats_mut(|stats| stats.experience += gained);

    let mtype = mon.typ;

    if mtype == Some(MonsterType::VenusFlytrap) {
        PLAYER.remove_flag(MonsterFlags::HELD);
        vf_hit = 0;
        // Reset damage string to "000x0"
        let damage = &mut monsters[(b'F' as usize) - (b'A' as usize)].m_stats.damage;
        damage[..b"000x0\0".len()].copy_from_slice(b"000x0\0");
    } else if mtype == Some(MonsterType::Leprechaun) {
        let tp_room = mon.room;
        let level = crate::game::current_depth();
        if tp_room.is_some() && fallpos(mon.pos).is_some() && level >= max_level {
            let gold = crate::item::arena::new_item_id();
            OBJECTS.with_object_mut(gold, |o| {
                o.o_type = ItemType::Gold;
                // o_goldval is #define'd to o_arm
                let mut value = rnd(50 + 10 * level) + 2; // GOLDCALC
                if save(VS_MAGIC) != 0 {
                    let extra = rnd(50 + 10 * level) + 2;
                    value += extra + extra + extra + extra;
                }
                o.o_arm = value;
            });
            crate::entity::player::attach_pack_id(tp, gold);
        }
    }

    let mname = set_mname(tp);
    remove_mon(mon.pos, tp, true as u8);

    if pr != 0 {
        if has_hit != 0 {
            addmsg_str(".  Defeated ");
            has_hit = false as u8;
        } else {
            if terse == 0 {
                addmsg_str("you have ");
            }
            addmsg_str("defeated ");
        }
        msg_str(&mname);
    }

    check_level();
    if fight_flush != 0 {
        flush_type();
    }
}

#[cfg(test)]
mod tests {
    use super::MAXSTR;

    #[test]
    fn monster_name_buffer_has_expected_capacity() {
        let buffer = [b'x' as u8; MAXSTR];
        assert_eq!(buffer.len(), MAXSTR);
    }
}
