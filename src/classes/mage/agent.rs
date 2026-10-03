//! The Mage class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{
        arcane_blast, arcane_missiles, cold_snap, evocation, frostbolt, ice_lance, mana_gems,
    },
    talents::{arcane_concentration, fingers_of_frost, missile_barrage, winters_chill},
};

/// What a Mage spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MageSpell {
    Frostbolt,
    ArcaneBlast,
    IceLance,
    ArcaneMissiles,
    ArcaneMissile,
    ColdSnap,
    Evocation,
    /// A mana gem by its index in Go's order, smallest first.
    ManaGem(usize),
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MageAura {
    WintersChill,
    WintersChillTrigger,
    FingersOfFrost,
    FingersOfFrostTrigger,
    Clearcasting,
    ArcaneConcentrationTrigger,
    MissileBarrage,
    MissileBarrageTrigger,
    EvocationRegen,
    ArcaneCharges,
}

/// Mage state that Go keeps in the `Mage` struct and its closures.
#[derive(Default)]
pub(crate) struct MageAgent {
    winters_chill: Option<Rc<winters_chill::WintersChill>>,
    fingers_of_frost: Option<fingers_of_frost::FingersOfFrost>,
    arcane_concentration: Option<Rc<arcane_concentration::ArcaneConcentration>>,
    missile_barrage: Option<Rc<missile_barrage::MissileBarrage>>,
    arcane_charges: Option<Rc<arcane_blast::ArcaneCharges>>,
    ice_lance_frozen_multiplier: f64,
    /// Arcane Missiles channel spell to the missile spell of the same rank.
    missiles: Vec<(SpellId, SpellId)>,
    gems: mana_gems::ManaGems,
    evocation_regen: Option<(AuraRef, f64)>,
}

/// Player aura labels claimed by implemented class effects.
fn class_auras(prepared: &PreparedV2) -> Vec<(String, MageAura)> {
    let mut auras = Vec::new();
    for effect in &prepared.effects {
        let (aura, trigger, kinds) = match effect {
            Effect::WintersChill {
                aura, trigger_aura, ..
            } => (
                aura,
                trigger_aura,
                (MageAura::WintersChill, MageAura::WintersChillTrigger),
            ),
            Effect::FingersOfFrost {
                aura, trigger_aura, ..
            } => (
                aura,
                trigger_aura,
                (MageAura::FingersOfFrost, MageAura::FingersOfFrostTrigger),
            ),
            Effect::ArcaneConcentration {
                aura, trigger_aura, ..
            } => (
                aura,
                trigger_aura,
                (MageAura::Clearcasting, MageAura::ArcaneConcentrationTrigger),
            ),
            Effect::MissileBarrage {
                aura, trigger_aura, ..
            } => (
                aura,
                trigger_aura,
                (MageAura::MissileBarrage, MageAura::MissileBarrageTrigger),
            ),
            Effect::Evocation { regen_aura, .. } => {
                auras.push((regen_aura.clone(), MageAura::EvocationRegen));
                continue;
            }
            Effect::ArcaneBlast { aura, .. } => {
                auras.push((aura.clone(), MageAura::ArcaneCharges));
                continue;
            }
            _ => continue,
        };
        auras.push((aura.clone(), kinds.0));
        auras.push((trigger.clone(), kinds.1));
    }
    auras
}

