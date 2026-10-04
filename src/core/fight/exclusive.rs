//! Go exclusive_effect.go for single aura categories: at most one member aura is active, a
//! stronger or longer-lasting active member refuses a newcomer, and a newcomer that wins
//! deactivates the member it replaces. A stacking member bids for its stacks, as
//! spelldata's parsed debuffs do, and a class may set a member's bid, as the rogue's Expose
//! Armor does. The target's major armor category sets the target's armor from its active
//! member: a stacking member's stacks, or the bid of one that does not stack.

use crate::core::time::NEVER_EXPIRES;

use super::{Agent, AuraRef, Fight};

/// One member: its aura, Go `ExclusiveEffect.Priority`, its spell ID and `isEnabled`.
#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub(crate) aura: AuraRef,
    pub(crate) priority: f64,
    pub(crate) spell_id: i32,
    /// A stacking member's bid for each stack.
    per_stack: Option<f64>,
    enabled: bool,
}

impl Member {
    pub(crate) fn new(aura: AuraRef, priority: f64, spell_id: i32, per_stack: Option<f64>) -> Self {
        Member {
            aura,
            priority,
            spell_id,
            per_stack,
            enabled: false,
        }
    }
}

/// Go `ExclusiveCategory` with `SingleAura`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Category {
    pub(crate) members: Vec<Member>,
    /// Go `activeEffect`, by member.
    active: Option<usize>,
}

impl Category {
    pub(crate) fn new(members: Vec<Member>) -> Self {
        Category {
            members,
            active: None,
        }
    }

    /// Go `GetHighestPrioActiveEffect`: the first enabled member of the highest priority.
    fn highest_enabled(&self) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (index, entry) in self.members.iter().enumerate() {
            if entry.enabled && best.is_none_or(|b| entry.priority > self.members[b].priority) {
                best = Some(index);
            }
        }
        best
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
            self.exclusive_armor(category);
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
                state.active = state.highest_enabled();
            }
            self.exclusive_armor(category);
        }
    }

    /// spelldata's stacking bid: Go `ExclusiveEffect.SetPriority` to the stacks' worth.
    pub(crate) fn exclusive_stacks_changed(&mut self, aura: AuraRef, stacks: i32) {
        for (category, member) in self.memberships(aura) {
            let Some(per_stack) = self.exclusive[category].members[member].per_stack else {
                continue;
            };
            self.set_member_priority(category, member, per_stack * f64::from(stacks));
        }
    }

    /// Go `ExclusiveEffect.SetPriority` for each of the aura's effects.
    pub(crate) fn set_exclusive_priority(&mut self, aura: AuraRef, priority: f64) {
        for (category, member) in self.memberships(aura) {
            self.set_member_priority(category, member, priority);
        }
    }

    /// Go `ExclusiveEffect.SetPriority`: a disabled member only takes the bid. An enabled one
    /// takes or gives up the category when the strongest enabled member changes, and keeps
    /// the active member otherwise.
    fn set_member_priority(&mut self, category: usize, member: usize, priority: f64) {
        let state = &mut self.exclusive[category];
        let current = state.active;
        state.members[member].priority = priority;
        if !state.members[member].enabled {
            return;
        }
        let strongest = state.highest_enabled();
        if current == Some(member) || strongest == Some(member) {
            state.active = strongest;
        }
        self.exclusive_armor(category);
    }

    /// The target's armor from the major armor category's active member, at its stacks.
    fn exclusive_armor(&mut self, category: usize) {
        let Some((armor_category, armor)) = &self.armor_category else {
            return;
        };
        if *armor_category != category {
            return;
        }
        let active = self.exclusive[category]
            .active
            .map(|member| &self.exclusive[category].members[member]);
        let base = match active {
            Some(member) if member.per_stack.is_none() => armor[0] - member.priority,
            Some(member) => {
                let stacks = self.aura(member.aura).stacks;
                armor[(stacks.max(0) as usize).min(armor.len() - 1)]
            }
            None => armor[0],
        };
        self.target_armor = base + self.target_armor_delta;
    }

    /// The active member of the first category the aura belongs to, and its bid: Go
    /// `aura.ExclusiveEffects[0].Category.GetActiveEffect()`.
    pub(crate) fn exclusive_active_for(&self, aura: AuraRef) -> Option<(AuraRef, f64)> {
        let (category, _) = self.memberships(aura).into_iter().next()?;
        self.exclusive_active(category)
    }

    /// Whether a category has an active member, Go `AnyActive`, and which aura it is.
    pub(crate) fn exclusive_active(&self, category: usize) -> Option<(AuraRef, f64)> {
        let state = &self.exclusive[category];
        state
            .active
            .map(|member| (state.members[member].aura, state.members[member].priority))
    }
}
