//! Conflagrate (18932), from Go sim/warlock/conflagrate.go: castable only while Immolate's
//! dot is up, a Fire hit dealt at once, then the dot burns away unless Shadow and Flame's
//! chance spares it.

use crate::core::fight::{Agent, DotId, Fight, Side, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct Conflagrate {
    /// Immolate's dot on the target.
    pub(crate) immolate: DotId,
    keep_immolate_chance: f64,
    rng_label: String,
}

impl Conflagrate {
    pub(crate) fn new(immolate: DotId, keep_immolate_chance: f64, rng_label: &str) -> Self {
        Self {
            immolate,
            keep_immolate_chance,
            rng_label: rng_label.to_string(),
        }
    }

    /// `ExtraCastCondition`.
    pub(crate) fn can_cast<A: Agent>(&self, fight: &Fight<A>) -> bool {
        fight.aura(fight.dots[self.immolate].aura).active
    }

    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        let base = fight.roll_damage_effect(spell);
        let result = fight.calc_damage(spell, target, base);
        fight.deal_damage(spell, result, false);
        let aura = fight.dots[fight.dot_on(self.immolate, target)].aura;
        if fight.aura(aura).active && !fight.proc(self.keep_immolate_chance, &self.rng_label) {
            fight.deactivate_aura(aura);
        }
    }
}
