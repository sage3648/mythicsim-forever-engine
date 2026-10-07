//! Go sim/druid/items.go and item_sets.go: the idols and the Wolfshead Helm the class wires,
//! and the item sets it registers.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use crate::prepare::character::constants::SPELL_CRIT_RATING_PER_CRIT_PERCENT;
use crate::prepare::env::Environment;
use crate::prepare::item_sets::ItemSet;
use crate::prepare::sim::{AuraConfig, AuraId, Sim, UnitId};
use crate::prepare::spell::ProcMask;
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::{masks, Druid};

impl Druid {
    /// The `core.NewItemEffect` registrations of Go's druid package: Idol of the Moon (23197),
    /// Idol of Ferocity (22397), Idol of Brutality (23198) and Wolfshead Helm (8345). Answers
    /// whether the class registers an effect for the item.
    pub(super) fn apply_idol_effect(&mut self, sim: &mut Sim, unit: UnitId, item: i32) -> bool {
        match item {
            23197 => {
                // Idol of the Moon.
                let aura = sim.register_aura(
                    unit,
                    AuraConfig {
                        label: "Improved Moonfire".to_string(),
                        ..AuraConfig::default()
                    },
                );
                sim.attach_spell_mod(
                    aura,
                    SpellModConfig {
                        class_mask: masks::MOONFIRE,
                        kind: SpellModType::BaseDamageFlat,
                        float_value: 33.0,
                        ..SpellModConfig::default()
                    },
                );
                sim.make_permanent(aura);
                true
            }
            22397 => {
                // Idol of Ferocity: reduces the energy cost of Claw and Rake by 2.
                sim.add_static_mod(
                    unit,
                    SpellModConfig {
                        class_mask: masks::CLAW | masks::RAKE,
                        kind: SpellModType::PowerCostFlat,
                        int_value: -2,
                        ..SpellModConfig::default()
                    },
                );
                true
            }
            23198 => {
                // Idol of Brutality: reduces the rage cost of Maul, Swipe and Primal Bite by 2.
                sim.add_static_mod(
                    unit,
                    SpellModConfig {
                        class_mask: masks::MAUL | masks::SWIPE | masks::PRIMAL_BITE,
                        kind: SpellModType::PowerCostFlat,
                        int_value: -2,
                        ..SpellModConfig::default()
                    },
                );
                true
            }
            8345 => {
                // Wolfshead Helm: the bonus belongs to Shifting Power and Enrage.
                let gain = Rc::clone(&self.st);
                let expire = Rc::clone(&self.st);
                let aura = sim.register_aura(
                    unit,
                    AuraConfig {
                        label: "Wolfshead Helm".to_string(),
                        action_id: Some(ActionId::spell(17768)),
                        on_gain: Some(Rc::new(move |_: &mut Sim, _| {
                            gain.wolfshead_shifting_power_energy
                                .set(gain.wolfshead_shifting_power_energy.get() + 20.0);
                            gain.wolfshead_enrage_rage
                                .set(gain.wolfshead_enrage_rage.get() + 5.0);
                        })),
                        on_expire: Some(Rc::new(move |_: &mut Sim, _| {
                            expire
                                .wolfshead_shifting_power_energy
                                .set(expire.wolfshead_shifting_power_energy.get() - 20.0);
                            expire
                                .wolfshead_enrage_rage
                                .set(expire.wolfshead_enrage_rage.get() - 5.0);
                        })),
                        ..AuraConfig::default()
                    },
                );
                sim.make_permanent(aura);
                true
            }
            _ => false,
        }
    }
}

fn stats(pairs: &[(Stat, f64)]) -> Stats {
    Stats::from_pairs(pairs)
}

fn feralheart_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        stats(&[
            (Stat::ArcaneResistance, 8.0),
            (Stat::FireResistance, 8.0),
            (Stat::FrostResistance, 8.0),
            (Stat::NatureResistance, 8.0),
            (Stat::ShadowResistance, 8.0),
        ]),
    );
}

