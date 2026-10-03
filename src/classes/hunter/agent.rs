//! The Hunter class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::{
        fight::{
            action_string, melee::Hand, Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult,
        },
        time::{go_string, NS_PER_MILLISECOND},
    },
};

use super::pet::{self as hunter_pet, PetAi, PetAuras};
use super::spells::{
    aspect_of_the_hawk::AspectOfTheHawk, rapid_fire::RapidFire, serpent_sting,
    serpent_sting::SerpentSting, shots, summon_hawk::SummonHawk,
};

/// What a Hunter spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HunterSpell {
    AimedShot,
    SniperShot,
    MultiShot,
    SerpentSting,
    AspectOfTheHawk,
    RapidFire,
    SummonHawk,
    /// A hawk slot's dot spell.
    Hawk,
    /// A pet ability, by its position among the rolled ranges.
    PetStrike(usize),
    Intimidation,
    BestialWrath,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HunterAura {
    AspectOfTheHawk,
    QuickShots,
    RapidFire,
    Intimidation,
    BestialWrath,
    FrenzyTrigger,
    FrenzyEffect,
}

/// Hunter state that Go keeps in the `Hunter` struct and its closures.
#[derive(Default)]
pub(crate) struct HunterAgent {
    aimed_shot_bonus: f64,
    sniper_shot_bonus: f64,
    serpent_sting: Option<SerpentSting>,
    aspect: Option<Rc<AspectOfTheHawk>>,
    rapid_fire: Option<RapidFire>,
    summon_hawk: Option<Rc<SummonHawk>>,
    /// The pet abilities' rolled ranges.
    pet_strikes: Vec<(f64, f64)>,
    pet_ai: Option<Rc<PetAi>>,
    pet_auras: Rc<PetAuras>,
}

/// Player aura labels claimed by implemented class effects.
fn class_auras(prepared: &PreparedV2) -> Vec<(String, HunterAura)> {
    let mut auras = Vec::new();
    for effect in &prepared.effects {
        match effect {
            Effect::AspectOfTheHawk {
                aura, proc_aura, ..
            } => {
                auras.push((aura.clone(), HunterAura::AspectOfTheHawk));
                if let Some(proc_aura) = proc_aura {
                    auras.push((proc_aura.clone(), HunterAura::QuickShots));
                }
            }
            Effect::RapidFire { aura, .. } => auras.push((aura.clone(), HunterAura::RapidFire)),
            _ => {}
        }
    }
    auras
}

/// Pet aura labels claimed by implemented class effects.
fn pet_auras(prepared: &PreparedV2) -> Vec<(String, HunterAura)> {
    let mut auras = Vec::new();
    for effect in &prepared.effects {
        match effect {
            Effect::Intimidation { aura, .. } => {
                auras.push((aura.clone(), HunterAura::Intimidation))
            }
            Effect::BestialWrath { aura, .. } => {
                auras.push((aura.clone(), HunterAura::BestialWrath))
            }
            Effect::Frenzy {
                trigger_aura, aura, ..
            } => {
                auras.push((trigger_aura.clone(), HunterAura::FrenzyTrigger));
                auras.push((aura.clone(), HunterAura::FrenzyEffect));
            }
            _ => {}
        }
    }
    auras
}

/// The pet abilities' spell IDs and rolled ranges, in effect order.
fn pet_strikes(prepared: &PreparedV2) -> Vec<(i32, (f64, f64))> {
    prepared
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::HunterPetStrike {
                spell_id,
                min_damage,
                max_damage,
            } => Some((*spell_id, (*min_damage, *max_damage))),
            _ => None,
        })
        .collect()
}

/// The class behavior of an exported spell, if Rust implements it.
pub(crate) fn spell_behavior(spell: &ExportedSpell) -> Option<HunterSpell> {
    let id = spell.action_id.clone().unwrap_or_default();
    let class = spell.class_spell.as_deref()?;
    if id.item_id != 0 {
        return None;
    }
    if id.tag != 0 {
        return (class == "summon_hawk" && spell.dot.is_some()).then_some(HunterSpell::Hawk);
    }
    match class {
        "aimed_shot" => Some(HunterSpell::AimedShot),
        "sniper_shot" => Some(HunterSpell::SniperShot),
        "multi_shot" => Some(HunterSpell::MultiShot),
        "serpent_sting" if spell.dot.is_some() => Some(HunterSpell::SerpentSting),
        "aspect_of_the_hawk" => Some(HunterSpell::AspectOfTheHawk),
        "rapid_fire" => Some(HunterSpell::RapidFire),
        "summon_hawk" => Some(HunterSpell::SummonHawk),
        "intimidation" => Some(HunterSpell::Intimidation),
        "bestial_wrath" => Some(HunterSpell::BestialWrath),
        _ => None,
    }
}

