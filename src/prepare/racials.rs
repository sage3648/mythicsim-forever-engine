//! Go sim/core/racials.go.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;

use super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField, StatMultiplier};
use super::character::{cooldown_type, MajorCooldown};
use super::env::Environment;
use super::items::slot;
use super::sim::{
    AuraConfig, AuraId, BuildPhase, Cooldown, EventCallbacks, Sim, UnitId, MILLISECOND,
    NEVER_EXPIRES, SECOND,
};
use super::spell::{
    school, Cast, CastConfig, DefenseType, ProcMask, Resource, SpellConfig, SpellFlag, GCD_DEFAULT,
};
use super::spell_mod::{SpellModConfig, SpellModType};
use super::stats::{SchoolIndex, Stat, Stats};
use super::Refusal;

const MINUTE: i64 = 60 * SECOND;

/// Go `EurekaSpells`: names, as the class's own spell masks, the abilities the client lists on
/// the three effects of its Eureka! rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EurekaSpells {
    /// Effect 0, SPELLMOD_COST -10%.
    pub cost: i64,
    /// Effect 1, SPELLMOD_DAMAGE +10%: the spells whose direct hits gain the bonus.
    pub damage: i64,
    /// Effect 2, SPELLMOD_DOT +10%: the spells whose dot ticks gain the bonus.
    pub tick: i64,
}

