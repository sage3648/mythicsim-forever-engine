//! Last Stand (12975), from Go sim/warrior/talents_protection.go `registerLastStand` and
//! health.go `UpdateMaxHealth`: a survival cooldown whose aura raises maximum health by its
//! share of the maximum health at activation and gains that much health, then on expiry takes
//! the maximum back and removes the same health, leaving at least 1. The maximum health rides
//! on the stat aura combinations.

use crate::{
    contracts::prepared_v2::{ActionId, Effect},
    core::fight::{Agent, AuraRef, Fight},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct LastStand {
    pub(crate) aura: AuraRef,
    pub(crate) health_share: f64,
    pub(crate) metrics: usize,
    stat_bit: u32,
    /// Go's `bonusHealth`: the health the last gain added.
    bonus: f64,
}

impl LastStand {
    pub(crate) fn bind<A: Agent>(
        fight: &mut Fight<A>,
        effects: &[Effect],
        aura: &str,
        health_share: f64,
        metrics_action_id: ActionId,
    ) -> Result<Self, String> {
        let stat_bit = Fight::<A>::stat_aura_bit(effects, aura)
            .ok_or_else(|| format!("{aura} is not a stat aura"))?;
        Ok(LastStand {
            aura: fight.player_aura(aura)?,
            health_share,
            metrics: fight.new_health_metrics(metrics_action_id),
            stat_bit,
            bonus: 0.0,
        })
    }

    /// The aura's OnGain: the share of the current maximum, the higher maximum, then the
    /// health gain.
    pub(crate) fn on_gain<A: Agent>(mut self, fight: &mut Fight<A>) -> Self {
        self.bonus = fight.player_max_health() * self.health_share;
        fight.set_stat_aura(self.stat_bit, true);
        fight.gain_health(self.bonus, self.metrics);
        self
    }

    /// The aura's OnExpire: the lower maximum, then the health it added, leaving at least 1.
    pub(crate) fn on_expire<A: Agent>(self, fight: &mut Fight<A>) {
        fight.set_stat_aura(self.stat_bit, false);
        let amount = self.bonus.min(fight.player.health - 1.0).max(0.0);
        fight.remove_health(amount);
    }
}
