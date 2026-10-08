//! Go buffs/drivers.go `AttachInnervateRegen`, shared by the druid's own Innervate and the
//! external one any mana class receives (`driveInnervates`): while the aura is up the player has
//! full spirit regeneration at five times the rate, and the bonus mana lands in regeneration
//! metrics of the aura's own, apart from the druid's cast cost. The generated aura states no
//! stat, so the regeneration is all the aura changes.

use crate::contracts::prepared_v2::Effect;

use super::{Agent, Fight};

/// One aura that carries the regeneration: its multiplier and metrics.
#[derive(Clone, Debug)]
pub(crate) struct InnervateRegen {
    multiplier: f64,
    metrics: usize,
}

impl<A: Agent> Fight<A> {
    /// Bind the Innervate regeneration of every aura but the druid's own, each with metrics of its
    /// own, as Go's `AttachInnervateRegen` creates them. An aura's behavior is its position here.
    pub(crate) fn bind_innervate_regens(&mut self, effects: &[Effect]) {
        for effect in effects {
            let Effect::InnervateRegen {
                spirit_regen_multiplier,
                regen_metrics_action_id,
                ..
            } = effect
            else {
                continue;
            };
            let metrics = self.new_mana_metrics(regen_metrics_action_id.clone());
            self.innervate_regens.push(InnervateRegen {
                multiplier: *spirit_regen_multiplier,
                metrics,
            });
        }
    }

    /// The `OnGain` of a regeneration aura, other than the druid's own.
    pub(crate) fn innervate_regen_gained(&mut self, index: usize) {
        let InnervateRegen {
            multiplier,
            metrics,
        } = self.innervate_regens[index];
        self.innervate_gained(metrics, multiplier);
    }

    /// The `OnExpire` of a regeneration aura, other than the druid's own.
    pub(crate) fn innervate_regen_expired(&mut self, index: usize) {
        let multiplier = self.innervate_regens[index].multiplier;
        self.innervate_expired(multiplier);
    }

    /// Go `AttachInnervateRegen`'s `OnGain`: attribution starts before the aura changes the
    /// regeneration.
    pub(crate) fn innervate_gained(&mut self, metrics: usize, multiplier: f64) {
        self.start_spirit_attribution(metrics);
        self.player.force_full_spirit_regen = true;
        self.player.spirit_regen_multiplier *= multiplier;
        self.update_mana_regen_rates();
    }

    /// Go `AttachInnervateRegen`'s `OnExpire`.
    pub(crate) fn innervate_expired(&mut self, multiplier: f64) {
        self.stop_spirit_attribution();
        self.player.force_full_spirit_regen = false;
        self.player.spirit_regen_multiplier /= multiplier;
        self.update_mana_regen_rates();
    }
}
