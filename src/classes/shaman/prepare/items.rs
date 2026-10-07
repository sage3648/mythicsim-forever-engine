//! Go `sim/shaman/item_sets.go` and `items.go`: the Shaman's item sets and the item effects it
//! registers, Go's `core.NewItemSet` and `core.NewItemEffect` calls.
//!
//! Most bonuses are totem radius or healing, neither modelled (master applies none either).

use std::cell::Cell;
use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::env::Environment;
use crate::prepare::item_sets::ItemSet;
use crate::prepare::sim::{AuraConfig, AuraId, Cooldown, Sim, UnitId, SECOND};
use crate::prepare::spell::{school, CastConfig, ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::spells::spell_action;
use super::{masks, Shaman};

/// The Shaman's sets: Go's `core.NewItemSet` calls.
pub(crate) static ITEM_SETS: &[ItemSet] = &[
    ItemSet {
        id: 0,
        name: "The Five Thunders",
        alternative_name: "",
        bonuses: &[
            (2, five_thunders_2),
            (3, five_thunders_3),
            (4, five_thunders_4),
            (5, no_bonus),
            (6, five_thunders_6),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "The Earthfury",
        alternative_name: "",
        bonuses: &[(3, no_bonus), (5, no_bonus), (8, no_bonus)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "The Ten Storms",
        alternative_name: "",
        bonuses: &[(3, no_bonus), (5, ten_storms_5), (8, no_bonus)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Stormcaller's Garb",
        alternative_name: "",
        bonuses: &[(3, stormcallers_garb_3), (5, no_bonus)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Gift of the Gathering Storm",
        alternative_name: "",
        bonuses: &[(3, gathering_storm_3)],
        required_profession: "",
    },
    // PvP sets: +40 Attack Power, +2% crit on Shock spells at 4 pieces, +20 Stamina.
    ItemSet {
        id: 0,
        name: "Champion's Earthshaker",
        alternative_name: "",
        bonuses: &[(2, pvp_attack_power), (4, pvp_shock_crit), (6, pvp_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Champion's Stormcaller",
        alternative_name: "",
        bonuses: &[(2, pvp_attack_power), (4, pvp_shock_crit), (6, pvp_stamina)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Warlord's Earthshaker",
        alternative_name: "",
        bonuses: &[(2, pvp_stamina), (4, pvp_shock_crit), (6, pvp_attack_power)],
        required_profession: "",
    },
];

/// A bonus Go leaves empty.
fn no_bonus(_env: &mut Environment, _aura: AuraId) {}

/// +8 All Resistances.
fn five_thunders_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[
            (Stat::ArcaneResistance, 8.0),
            (Stat::FireResistance, 8.0),
            (Stat::FrostResistance, 8.0),
            (Stat::NatureResistance, 8.0),
            (Stat::ShadowResistance, 8.0),
        ]),
    );
}

/// Restores 8 mana per 5 sec.
fn five_thunders_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::MP5, 8.0);
}

/// 4% chance on spell cast to increase damage and healing by up to 65 for 10 sec.
fn five_thunders_4(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    let sim = &mut env.sim;
    sim.new_temporary_stats_aura(
        unit,
        "The Furious Storm",
        &spell_action(27775),
        Stats::from_pairs(&[(Stat::SpellDamage, 65.0), (Stat::HealingPower, 65.0)]),
        10 * SECOND,
    );
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: spell_action(450626),
            name: "Item - The Furious Storm Proc (Spell Cast)".to_string(),
            callback: CallbackMask::ON_CAST_COMPLETE,
            proc_mask: ProcMask(ProcMask::SPELL_DAMAGE.0 | ProcMask::SPELL_HEALING.0),
            proc_chance: 0.04,
            ..ProcTrigger::default()
        },
    );
}

/// +23 damage and healing done by magical spells and effects.
fn five_thunders_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[(Stat::SpellDamage, 23.0), (Stat::HealingPower, 23.0)]),
    );
}

/// Improves your chance to get a critical strike with Nature spells by 3%.
fn ten_storms_5(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::BonusCritPercent,
            school: school::NATURE,
            float_value: 3.0,
            ..SpellModConfig::default()
        },
    );
}

