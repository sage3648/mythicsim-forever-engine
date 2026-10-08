//! Go apl.go: compile the parsed rotation and run `DoNextAction`.

use crate::{
    contracts::prepared_v2::ActionId,
    core::time::NEVER_EXPIRES,
    rotation::{
        bind, compile_bool_value, compile_condition, compile_duration_value, compile_raw_value,
        Action as ParsedAction, CompareOp, CompiledCondition, DotAt, FoundAura, Lookup, MathOp,
        Reference, Rotation, Step, Unit, Value, ValueType,
    },
};

use super::{
    cast::MAX_SPELL_QUEUE_WINDOW, exclusive::RefreshReading, Agent, AuraRef, DotId, Fight, Side,
    SpellId,
};

use std::{ops::Range, rc::Rc};

mod lowered;

use lowered::Cond;

pub(crate) type Compiled = crate::rotation::Compiled<AuraRef>;

#[derive(Clone, Debug)]
pub(crate) enum Act {
    /// A cast on the unit the action names: the current target, or the player for
    /// `castFriendlySpell` at the player.
    Cast(SpellId, Side),
    Autocast,
    /// Go `APLActionStrictSequence`: its casts and the next one to run.
    StrictSequence {
        spells: Vec<SpellId>,
        next: usize,
    },
    /// Go `APLActionSequence`: its casts and the next one to run.
    Sequence {
        spells: Vec<SpellId>,
        next: usize,
    },
    /// Go `APLActionChannelSpell` with an interrupt condition.
    Channel {
        spell: SpellId,
        interrupt: Rc<Compiled>,
        allow_recast: bool,
    },
    /// Go `APLActionMove`: the range from the target the player runs to, read as a float.
    Move(Rc<Compiled>),
    /// Go `APLActionMoveDuration`: the duration of the move, read as a duration.
    MoveDuration(Rc<Compiled>),
    /// Go `APLActionGroupReference`: the first of an instance's actions that is ready runs. A
    /// reference that bound no instance is never ready.
    Group {
        instance: Option<usize>,
    },
    /// Go `APLActionMultidot`: its dot count after the encounter's target count capped it,
    /// and its overlap.
    Multidot {
        spell: SpellId,
        /// The spell whose dot it reads: its own, or its related dot spell's.
        dot_spell: SpellId,
        max_dots: i32,
        overlap: Option<Rc<Compiled>>,
    },
}

/// The compiled rotation: the priority list's items first, then those of each group instance.
#[derive(Debug)]
pub(crate) struct Compilation {
    pub(crate) items: Vec<Item>,
    /// How many of the items are the priority list's.
    pub(crate) priority: usize,
    /// Where each group instance's items are.
    pub(crate) groups: Vec<Range<usize>>,
}

/// A prepull action: a cast, or Go `APLActionActivateAura`.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PrepullAct {
    Cast(SpellId),
    ActivateAura(AuraRef),
    /// Go `APLActionMove`: the player runs to the range from the target.
    Move(f64),
    /// Go `APLActionMoveDuration`: the player moves for the duration.
    MoveDuration(i64),
}

