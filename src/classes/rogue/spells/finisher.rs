//! Finishers, from Go sim/rogue/rogue.go `ApplyFinisher`: a finisher that takes effect spends
//! every combo point, then Relentless Strikes and Ruthlessness roll on the points spent.

use crate::{
    contracts::prepared_v2::ActionId,
    core::fight::{Agent, Fight, ResourceKind, SpellId},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Finisher {
    relentless_strikes: bool,
    relentless_strikes_chance_per_point: f64,
    relentless_strikes_energy: f64,
    relentless_strikes_metrics: usize,
    ruthlessness_chance: f64,
    ruthlessness_metrics: usize,
}

/// Go registers both metrics in `Initialize`, whatever the talents.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    relentless_strikes: bool,
    relentless_strikes_chance_per_point: f64,
    relentless_strikes_energy: f64,
    relentless_strikes_action: &ActionId,
    ruthlessness_chance: f64,
    ruthlessness_action: &ActionId,
) -> Finisher {
    let ruthlessness_metrics =
        fight.new_resource_metrics(ruthlessness_action.clone(), ResourceKind::ComboPoints);
    let relentless_strikes_metrics =
        fight.new_resource_metrics(relentless_strikes_action.clone(), ResourceKind::Energy);
    Finisher {
        relentless_strikes,
        relentless_strikes_chance_per_point,
        relentless_strikes_energy,
        relentless_strikes_metrics,
        ruthlessness_chance,
        ruthlessness_metrics,
    }
}

impl Finisher {
    /// Go `ApplyFinisher`.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let points = fight.energy_bar().combo_points;
        let (_, combo_metrics) = fight.spells[spell]
            .energy_metrics
            .expect("a finisher costs energy");
        fight.spend_combo_points(combo_metrics);
        if self.relentless_strikes
            && fight.proc(
                self.relentless_strikes_chance_per_point * f64::from(points),
                "Relentless Strikes",
            )
        {
            fight.add_energy(
                self.relentless_strikes_energy,
                self.relentless_strikes_metrics,
            );
        }
        // Go reads the talent rank first; an untaken talent has no chance and draws nothing.
        if self.ruthlessness_chance > 0.0 && fight.proc(self.ruthlessness_chance, "Ruthlessness") {
            fight.add_combo_points(1, self.ruthlessness_metrics);
        }
    }
}

/// The combo point metrics of an energy spell, Go `Spell.ComboPointMetrics`.
pub(crate) fn combo_point_metrics<A: Agent>(fight: &Fight<A>, spell: SpellId) -> usize {
    fight.spells[spell]
        .energy_metrics
        .expect("the spell costs energy")
        .1
}