/// Lightning Bolt, Chain Lightning and Shock hits have a 20% chance to grant up to 50 Nature
/// damage for 8 sec.
fn stormcallers_garb_3(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    let sim = &mut env.sim;
    sim.new_temporary_stats_aura(
        unit,
        "Stormcaller's Wrath",
        &spell_action(26121),
        Stats::from_pairs(&[(Stat::NatureDamage, 50.0)]),
        8 * SECOND,
    );
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Stormcaller Spelldamage Bonus".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            class_spell_mask: masks::LIGHTNING_BOLT
                | masks::CHAIN_LIGHTNING
                | masks::OVERLOAD
                | masks::SHOCK,
            // Master's overloads share the Lightning Bolt / Chain Lightning spell code.
            can_proc_from_procs: true,
            proc_chance: 0.20,
            ..ProcTrigger::default()
        },
    );
}

/// Increases the chain target damage multiplier of your Chain Lightning spell by 5%.
fn gathering_storm_3(env: &mut Environment, aura: AuraId) {
    let bonus: Rc<Cell<f64>> = env
        .agent
        .as_any_mut()
        .and_then(|agent| agent.downcast_mut::<Shaman>())
        .expect("a Shaman set bonus applies to a Shaman")
        .chain_lightning_bounce_bonus
        .clone();
    let sim = &mut env.sim;
    // AttachAdditivePseudoStatBuff(&shaman.ChainLightningBounceBonus, 0.05)
    let on_gain = bonus.clone();
    sim.apply_on_gain(
        aura,
        Rc::new(move |_: &mut Sim, _| on_gain.set(on_gain.get() + 0.05)),
    );
    let on_expire = bonus.clone();
    sim.apply_on_expire(
        aura,
        Rc::new(move |_: &mut Sim, _| on_expire.set(on_expire.get() - 0.05)),
    );
    if sim.aura(aura).active {
        bonus.set(bonus.get() + 0.05);
    }
}

fn pvp_attack_power(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[(Stat::AttackPower, 40.0), (Stat::RangedAttackPower, 40.0)]),
    );
}

fn pvp_shock_crit(env: &mut Environment, aura: AuraId) {
    env.sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::BonusCritPercent,
            class_mask: masks::SHOCK,
            float_value: 2.0,
            ..SpellModConfig::default()
        },
    );
}

fn pvp_stamina(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Stamina, 20.0);
}

/// The shaman package's `core.NewItemEffect` calls.
pub(crate) fn apply_item_effect(sim: &mut Sim, unit: UnitId, item: i32) -> bool {
    match item {
        23199 => {
            totem_of_the_storm(sim, unit);
            true
        }
        19956 => {
            wushoolays_charm_of_spirits(sim, unit);
            true
        }
        _ => false,
    }
}

/// Totem of the Storm: +33 damage to Lightning Bolt and Chain Lightning.
fn totem_of_the_storm(sim: &mut Sim, unit: UnitId) {
    let aura = sim.register_aura(
        unit,
        AuraConfig {
            label: "Increased Lightning Damage".to_string(),
            ..AuraConfig::default()
        },
    );
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::BaseDamageFlat,
            float_value: 33.0,
            class_mask: masks::LIGHTNING_BOLT | masks::CHAIN_LIGHTNING | masks::OVERLOAD,
            ..SpellModConfig::default()
        },
    );
    sim.make_permanent(aura);
}

