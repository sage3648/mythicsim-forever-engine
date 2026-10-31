//! The healer Priest's heals and heal talents, which Rust alone sims (see `prepare::heals`): every
//! rank of Lesser Heal, Heal, Greater Heal, Flash Heal, Renew, Prayer of Healing and Power Word:
//! Shield, and the talents' Binding Heal and Prayer of Mending, from the client rows of build
//! 1.60.1.70425.
//!
//! Each heal lands on players who need it: Prayer of Healing on the party of five, Binding Heal
//! on its target and the priest, Prayer of Mending on each of the five players its charges reach
//! (under Forever's rules, as the party's damage moves them: `prepare::party_damage`).
//! Power Word: Shield goes up as often as its cooldown lets it, each time on another player, so
//! Weakened Soul never stops it. Penance's heal, Desperate Prayer, Inner Focus's free heal and
//! the talents that need a heal's target or its history (Divine Aegis, Renewed Hope, Litany of
//! Light) are not modelled yet.

use crate::contracts::prepared_v2::SmartHeal;
use crate::core::time::NS_PER_SECOND;
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::heals::{
    apply_heal_talent, register_channel_heal, register_heal, register_heal_from, talent_rank,
    ChannelHeal, HealShape,
};
use crate::prepare::sim::{Duration, Rules, Sim, UnitId};
use crate::prepare::spelldata::store::must_find;
use crate::prepare::spelldata::Spell as Row;

use super::masks;
use super::Priest;

const LESSER_HEAL: &[i32] = &[2050, 2052, 2053];
const HEAL: &[i32] = &[2054, 2055, 6063, 6064];
const GREATER_HEAL: &[i32] = &[2060, 10963, 10964, 10965, 25314];
const FLASH_HEAL: &[i32] = &[2061, 9472, 9473, 9474, 10915, 10916, 10917];
const RENEW: &[i32] = &[
    139, 6074, 6075, 6076, 6077, 6078, 10927, 10928, 10929, 25315,
];
const PRAYER_OF_HEALING: &[i32] = &[596, 996, 10960, 10961, 25316];
const POWER_WORD_SHIELD: &[i32] = &[17, 592, 600, 3747, 6065, 6066, 10898, 10899, 10900, 10901];
/// Binding Heal, a Forever talent: Flash Heal's numbers on the target and the priest.
const BINDING_HEAL: &[i32] = &[401937, 1240770, 1240771, 1240772, 1240773, 1240774];
/// Holy Binding (1328569, push 113554): the priest's heal is 50% of the Flash Heal.
const HOLY_BINDING_SHARE: f64 = 0.5;
/// Prayer of Mending, a Forever talent: the heal is the cast row's dummy effect, and the buff's
/// five charges each heal a player.
const PRAYER_OF_MENDING: &[i32] = &[401859, 1240826, 1240827];
const PRAYER_OF_MENDING_CHARGES: u32 = 5;
/// How long Prayer of Mending's buff waits on each player: the duration of the buff row the
/// cast's tooltip names (401877 for rank 1, 30 seconds).
fn mending_duration(row: &Row) -> Duration {
    row.refs()
        .into_iter()
        .map(|buff| buff.duration())
        .find(|&duration| duration > 0)
        .unwrap_or(30 * NS_PER_SECOND)
}

/// Penance, a Discipline talent, by rank: the cast rows.
const PENANCE: &[i32] = &[402174, 1240720, 1240721, 1316995];
/// Penance's channel ticks after the bolt that lands with the cast.
const PENANCE_HEAL_TICKS: i32 = 2;

/// Prayer of Healing reaches the priest's party.
const PARTY: u32 = 5;

/// The heal talents, as (talent, client row, ranks). Each applies every spell modifier its row
/// states to the heals its client masks name: Spiritual Healing to the direct heals and to
/// Renew's ticks, Holy Specialization's crit, Improved Renew, Divine Fury's cast time, Improved
/// Healing's and Mental Agility's cost, Improved Power Word: Shield's absorb, Soul Warding's
/// cooldown and cost, and Twin Disciplines' healing.
const HEAL_TALENTS: &[(&str, i32, i32)] = &[
    ("spiritual_healing", 14898, 3),
    ("holy_specialization", 14889, 5),
    ("improved_renew", 14908, 3),
    ("divine_fury", 18530, 5),
    ("improved_healing", 14912, 3),
    ("mental_agility", 14520, 3),
    ("improved_power_word_shield", 14748, 3),
    ("soul_warding", 402000, 1),
    ("twin_disciplines", 1225132, 5),
];

