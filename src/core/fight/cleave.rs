//! Go spell_result.go's cleave and batched damage helpers: which targets a cleave hits, and
//! dealing a cast's results together after every one has been calculated.

use super::{Agent, Fight, Side, SpellId, SpellResult};

impl<A: Agent> Fight<A> {
    /// The targets Go `cleaveIteration` hits: the first target, then the next ones in unit
    /// index order, wrapping, up to `max_targets` and no more than the fight has.
    pub(crate) fn cleave_targets(&self, first: Side, max_targets: usize) -> Vec<Side> {
        let count = max_targets.min(self.targets.len());
        let mut targets = Vec::with_capacity(count);
        let mut current = first;
        for _ in 0..count {
            targets.push(current);
            current = self.next_target(current);
        }
        targets
    }

    /// Go `DealBatchedAoeDamage` and `DealBatchedPeriodicDamage`: deal each result of a cast in
    /// turn, once every one has been calculated, so a proc on an early target cannot change
    /// the calculation of a later one.
    pub(crate) fn deal_batched_aoe_damage(
        &mut self,
        spell: SpellId,
        results: &[SpellResult],
        periodic: bool,
    ) {
        for &result in results {
            self.deal_damage(spell, result, periodic);
        }
    }
}
