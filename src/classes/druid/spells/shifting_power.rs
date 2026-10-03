//! Shifting Power (1322605), from Go sim/druid/shifting_power.go: mana into energy, in Cat
//! Form.

use crate::{
    contracts::prepared_v2::ActionId,
    core::fight::{Agent, Fight, ResourceKind, SpellId},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShiftingPower {
    energy: f64,
    metrics: usize,
}

/// Go registers the energy metrics just before the spell's mana cost.
pub(crate) fn bind<A: Agent>(fight: &mut Fight<A>, spell: SpellId, energy: f64) -> ShiftingPower {
    let id: ActionId = fight.spells[spell].id.clone();
    let metrics = fight.new_resource_metrics_before_cost(spell, id, ResourceKind::Energy);
    ShiftingPower { energy, metrics }
}

impl ShiftingPower {
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.add_energy(self.energy, self.metrics);
    }
}
