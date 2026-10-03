//! The Rogue class agent for the fight runtime: class spells and auras by name, and the hooks
//! that dispatch to each spell's and talent's module.
//!
//! Go's `BreakStealth` opens every Rogue strike. Stealth is never active in scope: the gate
//! rejects a rotation that reaches the Stealth spell, which has no behavior here.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{
        backstab::Backstab,
        eviscerate::Eviscerate,
        finisher::{self, Finisher},
        poisons::{self, PoisonProc},
        sinister_strike,
        slice_and_dice::SliceAndDice,
    },
    talents::{adrenaline_rush::AdrenalineRush, blade_flurry::BladeFlurry},
};

/// What a Rogue spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RogueSpell {
    SinisterStrike,
    Backstab,
    Eviscerate,
    SliceAndDice,
    BladeFlurry,
    AdrenalineRush,
    InstantPoison,
    DeadlyPoison,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RogueAura {
    InstantPoisonTrigger,
    DeadlyPoisonTrigger,
    SliceAndDice,
    BladeFlurry,
    AdrenalineRush,
}

/// Rogue state that Go keeps in the `Rogue` struct and its closures.
#[derive(Default)]
pub(crate) struct RogueAgent {
    sinister_strike: f64,
    backstab: Option<Backstab>,
    eviscerate: Option<Eviscerate>,
    slice_and_dice: Option<SliceAndDice>,
    blade_flurry: Option<BladeFlurry>,
    adrenaline_rush: Option<AdrenalineRush>,
    finisher: Option<Finisher>,
    instant_poison: Option<PoisonProc>,
    instant_poison_damage: (f64, f64),
    deadly_poison: Option<PoisonProc>,
    deadly_poison_tick: f64,
}

/// Whether a prepared input carries the effect of a kind.
fn has_effect(prepared: &PreparedV2, kind: &str) -> bool {
    prepared.effects.iter().any(|effect| effect.kind() == kind)
}

/// Whether a weapon proc on the named hands hears a spell: no Suppress Weapon Procs flag, and
/// a proc mask that names one of the hands.
fn weapon_proc_hears(spell: &ExportedSpell, hands: &[String]) -> bool {
    !spell.has_flag("SpellFlagSuppressWeaponProcs")
        && spell.proc_mask.iter().any(|mask| hands.contains(mask))
}