/// Wushoolay's Charm of Spirits: Use: increases the damage dealt by your Lightning Shield spell
/// by 100% for 20 sec (24499). 3 min cooldown, 20 sec on the burst trinket category. The
/// client's mod is additive (aura 108), so it adds to Improved Lightning Shield.
fn wushoolays_charm_of_spirits(sim: &mut Sim, unit: UnitId) {
    let duration = 20 * SECOND;
    let aura = sim.register_aura(
        unit,
        AuraConfig {
            label: "Energized Shield".to_string(),
            action_id: Some(spell_action(24499)),
            duration,
            ..AuraConfig::default()
        },
    );
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::DamageDoneFlat,
            float_value: 1.0,
            class_mask: masks::LIGHTNING_SHIELD,
            ..SpellModConfig::default()
        },
    );

    let timer = sim.new_timer(unit);
    let shared = sim.get_offensive_trinket_cd(unit);
    let spell = sim.register_spell(
        unit,
        SpellConfig {
            action_id: crate::contracts::prepared_v2::ActionId::item(19956),
            spell_school: school::NATURE,
            proc_mask: ProcMask::EMPTY,
            flags: SpellFlag::NO_ON_CAST_COMPLETE,
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(timer),
                    duration: 180 * SECOND,
                },
                shared_cd: Cooldown {
                    timer: Some(shared),
                    duration,
                },
                ..CastConfig::default()
            },
            ..SpellConfig::default()
        },
    );
    sim.add_major_cooldown(
        unit,
        MajorCooldown {
            spell,
            priority: 0,
            cooldown_type: cooldown_type::DPS,
            allow_spell_queueing: false,
            timings: Vec::new(),
        },
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::contracts::request::Request;
    use crate::prepare::character::cooldown_type;
    use crate::prepare::env::Environment;
    use crate::prepare::sim::{NEVER_EXPIRES, SECOND};
    use crate::prepare::spell::school;

    const REQUEST: &str =
        include_str!("../../../../tests/classes/shaman/prepare/elemental.request.json");

    /// The stripped Elemental request wearing exactly these items.
    fn prepared_with(items: &[i32]) -> Environment {
        let mut value: serde_json::Value = serde_json::from_str(REQUEST).expect("a request");
        let items: Vec<_> = items.iter().map(|id| json!({ "id": id })).collect();
        value["raid"]["parties"][0]["players"][0]["equipment"] = json!({ "items": items });
        let bytes = serde_json::to_vec(&value).expect("json");
        let request = Request::from_json(&bytes).expect("parsed");
        Environment::new(request.message(), crate::classes::prepare_agent).expect("prepared")
    }

    /// Totem of the Storm (items.go): a permanent aura carrying the Lightning Bolt and Chain
    /// Lightning damage mod.
    #[test]
    fn totem_of_the_storm_registers_a_permanent_aura() {
        let env = prepared_with(&[23199]);
        let aura = env
            .sim
            .get_aura(env.player, "Increased Lightning Damage")
            .map(|id| env.sim.aura(id))
            .expect("the totem's aura");
        assert!(aura.active);
        assert_eq!(aura.duration, NEVER_EXPIRES);
    }

    /// Wushoolay's Charm of Spirits (items.go): Energized Shield for 20 seconds, and a use spell
    /// on a 3 minute cooldown that shares the burst trinket timer, a DPS cooldown.
    #[test]
    fn wushoolays_charm_registers_its_aura_and_use_spell() {
        let env = prepared_with(&[19956]);
        let sim = &env.sim;
        let aura = sim
            .get_aura(env.player, "Energized Shield")
            .map(|id| sim.aura(id))
            .expect("the charm's aura");
        assert_eq!(aura.duration, 20 * SECOND);
        let spell = sim
            .unit(env.player)
            .spellbook
            .iter()
            .map(|id| sim.spell(*id))
            .find(|spell| spell.action_id.item_id == 19956)
            .expect("the use spell");
        assert_eq!(spell.spell_school, school::NATURE);
        assert_eq!(spell.cd.duration, 180 * SECOND);
        assert_eq!(spell.shared_cd.duration, 20 * SECOND);
        let cooldowns = &sim.character(env.player).initial_major_cooldowns;
        assert!(cooldowns.iter().any(|mcd| {
            sim.spell(mcd.spell).action_id.item_id == 19956
                && mcd.cooldown_type == cooldown_type::DPS
        }));
    }

    /// The Five Thunders' 4 piece (item_sets.go) holds a 10 second Furious Storm aura.
    #[test]
    fn the_five_thunders_four_piece_registers_the_furious_storm() {
        let env = prepared_with(&[22095, 22096, 22097, 22098]);
        let aura = env
            .sim
            .get_aura(env.player, "The Furious Storm")
            .map(|id| env.sim.aura(id))
            .expect("the set's aura");
        assert_eq!(aura.duration, 10 * SECOND);
    }

    /// Gift of the Gathering Storm's 3 piece raises Chain Lightning's bounce multiplier by 5%,
    /// which the exporter reads after the reset.
    #[test]
    fn the_gathering_storm_set_raises_the_chain_lightning_bounce() {
        let bounce = |items: &[i32]| {
            let env = prepared_with(items);
            let effects = env.agent.export_effects(&env, &mut Vec::new());
            effects
                .iter()
                .find(|effect| effect["kind"] == "chain_lightning")
                .and_then(|effect| effect["bounce_bonus"].as_f64())
                .expect("the chain lightning effect")
        };
        assert_eq!(bounce(&[21398, 21399, 21400]), 0.05);
        assert_eq!(bounce(&[21398, 21399]), 0.0);
    }
}
