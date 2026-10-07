//! Go sim/core/incapacitate.go: the flags several effects can own at once, derived from the
//! auras that are active rather than written by whichever effect ends first.

use super::sim::{Sim, UnitId};

/// Go `FearAuraTag`.
pub(crate) const FEAR_AURA_TAG: &str = "Fear";
/// Go `StunAuraTag`.
pub(crate) const STUN_AURA_TAG: &str = "Stun";
/// Go `ReducedAvoidanceAuraTag`: the tank hardcast aura, which suppresses avoidance the way a
/// stun does and so shares the `Stunned` flag with the stun kind.
pub(crate) const REDUCED_AVOIDANCE_AURA_TAG: &str = "Reduced Avoidance";

impl Sim {
    /// Go `unit.HasActiveAuraWithTag`.
    pub(crate) fn has_active_aura_with_tag(&self, unit: UnitId, tag: &str) -> bool {
        self.auras_with_tag(unit, tag)
            .into_iter()
            .any(|aura| self.aura(aura).active)
    }

    /// Go `unit.refreshIncapacitateState`.
    pub(crate) fn refresh_incapacitate_state(&mut self, unit: UnitId) {
        let feared = self.has_active_aura_with_tag(unit, FEAR_AURA_TAG);
        let stunned = self.has_active_aura_with_tag(unit, STUN_AURA_TAG);
        let reduced = self.has_active_aura_with_tag(unit, REDUCED_AVOIDANCE_AURA_TAG);
        let pseudo = &mut self.unit_mut(unit).pseudo_stats;
        pseudo.incapacitated = feared || stunned;
        pseudo.stunned = stunned || reduced;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::sim::{AuraConfig, EnvState, NEVER_EXPIRES};
    use crate::prepare::sim::{Unit, UnitType};
    use std::rc::Rc;

    fn tagged(sim: &mut Sim, unit: UnitId, label: &str, tag: &str) -> crate::prepare::sim::AuraId {
        sim.register_aura(
            unit,
            AuraConfig {
                label: label.to_string(),
                tag: tag.to_string(),
                duration: NEVER_EXPIRES,
                on_gain: Some(Rc::new(|sim: &mut Sim, aura| {
                    let unit = sim.aura(aura).unit;
                    sim.refresh_incapacitate_state(unit);
                })),
                on_expire: Some(Rc::new(|sim: &mut Sim, aura| {
                    let unit = sim.aura(aura).unit;
                    sim.refresh_incapacitate_state(unit);
                })),
                ..Default::default()
            },
        )
    }

    #[test]
    fn reduced_avoidance_stuns_without_incapacitating_and_a_stun_outlives_it() {
        let mut sim = Sim::new();
        let unit = sim.add_unit(Unit::new(UnitType::Player, "unit".to_string()));
        sim.state = EnvState::Constructed;
        let reduced = tagged(
            &mut sim,
            unit,
            "Reduced avoidance",
            REDUCED_AVOIDANCE_AURA_TAG,
        );
        let stun = tagged(&mut sim, unit, "Stun", STUN_AURA_TAG);

        sim.activate(reduced);
        assert!(sim.unit(unit).pseudo_stats.stunned);
        assert!(!sim.unit(unit).pseudo_stats.incapacitated);

        sim.activate(stun);
        assert!(sim.unit(unit).pseudo_stats.incapacitated);
        sim.deactivate(reduced);
        // The stun still holds both flags: a flag is derived, not cleared by the first to end.
        assert!(sim.unit(unit).pseudo_stats.stunned);
        sim.deactivate(stun);
        assert!(!sim.unit(unit).pseudo_stats.stunned);
        assert!(!sim.unit(unit).pseudo_stats.incapacitated);
    }
}
