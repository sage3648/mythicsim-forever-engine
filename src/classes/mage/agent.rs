//! The Mage class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{frostbolt, ice_lance},
    talents::{fingers_of_frost, winters_chill},
};

/// What a Mage spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MageSpell {
    Frostbolt,
    IceLance,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MageAura {
    WintersChill,
    WintersChillTrigger,
    FingersOfFrost,
    FingersOfFrostTrigger,
}

/// Mage state that Go keeps in the `Mage` struct and its closures.
#[derive(Default)]
pub(crate) struct MageAgent {
    winters_chill: Option<winters_chill::WintersChill>,
    fingers_of_frost: Option<fingers_of_frost::FingersOfFrost>,
    ice_lance_frozen_multiplier: f64,
}

/// Player aura labels claimed by implemented class effects.
fn class_auras(prepared: &PreparedV2) -> Vec<(String, MageAura)> {
    let mut auras = Vec::new();
    for effect in &prepared.effects {
        match effect {
            Effect::WintersChill {
                aura, trigger_aura, ..
            } => {
                auras.push((aura.clone(), MageAura::WintersChill));
                auras.push((trigger_aura.clone(), MageAura::WintersChillTrigger));
            }
            Effect::FingersOfFrost {
                aura, trigger_aura, ..
            } => {
                auras.push((aura.clone(), MageAura::FingersOfFrost));
                auras.push((trigger_aura.clone(), MageAura::FingersOfFrostTrigger));
            }
            _ => {}
        }
    }
    auras
}

impl MageAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(spell: &ExportedSpell) -> Option<MageSpell> {
        match spell.class_spell.as_deref()? {
            "frostbolt" if spell.damage_effect.is_some() => Some(MageSpell::Frostbolt),
            "ice_lance" if spell.damage_effect.is_some() => Some(MageSpell::IceLance),
            _ => None,
        }
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<MageAgent>, String> {
        let auras = class_auras(prepared);
        let mut fight = Fight::new(
            prepared,
            MageAgent::default(),
            MageAgent::spell,
            |unit, label| {
                (unit == "player")
                    .then(|| {
                        auras
                            .iter()
                            .find(|(name, _)| name == label)
                            .map(|(_, kind)| *kind)
                    })
                    .flatten()
            },
        )?;
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
                    fight.agent.winters_chill = Some(bound);
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
                Effect::IceLance {
                    frozen_multiplier, ..
                } => fight.agent.ice_lance_frozen_multiplier = *frozen_multiplier,
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

    fn winters_chill(fight: &Fight<Self>) -> winters_chill::WintersChill {
        fight
            .agent
            .winters_chill
            .clone()
            .expect("Winter's Chill is bound")
    }
}

impl Agent for MageAgent {
    type Spell = MageSpell;
    type Aura = MageAura;

    fn apply_effects(fight: &mut Fight<Self>, spell: SpellId, target: Side, behavior: MageSpell) {
        match behavior {
            MageSpell::Frostbolt => frostbolt::apply(fight, spell, target),
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
        }
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: MageAura) {
        match kind {
            MageAura::WintersChill => Self::winters_chill(fight).on_gain(fight),
            MageAura::FingersOfFrost => {
                Self::with_fingers(fight, |state, fight| state.on_gain(fight))
            }
            MageAura::WintersChillTrigger | MageAura::FingersOfFrostTrigger => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: MageAura) {
        match kind {
            MageAura::WintersChill => Self::winters_chill(fight).on_expire(fight),
            MageAura::FingersOfFrost => {
                Self::with_fingers(fight, |state, fight| state.on_expire(fight))
            }
            MageAura::WintersChillTrigger | MageAura::FingersOfFrostTrigger => {}
        }
    }

    fn on_stacks_change(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: MageAura,
        _old: i32,
        new: i32,
    ) {
        if kind == MageAura::WintersChill {
            Self::winters_chill(fight).on_stacks_change(fight, new);
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
        if kind == MageAura::FingersOfFrost
            && Self::with_fingers(fight, |state, fight| state.on_cast_complete(fight, spell))
        {
            // Go OnCastComplete runs after the damage roll, so the consuming cast keeps the bonus.
            fight.remove_stack(aura);
        }
    }
}