impl HunterAgent {
    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<HunterAgent>, String> {
        let auras = class_auras(prepared);
        let pet_aura_kinds = pet_auras(prepared);
        let strikes = pet_strikes(prepared);
        let behavior = |spell: &ExportedSpell| {
            let id = spell.action_id.clone().unwrap_or_default();
            if spell.class_spell.as_deref() == Some("pet_damage") && id.tag == 0 {
                return strikes
                    .iter()
                    .position(|(spell_id, _)| *spell_id == id.spell_id)
                    .map(HunterSpell::PetStrike);
            }
            spell_behavior(spell)
        };
        let mut fight = Fight::new(prepared, HunterAgent::default(), behavior, |unit, label| {
            let list = match unit {
                "player" => &auras,
                "pet" => &pet_aura_kinds,
                _ => return None,
            };
            list.iter()
                .find(|(name, _)| name == label)
                .map(|(_, kind)| *kind)
        })?;
        fight.agent.pet_strikes = strikes.iter().map(|(_, range)| *range).collect();
        let mut pet_auras = PetAuras::default();
        let find_spell = |fight: &Fight<HunterAgent>, id: i32| {
            fight.spells.iter().position(|spell| {
                spell.id.spell_id == id && spell.id.tag == 0 && spell.id.item_id == 0
            })
        };
        for effect in &prepared.effects {
            match effect {
                Effect::HunterPet {
                    rotation,
                    special_ability,
                    focus_dump,
                    extra_ability,
                    wait_ns,
                    melee_range,
                    move_to,
                    ..
                } => {
                    let ai = PetAi::bind(
                        &fight,
                        rotation,
                        *special_ability,
                        *focus_dump,
                        *extra_ability,
                        *wait_ns,
                        *melee_range,
                        *move_to,
                    )?;
                    fight.agent.pet_ai = Some(Rc::new(ai));
                }
                Effect::Intimidation {
                    aura, active_crit, ..
                } => {
                    pet_auras.intimidation =
                        Some((PetAuras::pet_aura(&fight, aura)?, *active_crit));
                }
                Effect::BestialWrath {
                    aura,
                    damage_multiplier,
                    ..
                } => {
                    pet_auras.bestial_wrath =
                        Some((PetAuras::pet_aura(&fight, aura)?, *damage_multiplier));
                }
                Effect::Frenzy {
                    trigger_aura,
                    aura,
                    proc_chance,
                    speed_multiplier,
                    ..
                } => {
                    pet_auras.frenzy = Some((
                        PetAuras::pet_aura(&fight, trigger_aura)?,
                        PetAuras::pet_aura(&fight, aura)?,
                        *proc_chance,
                        *speed_multiplier,
                    ));
                }
                Effect::AimedShot { flat_bonus, .. } => fight.agent.aimed_shot_bonus = *flat_bonus,
                Effect::SniperShot { flat_bonus, .. } => {
                    fight.agent.sniper_shot_bonus = *flat_bonus
                }
                Effect::SerpentSting {
                    spell_id,
                    tick_base,
                    attack_power_share,
                    tick_outcome,
                } => {
                    let spell = find_spell(&fight, *spell_id)
                        .ok_or_else(|| format!("Serpent Sting {spell_id} is not registered"))?;
                    let sting = SerpentSting::bind(
                        &fight,
                        spell,
                        *tick_base,
                        *attack_power_share,
                        tick_outcome,
                    )?;
                    fight.agent.serpent_sting = Some(sting);
                }
                Effect::AspectOfTheHawk {
                    aura,
                    proc_aura,
                    haste_multiplier,
                    proc_chance,
                    ..
                } => {
                    let quick_shots = match (proc_aura, haste_multiplier, proc_chance) {
                        (Some(label), Some(haste), Some(chance)) => {
                            Some((label.as_str(), *haste, *chance))
                        }
                        (None, None, None) => None,
                        _ => return Err("Deadly Aspects is incomplete".into()),
                    };
                    let bound = AspectOfTheHawk::bind(
                        &fight,
                        &prepared.effects,
                        &prepared.player.spells,
                        aura,
                        quick_shots,
                    )?;
                    fight.agent.aspect = Some(Rc::new(bound));
                }
                Effect::RapidFire {
                    aura,
                    haste_multiplier,
                    ..
                } => {
                    fight.agent.rapid_fire = Some(RapidFire::bind(&fight, aura, *haste_multiplier)?)
                }
                Effect::SummonHawk {
                    base_damage,
                    attack_power_share,
                    always_hits,
                    hawk_spells,
                    ..
                } => {
                    let bound = SummonHawk::bind(
                        &mut fight,
                        *base_damage,
                        *attack_power_share,
                        *always_hits,
                        hawk_spells,
                    )?;
                    fight.agent.summon_hawk = Some(Rc::new(bound));
                }
                _ => {}
            }
        }
        fight.agent.pet_auras = Rc::new(pet_auras);
        Ok(fight)
    }