impl RogueAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    fn spell(prepared: &PreparedV2, spell: &ExportedSpell) -> Option<RogueSpell> {
        let has = |kind| has_effect(prepared, kind);
        let energy = spell
            .cost
            .as_ref()
            .is_some_and(|cost| cost.resource == "energy");
        match spell.class_spell.as_deref()? {
            "sinister_strike" if energy && has("sinister_strike") => {
                Some(RogueSpell::SinisterStrike)
            }
            "backstab" if energy && has("backstab") => Some(RogueSpell::Backstab),
            "eviscerate" if energy && has("eviscerate") => Some(RogueSpell::Eviscerate),
            "slice_and_dice" if energy && has("slice_and_dice") => Some(RogueSpell::SliceAndDice),
            "blade_flurry" if has("blade_flurry") => Some(RogueSpell::BladeFlurry),
            "adrenaline_rush" if has("adrenaline_rush") => Some(RogueSpell::AdrenalineRush),
            "instant_poison" if has("instant_poison") => Some(RogueSpell::InstantPoison),
            "deadly_poison" if spell.dot.is_some() && has("deadly_poison") => {
                Some(RogueSpell::DeadlyPoison)
            }
            _ => None,
        }
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<RogueAgent>, String> {
        let auras: Vec<(String, RogueAura)> = prepared
            .effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::InstantPoison { trigger_aura, .. } => {
                    Some((trigger_aura.clone(), RogueAura::InstantPoisonTrigger))
                }
                Effect::DeadlyPoison { trigger_aura, .. } => {
                    Some((trigger_aura.clone(), RogueAura::DeadlyPoisonTrigger))
                }
                Effect::SliceAndDice { aura, .. } => Some((aura.clone(), RogueAura::SliceAndDice)),
                Effect::BladeFlurry { aura, .. } => Some((aura.clone(), RogueAura::BladeFlurry)),
                Effect::AdrenalineRush { aura, .. } => {
                    Some((aura.clone(), RogueAura::AdrenalineRush))
                }
                _ => None,
            })
            .collect();
        let spell = |exported: &ExportedSpell| RogueAgent::spell(prepared, exported);
        let mut fight = Fight::new(prepared, RogueAgent::default(), spell, |unit, label| {
            (unit == "player")
                .then(|| {
                    auras
                        .iter()
                        .find(|(name, _)| name == label)
                        .map(|(_, kind)| *kind)
                })
                .flatten()
        })?;
        let class_spell = |fight: &Fight<RogueAgent>, kind: RogueSpell| {
            fight.spells.iter().position(|spell| {
                matches!(spell.behavior, crate::core::fight::SpellBehavior::Class(k) if k == kind)
            })
        };
        let poison = |fight: &Fight<RogueAgent>,
                      kind: RogueSpell,
                      label: &str,
                      hands: &[String],
                      chance: f64|
         -> Result<PoisonProc, String> {
            let spell = class_spell(fight, kind).ok_or_else(|| format!("{label} has no spell"))?;
            Ok(PoisonProc {
                label: label.to_string(),
                spell,
                chance,
                eligible: prepared
                    .player
                    .spells
                    .iter()
                    .map(|exported| weapon_proc_hears(exported, hands))
                    .collect(),
            })
        };
        for effect in &prepared.effects {
            match effect {
                Effect::SinisterStrike { base_damage, .. } => {
                    fight.agent.sinister_strike = *base_damage;
                }
                Effect::Backstab {
                    base_damage,
                    main_hand_dagger,
                    extra_combo_point_chance,
                    extra_combo_point_action,
                    ..
                } => {
                    let extra_combo_point_metrics = fight.new_resource_metrics(
                        extra_combo_point_action.clone(),
                        crate::core::fight::ResourceKind::ComboPoints,
                    );
                    fight.agent.backstab = Some(Backstab {
                        base_damage: *base_damage,
                        main_hand_dagger: *main_hand_dagger,
                        extra_combo_point_chance: *extra_combo_point_chance,
                        extra_combo_point_metrics,
                    });
                }
                Effect::Eviscerate {
                    damage_average,
                    damage_variance,
                    combo_point_damage,
                    attack_power_per_combo_point,
                    ..
                } => {
                    fight.agent.eviscerate = Some(Eviscerate {
                        damage_average: *damage_average,
                        damage_variance: *damage_variance,
                        combo_point_damage: *combo_point_damage,
                        attack_power_per_combo_point: *attack_power_per_combo_point,
                    });
                }
                Effect::SliceAndDice {
                    aura,
                    durations_ns,
                    melee_speed_multiplier,
                    ..
                } => {
                    fight.agent.slice_and_dice = Some(SliceAndDice {
                        aura: fight.player_aura(aura)?,
                        durations: durations_ns.clone(),
                        melee_speed_multiplier: *melee_speed_multiplier,
                    });
                }
                Effect::BladeFlurry {
                    aura,
                    attack_speed_multiplier,
                    ..
                } => {
                    fight.agent.blade_flurry = Some(BladeFlurry {
                        aura: fight.player_aura(aura)?,
                        attack_speed_multiplier: *attack_speed_multiplier,
                    });
                }
                Effect::AdrenalineRush {
                    aura,
                    regen_multiplier,
                    energy_threshold,
                    ..
                } => {
                    fight.agent.adrenaline_rush = Some(AdrenalineRush {
                        aura: fight.player_aura(aura)?,
                        regen_multiplier: *regen_multiplier,
                        energy_threshold: *energy_threshold,
                    });
                }
                Effect::RogueFinisher {
                    relentless_strikes,
                    relentless_strikes_chance_per_point,
                    relentless_strikes_energy,
                    relentless_strikes_action,
                    ruthlessness_chance,
                    ruthlessness_action,
                } => {
                    let bound = finisher::bind(
                        &mut fight,
                        *relentless_strikes,
                        *relentless_strikes_chance_per_point,
                        *relentless_strikes_energy,
                        relentless_strikes_action,
                        *ruthlessness_chance,
                        ruthlessness_action,
                    );
                    fight.agent.finisher = Some(bound);
                }
                Effect::InstantPoison {
                    trigger_aura,
                    proc_mask,
                    proc_chance,
                    min_damage,
                    max_damage,
                    ..
                } => {
                    let bound = poison(
                        &fight,
                        RogueSpell::InstantPoison,
                        trigger_aura,
                        proc_mask,
                        *proc_chance,
                    )?;
                    fight.agent.instant_poison = Some(bound);
                    fight.agent.instant_poison_damage = (*min_damage, *max_damage);
                }
                Effect::DeadlyPoison {
                    trigger_aura,
                    proc_mask,
                    proc_chance,
                    tick_damage,
                    ..
                } => {
                    let bound = poison(
                        &fight,
                        RogueSpell::DeadlyPoison,
                        trigger_aura,
                        proc_mask,
                        *proc_chance,
                    )?;
                    let dot = fight.spells[bound.spell]
                        .dot
                        .ok_or("Deadly Poison has no dot")?;
                    // The dot is 25349, which carries Periodic Can Crit in the client.
                    fight.dots[dot].tick_can_crit = true;
                    fight.agent.deadly_poison = Some(bound);
                    fight.agent.deadly_poison_tick = *tick_damage;
                }
                _ => {}
            }
        }
        Ok(fight)
    }

    fn finisher(fight: &Fight<Self>) -> Finisher {
        fight.agent.finisher.expect("the finisher is bound")
    }
}

