//! The Mage class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use crate::{
    contracts::prepared_v2::{PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, Fight, Side, SpellId},
};

use super::spells::frostbolt;

/// What a Mage spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MageSpell {
    Frostbolt,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MageAura {}

/// Mage state that Go keeps in the `Mage` struct and its closures.
#[derive(Default)]
pub(crate) struct MageAgent {}

impl MageAgent {
    pub(crate) fn new(_prepared: &PreparedV2) -> Self {
        MageAgent {}
    }

    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(spell: &ExportedSpell) -> Option<MageSpell> {
        match spell.class_spell.as_deref()? {
            "frostbolt" if spell.damage_effect.is_some() => Some(MageSpell::Frostbolt),
            _ => None,
        }
    }

    /// The class behavior of an exported aura, by unit and label.
    pub(crate) fn aura(_unit: &str, _label: &str) -> Option<MageAura> {
        None
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<MageAgent>, String> {
        Fight::new(
            prepared,
            MageAgent::new(prepared),
            MageAgent::spell,
            MageAgent::aura,
        )
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

    fn on_gain(_fight: &mut Fight<Self>, _aura: AuraRef, kind: MageAura) {
        match kind {}
    }
}