    fn aspect(fight: &Fight<Self>) -> Rc<AspectOfTheHawk> {
        fight
            .agent
            .aspect
            .clone()
            .expect("Aspect of the Hawk is bound")
    }

    fn rapid_fire(fight: &Fight<Self>) -> RapidFire {
        fight.agent.rapid_fire.expect("Rapid Fire is bound")
    }

    fn summon_hawk(fight: &Fight<Self>) -> Rc<SummonHawk> {
        fight
            .agent
            .summon_hawk
            .clone()
            .expect("Summon Hawk is bound")
    }
}

impl Agent for HunterAgent {
    type Spell = HunterSpell;
    type Aura = HunterAura;

    fn apply_effects(fight: &mut Fight<Self>, spell: SpellId, target: Side, behavior: HunterSpell) {
        match behavior {
            HunterSpell::AimedShot => {
                let bonus = fight.agent.aimed_shot_bonus;
                shots::apply(fight, spell, target, bonus);
            }
            HunterSpell::SniperShot => {
                let bonus = fight.agent.sniper_shot_bonus;
                shots::apply(fight, spell, target, bonus);
            }
            HunterSpell::MultiShot => shots::apply(fight, spell, target, 0.0),
            HunterSpell::SerpentSting => serpent_sting::apply(fight, spell, target),
            HunterSpell::AspectOfTheHawk => {
                let aura = Self::aspect(fight).aura;
                fight.activate_aura(aura);
            }
            HunterSpell::RapidFire => {
                let aura = Self::rapid_fire(fight).aura;
                fight.activate_aura(aura);
            }
            HunterSpell::SummonHawk => Self::summon_hawk(fight).apply(fight, spell, target),
            HunterSpell::Hawk => panic!("a hawk is never cast"),
            HunterSpell::PetStrike(index) => {
                let range = fight.agent.pet_strikes[index];
                hunter_pet::strike(fight, spell, target, range);
            }
            HunterSpell::Intimidation => {
                let (aura, _) = fight
                    .agent
                    .pet_auras
                    .intimidation
                    .expect("Intimidation is bound");
                fight.activate_aura(aura);
            }
            HunterSpell::BestialWrath => {
                let (aura, _) = fight
                    .agent
                    .pet_auras
                    .bestial_wrath
                    .expect("Bestial Wrath is bound");
                fight.activate_aura(aura);
            }
        }
    }