fn feralheart_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::MP5, 8.0);
}

/// Nature's Bounty: 2% chance to restore 200 mana on spell cast, 20 energy on a white hit, or
/// 10 rage when struck.
fn feralheart_4(env: &mut Environment, aura: AuraId) {
    let action = ActionId::spell(450608);
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: action.clone(),
            name: "Nature's Bounty (Mana)".to_string(),
            callback: CallbackMask::ON_CAST_COMPLETE,
            proc_mask: ProcMask::SPELL_DAMAGE | ProcMask::SPELL_HEALING,
            proc_chance: 0.02,
            ..ProcTrigger::default()
        },
    );
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: action.clone(),
            name: "Nature's Bounty (Energy)".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            proc_mask: ProcMask::MELEE_WHITE_HIT,
            proc_chance: 0.02,
            ..ProcTrigger::default()
        },
    );
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: action,
            name: "Nature's Bounty (Rage)".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            proc_mask: ProcMask::MELEE,
            proc_chance: 0.02,
            ..ProcTrigger::default()
        },
    );
}

fn nothing(_: &mut Environment, _: AuraId) {}

fn feralheart_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        stats(&[
            (Stat::SpellDamage, 23.0),
            (Stat::HealingPower, 23.0),
            (Stat::AttackPower, 40.0),
            (Stat::RangedAttackPower, 40.0),
        ]),
    );
}

fn cenarion_5(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(
        aura,
        Stat::SpellCritRating,
        2.0 * SPELL_CRIT_RATING_PER_CRIT_PERCENT,
    );
}

fn stormrage_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_additive_pseudo_stat_buff(
        aura,
        PseudoStatField::Custom(|p| &mut p.spirit_regen_rate_casting),
        0.15,
    );
}

fn symbols_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Symbols of Unending Life Finisher Bonus".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::MISS | HitOutcome::DODGE | HitOutcome::BLOCK | HitOutcome::PARRY,
            class_spell_mask: masks::FEROCIOUS_BITE | masks::RIP,
            ..ProcTrigger::default()
        },
    );
}

fn refuge_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        stats(&[(Stat::HealingPower, 44.0), (Stat::SpellDamage, 15.0)]),
    );
}

fn stamina_20(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Stamina, 20.0);
}

fn sanctuary_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        stats(&[(Stat::AttackPower, 40.0), (Stat::RangedAttackPower, 40.0)]),
    );
}

/// Go's druid `core.NewItemSet` calls, in Go's registration order.
pub(crate) static ITEM_SETS: &[ItemSet] = &[
    ItemSet {
        id: 0,
        name: "Feralheart Raiment",
        alternative_name: "",
        bonuses: &[
            (2, feralheart_2),
            (3, feralheart_3),
            (4, feralheart_4),
            (5, nothing),
            (6, feralheart_6),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Cenarion Raiment",
        alternative_name: "",
        bonuses: &[(3, nothing), (5, cenarion_5), (8, nothing)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Stormrage Raiment",
        alternative_name: "",
        bonuses: &[(3, stormrage_3), (5, nothing), (8, nothing)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Symbols of Unending Life",
        alternative_name: "",
        bonuses: &[(3, symbols_3)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Champion's Refuge",
        alternative_name: "",
        bonuses: &[(2, refuge_2), (4, nothing), (6, stamina_20)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Lieutenant Commander's Refuge",
        alternative_name: "",
        bonuses: &[(2, refuge_2), (4, nothing), (6, stamina_20)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Field Marshal's Sanctuary",
        alternative_name: "",
        bonuses: &[(2, stamina_20), (3, nothing), (6, sanctuary_6)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Warlord's Sanctuary",
        alternative_name: "",
        bonuses: &[(2, stamina_20), (3, nothing), (6, sanctuary_6)],
        required_profession: "",
    },
];
