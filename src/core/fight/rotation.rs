//! Go apl.go: compile the parsed rotation and run `DoNextAction`.

use crate::{
    contracts::prepared_v2::ActionId,
    core::time::NEVER_EXPIRES,
    rotation::{
        compile_bool_value, compile_condition, Action as ParsedAction, CompareOp,
        CompiledCondition, FoundAura, Lookup, MathOp, MissingAura, Rotation, ValueType,
    },
};

use super::{cast::MAX_SPELL_QUEUE_WINDOW, Agent, AuraRef, DotId, Fight, Side, SpellId};

pub(crate) type Compiled = crate::rotation::Compiled<AuraRef>;

#[derive(Clone, Debug)]
pub(crate) enum Act {
    Cast(SpellId),
    Autocast,
    /// Go `APLActionStrictSequence`: its casts and the next one to run.
    StrictSequence {
        spells: Vec<SpellId>,
        next: usize,
    },
    /// Go `APLActionChannelSpell` with an interrupt condition.
    Channel {
        spell: SpellId,
        interrupt: Compiled,
        allow_recast: bool,
    },
}

/// A ready action. Go keeps the cooldown found by `IsReady` for `Execute`.
enum Ready {
    Cast(SpellId),
    Autocast(usize),
    /// A strict sequence taking control of the rotation.
    Sequence(usize),
    /// A channel and the item whose interrupt condition it carries.
    Channel(usize, SpellId),
}

#[derive(Clone, Debug)]
pub(crate) struct Item {
    condition: Option<Compiled>,
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
    pub(crate) fn compile_prepull(&self, rotation: &Rotation) -> Vec<(i64, SpellId)> {
        let mut prepull: Vec<(i64, SpellId)> = rotation
            .prepull
            .iter()
            .filter_map(|prepull| match &prepull.action {
                ParsedAction::CastSpell(id) => self
                    .apl_cast_spell(id)
                    .map(|spell| (prepull.do_at_ns, spell)),
                _ => None,
            })
            .collect();
        prepull.sort_by_key(|(do_at, _)| *do_at);
        prepull
    }

    /// Go `newAPLRotation` for the supported subset. Conditions compile as the pinned
    /// reference does; coverage rejects rotations where community #622 would differ.
    pub(crate) fn compile_rotation(&self, rotation: &Rotation) -> Vec<Item> {
        let find = |side: Side, id: &ActionId| {
            let tracker = &self.trackers[side.index()];
            tracker.find_by_id(id).map(|index| FoundAura {
                aura: AuraRef { side, index },
                max_stacks: tracker.auras[index].max_stacks,
            })
        };
        let aura = |id: &ActionId| find(Side::Player, id);
        let target_aura = |id: &ActionId| find(Side::Target, id);
        let spell = |id: &ActionId| self.apl_spell(id);
        // Go `GetAPLDot` through `Spell.Dot`: the spell's own dot or its related dot spell's.
        let dot = |id: &ActionId| {
            self.apl_spell(id)
                .and_then(|spell| match self.spells[spell].dot {
                    Some(_) => Some(spell),
                    None => self.spells[spell].related_dot_spell.filter(|&related| {
                        self.spells.get(related).is_some_and(|s| s.dot.is_some())
                    }),
                })
        };
        let lookup = Lookup {
            aura: &aura,
            target_aura: &target_aura,
            spell: &spell,
            dot: &dot,
        };
        let mut items = Vec::new();
        for item in &rotation.priority_list {
            let action = match &item.action {
                // Go GetAPLCastSpell: an unknown spell drops the action.
                ParsedAction::CastSpell(id) => match self.apl_cast_spell(id) {
                    Some(spell) => Act::Cast(spell),
                    None => continue,
                },
                ParsedAction::AutocastOtherCooldowns => Act::Autocast,
                // Go newActionStrictSequence drops unknown casts, and the action when none
                // remain.
                ParsedAction::StrictSequence(ids) => {
                    let spells: Vec<SpellId> = ids
                        .iter()
                        .filter_map(|id| self.apl_cast_spell(id))
                        .collect();
                    if spells.is_empty() {
                        continue;
                    }
                    Act::StrictSequence { spells, next: 0 }
                }
                // Go newActionChannelSpell: without an interrupt condition it is a cast;
                // otherwise the spell must be a channel.
                ParsedAction::ChannelSpell {
                    spell,
                    interrupt_if,
                    allow_recast,
                } => match compile_bool_value(interrupt_if.as_ref(), &lookup, MissingAura::Dropped)
                {
                    None => match self.apl_cast_spell(spell) {
                        Some(spell) => Act::Cast(spell),
                        None => continue,
                    },
                    Some(interrupt) => match self.apl_spell(spell) {
                        Some(spell) if self.spells[spell].flags.channeled => Act::Channel {
                            spell,
                            interrupt,
                            allow_recast: *allow_recast,
                        },
                        _ => continue,
                    },
                },
            };
            let condition =
                match compile_condition(item.condition.as_ref(), &lookup, MissingAura::Dropped) {
                    // A constant false condition prunes the action; its spells already left
                    // the major cooldowns in Go's export.
                    CompiledCondition::Pruned => continue,
                    CompiledCondition::Always => None,
                    CompiledCondition::When(condition) => Some(condition),
                };
            items.push(Item { condition, action });
        }
        items
    }