    fn cast_time(fight: &Fight<Self>, spell: SpellId, behavior: HunterSpell) -> Option<i64> {
        match behavior {
            HunterSpell::AimedShot
            | HunterSpell::SniperShot
            | HunterSpell::MultiShot
            | HunterSpell::SerpentSting => shots::cast_time(fight, spell),
            _ => None,
        }
    }

    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: HunterSpell) -> bool {
        match behavior {
            HunterSpell::AspectOfTheHawk => !fight.aura(Self::aspect(fight).aura).active,
            HunterSpell::RapidFire => !fight.aura(Self::rapid_fire(fight).aura).active,
            _ => true,
        }
    }

    fn should_activate(fight: &Fight<Self>, _spell: SpellId, behavior: HunterSpell) -> bool {
        match behavior {
            HunterSpell::RapidFire => !fight.aura(Self::rapid_fire(fight).aura).active,
            _ => true,
        }
    }

    fn on_travel(
        fight: &mut Fight<Self>,
        spell: SpellId,
        result: SpellResult,
        behavior: HunterSpell,
    ) {
        match behavior {
            HunterSpell::SerpentSting => {
                let sting = fight
                    .agent
                    .serpent_sting
                    .clone()
                    .expect("Serpent Sting is bound");
                if let Some(attack_power) = serpent_sting::on_travel(fight, spell, result, &sting) {
                    if let Some(bound) = fight.agent.serpent_sting.as_mut() {
                        bound.snapshotted(attack_power);
                    }
                }
            }
            _ => fight.deal_damage(spell, result, false),
        }
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: HunterSpell) {
        match behavior {
            HunterSpell::SerpentSting => {
                let sting = fight
                    .agent
                    .serpent_sting
                    .clone()
                    .expect("Serpent Sting is bound");
                sting.tick(fight, dot);
            }
            HunterSpell::Hawk => SummonHawk::tick(fight, dot),
            _ => {}
        }
    }

    /// Go hunter.go wraps the main hand auto's `ApplyEffects` with a line for a swing that
    /// fired later than an uncontested rotation would have.
    fn before_melee_auto(fight: &mut Fight<Self>, spell: SpellId, hand: Hand) {
        let delay = fight.autos.mh.pending_swing_delay;
        if fight.log.is_none()
            || hand != Hand::Main
            || fight.spells[spell].caster != Side::Player
            || fight.spells[spell].id.tag != 1
            || delay <= NS_PER_MILLISECOND
        {
            return;
        }
        let line = format!(
            "{} delayed by {}, was ready at {}",
            action_string(&fight.spells[spell].id),
            go_string(delay),
            go_string(fight.now - delay)
        );
        fight.player_log(&line);
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: HunterAura) {
        match kind {
            HunterAura::AspectOfTheHawk => Self::aspect(fight).on_gain(fight),
            HunterAura::QuickShots => Self::aspect(fight).quick_shots_changed(fight, true),
            HunterAura::RapidFire => Self::rapid_fire(fight).changed(fight, true),
            HunterAura::Intimidation => fight
                .agent
                .pet_auras
                .clone()
                .intimidation_changed(fight, true),
            HunterAura::BestialWrath => fight
                .agent
                .pet_auras
                .clone()
                .bestial_wrath_changed(fight, true),
            HunterAura::FrenzyEffect => fight.agent.pet_auras.clone().frenzy_changed(fight, true),
            HunterAura::FrenzyTrigger => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: HunterAura) {
        match kind {
            HunterAura::AspectOfTheHawk => Self::aspect(fight).on_expire(fight),
            HunterAura::QuickShots => Self::aspect(fight).quick_shots_changed(fight, false),
            HunterAura::RapidFire => Self::rapid_fire(fight).changed(fight, false),
            HunterAura::Intimidation => fight
                .agent
                .pet_auras
                .clone()
                .intimidation_changed(fight, false),
            HunterAura::BestialWrath => fight
                .agent
                .pet_auras
                .clone()
                .bestial_wrath_changed(fight, false),
            HunterAura::FrenzyEffect => fight.agent.pet_auras.clone().frenzy_changed(fight, false),
            HunterAura::FrenzyTrigger => {}
        }
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: HunterAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        match kind {
            HunterAura::AspectOfTheHawk => Self::aspect(fight).on_spell_hit_dealt(fight, spell),
            HunterAura::Intimidation => fight
                .agent
                .pet_auras
                .clone()
                .intimidation_hit(fight, result),
            HunterAura::FrenzyTrigger => fight
                .agent
                .pet_auras
                .clone()
                .frenzy_hit(fight, spell, result),
            _ => {}
        }
    }

    fn on_delayed_proc(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: HunterAura,
        _spell: SpellId,
        _result: SpellResult,
    ) {
        if kind == HunterAura::FrenzyTrigger {
            fight.agent.pet_auras.clone().frenzy_proc(fight);
        }
    }

    fn pet_rotation(fight: &mut Fight<Self>) {
        if let Some(ai) = fight.agent.pet_ai.clone() {
            ai.rotation(fight);
        }
    }
}
