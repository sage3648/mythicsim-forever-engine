//! Go exclusive_effect.go for single aura categories: at most one member aura is active, a
//! stronger or longer-lasting active member refuses a newcomer, and a newcomer that wins
//! deactivates the member it replaces. A stacking member bids for its stacks, as
//! spelldata's parsed debuffs do, and a class may set a member's bid, as the rogue's Expose
//! Armor does. The target's major armor category sets the target's armor from its active
//! member: a stacking member's stacks, or the bid of one that does not stack.
//!
//! Go's aura metrics report how long each exclusive effect held its category. Every other
//! category of a unit is tracked for that alone: it follows Go's rules for which effect is
//! active, but never refuses or deactivates an aura, so the fight runs as before.

use std::collections::HashMap;

use crate::core::time::{seconds, NEVER_EXPIRES};

use super::{Agent, AuraRef, Fight, Side};

/// Go's applied uptime of one exclusive effect: since when it holds its category, its time
/// this fight, and the sum and count of the fights done.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Uptime {
    active_since: i64,
    this_fight: i64,
    sum: i64,
    iterations: u32,
}

/// One member: its aura, Go `ExclusiveEffect.Priority`, its spell ID and `isEnabled`.
#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub(crate) aura: AuraRef,
    pub(crate) priority: f64,
    pub(crate) spell_id: i32,
    /// A stacking member's bid for each stack.
    per_stack: Option<f64>,
    enabled: bool,
    uptime: Uptime,
}

impl Member {
    pub(crate) fn new(aura: AuraRef, priority: f64, spell_id: i32, per_stack: Option<f64>) -> Self {
        Member {
            aura,
            priority,
            spell_id,
            per_stack,
            enabled: false,
            uptime: Uptime::default(),
        }
    }
}

/// One exclusive effect of an aura: an enforced or tracked category and the member.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EffectRef {
    tracking: bool,
    category: usize,
    member: usize,
}

/// Go `ExclusiveCategory`: one the runtime enforces, with `SingleAura`, or one it only tracks.
#[derive(Clone, Debug, Default)]
pub(crate) struct Category {
    /// Go `ExclusiveCategory.Name`.
    pub(crate) name: String,
    pub(crate) members: Vec<Member>,
    /// Go `activeEffect`, by member.
    active: Option<usize>,
}

impl Category {
    pub(crate) fn new(members: Vec<Member>) -> Self {
        Category {
            name: String::new(),
            members,
            active: None,
        }
    }