/// Go `applyRaceEffects`. A race with no racial effects registers nothing, as in Go.
pub(crate) fn apply_race_effects(env: &mut Environment) -> Result<(), Refusal> {
    let unit = env.player;
    let race = env.sim.character(unit).race.clone();
    match race.as_str() {
        "RaceDwarf" => {
            apply_weapon_specialization(env, "Mace Specialization", 1259719, 1.0, "WeaponTypeMace");
            apply_creature_type_slaying(env, "MobTypeBeast");

            let action_id = ActionId::spell(20594);
            let sim = &mut env.sim;
            let stone_form = sim.register_aura(
                unit,
                AuraConfig {
                    label: "Stoneform".to_string(),
                    action_id: Some(action_id.clone()),
                    duration: 8 * SECOND,
                    ..AuraConfig::default()
                },
            );
            sim.attach_multiplicative_pseudo_stat_buff(
                stone_form,
                PseudoStatField::SchoolDamageTakenMultiplier(SchoolIndex::Physical),
                0.9,
            );
            let timer = sim.new_timer(unit);
            let spell = sim.register_spell(
                unit,
                SpellConfig {
                    action_id,
                    flags: SpellFlag::NO_ON_CAST_COMPLETE,
                    cast: CastConfig {
                        default_cast: Cast {
                            gcd: GCD_DEFAULT,
                            ..Cast::default()
                        },
                        cd: Cooldown {
                            timer: Some(timer),
                            duration: 3 * MINUTE,
                        },
                        ..CastConfig::default()
                    },
                    related_self_buff: Some(stone_form),
                    ..SpellConfig::default()
                },
            );
            sim.add_major_cooldown(unit, major_cooldown(spell, cooldown_type::SURVIVAL));
        }
        "RaceGnome" => {
            apply_expansive_mind(&mut env.sim, unit);
            apply_eureka(env);
        }
        "RaceHuman" => {
            env.sim.unit_mut(unit).sdm.multiply_stat(Stat::Spirit, 1.05);
            apply_weapon_specialization(env, "Sword Specialization", 20597, 2.0, "WeaponTypeSword");
        }
        "RaceNightElf" => {
            let sim = &mut env.sim;
            sim.unit_mut(unit).pseudo_stats.base_dodge_chance += 0.01;
            sim.unit_mut(unit).pseudo_stats.movement_speed_multiplier *= 1.02;
            let timer = sim.new_timer(unit);
            sim.register_temporary_stats_on_use_cd(
                unit,
                "Elune's Light",
                Stats::from_pairs(&[
                    (Stat::PhysicalCritPercent, 10.0),
                    (Stat::SpellCritPercent, 10.0),
                ]),
                15 * SECOND,
                SpellConfig {
                    action_id: ActionId::spell(1259799),
                    cast: CastConfig {
                        cd: Cooldown {
                            timer: Some(timer),
                            duration: 3 * MINUTE,
                        },
                        ..CastConfig::default()
                    },
                    ..SpellConfig::default()
                },
            );
        }
        "RaceOrc" => {
            apply_weapon_specialization(env, "Axe Specialization", 20574, 1.0, "WeaponTypeAxe");

            let sim = &mut env.sim;
            let blood_fury_id = ActionId::spell(20572);
            let blood_fury_aura = sim.new_temporary_stat_multiplier_aura(
                unit,
                AuraConfig {
                    label: "Blood Fury".to_string(),
                    action_id: Some(blood_fury_id.clone()),
                    duration: 15 * SECOND,
                    ..AuraConfig::default()
                },
                &[
                    StatMultiplier {
                        stat: Stat::AttackPower,
                        multiplier: 1.1,
                    },
                    StatMultiplier {
                        stat: Stat::RangedAttackPower,
                        multiplier: 1.1,
                    },
                    StatMultiplier {
                        stat: Stat::SpellDamage,
                        multiplier: 1.1,
                    },
                    StatMultiplier {
                        stat: Stat::HealingPower,
                        multiplier: 1.1,
                    },
                ],
            );
            let blood_fury_cd = Cooldown {
                timer: Some(sim.new_timer(unit)),
                duration: 2 * MINUTE,
            };
            sim.aura_mut(blood_fury_aura.aura).icd = Some(blood_fury_cd);
            let blood_fury = sim.register_spell(
                unit,
                SpellConfig {
                    action_id: blood_fury_id,
                    flags: SpellFlag::NO_ON_CAST_COMPLETE,
                    cast: CastConfig {
                        cd: blood_fury_cd,
                        ..CastConfig::default()
                    },
                    related_self_buff: Some(blood_fury_aura.aura),
                    ..SpellConfig::default()
                },
            );
            sim.add_major_cooldown(unit, major_cooldown(blood_fury, cooldown_type::DPS));

            let shatter_curse_id = ActionId::spell(1299026);
            let shatter_curse_aura = sim.register_aura(
                unit,
                AuraConfig {
                    label: "Shatter Curse".to_string(),
                    action_id: Some(shatter_curse_id.clone()),
                    duration: 8 * SECOND,
                    ..AuraConfig::default()
                },
            );
            for school in [
                SchoolIndex::Arcane,
                SchoolIndex::Fire,
                SchoolIndex::Frost,
                SchoolIndex::Holy,
                SchoolIndex::Nature,
                SchoolIndex::Shadow,
            ] {
                sim.attach_multiplicative_pseudo_stat_buff(
                    shatter_curse_aura,
                    PseudoStatField::SchoolDamageTakenMultiplier(school),
                    0.85,
                );
            }
            let timer = sim.new_timer(unit);
            let shatter_curse = sim.register_spell(
                unit,
                SpellConfig {
                    action_id: shatter_curse_id,
                    flags: SpellFlag::NO_ON_CAST_COMPLETE,
                    cast: CastConfig {
                        default_cast: Cast {
                            gcd: GCD_DEFAULT,
                            ..Cast::default()
                        },
                        cd: Cooldown {
                            timer: Some(timer),
                            duration: 3 * MINUTE,
                        },
                        ..CastConfig::default()
                    },
                    related_self_buff: Some(shatter_curse_aura),
                    ..SpellConfig::default()
                },
            );
            sim.add_major_cooldown(unit, major_cooldown(shatter_curse, cooldown_type::SURVIVAL));
        }
        "RaceTauren" => {
            let sim = &mut env.sim;
            sim.unit_mut(unit).sdm.multiply_stat(Stat::Health, 1.05);
            sim.add_stat(unit, Stat::PhysicalHitPercent, 1.0);
            sim.add_stat(unit, Stat::SpellHitPercent, 1.0);
        }
        "RaceTroll" => {
            apply_creature_type_slaying(env, "MobTypeBeast");

            let sim = &mut env.sim;
            let action_id = ActionId::spell(20554);
            let berserking_aura = sim.register_aura(
                unit,
                AuraConfig {
                    label: "Berserking".to_string(),
                    action_id: Some(action_id.clone()),
                    duration: 10 * SECOND,
                    ..AuraConfig::default()
                },
            );
            sim.attach_multiply_attack_speed(berserking_aura, 1.1);
            sim.attach_multiply_cast_speed(berserking_aura, 1.1);
            let timer = sim.new_timer(unit);
            let berserking = sim.register_spell(
                unit,
                SpellConfig {
                    action_id,
                    flags: SpellFlag::NO_ON_CAST_COMPLETE,
                    cast: CastConfig {
                        cd: Cooldown {
                            timer: Some(timer),
                            duration: 3 * MINUTE,
                        },
                        ..CastConfig::default()
                    },
                    related_self_buff: Some(berserking_aura),
                    ..SpellConfig::default()
                },
            );
            sim.add_major_cooldown(unit, major_cooldown(berserking, cooldown_type::DPS));
        }
        "RaceUndead" => apply_touch_of_the_grave(&mut env.sim, unit),
        "RaceHighOrderSkyborne" => {
            apply_skyborne_shared_racials(env);

            let sim = &mut env.sim;
            let energized = sim.register_aura(
                unit,
                AuraConfig {
                    label: "Energized".to_string(),
                    action_id: Some(ActionId::spell(1270842)),
                    duration: 15 * SECOND,
                    on_gain: Some(Rc::new(|sim: &mut Sim, aura: AuraId| {
                        let unit = sim.aura(aura).unit;
                        if sim.has_mana_bar(unit) {
                            sim.multiply_mana_regen_speed(unit, 2.0);
                        }
                    })),
                    on_expire: Some(Rc::new(|sim: &mut Sim, aura: AuraId| {
                        let unit = sim.aura(aura).unit;
                        if sim.has_mana_bar(unit) {
                            sim.multiply_mana_regen_speed(unit, 0.5);
                        }
                    })),
                    ..AuraConfig::default()
                },
            );
            // Client 1.60.1.70058: Read Ley Line has a 2 sec cast (SpellMisc CastingTimeIndex).
            let timer = sim.new_timer(unit);
            sim.register_spell(
                unit,
                SpellConfig {
                    action_id: ActionId::spell(1259705),
                    flags: SpellFlag::APL | SpellFlag::NO_ON_CAST_COMPLETE,
                    cast: CastConfig {
                        default_cast: Cast {
                            gcd: GCD_DEFAULT,
                            cast_time: 2 * SECOND,
                            ..Cast::default()
                        },
                        cd: Cooldown {
                            timer: Some(timer),
                            duration: 2 * MINUTE,
                        },
                        ..CastConfig::default()
                    },
                    related_self_buff: Some(energized),
                    ..SpellConfig::default()
                },
            );
        }
        "RaceWindshaperSkyborne" => {
            apply_skyborne_shared_racials(env);

            let sim = &mut env.sim;
            let elemental_blessing = sim.register_aura(
                unit,
                AuraConfig {
                    label: "Elemental Blessing".to_string(),
                    action_id: Some(ActionId::spell(1259688)),
                    duration: 30 * SECOND,
                    on_gain: Some(Rc::new(|sim: &mut Sim, aura: AuraId| {
                        let unit = sim.aura(aura).unit;
                        sim.multiply_movement_speed(unit, 1.1);
                    })),
                    on_expire: Some(Rc::new(|sim: &mut Sim, aura: AuraId| {
                        let unit = sim.aura(aura).unit;
                        sim.multiply_movement_speed(unit, 1.0 / 1.1);
                    })),
                    ..AuraConfig::default()
                },
            );
            // Client 1.60.1.70058: Skysight has a 0.5 sec cast (SpellMisc CastingTimeIndex).
            let timer = sim.new_timer(unit);
            sim.register_spell(
                unit,
                SpellConfig {
                    action_id: ActionId::spell(1259686),
                    flags: SpellFlag::APL | SpellFlag::NO_ON_CAST_COMPLETE,
                    cast: CastConfig {
                        default_cast: Cast {
                            gcd: GCD_DEFAULT,
                            cast_time: 500 * MILLISECOND,
                            ..Cast::default()
                        },
                        cd: Cooldown {
                            timer: Some(timer),
                            duration: 2 * MINUTE,
                        },
                        ..CastConfig::default()
                    },
                    related_self_buff: Some(elemental_blessing),
                    ..SpellConfig::default()
                },
            );
        }
        _ => {}
    }
    Ok(())
}

