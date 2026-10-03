//! Go exclusive_effect.go for single aura categories: at most one member aura is active, a
//! stronger or longer-lasting active member refuses a newcomer, and a newcomer that wins
//! deactivates the member it replaces.

use crate::core::time::NEVER_EXPIRES;

use super::{Agent, AuraRef, Fight};

/// One member: its aura, Go `ExclusiveEffect.Priority`, its spell ID and `isEnabled`.
#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub(crate) aura: AuraRef,
    pub(crate) priority: f64,
    pub(crate) spell_id: i32,
    enabled: bool,
}

/// Go `ExclusiveCategory` with `SingleAura`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Category {
    pub(crate) members: Vec<Member>,
    /// Go `activeEffect`, by member.
    active: Option<usize>,
}

impl Category {
    pub(crate) fn new(members: Vec<(AuraRef, f64, i32)>) -> Self {
        Category {
            members: members
                .into_iter()
                .map(|(aura, priority, spell_id)| Member {
                    aura,
                    priority,
                    spell_id,
                    enabled: false,
                })
                .collect(),
            active: None,
        }
    }
}

impl<A: Agent> Fight<A> {
    /// Go `RemainingDuration`.
    fn remaining_for_exclusive(&self, aura: AuraRef) -> i64 {
        self.aura(aura).remaining(self.now)
    }

    /// Go `ExclusiveEffect.outlasts`: the active aura's remaining time beats the newcomer's
    /// full duration.
    fn outlasts(&self, active: AuraRef, newcomer: AuraRef) -> bool {
        let remaining = self.remaining_for_exclusive(active);
        let duration = self.aura(newcomer).duration;
        remaining > duration && !(remaining == NEVER_EXPIRES && duration == NEVER_EXPIRES)
    }

    /// The categories and members an aura belongs to.
    fn memberships(&self, aura: AuraRef) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        for (category, state) in self.exclusive.iter().enumerate() {
            for (member, entry) in state.members.iter().enumerate() {
                if entry.aura == aura {
                    found.push((category, member));
                }
            }
        }
        found
    }

    /// Go `ExclusiveEffect.Activate` for each of the aura's effects, before it activates:
    /// false when an active member refuses it.
    pub(crate) fn activate_exclusive(&mut self, aura: AuraRef) -> bool {
        for (category, member) in self.memberships(aura) {
            if self.exclusive[category].members[member].enabled {
                continue;
            }
            let priority = self.exclusive[category].members[member].priority;
            if let Some(active) = self.exclusive[category].active.filter(|&a| a != member) {
                let active_member = self.exclusive[category].members[active].clone();
                if active_member.priority > priority
                    || (active_member.priority == priority
                        && self.outlasts(active_member.aura, aura))
                {
                    return false;
                }
            }
            self.exclusive[category].members[member].enabled = true;
            match self.exclusive[category].active {
                None => self.exclusive[category].active = Some(member),
                Some(active) => {
                    let active_member = self.exclusive[category].members[active].clone();
                    let newcomer_spell = self.exclusive[category].members[member].spell_id;
                    // Go keepsTie: another spell whose active effect outlasts the newcomer.
                    let keeps_tie = active_member.spell_id != newcomer_spell
                        && self.outlasts(active_member.aura, aura);
                    if priority > active_member.priority
                        || (priority == active_member.priority && !keeps_tie)
                    {
                        if active != member {
                            self.deactivate_aura(active_member.aura);
                        }
                        self.exclusive[category].active = Some(member);
                    }
                }
            }
        }
        true
    }

    /// Go `ExclusiveEffect.Deactivate` for each of the aura's effects.
    pub(crate) fn deactivate_exclusive(&mut self, aura: AuraRef) {
        for (category, member) in self.memberships(aura) {
            let state = &mut self.exclusive[category];
            if !state.members[member].enabled {
                continue;
            }
            state.members[member].enabled = false;
            if state.active == Some(member) {
                // Go GetHighestPrioActiveEffect: the first enabled member of the highest
                // priority.
                let mut best: Option<usize> = None;
                for (index, entry) in state.members.iter().enumerate() {
                    if entry.enabled
                        && best.is_none_or(|b| entry.priority > state.members[b].priority)
                    {
                        best = Some(index);
                    }
                }
                state.active = best;
            }
        }
    }

    /// Whether a category has an active member, Go `AnyActive`, and which aura it is.
    pub(crate) fn exclusive_active(&self, category: usize) -> Option<(AuraRef, f64)> {
        let state = &self.exclusive[category];
        state
            .active
            .map(|member| (state.members[member].aura, state.members[member].priority))
    }
}
