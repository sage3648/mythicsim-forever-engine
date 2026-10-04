//! Hunter set bonus procs that restore mana, from Go sim/common/forever/item_sets_classic.go
//! (Beaststalker Armor) and sim/hunter/item_sets.go (Beastmaster and Cryptstalker Armor): a
//! proc trigger on the set bonus aura hears the listed spells' hits of an outcome, rolls its
//! chance, and a spell batch window later restores mana to a character with a mana bar.

use crate::{
    contracts::prepared_v2::Effect,
    core::fight::{
        Agent, AuraRef, Fight, ResourceKind, SpellId, SpellResult, OUTCOME_CRIT, OUTCOME_LANDED,
    },
};

/// A bound set bonus mana proc.
#[derive(Clone, Debug)]
pub(crate) struct ManaProc {
    pub(crate) trigger: AuraRef,
    spells: Vec<bool>,
    /// The outcome bits the hit needs.
    outcome: u16,
    chance: f64,
    mana: f64,
    metrics: usize,
}

impl ManaProc {
    /// Bind an exported mana proc, if the effect is one.
    pub(crate) fn bind<A: Agent>(
        fight: &mut Fight<A>,
        effect: &Effect,
    ) -> Result<Option<Self>, String> {
        let Effect::HunterSetManaProc {
            trigger_aura,
            spells,
            outcome,
            proc_chance,
            mana,
            metrics_action_id,
            delay_ns,
        } = effect
        else {
            return Ok(None);
        };
        if *delay_ns != crate::core::fight::SPELL_BATCH_WINDOW {
            return Err(format!(
                "{trigger_aura} waits {delay_ns}ns, not a spell batch window"
            ));
        }
        let outcome = match outcome.as_str() {
            "landed" => OUTCOME_LANDED,
            "crit" => OUTCOME_CRIT,
            other => return Err(format!("{trigger_aura} hears {other} hits")),
        };
        let mut by_spell = vec![false; fight.spells.len()];
        for &spell in spells {
            if let Some(slot) = by_spell.get_mut(spell) {
                *slot = true;
            }
        }
        let metrics = fight.new_resource_metrics(metrics_action_id.clone(), ResourceKind::Mana);
        Ok(Some(ManaProc {
            trigger: fight.player_aura(trigger_aura)?,
            spells: by_spell,
            outcome,
            chance: *proc_chance,
            mana: *mana,
            metrics,
        }))
    }

    /// The trigger's `OnSpellHitDealt`: a listed spell's hit of the outcome rolls the chance,
    /// then the handler waits a spell batch window.
    pub(crate) fn on_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.spells[spell] || result.outcome & self.outcome == 0 {
            return;
        }
        if self.chance != 1.0 && fight.random_for_aura(self.trigger) > self.chance {
            return;
        }
        fight.schedule_delayed_proc(self.trigger, spell, *result);
    }

    /// The handler: mana to a character with a mana bar.
    pub(crate) fn restore<A: Agent>(&self, fight: &mut Fight<A>) {
        if fight.has_mana_bar() {
            fight.add_mana(self.mana, self.metrics);
        }
    }
}