impl Agent for RogueAgent {
    type Spell = RogueSpell;
    type Aura = RogueAura;

    fn apply_effects(fight: &mut Fight<Self>, spell: SpellId, target: Side, behavior: RogueSpell) {
        match behavior {
            RogueSpell::SinisterStrike => {
                let base = fight.agent.sinister_strike;
                sinister_strike::apply(fight, spell, target, base);
            }
            RogueSpell::Backstab => {
                let backstab = fight.agent.backstab.expect("Backstab is bound");
                backstab.apply(fight, spell, target);
            }
            RogueSpell::Eviscerate => {
                let eviscerate = fight.agent.eviscerate.expect("Eviscerate is bound");
                let finisher = Self::finisher(fight);
                eviscerate.apply(fight, spell, target, &finisher);
            }
            RogueSpell::SliceAndDice => {
                let slice = fight
                    .agent
                    .slice_and_dice
                    .clone()
                    .expect("Slice and Dice is bound");
                let finisher = Self::finisher(fight);
                slice.apply(fight, spell, &finisher);
            }
            RogueSpell::BladeFlurry => {
                let flurry = fight.agent.blade_flurry.expect("Blade Flurry is bound");
                flurry.apply(fight);
            }
            RogueSpell::AdrenalineRush => {
                let rush = fight
                    .agent
                    .adrenaline_rush
                    .expect("Adrenaline Rush is bound");
                rush.apply(fight);
            }
            RogueSpell::InstantPoison => {
                let (min, max) = fight.agent.instant_poison_damage;
                poisons::instant_poison(fight, spell, target, min, max);
            }
            RogueSpell::DeadlyPoison => {
                let tick = fight.agent.deadly_poison_tick;
                poisons::deadly_poison(fight, spell, target, tick);
            }
        }
    }

    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: RogueSpell) -> bool {
        match behavior {
            RogueSpell::Backstab => fight
                .agent
                .backstab
                .is_some_and(|backstab| backstab.can_cast(fight)),
            RogueSpell::Eviscerate | RogueSpell::SliceAndDice => {
                fight.energy_bar().combo_points > 0
            }
            _ => true,
        }
    }

    fn modify_cast(fight: &mut Fight<Self>, spell: SpellId, behavior: RogueSpell) {
        if matches!(behavior, RogueSpell::Eviscerate | RogueSpell::SliceAndDice) {
            let points = fight.energy_bar().combo_points as usize;
            fight.set_metrics_split(spell, points);
        }
    }

    fn should_activate(fight: &Fight<Self>, _spell: SpellId, behavior: RogueSpell) -> bool {
        match behavior {
            RogueSpell::AdrenalineRush => fight
                .agent
                .adrenaline_rush
                .is_some_and(|rush| rush.should_activate(fight)),
            _ => true,
        }
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: RogueSpell) {
        if behavior == RogueSpell::DeadlyPoison {
            fight.snapshot_dot_tick(dot);
        }
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: RogueAura) {
        match kind {
            RogueAura::SliceAndDice => {
                let slice = fight.agent.slice_and_dice.clone().expect("bound");
                slice.on_gain(fight);
            }
            RogueAura::BladeFlurry => fight.agent.blade_flurry.expect("bound").on_gain(fight),
            RogueAura::AdrenalineRush => fight.agent.adrenaline_rush.expect("bound").on_gain(fight),
            _ => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: RogueAura) {
        match kind {
            RogueAura::SliceAndDice => {
                let slice = fight.agent.slice_and_dice.clone().expect("bound");
                slice.on_expire(fight);
            }
            RogueAura::BladeFlurry => fight.agent.blade_flurry.expect("bound").on_expire(fight),
            RogueAura::AdrenalineRush => {
                fight.agent.adrenaline_rush.expect("bound").on_expire(fight)
            }
            _ => {}
        }
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: RogueAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let poison = match kind {
            RogueAura::InstantPoisonTrigger => fight.agent.instant_poison.clone(),
            RogueAura::DeadlyPoisonTrigger => fight.agent.deadly_poison.clone(),
            // Blade Flurry's listener needs a second target.
            _ => None,
        };
        if let Some(poison) = poison {
            poison.on_spell_hit_dealt(fight, spell, result);
        }
    }
}