fn major_cooldown(spell: super::sim::SpellId, cooldown_type: u32) -> MajorCooldown {
    MajorCooldown {
        spell,
        priority: 0,
        cooldown_type,
        allow_spell_queueing: false,
        timings: Vec::new(),
    }
}

/// Go `applySkyborneSharedRacials`.
fn apply_skyborne_shared_racials(env: &mut Environment) {
    apply_creature_type_slaying(env, "MobTypeElemental");
    let pseudo = &mut env.sim.unit_mut(env.player).pseudo_stats;
    pseudo.attack_speed_multiplier *= 1.01;
    pseudo.cast_speed_multiplier *= 1.01;
}

/// Go `applyCreatureTypeSlaying`: Beast Slaying (troll 20557), Big Game Hunter (dwarf 1259721)
/// and Elemental Insight (Skyborne 1259707) are one 5% damage done versus the creature type.
/// Go changes the character's attack tables once they exist, after finalization.
fn apply_creature_type_slaying(env: &mut Environment, mob_type: &'static str) {
    let player = env.player;
    env.post_finalize
        .push(Rc::new(move |env: &mut Environment| {
            let attacker = env.sim.unit(player).unit_index as usize;
            for defender in env.sim.all_units() {
                if env.sim.unit(defender).mob_type == mob_type {
                    let index = env.sim.unit(defender).unit_index as usize;
                    env.attack_tables[attacker][index].damage_dealt_multiplier *= 1.05;
                }
            }
        }));
}

