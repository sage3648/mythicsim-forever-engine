//! Innervate (29166), from Go sim/druid/innervate.go and core/buffs/drivers.go
//! `AttachInnervateRegen`: full spirit regeneration at five times the rate for its duration,
//! with the bonus mana credited to its own regeneration metrics. The regeneration is the
//! shared runtime's, which the external Innervate any mana class receives uses too.

use crate::{
    contracts::prepared_v2::ActionId,
    core::fight::{Agent, AuraRef, Fight},
};

/// Go `buffs.InnervatesCategory`: the tag of every copy of the Innervate aura.
const INNERVATES_CATEGORY: &str = "Innervate";

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
    /// Go `ExtraCastCondition`: no Innervate on the druid, its own or an external one.
    pub(crate) fn can_cast<A: Agent>(&self, fight: &Fight<A>) -> bool {
        !fight.player_has_active_aura_with_tag(INNERVATES_CATEGORY)
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.innervate_gained(self.metrics, self.spirit_regen_multiplier);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.innervate_expired(self.spirit_regen_multiplier);
    }
}
