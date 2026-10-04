//! Innervate (29166), from Go sim/druid/innervate.go and core/buffs/drivers.go
//! `AttachInnervateRegen`: full spirit regeneration at five times the rate for its duration,
//! with the bonus mana credited to its own regeneration metrics.

use crate::{
    contracts::prepared_v2::ActionId,
    core::fight::{Agent, AuraRef, Fight},
};

#[derive(Clone, Debug)]
pub(crate) struct Innervate {
    pub(crate) aura: AuraRef,
    spirit_regen_multiplier: f64,
    metrics: usize,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    spirit_regen_multiplier: f64,
    metrics_action_id: &ActionId,
) -> Result<Innervate, String> {
    let aura = fight.player_aura(aura)?;
    let metrics = fight.new_mana_metrics(metrics_action_id.clone());
    Ok(Innervate {
        aura,
        spirit_regen_multiplier,
        metrics,
    })
}

impl Innervate {
    /// Go `ExtraCastCondition`: no Innervate already on the druid.
    pub(crate) fn can_cast<A: Agent>(&self, fight: &Fight<A>) -> bool {
        !fight.aura(self.aura).active
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.start_spirit_attribution(self.metrics);
        fight.player.force_full_spirit_regen = true;
        fight.player.spirit_regen_multiplier *= self.spirit_regen_multiplier;
        fight.update_mana_regen_rates();
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.stop_spirit_attribution();
        fight.player.force_full_spirit_regen = false;
        fight.player.spirit_regen_multiplier /= self.spirit_regen_multiplier;
        fight.update_mana_regen_rates();
    }
}