/// Go `applyWeaponSpecialization`: the weapon racials (Human Sword, Orc Axe, Dwarf Mace) raise
/// melee, ability and spell crit, but not the ranged auto attack. The stat buffs stay global and
/// a crit modifier on `RANGED_AUTO` takes the bonus back off the auto attack.
pub(crate) fn apply_weapon_specialization(
    env: &mut Environment,
    label: &str,
    spell_id: i32,
    crit_percent: f64,
    weapon_type: &str,
) {
    let unit = env.player;
    let has_weapon_equipped = {
        let character = env.sim.character(unit);
        !character.disable_weapon_spec
            && (character.equipment[slot::MAIN_HAND].weapon_type == weapon_type
                || character.equipment[slot::OFF_HAND].weapon_type == weapon_type)
    };
    let sim = &mut env.sim;
    let aura = sim.register_aura(
        unit,
        AuraConfig {
            label: label.to_string(),
            action_id: Some(ActionId::spell(spell_id)),
            duration: NEVER_EXPIRES,
            build_phase: if has_weapon_equipped {
                BuildPhase::BASE
            } else {
                BuildPhase::NONE
            },
            ..AuraConfig::default()
        },
    );
    sim.attach_stat_buff(aura, Stat::PhysicalCritPercent, crit_percent);
    sim.attach_stat_buff(aura, Stat::SpellCritPercent, crit_percent);
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            kind: SpellModType::BonusCritPercent,
            proc_mask: ProcMask::RANGED_AUTO,
            float_value: -crit_percent,
            ..SpellModConfig::default()
        },
    );
    if has_weapon_equipped {
        sim.make_permanent(aura);
    }
    // Go's item swap callback re-checks the weapons on a swap; item swapping is refused.
}

/// Go `applyExpansiveMind`.
fn apply_expansive_mind(sim: &mut Sim, unit: UnitId) {
    match sim.character(unit).class.as_str() {
        "ClassWarrior" => sim.unit_mut(unit).rage_bar.max_rage *= 1.05,
        "ClassRogue" => sim.unit_mut(unit).energy_bar.max_energy *= 1.05,
        "ClassPriest" | "ClassMage" | "ClassWarlock" => {
            sim.unit_mut(unit).sdm.multiply_stat(Stat::Mana, 1.05);
        }
        _ => {}
    }
}