    /// The same category on another target, which holds copies of the first target's auras
    /// at the same positions, with no member enabled.
    pub(crate) fn on_unit(&self, side: super::Side) -> Category {
        Category {
            members: self
                .members
                .iter()
                .map(|member| Member {
                    aura: AuraRef {
                        side,
                        index: member.aura.index,
                    },
                    enabled: false,
                    uptime: Uptime::default(),
                    ..member.clone()
                })
                .collect(),
            name: self.name.clone(),
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

/// How one exclusive effect of an aura an `auraShouldRefresh` asks about reads in a fight.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RefreshReading {
    /// The aura holds its category alone: Go refreshes it once inactive or within the overlap
    /// of expiring.
    Own,
    /// Another aura holds the category for good and outranks the effect: Go never refreshes.
    Never,
    /// Go's rule against the category's active effect, read from the category the runtime
    /// keeps for the effect: one it enforces or one it tracks.
    Live,
}

impl<A: Agent> Fight<A> {
    /// Go `Aura.ShouldRefreshExclusiveEffects` for the aura's effect at a position: true when
    /// nothing holds the category or the effect outbids what does, when the aura stacks and
    /// its bid at full stacks would outbid it, and, on an equal bid, when no effect of that bid
    /// has more than the overlap left. An aura without that effect has nothing to refresh.
    pub(crate) fn exclusive_should_refresh(
        &self,
        aura: AuraRef,
        position: usize,
        overlap: i64,
    ) -> bool {
        let Some(effect) = self.aura_effects.get(&aura).and_then(|e| e.get(position)) else {
            return false;
        };
        let state = if effect.tracking {
            &self.exclusive_tracking[effect.category]
        } else {
            &self.exclusive[effect.category]
        };
        let Some(active) = state.active else {
            return true;
        };
        let bid = state.members[effect.member].priority;
        let held = state.members[active].priority;
        if bid > held {
            return true;
        }
        let aura_state = self.aura(aura);
        if aura_state.max_stacks > 0 {
            // Go divides by the stacks: none gives infinity, or not a number for a bid of zero.
            let at_full = bid / f64::from(aura_state.stacks) * f64::from(aura_state.max_stacks);
            if at_full > held {
                return true;
            }
        }
        bid == held
            && !state.members.iter().any(|other| {
                other.priority == bid && self.remaining_for_exclusive(other.aura) > overlap
            })
    }

    /// The enforced category of a single aura category the runtime derives from the exported
    /// memberships, for an aura an `auraShouldRefresh` reads whose category no class describes.
    /// Its members are the unit's auras that belong to it, in the category's order, bidding
    /// what Go registered them with.
    fn derive_enforced_category(
        &mut self,
        exported: &[(
            AuraRef,
            Vec<crate::contracts::prepared_v2::ExclusiveMembership>,
        )],
        side: Side,
        name: &str,
    ) {
        if self.enforced_category(side, name).is_some() {
            return;
        }
        let mut members: Vec<(u32, Member)> = Vec::new();
        for (aura, memberships) in exported {
            if aura.side != side {
                continue;
            }
            for membership in memberships.iter().filter(|m| m.category == name) {
                let spell_id = self
                    .aura(*aura)
                    .action_id
                    .as_ref()
                    .map_or(0, |id| id.spell_id);
                members.push((
                    membership.position,
                    Member::new(*aura, membership.priority, spell_id, None),
                ));
            }
        }
        if members.is_empty() {
            return;
        }
        members.sort_by_key(|(position, _)| *position);
        let mut category = Category::new(members.into_iter().map(|(_, member)| member).collect());
        category.name = name.to_string();
        self.exclusive.push(category);
    }

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

    fn category_mut(&mut self, tracking: bool, category: usize) -> &mut Category {
        if tracking {
            &mut self.exclusive_tracking[category]
        } else {
            &mut self.exclusive[category]
        }
    }

    /// Go `ExclusiveCategory.SetActive`'s uptime: the leaving effect adds the time it held the
    /// category, up to its aura's expiry while the aura is active, and the new one starts now.
    fn set_category_active(&mut self, tracking: bool, category: usize, new: Option<usize>) {
        let now = self.now;
        let old = self.category_mut(tracking, category).active;
        if old == new {
            return;
        }
        // A movement speed effect multiplies the player's speed as it takes hold and gives it
        // back as it lets go, at the bid it holds the category with.
        let moves = !tracking && self.movement_categories.contains(&category);
        if let Some(old) = old {
            let aura = self.category_mut(tracking, category).members[old].aura;
            let state = self.aura(aura);
            let end = if state.active {
                now.min(state.expires)
            } else {
                now
            };
            let uptime = &mut self.category_mut(tracking, category).members[old].uptime;
            uptime.this_fight += (end - uptime.active_since.max(0)).max(0);
            if moves {
                let bonus = self.exclusive[category].members[old].priority;
                self.multiply_movement_speed(1.0 / (1.0 + bonus));
            }
        }
        let state = self.category_mut(tracking, category);
        state.active = new;
        if let Some(new) = new {
            state.members[new].uptime.active_since = now;
            if moves {
                let bonus = self.exclusive[category].members[new].priority;
                self.multiply_movement_speed(1.0 + bonus);
            }
        }
        if tracking {
            let holder = |state: &Category, member: Option<usize>| {
                member.map(|member| state.members[member].aura)
            };
            let state = &self.exclusive_tracking[category];
            let (old, new) = (holder(state, old), holder(state, new));
            self.power_infusion_category_change(category, old, new);
            A::on_tracked_category_change(self, category);
        }
    }

    /// The name of a tracked category and the aura whose effect holds it, if any.
    pub(crate) fn tracked_category_holder(&self, category: usize) -> (&str, Option<AuraRef>) {
        let state = &self.exclusive_tracking[category];
        let holder = state.active.map(|member| state.members[member].aura);
        (state.name.as_str(), holder)
    }

    /// Go `ExclusiveEffect.Activate` for the aura's tracked effects, which never refuse.
    pub(crate) fn track_exclusive_activate(&mut self, aura: AuraRef) {
        let Some(effects) = self.aura_effects.get(&aura).cloned() else {
            return;
        };
        for effect in effects.into_iter().filter(|effect| effect.tracking) {
            let (category, member) = (effect.category, effect.member);
            let state = &mut self.exclusive_tracking[category];
            if state.members[member].enabled {
                continue;
            }
            state.members[member].enabled = true;
            let priority = state.members[member].priority;
            match state.active {
                None => self.set_category_active(true, category, Some(member)),
                Some(active) => {
                    let active_member = state.members[active].clone();
                    let newcomer = state.members[member].clone();
                    // Go keepsTie: another spell whose active effect outlasts the newcomer.
                    let keeps_tie = active_member.spell_id != newcomer.spell_id
                        && self.outlasts(active_member.aura, newcomer.aura);
                    if priority > active_member.priority
                        || (priority == active_member.priority && !keeps_tie)
                    {
                        self.set_category_active(true, category, Some(member));
                    }
                }
            }
        }
    }

    /// Go `ExclusiveEffect.Deactivate` for the aura's tracked effects.
    pub(crate) fn track_exclusive_deactivate(&mut self, aura: AuraRef) {
        let Some(effects) = self.aura_effects.get(&aura).cloned() else {
            return;
        };
        for effect in effects.into_iter().filter(|effect| effect.tracking) {
            let (category, member) = (effect.category, effect.member);
            let state = &mut self.exclusive_tracking[category];
            if !state.members[member].enabled {
                continue;
            }
            state.members[member].enabled = false;
            if state.active == Some(member) {
                let next = state.highest_enabled();
                self.set_category_active(true, category, next);
            }
        }
    }

    /// Go `Aura.Deactivate`'s first step: each of the aura's effects that holds its category
    /// adds its time up to the aura's expiry, and counts again from now.
    pub(crate) fn close_exclusive_uptime(&mut self, aura: AuraRef) {
        let Some(effects) = self.aura_effects.get(&aura).cloned() else {
            return;
        };
        let (now, expires) = (self.now, self.aura(aura).expires);
        for effect in effects {
            let state = self.category_mut(effect.tracking, effect.category);
            if state.active == Some(effect.member) {
                let uptime = &mut state.members[effect.member].uptime;
                uptime.this_fight += (now.min(expires) - uptime.active_since.max(0)).max(0);
                uptime.active_since = now;
            }
        }
    }

    /// Go `auraTracker.doneIteration` for every exclusive effect: the fight's uptime joins the
    /// sum, and the next fight starts from none.
    pub(crate) fn exclusive_done_iteration(&mut self) {
        for category in self
            .exclusive
            .iter_mut()
            .chain(self.exclusive_tracking.iter_mut())
        {
            for member in &mut category.members {
                member.uptime.sum += member.uptime.this_fight;
                member.uptime.this_fight = 0;
                member.uptime.iterations += 1;
            }
        }
    }

    /// Go `GetMetricsProto`'s effects of an aura: each effect's category and its average uptime
    /// over the fights done, in the aura's order.
    pub(crate) fn exclusive_effect_reports(&self, aura: AuraRef) -> Vec<(String, f64)> {
        let Some(effects) = self.aura_effects.get(&aura) else {
            return Vec::new();
        };
        effects
            .iter()
            .filter_map(|effect| {
                let category = if effect.tracking {
                    &self.exclusive_tracking[effect.category]
                } else {
                    &self.exclusive[effect.category]
                };
                let uptime = category.members[effect.member].uptime;
                (uptime.iterations > 0).then(|| {
                    (
                        category.name.clone(),
                        seconds(uptime.sum) / f64::from(uptime.iterations),
                    )
                })
            })
            .collect()
    }

    /// Build the categories the runtime only tracks and every aura's effects, from each unit's
    /// exported memberships: a category the runtime enforces keeps its own members, and every
    /// other category is tracked. The targets past the first hold copies of the first's.
    pub(crate) fn build_exclusive_tracking(&mut self) {
        let mut exported = self.exported_memberships.clone();
        for extra in 0..self.targets.len().saturating_sub(1) {
            let side = Side::Extra(extra as u8);
            let copies: Vec<_> = exported
                .iter()
                .filter(|(aura, _)| aura.side == Side::Target)
                .map(|(aura, memberships)| (self.aura_on(*aura, side), memberships.clone()))
                .collect();
            exported.extend(copies);
        }
        // A single aura category an `auraShouldRefresh` reads, or the player's movement speed
        // effects bid in, and no class describes is enforced from what the auras export, on
        // every target that holds a copy.
        for (side, name) in std::mem::take(&mut self.derived_categories) {
            self.derive_enforced_category(&exported, side, &name);
            if side == Side::Target {
                for extra in 0..self.targets.len().saturating_sub(1) {
                    self.derive_enforced_category(&exported, Side::Extra(extra as u8), &name);
                }
            }
        }
        for name in self.movement_category_names.clone() {
            self.movement_categories
                .extend(self.enforced_category(Side::Player, name));
        }
        // The tracked categories by unit and name, in the order first seen, each with its
        // members by position.
        let mut tracked: HashMap<(Side, String), Vec<(u32, Member)>> = HashMap::new();
        let mut order: Vec<(Side, String)> = Vec::new();
        for (aura, memberships) in &exported {
            for membership in memberships {
                if self
                    .enforced_category(aura.side, &membership.category)
                    .is_some()
                {
                    continue;
                }
                let key = (aura.side, membership.category.clone());
                if !tracked.contains_key(&key) {
                    order.push(key.clone());
                }
                let spell_id = self
                    .aura(*aura)
                    .action_id
                    .as_ref()
                    .map_or(0, |id| id.spell_id);
                tracked.entry(key).or_default().push((
                    membership.position,
                    Member::new(*aura, membership.priority, spell_id, None),
                ));
            }
        }
        let mut index: HashMap<(Side, String), usize> = HashMap::new();
        for key in order {
            let mut members = tracked.remove(&key).expect("collected above");
            members.sort_by_key(|(position, _)| *position);
            index.insert(key.clone(), self.exclusive_tracking.len());
            self.exclusive_tracking.push(Category {
                name: key.1,
                members: members.into_iter().map(|(_, member)| member).collect(),
                active: None,
            });
        }
        for (aura, memberships) in exported {
            let mut effects = Vec::new();
            for membership in memberships {
                let found = match self.enforced_category(aura.side, &membership.category) {
                    Some(category) => self.exclusive[category]
                        .members
                        .iter()
                        .position(|member| member.aura == aura)
                        .map(|member| EffectRef {
                            tracking: false,
                            category,
                            member,
                        }),
                    None => index
                        .get(&(aura.side, membership.category.clone()))
                        .and_then(|&category| {
                            self.exclusive_tracking[category]
                                .members
                                .iter()
                                .position(|member| member.aura == aura)
                                .map(|member| EffectRef {
                                    tracking: true,
                                    category,
                                    member,
                                })
                        }),
                };
                effects.extend(found);
            }
            if !effects.is_empty() {
                self.aura_effects.insert(aura, effects);
            }
        }
    }

    /// The enforced category of a unit by name.
    pub(crate) fn enforced_category(&self, side: Side, name: &str) -> Option<usize> {
        self.exclusive.iter().position(|category| {
            category.name == name
                && category
                    .members
                    .first()
                    .is_some_and(|member| member.aura.side == side)
        })
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
                None => self.set_category_active(false, category, Some(member)),
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
                        self.set_category_active(false, category, Some(member));
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
                let next = state.highest_enabled();
                self.set_category_active(false, category, next);
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
            self.set_category_active(false, category, strongest);
        }
        self.exclusive_armor(category);
    }

    /// A target's armor from its major armor category's active member, at its stacks.
    fn exclusive_armor(&mut self, category: usize) {
        let Some((armor_category, armor)) = &self.armor_category else {
            return;
        };
        let target = if *armor_category == category {
            super::Side::Target
        } else {
            match self
                .extra_armor_categories
                .iter()
                .find(|(copy, _)| *copy == category)
            {
                Some(&(_, side)) => side,
                None => return,
            }
        };
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
        let unit = &mut self.targets[target.target_position().expect("a target")];
        unit.armor = base + unit.armor_delta;
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
