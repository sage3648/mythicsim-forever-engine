//! Mana gems, from Go sim/mage/mana_gems.go. One of each gem is carried and all share the
//! conjured cooldown. Each gem waits until every larger gem has been used, and the major
//! cooldown manager uses a gem only when all of it fits, one regeneration tick included.

use crate::core::fight::{Agent, Fight, SpellId};

#[derive(Clone, Debug, Default)]
pub(crate) struct ManaGems {
    /// Gem mana in Go's order, smallest first.
    pub(crate) mana: Vec<f64>,
    pub(crate) used: Vec<bool>,
    regen_window: f64,
}

impl ManaGems {
    pub(crate) fn new(mana: Vec<f64>, regen_window: f64) -> Self {
        let used = vec![false; mana.len()];
        ManaGems {
            mana,
            used,
            regen_window,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.used.iter_mut().for_each(|used| *used = false);
    }

    /// Go `ExtraCastCondition`.
    pub(crate) fn available(&self, gem: usize) -> bool {
        !self.used[gem]
    }

    /// Go `MajorCooldown.ShouldActivate` for a gem.
    pub(crate) fn should_activate<A: Agent>(&self, fight: &Fight<A>, gem: usize) -> bool {
        if self.used[gem + 1..].iter().any(|used| !used) {
            return false;
        }
        let total_regen = crate::mechanics::mana::regen_per_second_casting(fight.regen_inputs())
            * self.regen_window;
        fight.player.powers.max_mana - (fight.player.mana + total_regen) >= self.mana[gem]
    }
}

/// Go gem `ApplyEffects`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, mana: f64) {
    let metrics = fight.item_metrics(spell);
    fight.add_mana(mana, metrics);
}