/// Go `applyEureka`: Eureka! for the next three casts of the abilities the client's rows name.
/// A cast of a spell on none of the three lists neither costs less nor spends a charge.
fn apply_eureka(env: &mut Environment) {
    let unit = env.player;
    let class = env.sim.character(unit).class.clone();
    let mut proc_mask = ProcMask::SPECIAL;
    let (spell_id, resource_type) = match class.as_str() {
        "ClassRogue" => (1259812, Resource::Energy),
        "ClassWarrior" => (1259813, Resource::Rage),
        "ClassMage" => (1259817, Resource::Mana),
        "ClassWarlock" => (1259821, Resource::Mana),
        "ClassPriest" => {
            proc_mask = proc_mask | ProcMask::SPELL_HEALING;
            (1259823, Resource::Mana)
        }
        _ => return,
    };
    // Build 70009 states -10% cost (effect 0, SPELLMOD_COST) on all five class variants and
    // 70170 keeps it.
    const COST_REDUCTION: f64 = 0.1;
    let action_id = ActionId::spell(spell_id);
    const ANY_CLASS_SPELL: i64 = i64::MAX;
    let spells = env.agent.eureka_spells().unwrap_or(EurekaSpells {
        cost: ANY_CLASS_SPELL,
        damage: ANY_CLASS_SPELL,
        tick: 0,
    });
    let sim = &mut env.sim;

    let cost_mod = sim.add_dynamic_mod(
        unit,
        SpellModConfig {
            kind: SpellModType::PowerCostPct,
            class_mask: spells.cost,
            proc_mask,
            resource_type: Some(resource_type),
            float_value: -COST_REDUCTION,
            ..SpellModConfig::default()
        },
    );
    // A spell's damage multiplier covers its hits and its ticks alike, so the +10% goes on
    // every spell of either list, and the ticks of a spell that is only on the direct list take
    // it back.
    let damage_mod = sim.add_dynamic_mod(
        unit,
        SpellModConfig {
            kind: SpellModType::DamageDonePct,
            class_mask: spells.damage | spells.tick,
            proc_mask,
            float_value: 0.1,
            ..SpellModConfig::default()
        },
    );
    let direct_only = spells.damage & !spells.tick;
    let tick_cancel_mod = (direct_only != 0).then(|| {
        sim.add_dynamic_mod(
            unit,
            SpellModConfig {
                kind: SpellModType::DotDamageDonePct,
                class_mask: direct_only,
                proc_mask,
                float_value: 1.0 / 1.1 - 1.0,
                ..SpellModConfig::default()
            },
        )
    });

    let aura = sim.register_aura(
        unit,
        AuraConfig {
            label: "Eureka!".to_string(),
            action_id: Some(action_id.clone()),
            duration: 15 * SECOND,
            max_stacks: 3,
            on_gain: Some(Rc::new(move |sim: &mut Sim, aura: AuraId| {
                sim.set_stacks(aura, 3);
                sim.activate_spell_mod(cost_mod);
                sim.activate_spell_mod(damage_mod);
                if let Some(tick_cancel_mod) = tick_cancel_mod {
                    sim.activate_spell_mod(tick_cancel_mod);
                }
            })),
            on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                sim.deactivate_spell_mod(cost_mod);
                sim.deactivate_spell_mod(damage_mod);
                if let Some(tick_cancel_mod) = tick_cancel_mod {
                    sim.deactivate_spell_mod(tick_cancel_mod);
                }
            })),
            events: EventCallbacks {
                on_cast_complete: true,
                ..EventCallbacks::default()
            },
            ..AuraConfig::default()
        },
    );

    let timer = sim.new_timer(unit);
    let spell = sim.register_spell(
        unit,
        SpellConfig {
            action_id,
            flags: SpellFlag::NO_ON_CAST_COMPLETE,
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(timer),
                    duration: 2 * MINUTE,
                },
                ..CastConfig::default()
            },
            related_self_buff: Some(aura),
            ..SpellConfig::default()
        },
    );
    sim.add_major_cooldown(unit, major_cooldown(spell, cooldown_type::DPS));
}

/// Go `applyTouchOfTheGrave`.
fn apply_touch_of_the_grave(sim: &mut Sim, unit: UnitId) {
    let (aura_id, proc_chance) = match sim.character(unit).class.as_str() {
        "ClassWarrior" | "ClassPaladin" | "ClassRogue" => (1260189, 0.05),
        _ => (1260201, 0.1),
    };
    // The drain heals through health metrics Go creates here; they are not prepared state.
    sim.register_spell(
        unit,
        SpellConfig {
            action_id: ActionId::spell(1260198),
            spell_school: school::SHADOW,
            defense_type: DefenseType::Magic,
            proc_mask: ProcMask::EMPTY,
            flags: SpellFlag::PROC
                | SpellFlag::PASSIVE_SPELL
                | SpellFlag::IGNORE_ATTACKER_MODIFIERS
                | SpellFlag::NO_SPELL_MODS,
            damage_multiplier: 1.0,
            threat_multiplier: 1.0,
            ..SpellConfig::default()
        },
    );
    sim.make_proc_trigger_aura(
        unit,
        &ProcTrigger {
            name: "Touch of the Grave".to_string(),
            action_id: ActionId::spell(aura_id),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            proc_mask: ProcMask::MELEE | ProcMask::RANGED | ProcMask::SPELL_DAMAGE,
            outcome: HitOutcome::LANDED,
            proc_chance,
            icd: SECOND,
            ..ProcTrigger::default()
        },
    );
}

#[cfg(test)]
mod tests;
