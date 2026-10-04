//! Subtlety cooldowns, from Go sim/rogue/talents_subtlety.go: Premeditation's combo points
//! from Stealth, and Preparation, which resets the Rogue cooldowns it names and fires as a
//! major cooldown once Vanish is cooling down.

use crate::core::fight::{Agent, Fight, ResourceKind, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Premeditation {
    pub(crate) combo_points: i32,
    metrics: usize,
}

pub(crate) fn bind_premeditation<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    combo_points: i32,
) -> Premeditation {
    let id = fight.spells[spell].id.clone();
    let metrics = fight.new_resource_metrics(id, ResourceKind::ComboPoints);
    Premeditation {
        combo_points,
        metrics,
    }
}

impl Premeditation {
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.add_combo_points(self.combo_points, self.metrics);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Preparation {
    /// The spells whose cooldowns it resets, in Go's order.
    pub(crate) reset: Vec<SpellId>,
    pub(crate) vanish: Option<SpellId>,
}

impl Preparation {
    /// Go `Cooldown.Reset` on each named spell's cooldown timer.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        for &spell in &self.reset {
            if let Some((timer, _)) = fight.spells[spell].cd {
                fight.timers[timer] = crate::core::time::STARTING_CD_TIME;
            }
        }
    }

    /// Go `MajorCooldown.ShouldActivate`: Vanish is on cooldown.
    pub(crate) fn should_activate<A: Agent>(&self, fight: &Fight<A>) -> bool {
        self.vanish.is_some_and(|vanish| {
            fight.spells[vanish]
                .cd
                .is_some_and(|(timer, _)| fight.timers[timer] > fight.now)
        })
    }
}