/// A ready action. Go keeps the cooldown found by `IsReady` for `Execute`.
enum Ready {
    Cast(SpellId, Side),
    Autocast(usize),
    /// A strict sequence taking control of the rotation.
    Sequence(usize),
    /// A sequence's next step.
    SequenceStep(usize),
    /// A channel and the item whose interrupt condition it carries.
    Channel(usize, SpellId),
    /// A move to the range of the item's value from the target.
    Move(usize),
    /// A move for the duration of the item's value.
    MoveDuration(usize),
    /// A group reference and the item of its instance that runs.
    Group(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Item {
    /// Shared so evaluation, which can change the fight, never borrows the rotation.
    condition: Option<Rc<Cond>>,
    action: Act,
}

/// Go `APLRotation` fields beyond the priority list: the controlling strict sequence, the
/// sequence flag that lifts major cooldown restrictions, the running channel's interrupt
/// condition and the sequences hooked to the queued spell's action.
#[derive(Clone, Debug, Default)]
pub(crate) struct AplState {
    pub(crate) controlling: Option<usize>,
    pub(crate) in_sequence: bool,
    interrupt_channel_if: Option<usize>,
    allow_channel_recast: bool,
    pub(crate) queue_hooks: Vec<usize>,
}

impl<A: Agent> Fight<A> {
    /// Go `GetAuraByID` on a unit, as the rotation reads it. The rotation compiles before the
    /// targets past the first get their auras, which sit at the same positions as the first's.
    fn rotation_aura(&self, side: Side, id: &ActionId) -> Option<FoundAura<AuraRef>> {
        let source = if side.is_target() { Side::Target } else { side };
        let tracker = &self.trackers[source.index()];
        tracker.find_by_id(id).map(|index| FoundAura {
            aura: AuraRef { side, index },
            max_stacks: tracker.auras[index].max_stacks,
        })
    }

    /// Go `GetAPLDot`: the spell's area or self-only dot, the caster's `AOEDot`, which every
    /// unit shares, or else `Spell.Dot` of the unit: the spell's own dot or its related dot
    /// spell's, which sits on the targets and not on the player.
    fn rotation_dot(&self, id: &ActionId, unit: Unit) -> Option<SpellId> {
        let spell = self.apl_spell(id)?;
        let holder = match self.spells[spell].dot {
            Some(_) => spell,
            None => self.spells[spell]
                .related_dot_spell
                .filter(|&related| self.spells.get(related).is_some_and(|s| s.dot.is_some()))?,
        };
        let dot = self.spells[holder].dot?;
        (!self.dots[dot].side.is_target() || matches!(unit, Unit::Target(_))).then_some(holder)
    }

    /// Go `GetAPLSpell`: the first APL-flagged spell with the action ID, otherwise the first
    /// registered one. The potion action names the first combat potion.
    pub(crate) fn apl_spell(&self, id: &ActionId) -> Option<SpellId> {
        if id.other_id == "OtherActionPotion" {
            return self.spells.iter().position(|s| s.flags.combat_potion);
        }
        let apl = self.spells.iter().position(|s| &s.id == id && s.flags.apl);
        apl.or_else(|| self.spells.iter().position(|s| &s.id == id))
    }

    /// Go `GetAPLCastSpell`: a registered spell without the APL flag defers to an APL-flagged
    /// spell with the same action ignoring the tag, then to `GetAPLSpell`.
    fn apl_cast_spell(&self, id: &ActionId) -> Option<SpellId> {
        if let Some(spell) = self.spells.iter().find(|s| &s.id == id) {
            if !spell.flags.apl {
                let same = |s: &super::Spell<A::Spell>| {
                    s.id.spell_id == id.spell_id
                        && s.id.item_id == id.item_id
                        && s.id.other_id == id.other_id
                };
                if let Some(apl) = self.spells.iter().position(|s| same(s) && s.flags.apl) {
                    return Some(apl);
                }
            }
        }
        self.apl_spell(id)
    }

    /// Go's prepull registration: each castable action at its time, in a stable time order.
    pub(crate) fn compile_prepull(&self, rotation: &Rotation) -> Vec<(i64, PrepullAct)> {
        let find = |side: Side, id: &ActionId| self.rotation_aura(side, id);
        let aura = |id: &ActionId| find(Side::Player, id);
        let target_aura = |position: usize, id: &ActionId| find(Side::target(position), id);
        let spell = |id: &ActionId| self.apl_spell(id);
        let dot = |id: &ActionId, unit: Unit| self.rotation_dot(id, unit);
        let dot_base_duration = |id: &ActionId| {
            self.dot_base_durations
                .iter()
                .find(|(spell, _)| spell == id)
                .map(|(_, duration)| *duration)
        };
        let pet_aura_known = |pet: usize, id: &ActionId| {
            self.pet_agent_auras
                .get(pet)
                .is_some_and(|auras| auras.contains(id))
        };
        let lookup = Lookup {
            aura: &aura,
            target_aura: &target_aura,
            targets: self.targets.len(),
            spell: &spell,
            dot: &dot,
            dot_base_duration: &dot_base_duration,
            pet_aura_known: &pet_aura_known,
        };
        let mut prepull: Vec<(i64, PrepullAct)> = rotation
            .prepull
            .iter()
            // Go newAPLAction: a constant false condition drops the action; any other
            // condition is never evaluated before the prepull cast.
            .filter(|prepull| {
                compile_condition(prepull.condition.as_ref(), &lookup) != CompiledCondition::Pruned
            })
            .filter_map(|prepull| match &prepull.action {
                // Go newActionCastSpell: a target that names no unit drops the action. The runtime
                // casts a prepull action on the first target, which the gate checks.
                ParsedAction::CastSpell { spell: id, target } => target
                    .resolve(self.targets.len())
                    .and_then(|_| self.apl_cast_spell(id))
                    .map(|spell| (prepull.do_at_ns, PrepullAct::Cast(spell))),
                // Go GetAPLAura on the player: an unknown aura drops the action.
                ParsedAction::ActivateAura(id) => find(Side::Player, id)
                    .map(|found| (prepull.do_at_ns, PrepullAct::ActivateAura(found.aura))),
                // The parser reads a constant range or duration in a prepull action.
                ParsedAction::Move(Value::Const(range)) => {
                    Some((prepull.do_at_ns, PrepullAct::Move(range.float)))
                }
                ParsedAction::MoveDuration(Value::Const(duration)) => Some((
                    prepull.do_at_ns,
                    PrepullAct::MoveDuration(duration.duration_ns),
                )),
                _ => None,
            })
            .collect();
        prepull.sort_by_key(|(do_at, _)| *do_at);
        prepull
    }

    /// Go `newAPLRotation` for the supported subset: the items of the priority list, then the
    /// items of each instance of a group, and where each instance's items are.
    pub(crate) fn compile_rotation(&self, rotation: &Rotation) -> Result<Compilation, String> {
        let aura = |id: &ActionId| self.rotation_aura(Side::Player, id);
        let target_aura =
            |position: usize, id: &ActionId| self.rotation_aura(Side::target(position), id);
        let spell = |id: &ActionId| self.apl_spell(id);
        let dot = |id: &ActionId, unit: Unit| self.rotation_dot(id, unit);
        let dot_base_duration = |id: &ActionId| {
            self.dot_base_durations
                .iter()
                .find(|(spell, _)| spell == id)
                .map(|(_, duration)| *duration)
        };
        let pet_aura_known = |pet: usize, id: &ActionId| {
            self.pet_agent_auras
                .get(pet)
                .is_some_and(|auras| auras.contains(id))
        };
        let lookup = Lookup {
            aura: &aura,
            target_aura: &target_aura,
            targets: self.targets.len(),
            spell: &spell,
            dot: &dot,
            dot_base_duration: &dot_base_duration,
            pet_aura_known: &pet_aura_known,
        };
        // A reference whose condition is a constant false is pruned, and takes no part in
        // binding the groups.
        let live = |index: usize| {
            compile_condition(rotation.priority_list[index].condition.as_ref(), &lookup)
                != CompiledCondition::Pruned
        };
        let (rotation, binding) = bind(rotation, &live)?;
        let mut items = Vec::new();
        for (index, item) in rotation.priority_list.iter().enumerate() {
            let Some(action) = self.compile_action(
                &item.action,
                &lookup,
                binding.instance(Reference::Top(index)),
            ) else {
                continue;
            };
            let condition = match compile_condition(item.condition.as_ref(), &lookup) {
                // A constant false condition prunes the action; its spells already left
                // the major cooldowns in Go's export.
                CompiledCondition::Pruned => continue,
                CompiledCondition::Always => None,
                CompiledCondition::When(condition) => Some(Rc::new(Cond::lower(&condition))),
            };
            items.push(Item { condition, action });
        }
        let priority = items.len();
        let mut groups = Vec::new();
        for (instance, group) in binding.instances.iter().enumerate() {
            let start = items.len();
            for (index, item) in group.items.iter().enumerate() {
                let reference = Reference::Nested {
                    instance,
                    item: index,
                };
                let Some(action) =
                    self.compile_action(&item.action, &lookup, binding.instance(reference))
                else {
                    continue;
                };
                // Go `newAPLActionWithGroupVars` builds the condition and keeps a constant
                // whatever it holds.
                let condition = compile_bool_value(item.condition.as_ref(), &lookup)
                    .map(|condition| Rc::new(Cond::lower(&condition)));
                items.push(Item { condition, action });
            }
            groups.push(start..items.len());
        }
        Ok(Compilation {
            items,
            priority,
            groups,
        })
    }

    /// Go `newActionCastSpell` of a sequence's step: the spell the step casts, none for an
    /// unknown spell or a target that names no unit. The runtime casts a step on the first
    /// target, which the gate checks.
    fn step_spell(&self, step: &Step) -> Option<SpellId> {
        step.target.resolve(self.targets.len())?;
        self.apl_cast_spell(&step.spell)
    }

    /// Go's action constructors: the action a parsed one builds, or `None` where Go builds none.
    /// A reference runs the instance it is bound to.
    fn compile_action(
        &self,
        action: &ParsedAction,
        lookup: &Lookup<AuraRef>,
        instance: Option<usize>,
    ) -> Option<Act> {
        // Go `GetAPLMultidotSpell` reads `Spell.CurDot`: the spell's own dot on a target or
        // its related dot spell's. An area or self-only dot is the caster's `AOEDot`, which
        // `CurDot` does not return, so a multidot line for such a spell is dropped.
        let multidot_dot = |id: &ActionId| {
            self.apl_spell(id).and_then(|spell| {
                match self.spells[spell]
                    .dot
                    .filter(|&dot| self.dots[dot].side.is_target())
                {
                    Some(_) => Some(spell),
                    None => self.spells[spell].related_dot_spell.filter(|&related| {
                        self.spells.get(related).is_some_and(|s| s.dot.is_some())
                    }),
                }
            })
        };
        Some(match action {
            // Go newActionCastSpell: an unknown spell, or a target that is no unit, drops the
            // action.
            ParsedAction::CastSpell { spell: id, target } => {
                let unit = target.resolve(self.targets.len());
                match (self.apl_cast_spell(id), unit) {
                    (Some(spell), Some(Unit::Player)) => Act::Cast(spell, Side::Player),
                    (Some(spell), Some(Unit::Target(position))) => {
                        Act::Cast(spell, Side::target(position))
                    }
                    _ => return None,
                }
            }
            // Go newActionCastFriendlySpell at the player: the same cast on the player.
            ParsedAction::CastAtPlayer(id) => Act::Cast(self.apl_cast_spell(id)?, Side::Player),
            ParsedAction::AutocastOtherCooldowns => Act::Autocast,
            // Go newActionStrictSequence runs every step or none: a step the character
            // lacks drops the whole action (ElliotWood/Forever#625).
            ParsedAction::StrictSequence(steps) => {
                let spells = steps
                    .iter()
                    .map(|step| self.step_spell(step))
                    .collect::<Option<Vec<SpellId>>>()?;
                if spells.is_empty() {
                    return None;
                }
                Act::StrictSequence { spells, next: 0 }
            }
            // Go newActionSequence drops unknown casts, and the action when none remain.
            ParsedAction::Sequence(steps) => {
                let spells: Vec<SpellId> = steps
                    .iter()
                    .filter_map(|step| self.step_spell(step))
                    .collect();
                if spells.is_empty() {
                    return None;
                }
                Act::Sequence { spells, next: 0 }
            }
            // Go newActionChannelSpell: without an interrupt condition it is a cast;
            // otherwise the spell must be a channel. A target that names no unit drops the
            // action.
            ParsedAction::ChannelSpell {
                spell,
                target,
                interrupt_if,
                allow_recast,
            } => match (
                compile_bool_value(interrupt_if.as_ref(), lookup),
                target.resolve(self.targets.len()),
            ) {
                (None, Some(Unit::Target(position))) => {
                    Act::Cast(self.apl_cast_spell(spell)?, Side::target(position))
                }
                (None, Some(Unit::Player)) => Act::Cast(self.apl_cast_spell(spell)?, Side::Player),
                (Some(interrupt), Some(_)) => match self.apl_spell(spell) {
                    Some(spell) if self.spells[spell].flags.channeled => Act::Channel {
                        spell,
                        interrupt: Rc::new(interrupt),
                        allow_recast: *allow_recast,
                    },
                    _ => return None,
                },
                (_, None) => return None,
            },
            // Go GetAPLMultidotSpell: an unknown spell or one without a dot drops the
            // action; the encounter's one target caps the dot count.
            ParsedAction::Multidot {
                spell,
                max_dots,
                max_overlap,
            } => {
                let (spell, dot_spell) = self.apl_spell(spell).zip(multidot_dot(spell))?;
                Act::Multidot {
                    spell,
                    dot_spell,
                    max_dots: (*max_dots).min(self.targets.len() as i32),
                    overlap: compile_duration_value(max_overlap.as_ref(), lookup).map(Rc::new),
                }
            }
            // Go `newActionMove` and `newActionMoveDuration` always build the action. A value
            // that gives none panics where Go reads it, which the gate refuses.
            ParsedAction::Move(range) => Act::Move(Rc::new(compile_raw_value(range, lookup)?)),
            ParsedAction::MoveDuration(duration) => {
                Act::MoveDuration(Rc::new(compile_raw_value(duration, lookup)?))
            }
            // Go `newActionGroupReference`: no action without a group name; a reference that
            // binds no group is never ready.
            ParsedAction::GroupReference { name, .. } => {
                if name.is_empty() {
                    return None;
                }
                Act::Group { instance }
            }
            // Parsed only among the prepull actions.
            ParsedAction::ActivateAura(_) => return None,
        })
    }

    fn get_bool(&mut self, value: &Compiled) -> bool {
        match value {
            Compiled::Const(constant) => constant.boolean,
            Compiled::GroupUsed(used) => *used,
            Compiled::AuraIsActive(aura) => self.aura(*aura).active,
            // Go `APLValueAuraIsActive` with `includeReactionTime`: `Aura.TimeActive` is zero
            // while inactive and the time since the aura started otherwise.
            Compiled::AuraIsActiveAfterReaction(aura) => {
                let state = self.aura(*aura);
                state.active && self.now - state.start >= self.config.reaction
            }
            // Go `APLValueAuraIsInactive` with `includeReactionTime`: `Aura.TimeInactive` is
            // never-expiring for an aura that has not faded, the time since it faded otherwise.
            Compiled::AuraIsInactiveAfterReaction(aura) => {
                let state = self.aura(*aura);
                let inactive_for = if state.fade_time < 0 {
                    NEVER_EXPIRES
                } else {
                    self.now - state.fade_time
                };
                !state.active && inactive_for >= self.config.reaction
            }
            // Go `APLValueFrontOfTarget`.
            Compiled::FrontOfTarget => self.config.melee.in_front_of_target,
            // Go `ShouldRefreshExclusiveEffects`: any of the aura's effects asking for a refresh.
            Compiled::AuraShouldRefresh { aura, overlap } => {
                let window = self.get_duration(overlap);
                // The exporter reads the first target's aura, which its copies share.
                let first = AuraRef {
                    side: if aura.side.is_target() {
                        Side::Target
                    } else {
                        aura.side
                    },
                    index: aura.index,
                };
                let (_, readings) = self
                    .aura_refresh
                    .iter()
                    .find(|(refreshed, _)| *refreshed == first)
                    .expect("the gate requires a refresh reading");
                let state = self.aura(*aura);
                let remaining = state.remaining(self.now);
                readings
                    .iter()
                    .enumerate()
                    .any(|(position, reading)| match reading {
                        // Alone in its category: refreshed once inactive or within the overlap.
                        RefreshReading::Own => !state.active || remaining <= window,
                        // A permanent aura of another effect holds the category for good.
                        RefreshReading::Never => false,
                        RefreshReading::Live => {
                            self.exclusive_should_refresh(*aura, position, window)
                        }
                    })
            }
            Compiled::DotIsActive(dot) => self.dot_active(*dot),
            // Go `APLValueSpellIsReady`: ready, or ready within the spell queue window.
            Compiled::SpellIsReady(spell) => {
                self.spell_ready(*spell)
                    || self.spell_time_to_ready(*spell) <= MAX_SPELL_QUEUE_WINDOW
            }
            // Go `APLValueGCDIsReady`: ready, or ready within the spell queue window.
            Compiled::GcdIsReady => {
                self.gcd_ready() || self.gcd_time_to_ready() <= MAX_SPELL_QUEUE_WINDOW
            }
            // Go `APLValueIsExecutePhase`: the encounter's execute phase is at or below it.
            Compiled::IsExecutePhase(threshold) => self.execute_phase <= *threshold,
            // Go `APLValueSpellCanCast`: `CanCastOrQueue`, with its cost check's side effects.
            Compiled::SpellCanCast(spell) => self.can_cast_or_queue(*spell),
            Compiled::And(values) => values.iter().all(|value| self.get_bool(value)),
            Compiled::Or(values) => values.iter().any(|value| self.get_bool(value)),
            Compiled::Not(value) => !self.get_bool(value),
            Compiled::Compare { op, lhs, rhs } => match lhs.value_type() {
                ValueType::Bool => match op {
                    CompareOp::Eq => self.get_bool(lhs) == self.get_bool(rhs),
                    CompareOp::Ne => self.get_bool(lhs) != self.get_bool(rhs),
                    _ => false,
                },
                ValueType::Int => compare(*op, self.get_int(lhs), self.get_int(rhs)),
                ValueType::Float => compare_float(*op, self.get_float(lhs), self.get_float(rhs)),
                ValueType::Duration => compare(*op, self.get_duration(lhs), self.get_duration(rhs)),
                ValueType::String => false,
            },
            Compiled::Coerced { inner, .. } => match inner.value_type() {
                ValueType::Bool => self.get_bool(inner),
                ValueType::Int => self.get_int(inner) != 0,
                ValueType::Float => self.get_float(inner) != 0.0,
                ValueType::Duration => self.get_duration(inner) != 0,
                ValueType::String => false,
            },
            _ => false,
        }
    }

    fn get_int(&mut self, value: &Compiled) -> i32 {
        match value {
            Compiled::Const(constant) => constant.int,
            Compiled::AuraNumStacks(aura) => self.aura(*aura).stacks,
            // Go `APLValueAuraNumStacks` with `includeReactionTime`: the stacks of a reaction
            // time ago, which its stack change handler kept. The wrapping difference is Go's
            // for a handler that has not run since the reset.
            Compiled::AuraNumStacksAfterReaction(aura) => {
                let state = self.aura(*aura);
                if self.now.wrapping_sub(state.stack_update) >= self.config.reaction {
                    state.stacks
                } else {
                    state.previous_stacks
                }
            }
            Compiled::CurrentComboPoints => self.energy_bar().combo_points,
            // Go `ActiveTargetCount`: every target is active throughout.
            Compiled::NumberTargets => self.targets.len() as i32,
            // Go `APLValueMath.GetInt`: int32 arithmetic, which wraps.
            Compiled::Math { op, lhs, rhs } => {
                let (lhs, rhs) = (self.get_int(lhs), self.get_int(rhs));
                match op {
                    MathOp::Add => lhs.wrapping_add(rhs),
                    MathOp::Sub => lhs.wrapping_sub(rhs),
                    MathOp::Mul => lhs.wrapping_mul(rhs),
                    MathOp::Div => {
                        assert!(rhs != 0, "integer division by zero");
                        lhs.wrapping_div(rhs)
                    }
                }
            }
            Compiled::Coerced { inner, .. } => match inner.value_type() {
                ValueType::Bool => i32::from(self.get_bool(inner)),
                ValueType::Int => self.get_int(inner),
                ValueType::Float => self.get_float(inner) as i32,
                ValueType::Duration => crate::core::time::seconds(self.get_duration(inner)) as i32,
                ValueType::String => 0,
            },
            _ => 0,
        }
    }

    fn get_float(&mut self, value: &Compiled) -> f64 {
        match value {
            Compiled::Const(constant) => constant.float,
            Compiled::CurrentManaPercent => self.player.mana / self.player.powers.max_mana,
            // Go `Unit.CurrentHealthPercent`.
            Compiled::CurrentHealthPercent => self.player.health / self.player_max_health(),
            // Go `GetRemainingDurationPercent` for a fight timed by duration.
            Compiled::RemainingTimePercent => {
                (self.duration - self.now) as f64 / self.duration as f64
            }
            Compiled::CurrentMana => self.player.mana,
            Compiled::CurrentEnergy => self.energy_bar().current,
            Compiled::CurrentRage => self.current_rage(),
            Compiled::MaxEnergy => self.energy_bar().max,
            Compiled::MaxMana => self.player.powers.max_mana,
            // Go `APLValueSpellCurrentCost`: no cost reads zero.
            Compiled::SpellCurrentCost(spell) => self.current_cost(*spell),
            Compiled::NumberTargets => self.targets.len() as f64,
            // Go `APLValueMath.GetFloat`.
            Compiled::Math { op, lhs, rhs } => match op {
                MathOp::Add => self.get_float(lhs) + self.get_float(rhs),
                MathOp::Sub => self.get_float(lhs) - self.get_float(rhs),
                MathOp::Mul => self.get_float(lhs) * self.get_float(rhs),
                MathOp::Div
                    if lhs.value_type() == ValueType::Duration
                        && rhs.value_type() == ValueType::Duration =>
                {
                    let divisor = crate::core::time::seconds(self.get_duration(rhs));
                    assert!(divisor != 0.0, "Division by zero in duration / duration");
                    crate::core::time::seconds(self.get_duration(lhs)) / divisor
                }
                MathOp::Div => self.get_float(lhs) / self.get_float(rhs),
            },
            Compiled::Coerced { inner, .. } => match inner.value_type() {
                ValueType::Bool => f64::from(u8::from(self.get_bool(inner))),
                ValueType::Int => f64::from(self.get_int(inner)),
                ValueType::Float => self.get_float(inner),
                ValueType::Duration => crate::core::time::seconds(self.get_duration(inner)),
                ValueType::String => 0.0,
            },
            _ => 0.0,
        }
    }

    fn get_duration(&mut self, value: &Compiled) -> i64 {
        match value {
            Compiled::Const(constant) => constant.duration_ns,
            // Go `APLValueAuraRemainingTime`: zero when inactive.
            Compiled::AuraRemainingTime(aura) => self.aura_remaining_time(*aura),
            Compiled::RemainingTime => self.duration - self.now,
            // Go `Spell.TimeToReady`.
            Compiled::SpellTimeToReady(spell) => self.spell_time_to_ready(*spell),
            // Go `Dot.TimeUntilNextTick`: the next tick time is zero while inactive.
            Compiled::DotTimeToNextTick(at) => {
                let next = if self.dot_active(*at) {
                    self.dots[self.dot_of(*at)].tick_next_at
                } else {
                    0
                };
                next - self.now
            }
            Compiled::Math { op, lhs, rhs } => self.math_duration(*op, lhs, rhs),
            // Go `APLValueDotBaseDuration`: what the rotation captured when it was built.
            Compiled::DotBaseDuration(duration) => *duration,
            Compiled::TotemRemainingTime {
                totem,
                include_reaction_time,
            } => {
                let delay = if *include_reaction_time {
                    self.config.reaction
                } else {
                    0
                };
                let expires = A::totem_expiration(self, *totem);
                (expires + delay - self.now).max(0)
            }
            Compiled::CurrentTime => self.now,
            Compiled::TimeToNextEnergyTick => self.time_to_next_energy_tick(),
            // Go `APLValueDotRemainingTime`: zero when inactive.
            Compiled::DotRemainingTime(at) => {
                if self.dot_active(*at) {
                    let aura = self.aura(self.dots[self.dot_of(*at)].aura);
                    aura.expires - self.now
                } else {
                    0
                }
            }
            // Go `Spell.CastTime`: the default cast time with current cast speed, unrounded.
            Compiled::SpellCastTime(spell) => self.class_cast_time(*spell).unwrap_or_else(|| {
                let cast_time = self.spells[*spell].default_cast.cast_time;
                self.apply_cast_speed_for_spell(cast_time, *spell)
            }),
            // Go `APLValueAutoTimeToNext`.
            Compiled::AutoTimeToNext(kind) => (self.next_auto_attack_at(*kind) - self.now).max(0),
            // Go `APLValueAutoSwingTime`: the hand's current swing duration.
            Compiled::AutoSwingTime(kind) => match kind {
                // Go reads a hand of the unit's auto attacks.
                crate::rotation::SwingType::MainHand => self.autos.mh.cur_swing_duration.max(0),
                crate::rotation::SwingType::OffHand => self.autos.oh.cur_swing_duration.max(0),
                crate::rotation::SwingType::Ranged => self.autos.ranged.cur_swing_duration.max(0),
                crate::rotation::SwingType::Unknown => 0,
            },
            Compiled::Coerced { inner, .. } => match inner.value_type() {
                ValueType::Bool => {
                    if self.get_bool(inner) {
                        1
                    } else {
                        0
                    }
                }
                ValueType::Int => {
                    crate::rotation::duration_from_seconds(f64::from(self.get_int(inner)))
                }
                ValueType::Float => crate::rotation::duration_from_seconds(self.get_float(inner)),
                ValueType::Duration => self.get_duration(inner),
                ValueType::String => 0,
            },
            _ => 0,
        }
    }

    /// Go `APLValueMath.GetDuration`: int64 arithmetic, which wraps, and float products
    /// truncated toward zero.
    fn math_duration(&mut self, op: MathOp, lhs: &Compiled, rhs: &Compiled) -> i64 {
        fn scale<A: Agent>(fight: &mut Fight<A>, duration: i64, by: &Compiled) -> i64 {
            match by.value_type() {
                ValueType::Int => duration.wrapping_mul(i64::from(fight.get_int(by))),
                ValueType::Float => (duration as f64 * fight.get_float(by)) as i64,
                other => panic!("invalid {other:?} operand for duration multiplication"),
            }
        }
        match op {
            MathOp::Add => self.get_duration(lhs).wrapping_add(self.get_duration(rhs)),
            MathOp::Sub => self.get_duration(lhs).wrapping_sub(self.get_duration(rhs)),
            MathOp::Mul if lhs.value_type() == ValueType::Duration => {
                let duration = self.get_duration(lhs);
                scale(self, duration, rhs)
            }
            MathOp::Mul => {
                let duration = self.get_duration(rhs);
                scale(self, duration, lhs)
            }
            MathOp::Div => match rhs.value_type() {
                ValueType::Int => {
                    let divisor = self.get_int(rhs);
                    assert!(divisor != 0, "Division by zero in duration / int");
                    self.get_duration(lhs).wrapping_div(i64::from(divisor))
                }
                ValueType::Float => {
                    let divisor = self.get_float(rhs);
                    assert!(divisor != 0.0, "Division by zero in duration / float");
                    (self.get_duration(lhs) as f64 / divisor) as i64
                }
                other => panic!("invalid {other:?} divisor for duration division"),
            },
        }
    }

    /// Go `APLValueAuraRemainingTime`: zero when inactive.
    fn aura_remaining_time(&self, aura: AuraRef) -> i64 {
        let state = self.aura(aura);
        match (state.active, state.expires) {
            (false, _) => 0,
            (true, NEVER_EXPIRES) => NEVER_EXPIRES,
            (true, expires) => expires - self.now,
        }
    }

    /// The dot a compiled rotation reads: the spell's, on the target it names. An area or
    /// self-only dot is the same on every target.
    fn dot_of(&self, at: DotAt) -> DotId {
        let dot = self.spells[at.spell].dot.expect("compiled dots have a dot");
        self.dot_on(dot, Side::target(at.target))
    }

    /// Whether the dot of a compiled spell is active on its unit.
    pub(crate) fn dot_active(&self, at: DotAt) -> bool {
        self.aura(self.dots[self.dot_of(at)].aura).active
    }

    /// Go `APLActionCastSpell.IsReady`: castable or queueable, and major cooldowns wait for
    /// the GCD unless reactive or inside a sequence.
    fn cast_ready(&mut self, spell: SpellId) -> bool {
        self.can_cast_or_queue(spell) && {
            let flags = self.spells[spell].flags;
            !flags.mcd || flags.reactive || self.gcd_ready() || self.apl.in_sequence
        }
    }

    /// Go `APLActionStrictSequence.IsReady`. A ready sequence leaves the sequence flag set,
    /// as Go does.
    fn sequence_ready(&mut self, item: usize) -> bool {
        let Act::StrictSequence { ref spells, .. } = self.rotation[item].action else {
            unreachable!("item is a strict sequence");
        };
        let first = spells[0];
        self.apl.in_sequence = true;
        if self.gcd_time_to_ready() > MAX_SPELL_QUEUE_WINDOW || !self.cast_ready(first) {
            self.apl.in_sequence = false;
            return false;
        }
        true
    }

    /// Go `APLAction.IsReady`: the condition, then the action's readiness.
    fn item_ready(&mut self, item: usize) -> Option<Ready> {
        if let Some(condition) = self.rotation[item].condition.clone() {
            if !self.condition(&condition) {
                return None;
            }
        }
        match self.rotation[item].action {
            Act::Cast(spell, target) => {
                self.cast_ready(spell).then_some(Ready::Cast(spell, target))
            }
            Act::Autocast => self.autocast_ready().map(Ready::Autocast),
            Act::StrictSequence { .. } => {
                self.sequence_ready(item).then_some(Ready::Sequence(item))
            }
            // Go APLActionSequence.IsReady: the next step's readiness inside the sequence.
            Act::Sequence { ref spells, next } => {
                let step = spells.get(next).copied();
                self.apl.in_sequence = true;
                let ready = step.is_some_and(|spell| self.cast_ready(spell));
                self.apl.in_sequence = false;
                ready.then_some(Ready::SequenceStep(item))
            }
            Act::Channel { spell, .. } => self
                .can_cast_or_queue(spell)
                .then_some(Ready::Channel(item, spell)),
            // Go `APLActionMove.IsReady`: not already moving, a different range or the prepull,
            // and no cast or channel in progress.
            Act::Move(ref range) => {
                let range = Rc::clone(range);
                self.move_ready(&range).then_some(Ready::Move(item))
            }
            Act::MoveDuration(ref duration) => {
                let duration = Rc::clone(duration);
                self.move_duration_ready(&duration)
                    .then_some(Ready::MoveDuration(item))
            }
            // Go APLActionGroupReference.IsReady: any action of the group is ready.
            Act::Group { instance } => self.group_ready(instance).map(|_| Ready::Group(item)),
            // Go APLActionMultidot.IsReady: the overlap, then the target whose dot is down or
            // ends within it and which the spell can be cast or queued on.
            Act::Multidot { spell, .. } => self
                .multidot_ready(item)
                .map(|target| Ready::Cast(spell, target)),
        }
    }

    /// The first action of a group instance that is ready, Go `APLActionGroupReference.IsReady`
    /// and `Execute` both look for.
    fn group_ready(&mut self, instance: Option<usize>) -> Option<Ready> {
        let range = instance.and_then(|instance| self.group_ranges.get(instance).cloned())?;
        range.into_iter().find_map(|item| self.item_ready(item))
    }

    /// Go `APLActionMove.IsReady`: not already moving, a different range or the prepull, and no
    /// cast in progress.
    fn move_ready(&mut self, range: &Compiled) -> bool {
        !self.player.moving
            && (self.get_float(range) != self.config.distance || self.now < 0)
            && self.player.hardcast.expires < self.now
    }

    /// Go `APLActionMoveDuration.IsReady`: not moving, or the move ends this step, for a
    /// duration that is not zero, with no channel and no cast in progress, unless the cast
    /// allows moving.
    fn move_duration_ready(&mut self, duration: &Compiled) -> bool {
        if self.player.moving {
            let ends = self.player.movement.map(|movement| movement.end());
            if ends != Some(self.now) {
                return false;
            }
        }
        if self.get_duration(duration) == 0 {
            return false;
        }
        let hardcast = self.player.hardcast;
        let can_move = hardcast
            .spell
            .is_some_and(|spell| self.spells[spell].flags.can_cast_while_moving);
        (hardcast.expires < self.now || can_move) && self.player.channeled_dot.is_none()
    }

    /// Go `APLActionMultidot.IsReady`: the overlap, then the first target in unit index order
    /// whose dot is down or ends within it and which the spell can be cast or queued on.
    fn multidot_ready(&mut self, item: usize) -> Option<Side> {
        let Act::Multidot {
            spell,
            dot_spell,
            max_dots,
            ref overlap,
        } = self.rotation[item].action
        else {
            unreachable!("item is a multidot");
        };
        let overlap = overlap.clone();
        let overlap = overlap.map_or(0, |value| self.get_duration(&value));
        let dot = self.spells[dot_spell]
            .dot
            .expect("multidot spells have a dot");
        for position in 0..max_dots.max(0) as usize {
            let target = Side::target(position);
            let aura = self.aura(self.dots[self.dot_on(dot, target)].aura);
            let active = aura.active;
            let remaining = if active { aura.expires - self.now } else { 0 };
            if (!active || remaining < overlap) && self.can_cast_or_queue(spell) {
                return Some(target);
            }
        }
        None
    }

    /// Go `APLRotation.getNextAction`.
    fn next_action(&mut self) -> Option<Ready> {
        if let Some(item) = self.apl.controlling {
            return self.sequence_next_action(item);
        }
        (0..self.priority_len).find_map(|item| self.item_ready(item))
    }

    /// Go `APLActionStrictSequence.GetNextAction`.
    fn sequence_next_action(&mut self, item: usize) -> Option<Ready> {
        let Act::StrictSequence { ref spells, next } = self.rotation[item].action else {
            unreachable!("only strict sequences control the rotation");
        };
        let spell = spells[next];
        if self.cast_ready(spell) {
            // Off the GCD the step advances now; otherwise the cast queues and the step
            // stays until it fires.
            if self.gcd_ready() {
                self.advance_sequence(item);
            }
            Some(Ready::Cast(spell, Side::Target))
        } else if !self.can_queue_spell() {
            // A spell was queued this timestep: advance when its action fires.
            if !self.apl.queue_hooks.contains(&item) {
                self.apl.queue_hooks.push(item);
            }
            let fire_at = self.player.queued.expect("a queued spell").fire_at;
            self.set_rotation_timer(fire_at + 1);
            None
        } else if self.gcd_time_to_ready() <= MAX_SPELL_QUEUE_WINDOW {
            // The GCD is ready and the step is not: the sequence is bad, so leave it.
            self.relinquish_sequence(item);
            self.next_action()
        } else {
            None
        }
    }

    /// Go `APLActionStrictSequence.advanceSequence`, or a sequence's step advancing when its
    /// queued cast fires.
    pub(crate) fn advance_sequence(&mut self, item: usize) {
        if let Act::Sequence { ref mut next, .. } = self.rotation[item].action {
            *next += 1;
            return;
        }
        let Act::StrictSequence {
            ref spells,
            ref mut next,
        } = self.rotation[item].action
        else {
            unreachable!("item is a strict sequence");
        };
        *next += 1;
        if *next == spells.len() {
            self.relinquish_sequence(item);
        }
    }

    /// Go `APLActionStrictSequence.relinquishControl`.
    fn relinquish_sequence(&mut self, item: usize) {
        if let Act::StrictSequence { ref mut next, .. } = self.rotation[item].action {
            *next = 0;
        }
        self.apl.queue_hooks.retain(|&hooked| hooked != item);
        self.apl.in_sequence = false;
        assert_eq!(
            self.apl.controlling,
            Some(item),
            "wrong APL controlling action"
        );
        self.apl.controlling = None;
    }

    fn execute(&mut self, action: Ready) {
        match action {
            Ready::Cast(spell, target) => self.cast_or_queue(spell, target),
            Ready::Autocast(cooldown) => self.autocast(cooldown),
            Ready::Sequence(item) => {
                self.apl.in_sequence = true;
                assert!(self.apl.controlling.is_none(), "nested controlling actions");
                self.apl.controlling = Some(item);
            }
            // Go APLActionSequence.Execute: a cast advances the step at once; a queued one
            // advances it when its action fires, and the rotation waits until just after.
            Ready::SequenceStep(item) => {
                self.apl.in_sequence = true;
                let Act::Sequence { ref spells, next } = self.rotation[item].action else {
                    unreachable!("item is a sequence");
                };
                let spell = spells[next];
                self.cast_or_queue(spell, Side::Target);
                if self.can_queue_spell() {
                    self.advance_sequence(item);
                } else {
                    self.apl.queue_hooks.push(item);
                    let fire_at = self.player.queued.expect("a queued spell").fire_at;
                    self.set_rotation_timer(fire_at + 1);
                }
                self.apl.in_sequence = false;
            }
            // Go `APLActionMove.Execute`: the value is read again.
            Ready::Move(item) => {
                let Act::Move(ref range) = self.rotation[item].action else {
                    unreachable!("item is a move");
                };
                let range = Rc::clone(range);
                let range = self.get_float(&range);
                if self.log.is_some() {
                    let line = format!("[DEBUG] Moving to {range:.1} yards");
                    self.player_log(&line);
                }
                self.move_to(Side::Player, range);
            }
            // Go `APLActionGroupReference.Execute`: the first action that is ready, found again.
            Ready::Group(item) => {
                let Act::Group { instance } = self.rotation[item].action else {
                    unreachable!("item is a group reference");
                };
                if let Some(ready) = self.group_ready(instance) {
                    self.execute(ready);
                }
            }
            // Go `APLActionMoveDuration.Execute`.
            Ready::MoveDuration(item) => {
                let Act::MoveDuration(ref duration) = self.rotation[item].action else {
                    unreachable!("item is a move for a duration");
                };
                let duration = Rc::clone(duration);
                let duration = self.get_duration(&duration);
                self.move_duration(Side::Player, duration);
            }
            Ready::Channel(item, spell) => {
                self.cast_or_queue(spell, Side::Target);
                let Act::Channel { allow_recast, .. } = self.rotation[item].action else {
                    unreachable!("item is a channel");
                };
                self.apl.interrupt_channel_if = Some(item);
                self.apl.allow_channel_recast = allow_recast;
            }
        }
    }

    /// Go `Dot.ChannelCanBeInterrupted`.
    fn channel_can_be_interrupted(&mut self, dot: DotId) -> bool {
        let state = &self.dots[dot];
        if !state.channeled || state.remaining_ticks == 0 {
            return false;
        }
        let interrupt = match self.apl.interrupt_channel_if {
            Some(item) => match &self.rotation[item].action {
                Act::Channel { interrupt, .. } => Rc::clone(interrupt),
                _ => return false,
            },
            None => return false,
        };
        self.get_bool(&interrupt)
    }

    /// Whether two spells share a class spell mask, as Go `Spell.Matches` with the other's
    /// mask.
    fn same_class_spell(&self, a: SpellId, b: SpellId) -> bool {
        match (&self.spells[a].class_spell, &self.spells[b].class_spell) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Go `nextActionWouldRecastChannel`, evaluated with no channel running. The caller
    /// restores the channel.
    fn next_action_would_recast_channel(&mut self, dot: DotId) -> bool {
        let channeled = self.dots[dot].spell;
        self.player.channeled_dot = None;
        for item in 0..self.priority_len {
            if let Some(condition) = self.rotation[item].condition.clone() {
                if !self.condition(&condition) {
                    continue;
                }
            }
            let spell = match self.rotation[item].action {
                Act::Cast(spell, _) | Act::Channel { spell, .. } => spell,
                Act::Autocast | Act::Sequence { .. } => continue,
                // Go: a different action that is fully ready would be cast first.
                Act::Move(ref range) => {
                    let range = Rc::clone(range);
                    if self.move_ready(&range) {
                        return false;
                    }
                    continue;
                }
                Act::MoveDuration(ref duration) => {
                    let duration = Rc::clone(duration);
                    if self.move_duration_ready(&duration) {
                        return false;
                    }
                    continue;
                }
                // Go: a different action that is fully ready would be cast first.
                Act::Group { instance } => {
                    if self.group_ready(instance).is_some() {
                        return false;
                    }
                    continue;
                }
                // Go: a different action that is fully ready would be cast first.
                Act::Multidot { .. } => {
                    if self.multidot_ready(item).is_some() {
                        return false;
                    }
                    continue;
                }
                Act::StrictSequence { .. } => {
                    // A different action that is fully ready would be cast first.
                    if self.sequence_ready(item) {
                        return false;
                    }
                    continue;
                }
            };
            // Go uses CanCast, so a spell queued this timestep does not block the recast.
            let can_cast = self.can_cast(spell);
            if spell == channeled || self.same_class_spell(spell, channeled) {
                return can_cast;
            }
            if can_cast {
                return false;
            }
        }
        false
    }

    /// Go `APLRotation.shouldInterruptChannel`.
    pub(crate) fn should_interrupt_channel(&mut self) -> bool {
        let Some(dot) = self.player.channeled_dot else {
            return false;
        };
        if !self.channel_can_be_interrupted(dot) {
            return false;
        }
        let would_recast = self.next_action_would_recast_channel(dot);
        self.player.channeled_dot = Some(dot);
        if would_recast {
            self.apl.allow_channel_recast
        } else {
            true
        }
    }

    /// End a channel the rotation interrupts: no tick runs on expiry, then the rotation
    /// waits out the clip delay when the GCD is ready.
    pub(crate) fn interrupt_channel(&mut self, dot: DotId) {
        let delay = self.config.channel_clip_delay;
        self.dots[dot].tick_next_at = NEVER_EXPIRES;
        let aura = self.dots[dot].aura;
        self.deactivate_aura(aura);
        if self.gcd_ready() {
            self.wait_until(self.now + delay);
        }
    }

    /// Go channel OnExpire: the rotation forgets the channel's interrupt condition.
    pub(crate) fn forget_channel_interrupt(&mut self) {
        self.apl.interrupt_channel_if = None;
        self.apl.allow_channel_recast = false;
    }

    /// Go `APLRotation.DoNextAction`.
    pub(crate) fn do_next_action(&mut self) {
        if self.now < 0 || self.in_rotation {
            return;
        }
        if let Some(dot) = self.player.channeled_dot {
            // All ticks spent but the aura not yet expired: end it now.
            if self.dots[dot].remaining_ticks == 0 {
                let aura = self.dots[dot].aura;
                self.deactivate_aura(aura);
                return;
            }
            // Go also evaluates the interrupt condition when the GCD fires.
            if self.should_interrupt_channel() {
                self.interrupt_channel(dot);
            }
            return;
        }
        if self.player.rotation_timer > self.now {
            return;
        }
        self.in_rotation = true;
        // Go brings the unit's position up to date before the rotation reads it.
        self.update_position(Side::Player, false);
        let mut executed = 0;
        while let Some(action) = self.next_action() {
            assert!(executed <= 1000, "infinite rotation loop");
            self.execute(action);
            executed += 1;
        }
        self.in_rotation = false;
        if executed == 0 && self.log.is_some() {
            self.player_log("No available actions!");
        }
        if self.player.rotation_timer <= self.now {
            // A moving unit does not wait for its GCD.
            let mut next = self.now + self.config.reaction;
            if !self.player.moving {
                next = next.max(self.player.gcd);
            }
            self.wait_until(next);
        }
    }

    /// Go `APLRotation.reset` and each action's `Reset`.
    pub(crate) fn rotation_reset(&mut self, side: Side) {
        if side == Side::Player {
            self.in_rotation = false;
            self.apl.controlling = None;
            self.apl.interrupt_channel_if = None;
            self.apl.allow_channel_recast = false;
            for item in 0..self.rotation.len() {
                match self.rotation[item].action {
                    Act::StrictSequence { ref mut next, .. } => {
                        *next = 0;
                        self.apl.queue_hooks.retain(|&hooked| hooked != item);
                        self.apl.in_sequence = false;
                    }
                    Act::Sequence { ref mut next, .. } => {
                        *next = 0;
                        self.apl.queue_hooks.retain(|&hooked| hooked != item);
                    }
                    _ => {}
                }
            }
        }
    }
}

fn compare<T: PartialOrd>(op: CompareOp, lhs: T, rhs: T) -> bool {
    match op {
        CompareOp::Eq => lhs == rhs,
        CompareOp::Ne => lhs != rhs,
        CompareOp::Lt => lhs < rhs,
        CompareOp::Le => lhs <= rhs,
        CompareOp::Gt => lhs > rhs,
        CompareOp::Ge => lhs >= rhs,
    }
}

fn compare_float(op: CompareOp, lhs: f64, rhs: f64) -> bool {
    compare(op, lhs, rhs)
}
