//! Go pet.go for a registered pet that nothing summons.
//!
//! Go adds every registered pet to the environment. Each reset enables its unit and its
//! agent's Reset dismisses it again, logging its stats; each fight's end logs that no pet is
//! summoned. It is never enabled, so its auto attacks, auras and encounter start never run,
//! and no random number is drawn for it: a pet's swing timer reset only rolls an offset for
//! enemies, and only enabled units start the encounter. Its metrics report zero, and every
//! action metric lists one more unit.

use crate::contracts::prepared_v2::{ActionId, Effect};

use super::{ActionReport, ActionTotals, Agent, Fight};

/// A pet Go registers but never enables.
pub(crate) struct InertPet {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) unit_index: i32,
    /// The actions its metrics list, with zero results.
    pub(crate) actions: Vec<ActionTotals>,
    /// The auras its metrics list, never active.
    pub(crate) auras: Vec<ActionId>,
    /// Go `pet.GetStats().FlatString()`, logged when the reset dismisses it.
    pub(crate) dismissed_log: String,
}

/// The inert pets a prepared input describes, in Go's order.
pub(crate) fn inert_pets(effects: &[Effect]) -> Vec<InertPet> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::InertPet {
                name,
                label,
                unit_index,
                metrics_actions,
                auras,
                dismissed_log,
                ..
            } => Some(InertPet {
                name: name.clone(),
                label: label.clone(),
                unit_index: *unit_index,
                actions: metrics_actions
                    .iter()
                    .map(|action| ActionTotals {
                        id: action.action_id.clone(),
                        melee: action.melee_metrics,
                        passive: false,
                        school: action.school,
                        targets: [ActionReport::new(0), ActionReport::new(1)],
                    })
                    .collect(),
                auras: auras.clone(),
                dismissed_log: dismissed_log.clone(),
            }),
            _ => None,
        })
        .collect()
}

impl<A: Agent> Fight<A> {
    /// Go `Pet.reset` through the agent's Reset, which dismisses the enabled unit.
    pub(crate) fn reset_inert_pets(&mut self) {
        if self.log.is_none() {
            return;
        }
        for index in 0..self.pets.len() {
            let (label, stats) = (
                self.pets[index].label.clone(),
                self.pets[index].dismissed_log.clone(),
            );
            self.log_at(self.now, &label, "Pet dismissed");
            self.log_at(self.now, &label, &stats);
        }
    }

    /// Go `Pet.doneIteration`, before the owner's own: dismissing a disabled pet only logs.
    pub(crate) fn inert_pets_done_iteration(&mut self) {
        if self.log.is_none() {
            return;
        }
        for index in 0..self.pets.len() {
            let label = self.pets[index].label.clone();
            self.log_at(self.now, &label, "No pet summoned");
        }
    }

    /// The unit indexes every action metric lists after the target and the player.
    pub(crate) fn extra_unit_indexes(&self) -> Vec<i32> {
        self.pets.iter().map(|pet| pet.unit_index).collect()
    }
}