impl MageAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(spell: &ExportedSpell, gems: &[i32]) -> Option<MageSpell> {
        match spell.class_spell.as_deref()? {
            "cold_snap" => Some(MageSpell::ColdSnap),
            "evocation" if spell.dot.is_some() => Some(MageSpell::Evocation),
            "mana_gem" => {
                let item = spell.action_id.as_ref()?.item_id;
                gems.iter()
                    .position(|gem| *gem == item)
                    .map(MageSpell::ManaGem)
            }
            "frostbolt" if spell.damage_effect.is_some() => Some(MageSpell::Frostbolt),
            "arcane_blast" if spell.damage_effect.is_some() => Some(MageSpell::ArcaneBlast),
            "ice_lance" if spell.damage_effect.is_some() => Some(MageSpell::IceLance),
            "arcane_missiles_cast" if spell.dot.is_some() => Some(MageSpell::ArcaneMissiles),
            "arcane_missiles_tick" if spell.damage_effect.is_some() => {
                Some(MageSpell::ArcaneMissile)
            }
            _ => None,
        }
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<MageAgent>, String> {
        let auras = class_auras(prepared);
        let gems: Vec<i32> = prepared
            .effects
            .iter()
            .find_map(|effect| match effect {
                Effect::ManaGems { gems, .. } => Some(gems.iter().map(|gem| gem.item_id).collect()),
                _ => None,
            })
            .unwrap_or_default();
        let spell = |exported: &ExportedSpell| MageAgent::spell(exported, &gems);
        let mut fight = Fight::new(prepared, MageAgent::default(), spell, |unit, label| {
            (unit == "player")
                .then(|| {
                    auras
                        .iter()
                        .find(|(name, _)| name == label)
                        .map(|(_, kind)| *kind)
                })
                .flatten()
        })?;
        for effect in &prepared.effects {
            match effect {
                Effect::WintersChill {
                    aura,
                    trigger_aura,
                    proc_chance,
                    crit_per_stack,
                    ..
                } => {
                    let bound = winters_chill::bind(
                        &mut fight,
                        aura,
                        trigger_aura,
                        *proc_chance,
                        *crit_per_stack,
                    )?;
                    fight.agent.winters_chill = Some(Rc::new(bound));
                }
                Effect::FingersOfFrost {
                    aura,
                    trigger_aura,
                    proc_chance,
                    shatter_crit,
                    ..
                } => {
                    let bound = fingers_of_frost::bind(
                        &mut fight,
                        aura,
                        trigger_aura,
                        *proc_chance,
                        *shatter_crit,
                    )?;
                    fight.agent.fingers_of_frost = Some(bound);
                }
                Effect::ArcaneConcentration {
                    aura,
                    trigger_aura,
                    proc_chance,
                    ..
                } => {
                    let bound =
                        arcane_concentration::bind(&fight, aura, trigger_aura, *proc_chance)?;
                    fight.agent.arcane_concentration = Some(Rc::new(bound));
                }
                Effect::MissileBarrage {
                    aura,
                    arcane_blast_chance,
                    bolt_chance,
                    rng_label,
                    cost_percent_add,
                    tick_length_delta_ns,
                    ..
                } => {
                    let bound = missile_barrage::bind(
                        &mut fight,
                        aura,
                        *arcane_blast_chance,
                        *bolt_chance,
                        rng_label,
                        *cost_percent_add,
                        *tick_length_delta_ns,
                    )?;
                    fight.agent.missile_barrage = Some(Rc::new(bound));
                }
                Effect::IceLance {
                    frozen_multiplier, ..
                } => fight.agent.ice_lance_frozen_multiplier = *frozen_multiplier,
                Effect::ArcaneBlast {
                    aura,
                    damage_per_stack,
                    cost_per_stack,
                    ..
                } => {
                    let bound =
                        arcane_blast::bind(&mut fight, aura, *damage_per_stack, *cost_per_stack)?;
                    fight.agent.arcane_charges = Some(Rc::new(bound));
                }
                Effect::ManaGems {
                    gems,
                    regen_window_seconds,
                } => {
                    let mana = gems.iter().map(|gem| gem.mana).collect();
                    fight.agent.gems = mana_gems::ManaGems::new(mana, *regen_window_seconds);
                }
                Effect::Evocation {
                    regen_aura,
                    regen_multiplier,
                    ..
                } => {
                    let aura = fight.player_aura(regen_aura)?;
                    fight.agent.evocation_regen = Some((aura, *regen_multiplier));
                }
                Effect::ArcaneMissiles { ranks } => {
                    for rank in ranks {
                        let find = |id: i32| {
                            fight
                                .spells
                                .iter()
                                .position(|spell| spell.id.spell_id == id && spell.id.tag == 0)
                        };
                        if let (Some(channel), Some(missile)) =
                            (find(rank.channel_spell_id), find(rank.tick_spell_id))
                        {
                            fight.agent.missiles.push((channel, missile));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(fight)
    }

    /// Run a Fingers of Frost hook with its state taken out of the agent.
    fn with_fingers<T>(
        fight: &mut Fight<Self>,
        hook: impl FnOnce(&mut fingers_of_frost::FingersOfFrost, &mut Fight<Self>) -> T,
    ) -> T {
        let mut state = fight
            .agent
            .fingers_of_frost
            .take()
            .expect("Fingers of Frost is bound");
        let value = hook(&mut state, fight);
        fight.agent.fingers_of_frost = Some(state);
        value
    }

    fn winters_chill(fight: &Fight<Self>) -> Rc<winters_chill::WintersChill> {
        fight
            .agent
            .winters_chill
            .clone()
            .expect("Winter's Chill is bound")
    }

    fn arcane_concentration(fight: &Fight<Self>) -> Rc<arcane_concentration::ArcaneConcentration> {
        fight
            .agent
            .arcane_concentration
            .clone()
            .expect("Arcane Concentration is bound")
    }

    fn arcane_charges(fight: &Fight<Self>) -> Rc<arcane_blast::ArcaneCharges> {
        fight
            .agent
            .arcane_charges
            .clone()
            .expect("Arcane Blast is bound")
    }

    fn missile_barrage(fight: &Fight<Self>) -> Rc<missile_barrage::MissileBarrage> {
        fight
            .agent
            .missile_barrage
            .clone()
            .expect("Missile Barrage is bound")
    }
}

impl Agent for MageAgent {
    type Spell = MageSpell;
    type Aura = MageAura;

    fn apply_effects(fight: &mut Fight<Self>, spell: SpellId, target: Side, behavior: MageSpell) {
        match behavior {
            MageSpell::Frostbolt => frostbolt::apply(fight, spell, target),
            MageSpell::ArcaneBlast => {
                let charges = Self::arcane_charges(fight);
                arcane_blast::apply(fight, spell, target, &charges);
            }
            MageSpell::IceLance => {
                // Go IsTargetFrozen: Fingers of Frost is active.
                let frozen = fight
                    .agent
                    .fingers_of_frost
                    .as_ref()
                    .is_some_and(|fingers| fingers.frozen(fight));
                let multiplier = frozen.then_some(fight.agent.ice_lance_frozen_multiplier);
                ice_lance::apply(fight, spell, target, multiplier);
            }
            MageSpell::ArcaneMissiles => arcane_missiles::apply_channel(fight, spell),
            MageSpell::ArcaneMissile => arcane_missiles::apply_missile(fight, spell, target),
            MageSpell::ColdSnap => cold_snap::apply(fight),
            MageSpell::Evocation => evocation::apply(fight, spell),
            MageSpell::ManaGem(gem) => {
                let mana = fight.agent.gems.mana[gem];
                mana_gems::apply(fight, spell, mana);
                fight.agent.gems.used[gem] = true;
            }
        }
    }

    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: MageSpell) -> bool {
        match behavior {
            MageSpell::ManaGem(gem) => fight.agent.gems.available(gem),
            _ => true,
        }
    }

    fn should_activate(fight: &Fight<Self>, _spell: SpellId, behavior: MageSpell) -> bool {
        match behavior {
            MageSpell::ManaGem(gem) => fight.agent.gems.should_activate(fight, gem),
            // Go leaves Evocation to the rotation.
            MageSpell::Evocation => false,
            _ => true,
        }
    }

    fn reset(fight: &mut Fight<Self>) {
        fight.agent.gems.reset();
    }

    fn on_dot_gain(fight: &mut Fight<Self>, _dot: DotId, behavior: MageSpell) {
        if behavior == MageSpell::Evocation {
            let (aura, _) = fight.agent.evocation_regen.expect("Evocation is bound");
            fight.activate_aura(aura);
        }
    }

    fn on_dot_expire(fight: &mut Fight<Self>, _dot: DotId, behavior: MageSpell) {
        match behavior {
            MageSpell::Evocation => {
                let (aura, _) = fight.agent.evocation_regen.expect("Evocation is bound");
                fight.deactivate_aura(aura);
            }
            // The channel aura's own OnExpire runs before the dot's final tick.
            MageSpell::ArcaneMissiles => {
                if let Some(charges) = fight.agent.arcane_charges.clone() {
                    charges.on_channel_end(fight);
                }
            }
            _ => {}
        }
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: MageSpell) {
        if behavior == MageSpell::ArcaneMissiles {
            let channel = fight.dots[dot].spell;
            let side = fight.dots[dot].side;
            let missile = fight
                .agent
                .missiles
                .iter()
                .find(|(spell, _)| *spell == channel)
                .map(|(_, missile)| *missile)
                .expect("every channel rank has a missile");
            fight.cast(missile, side);
        }
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: MageAura) {
        match kind {
            MageAura::WintersChill => Self::winters_chill(fight).on_gain(fight),
            MageAura::FingersOfFrost => {
                Self::with_fingers(fight, |state, fight| state.on_gain(fight))
            }
            MageAura::Clearcasting => Self::arcane_concentration(fight).on_gain(fight),
            MageAura::MissileBarrage => Self::missile_barrage(fight).on_gain(fight),
            MageAura::ArcaneCharges => Self::arcane_charges(fight).on_gain(fight),
            MageAura::EvocationRegen => {
                let (_, multiplier) = fight.agent.evocation_regen.expect("Evocation is bound");
                evocation::regen_gain(fight, multiplier);
            }
            _ => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: MageAura) {
        match kind {
            MageAura::WintersChill => Self::winters_chill(fight).on_expire(fight),
            MageAura::FingersOfFrost => {
                Self::with_fingers(fight, |state, fight| state.on_expire(fight))
            }
            MageAura::Clearcasting => Self::arcane_concentration(fight).on_expire(fight),
            MageAura::MissileBarrage => Self::missile_barrage(fight).on_expire(fight),
            MageAura::ArcaneCharges => Self::arcane_charges(fight).on_expire(fight),
            MageAura::EvocationRegen => {
                let (_, multiplier) = fight.agent.evocation_regen.expect("Evocation is bound");
                evocation::regen_expire(fight, multiplier);
            }
            _ => {}
        }
    }

    fn on_stacks_change(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: MageAura,
        _old: i32,
        new: i32,
    ) {
        match kind {
            MageAura::WintersChill => Self::winters_chill(fight).on_stacks_change(fight, new),
            MageAura::ArcaneCharges => Self::arcane_charges(fight).on_stacks_change(fight, new),
            _ => {}
        }
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: MageAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        match kind {
            MageAura::WintersChillTrigger => {
                Self::winters_chill(fight).on_spell_hit_dealt(fight, spell, result)
            }
            MageAura::ArcaneConcentrationTrigger => {
                Self::arcane_concentration(fight).on_spell_hit_dealt(fight, spell, result)
            }
            MageAura::FingersOfFrostTrigger
                if Self::with_fingers(fight, |state, fight| {
                    state.should_proc(fight, spell, result)
                }) =>
            {
                let aura = fight.agent.fingers_of_frost.as_ref().expect("bound").aura;
                fight.activate_aura(aura);
                let max = fight.aura(aura).max_stacks;
                fight.set_stacks(aura, max);
            }
            _ => {}
        }
    }

    fn on_cast_complete(fight: &mut Fight<Self>, aura: AuraRef, kind: MageAura, spell: SpellId) {
        match kind {
            MageAura::FingersOfFrost
                if Self::with_fingers(fight, |state, fight| {
                    state.on_cast_complete(fight, spell)
                }) =>
            {
                // Go OnCastComplete runs after the damage roll, so the consuming cast keeps the bonus.
                fight.remove_stack(aura);
            }
            MageAura::Clearcasting => {
                Self::arcane_concentration(fight).on_cast_complete(fight, spell)
            }
            MageAura::MissileBarrage => Self::missile_barrage(fight).on_cast_complete(fight, spell),
            MageAura::MissileBarrageTrigger => Self::missile_barrage(fight).trigger(fight, spell),
            MageAura::ArcaneCharges => Self::arcane_charges(fight).on_cast_complete(fight, spell),
            _ => {}
        }
    }
}
