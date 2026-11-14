//! Water Shield (408510), from the client (1.60.1.70532): three globes for 10 minutes; a critical
//! spell of the shaman's, damage or heal, spends one and restores 2% of maximum mana (408511),
//! one globe every 3.5 sec at most. Client 70532 lets harmful spells spend globes (its proc flags
//! went from 156328 to 221864) and dropped the 15 sec cooldown. The same row hears the melee hits
//! the shaman takes (the 0x8 bit of those flags, at 100%): a landed swing of a target that tanks
//! the shaman spends a globe too, on the same internal cooldown.

use crate::contracts::prepared_v2::ActionId;
use crate::core::fight::{Agent, AuraRef, Fight, ResourceKind, SpellId, SpellResult};

#[derive(Clone, Debug)]
pub(crate) struct WaterShield {
    pub(crate) aura: AuraRef,
    pub(crate) globes: i32,
    /// The shared internal cooldown between globes.
    pub(crate) icd: i64,
    /// The share of maximum mana a globe restores.
    pub(crate) mana_fraction: f64,
    /// Whether a critical hit of the spell at each spellbook position spends a globe.
    pub(crate) triggers: Vec<bool>,
    /// The mana row of the globe spell (408511).
    pub(crate) metrics: usize,
}

impl WaterShield {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind<A: Agent>(
        fight: &mut Fight<A>,
        aura: &str,
        globes: i32,
        icd_ns: i64,
        mana_percent: f64,
        mana_spell_id: i32,
        trigger_spells: &[usize],
    ) -> Result<Self, String> {
        let aura = fight.player_aura(aura)?;
        let mut triggers = vec![false; fight.spells.len()];
        for &position in trigger_spells {
            *triggers.get_mut(position).ok_or_else(|| {
                format!("Water Shield trigger spell {position} is not exported")
            })? = true;
        }
        let metrics = fight.new_resource_metrics(
            ActionId {
                spell_id: mana_spell_id,
                ..ActionId::default()
            },
            ResourceKind::Mana,
        );
        Ok(Self {
            aura,
            globes,
            icd: icd_ns,
            mana_fraction: mana_percent / 100.0,
            triggers,
            metrics,
        })
    }

    /// The cast puts up every globe.
    pub(crate) fn cast<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_aura(self.aura);
        fight.activate_aura(self.aura);
        fight.set_stacks(self.aura, self.globes);
    }

    /// A damage or heal result of the shaman's: a critical hit of a trigger spell spends a globe
    /// once the internal cooldown is up. Returns when the next globe can go, if this one did.
    pub(crate) fn on_result<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
        ready_at: i64,
    ) -> Option<i64> {
        if !result.crit() || !self.triggers.get(spell).copied().unwrap_or(false) {
            return None;
        }
        if fight.now < ready_at || fight.aura(self.aura).stacks == 0 {
            return None;
        }
        Some(self.spend(fight))
    }

    /// The aura's OnSpellHitTaken for a target's swing: a landed hit spends a globe once the
    /// internal cooldown is up. Returns when the next globe can go, if this one did.
    pub(crate) fn on_hit_taken<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        result: &SpellResult,
        ready_at: i64,
    ) -> Option<i64> {
        if !result.landed() || fight.now < ready_at || fight.aura(self.aura).stacks == 0 {
            return None;
        }
        Some(self.spend(fight))
    }

    /// One globe goes for its share of maximum mana. Returns when the next can.
    fn spend<A: Agent>(&self, fight: &mut Fight<A>) -> i64 {
        fight.remove_stack(self.aura);
        if fight.has_mana_bar() {
            let amount = fight.player.powers.max_mana * self.mana_fraction;
            fight.add_mana(amount, self.metrics);
        }
        fight.now + self.icd
    }
}
