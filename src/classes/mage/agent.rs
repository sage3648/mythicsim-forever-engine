//! The Mage class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, Fight, Side, SpellId, SpellResult},
};

use super::{spells::frostbolt, talents::winters_chill};

/// What a Mage spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MageSpell {
    Frostbolt,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MageAura {
    WintersChill,
    WintersChillTrigger,
}

/// Mage state that Go keeps in the `Mage` struct and its closures.
#[derive(Default)]
pub(crate) struct MageAgent {
    winters_chill: Option<winters_chill::WintersChill>,
}

/// Player aura labels claimed by implemented class effects.
fn class_auras(prepared: &PreparedV2) -> Vec<(String, MageAura)> {
    let mut auras = Vec::new();
    for effect in &prepared.effects {
        if let Effect::WintersChill {
            aura, trigger_aura, ..
        } = effect
        {
            auras.push((aura.clone(), MageAura::WintersChill));
            auras.push((trigger_aura.clone(), MageAura::WintersChillTrigger));
        }
    }
    auras
}

impl MageAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(spell: &ExportedSpell) -> Option<MageSpell> {
        match spell.class_spell.as_deref()? {
            "frostbolt" if spell.damage_effect.is_some() => Some(MageSpell::Frostbolt),
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
            if let Effect::WintersChill {
                aura,
                trigger_aura,
                proc_chance,
                crit_per_stack,
                ..
            } = effect
            {
                let bound = winters_chill::bind(
                    &mut fight,
                    aura,
                    trigger_aura,
                    *proc_chance,
                    *crit_per_stack,
                )?;
                fight.agent.winters_chill = Some(bound);
            }
        }
        Ok(fight)
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
        }
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: MageAura) {
        match kind {
            MageAura::WintersChill => Self::winters_chill(fight).on_gain(fight),
            MageAura::WintersChillTrigger => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: MageAura) {
        match kind {
            MageAura::WintersChill => Self::winters_chill(fight).on_expire(fight),
            MageAura::WintersChillTrigger => {}
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
        if kind == MageAura::WintersChillTrigger {
            Self::winters_chill(fight).on_spell_hit_dealt(fight, spell, result);
        }
    }
}