    fn get_bool(&self, value: &Compiled) -> bool {
        match value {
            Compiled::Const(constant) => constant.boolean,
            Compiled::AuraIsActive(aura) => self.aura(*aura).active,
            Compiled::DotIsActive(spell) => self.dot_active(*spell),
            // Go `APLValueSpellIsReady`: ready, or ready within the spell queue window.
            Compiled::SpellIsReady(spell) => {
                self.spell_ready(*spell)
                    || self.spell_time_to_ready(*spell) <= MAX_SPELL_QUEUE_WINDOW
            }
            // Go `APLValueGCDIsReady`: ready, or ready within the spell queue window.
            Compiled::GcdIsReady => {
                self.gcd_ready() || self.gcd_time_to_ready() <= MAX_SPELL_QUEUE_WINDOW
            }
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

    fn get_int(&self, value: &Compiled) -> i32 {
        match value {
            Compiled::Const(constant) => constant.int,
            Compiled::AuraNumStacks(aura) => self.aura(*aura).stacks,
            // One target in scope.
            Compiled::NumberTargets => 1,
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

    fn get_float(&self, value: &Compiled) -> f64 {
        match value {
            Compiled::Const(constant) => constant.float,
            Compiled::CurrentManaPercent => self.player.mana / self.config.max_mana,
            Compiled::CurrentMana => self.player.mana,
            Compiled::NumberTargets => 1.0,
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

    fn get_duration(&self, value: &Compiled) -> i64 {
        match value {
            Compiled::Const(constant) => constant.duration_ns,
            // Go `APLValueAuraRemainingTime`: zero when inactive.
            Compiled::AuraRemainingTime(aura) => {
                let state = self.aura(*aura);
                match (state.active, state.expires) {
                    (false, _) => 0,
                    (true, crate::core::time::NEVER_EXPIRES) => crate::core::time::NEVER_EXPIRES,
                    (true, expires) => expires - self.now,
                }
            }
            Compiled::RemainingTime => self.duration - self.now,
            // Go `Spell.TimeToReady`.
            Compiled::SpellTimeToReady(spell) => self.spell_time_to_ready(*spell),
            // Go `Dot.TimeUntilNextTick`: the next tick time is zero while inactive.
            Compiled::DotTimeToNextTick(spell) => {
                let next = if self.dot_active(*spell) {
                    let dot = self.spells[*spell].dot.expect("compiled dots have a dot");
                    self.dots[dot].tick_next_at
                } else {
                    0
                };
                next - self.now
            }
            Compiled::Math { op, lhs, rhs } => self.math_duration(*op, lhs, rhs),
            Compiled::CurrentTime => self.now,
            // Go `APLValueDotRemainingTime`: zero when inactive.
            Compiled::DotRemainingTime(spell) => {
                if self.dot_active(*spell) {
                    let dot = self.spells[*spell].dot.expect("compiled dots have a dot");
                    let aura = self.aura(self.dots[dot].aura);
                    aura.expires - self.now
                } else {
                    0
                }
            }
            // Go `Spell.CastTime`: the default cast time with current cast speed, unrounded.
            Compiled::SpellCastTime(spell) => {
                let cast_time = self.spells[*spell].default_cast.cast_time;
                self.apply_cast_speed_for_spell(cast_time, *spell)
            }
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
    fn math_duration(&self, op: MathOp, lhs: &Compiled, rhs: &Compiled) -> i64 {
        let scale = |duration: i64, by: &Compiled| match by.value_type() {
            ValueType::Int => duration.wrapping_mul(i64::from(self.get_int(by))),
            ValueType::Float => (duration as f64 * self.get_float(by)) as i64,
            other => panic!("invalid {other:?} operand for duration multiplication"),
        };
        match op {
            MathOp::Add => self.get_duration(lhs).wrapping_add(self.get_duration(rhs)),
            MathOp::Sub => self.get_duration(lhs).wrapping_sub(self.get_duration(rhs)),
            MathOp::Mul if lhs.value_type() == ValueType::Duration => {
                scale(self.get_duration(lhs), rhs)
            }
            MathOp::Mul => scale(self.get_duration(rhs), lhs),
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

    /// Whether the dot of a compiled spell is active on its unit.
    fn dot_active(&self, spell: SpellId) -> bool {
        let dot = self.spells[spell].dot.expect("compiled dots have a dot");
        self.aura(self.dots[dot].aura).active
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
        if let Some(condition) = &self.rotation[item].condition {
            if !self.get_bool(condition) {
                return None;
            }
        }
        match self.rotation[item].action {
            Act::Cast(spell) => self.cast_ready(spell).then_some(Ready::Cast(spell)),
            Act::Autocast => self.autocast_ready().map(Ready::Autocast),
            Act::StrictSequence { .. } => {
                self.sequence_ready(item).then_some(Ready::Sequence(item))
            }
            Act::Channel { spell, .. } => self
                .can_cast_or_queue(spell)
                .then_some(Ready::Channel(item, spell)),
        }
    }

    /// Go `APLRotation.getNextAction`.
    fn next_action(&mut self) -> Option<Ready> {
        if let Some(item) = self.apl.controlling {
            return self.sequence_next_action(item);
        }
        (0..self.rotation.len()).find_map(|item| self.item_ready(item))
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
            Some(Ready::Cast(spell))
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

    /// Go `APLActionStrictSequence.advanceSequence`.
    pub(crate) fn advance_sequence(&mut self, item: usize) {
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
            Ready::Cast(spell) => self.cast_or_queue(spell, Side::Target),
            Ready::Autocast(cooldown) => self.autocast(cooldown),
            Ready::Sequence(item) => {
                self.apl.in_sequence = true;
                assert!(self.apl.controlling.is_none(), "nested controlling actions");
                self.apl.controlling = Some(item);
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
    fn channel_can_be_interrupted(&self, dot: DotId) -> bool {
        let state = &self.dots[dot];
        if !state.channeled || state.remaining_ticks == 0 {
            return false;
        }
        match self.apl.interrupt_channel_if {
            Some(item) => match &self.rotation[item].action {
                Act::Channel { interrupt, .. } => self.get_bool(interrupt),
                _ => false,
            },
            None => false,
        }
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
        for item in 0..self.rotation.len() {
            if let Some(condition) = &self.rotation[item].condition {
                if !self.get_bool(condition) {
                    continue;
                }
            }
            let spell = match self.rotation[item].action {
                Act::Cast(spell) | Act::Channel { spell, .. } => spell,
                Act::Autocast => continue,
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
            let next = (self.now + self.config.reaction).max(self.player.gcd);
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
                if let Act::StrictSequence { ref mut next, .. } = self.rotation[item].action {
                    *next = 0;
                    self.apl.queue_hooks.retain(|&hooked| hooked != item);
                    self.apl.in_sequence = false;
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