impl Priest {
    /// Registers every heal rank the healer knows.
    pub(super) fn register_heals(&mut self, sim: &mut Sim, unit: UnitId) {
        let mut ranks = Vec::new();
        let mut each = |sim: &mut Sim, ids: &[i32], shape: HealShape, shared_cd: bool| {
            let timer = shared_cd.then(|| sim.new_timer(unit));
            for &id in ids {
                ranks.push(register_heal(
                    sim,
                    unit,
                    must_find(id),
                    masks::HEAL,
                    shape,
                    timer,
                ));
            }
        };
        each(sim, LESSER_HEAL, HealShape::SINGLE, false);
        each(sim, HEAL, HealShape::SINGLE, false);
        each(sim, GREATER_HEAL, HealShape::SINGLE, false);
        // Push 113554 under Forever's rules replaces the Binding Heal talent with Holy Binding
        // (1328569) in the same field: each Flash Heal on another player heals the priest for
        // 50% as much (1328570), outside Shadowform, which a healer never takes. The healer's
        // Flash Heal lands on a player who needs it, so the priest's half is a second jump at
        // half the heal. Binding Heal's ranks leave the tree.
        let holy_binding = sim.rules == Rules::Forever && self.talents.bool("binding_heal");
        let flash_heal = if holy_binding {
            HealShape {
                targets: 2,
                jump_multiplier: HOLY_BINDING_SHARE,
                smart: None,
            }
        } else {
            HealShape::SINGLE
        };
        each(sim, FLASH_HEAL, flash_heal, false);
        each(sim, RENEW, HealShape::SINGLE, false);
        each(sim, PRAYER_OF_HEALING, HealShape::party(PARTY), false);
        each(sim, POWER_WORD_SHIELD, HealShape::SINGLE, true);
        if self.talents.bool("binding_heal") && sim.rules == Rules::GoReference {
            each(sim, BINDING_HEAL, HealShape::party(2), false);
        }
        // Penance on a friend, a Discipline talent: its heal bolt with the cast and on each of
        // the channel's two ticks a second apart, ranks sharing their category cooldown. The
        // cast's tooltip names the damage bolt first, then the heal bolt.
        if self.talents.bool("penance") {
            let timer = sim.new_timer(unit);
            for &id in PENANCE {
                let row = must_find(id);
                let bolt = row.refs()[1].heal_effect();
                let amount = bolt.average(CHARACTER_LEVEL);
                ranks.push(register_channel_heal(
                    sim,
                    unit,
                    row,
                    ChannelHeal {
                        direct: Some((amount, bolt.variance, bolt.coeff())),
                        tick: amount,
                        tick_coefficient: bolt.coeff(),
                        ticks: PENANCE_HEAL_TICKS,
                        period: NS_PER_SECOND,
                        tick_can_crit: true,
                    },
                    masks::HEAL,
                    HealShape::SINGLE,
                    Some(timer),
                ));
            }
        }
        if self.talents.bool("prayer_of_mending") {
            let timer = sim.new_timer(unit);
            for &id in PRAYER_OF_MENDING {
                let row = must_find(id);
                let shape = HealShape::party(PRAYER_OF_MENDING_CHARGES).smart(SmartHeal::Mending {
                    charges: PRAYER_OF_MENDING_CHARGES,
                    duration: mending_duration(row),
                });
                ranks.push(register_heal_from(
                    sim,
                    unit,
                    row,
                    row.effect_n(1),
                    masks::HEAL,
                    shape,
                    Some(timer),
                ));
            }
        }
        self.heals = ranks;
    }

    /// Applies the heal talents the healer took.
    pub(super) fn apply_heal_talents(&self, sim: &mut Sim, unit: UnitId) {
        for &(name, spell_id, max_ranks) in HEAL_TALENTS {
            apply_heal_talent(
                sim,
                unit,
                spell_id,
                max_ranks,
                talent_rank(&self.talents, name),
            );
        }
    }
}
