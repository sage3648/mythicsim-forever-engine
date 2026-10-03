//! The Paladin class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, Fight, Side, SpellId, SpellResult},
};

use super::spells::judgement_refresh;

/// What a Paladin spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaladinSpell {}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaladinAura {
    JudgementRefresh,
}

/// Paladin state that Go keeps in the `Paladin` struct and its closures.
#[derive(Default)]
pub(crate) struct PaladinAgent {
    judgement_refresh: Option<Rc<judgement_refresh::JudgementRefresh>>,
}

impl PaladinAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(_spell: &ExportedSpell) -> Option<PaladinSpell> {
        None
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<PaladinAgent>, String> {
        let mut auras = Vec::new();
        for effect in &prepared.effects {
            if let Effect::JudgementRefresh { trigger_aura, .. } = effect {
                auras.push((trigger_aura.clone(), PaladinAura::JudgementRefresh));
            }
        }
        let mut fight = Fight::new(
            prepared,
            PaladinAgent::default(),
            PaladinAgent::spell,
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
            if let Effect::JudgementRefresh {
                judgement_auras, ..
            } = effect
            {
                let bound = judgement_refresh::bind(&fight, judgement_auras)?;
                fight.agent.judgement_refresh = Some(Rc::new(bound));
            }
        }
        Ok(fight)
    }
}

impl Agent for PaladinAgent {
    type Spell = PaladinSpell;
    type Aura = PaladinAura;

    fn apply_effects(
        _fight: &mut Fight<Self>,
        _spell: SpellId,
        _target: Side,
        behavior: PaladinSpell,
    ) {
        match behavior {}
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: PaladinAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        match kind {
            PaladinAura::JudgementRefresh => {
                let refresh = fight
                    .agent
                    .judgement_refresh
                    .clone()
                    .expect("Judgement Refresh is bound");
                refresh.on_spell_hit_dealt(fight, spell, result);
            }
        }
    }
}
