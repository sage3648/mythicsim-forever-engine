//! Rotation conditions lowered for evaluation. Compilation resolves the operand types of
//! the common nodes and fuses a comparison with a constant right operand into one node, so
//! evaluating the usual condition walks few nodes and needs no type dispatch. Every other
//! node keeps its compiled form and goes through the general getters. Nodes are evaluated
//! in the compiled order, so reads with side effects happen exactly as before.

use super::{compare, Compiled};
use crate::core::fight::reads;
use crate::{
    core::{
        fight::{Agent, AuraRef, Fight, SpellId},
        time::NEVER_EXPIRES,
    },
    rotation::{CompareOp, DotAt, ValueType},
};

/// A boolean condition.
#[derive(Debug)]
pub(crate) enum Cond {
    Const(bool),
    AuraIsActive(AuraRef),
    DotIsActive(DotAt),
    IsExecutePhase(i32),
    SpellCanCast(SpellId),
    And(Box<[Cond]>),
    Or(Box<[Cond]>),
    Not(Box<Cond>),
    /// Go `APLValueCompare` with a constant right operand, the usual shape.
    DurationConst(CompareOp, Duration, i64),
    IntConst(CompareOp, Int, i32),
    FloatConst(CompareOp, Float, f64),
    /// Go `APLValueCompare` on two values.
    Duration(CompareOp, Box<(Duration, Duration)>),
    Int(CompareOp, Box<(Int, Int)>),
    Float(CompareOp, Box<(Float, Float)>),
    Other(Box<Compiled>),
}

#[derive(Debug)]
pub(crate) enum Duration {
    Const(i64),
    RemainingTime,
    CurrentTime,
    AuraRemainingTime(AuraRef),
    SpellTimeToReady(SpellId),
    Other(Box<Compiled>),
}

#[derive(Debug)]
pub(crate) enum Int {
    Const(i32),
    AuraNumStacks(AuraRef),
    CurrentComboPoints,
    /// One target in scope.
    NumberTargets,
    Other(Box<Compiled>),
}

#[derive(Debug)]
pub(crate) enum Float {
    Const(f64),
    CurrentManaPercent,
    CurrentMana,
    CurrentRage,
    CurrentEnergy,
    Other(Box<Compiled>),
}

impl Cond {
    /// Whether the condition compares the current energy anywhere.
    pub(crate) fn reads_energy(&self) -> bool {
        match self {
            Cond::And(values) | Cond::Or(values) => values.iter().any(Cond::reads_energy),
            Cond::Not(value) => value.reads_energy(),
            Cond::FloatConst(_, lhs, _) => matches!(lhs, Float::CurrentEnergy),
            Cond::Float(_, operands) => {
                matches!(operands.0, Float::CurrentEnergy)
                    || matches!(operands.1, Float::CurrentEnergy)
            }
            _ => false,
        }
    }

    /// Whether evaluating the condition can run a cost check (Go `APLValueSpellCanCast`).
    pub(crate) fn asks_cost(&self) -> bool {
        match self {
            Cond::SpellCanCast(_) => true,
            Cond::And(values) | Cond::Or(values) => values.iter().any(Cond::asks_cost),
            Cond::Not(value) => value.asks_cost(),
            Cond::DurationConst(_, lhs, _) => lhs.asks_cost(),
            Cond::IntConst(_, lhs, _) => lhs.asks_cost(),
            Cond::FloatConst(_, lhs, _) => lhs.asks_cost(),
            Cond::Duration(_, operands) => operands.0.asks_cost() || operands.1.asks_cost(),
            Cond::Int(_, operands) => operands.0.asks_cost() || operands.1.asks_cost(),
            Cond::Float(_, operands) => operands.0.asks_cost() || operands.1.asks_cost(),
            Cond::Other(value) => compiled_asks_cost(value),
            Cond::Const(_)
            | Cond::AuraIsActive(_)
            | Cond::DotIsActive(_)
            | Cond::IsExecutePhase(_) => false,
        }
    }

    /// Lower a compiled condition. Each node reads exactly what its compiled form reads.
    pub(crate) fn lower(value: &Compiled) -> Cond {
        match value {
            // The getters read a constant's boolean whatever its type.
            Compiled::Const(constant) => Cond::Const(constant.boolean),
            Compiled::AuraIsActive(aura) => Cond::AuraIsActive(*aura),
            Compiled::DotIsActive(dot) => Cond::DotIsActive(*dot),
            Compiled::IsExecutePhase(threshold) => Cond::IsExecutePhase(*threshold),
            Compiled::SpellCanCast(spell) => Cond::SpellCanCast(*spell),
            Compiled::And(values) => Cond::And(values.iter().map(Cond::lower).collect()),
            Compiled::Or(values) => Cond::Or(values.iter().map(Cond::lower).collect()),
            Compiled::Not(value) => Cond::Not(Box::new(Cond::lower(value))),
            Compiled::Compare { op, lhs, rhs } => {
                let op = *op;
                // The getters read a constant's field for the comparison's type.
                let constant = match rhs.as_ref() {
                    Compiled::Const(constant) => Some(constant),
                    _ => None,
                };
                match (lhs.value_type(), constant) {
                    (ValueType::Duration, Some(rhs)) => {
                        Cond::DurationConst(op, Duration::lower(lhs), rhs.duration_ns)
                    }
                    (ValueType::Duration, None) => {
                        Cond::Duration(op, Box::new((Duration::lower(lhs), Duration::lower(rhs))))
                    }
                    (ValueType::Int, Some(rhs)) => Cond::IntConst(op, Int::lower(lhs), rhs.int),
                    (ValueType::Int, None) => {
                        Cond::Int(op, Box::new((Int::lower(lhs), Int::lower(rhs))))
                    }
                    (ValueType::Float, Some(rhs)) => {
                        Cond::FloatConst(op, Float::lower(lhs), rhs.float)
                    }
                    (ValueType::Float, None) => {
                        Cond::Float(op, Box::new((Float::lower(lhs), Float::lower(rhs))))
                    }
                    _ => Cond::Other(Box::new(value.clone())),
                }
            }
            other => Cond::Other(Box::new(other.clone())),
        }
    }
}

/// Whether a compiled value can run a cost check (Go `APLValueSpellCanCast`).
fn compiled_asks_cost(value: &Compiled) -> bool {
    match value {
        Compiled::SpellCanCast(_) => true,
        Compiled::And(values) | Compiled::Or(values) => values.iter().any(compiled_asks_cost),
        Compiled::Not(value) | Compiled::Coerced { inner: value, .. } => compiled_asks_cost(value),
        Compiled::Compare { lhs, rhs, .. } | Compiled::Math { lhs, rhs, .. } => {
            compiled_asks_cost(lhs) || compiled_asks_cost(rhs)
        }
        Compiled::AuraShouldRefresh { overlap, .. } => compiled_asks_cost(overlap),
        _ => false,
    }
}

impl Duration {
    fn asks_cost(&self) -> bool {
        matches!(self, Duration::Other(value) if compiled_asks_cost(value))
    }

    fn lower(value: &Compiled) -> Duration {
        match value {
            Compiled::Const(constant) => Duration::Const(constant.duration_ns),
            Compiled::RemainingTime => Duration::RemainingTime,
            Compiled::CurrentTime => Duration::CurrentTime,
            Compiled::AuraRemainingTime(aura) => Duration::AuraRemainingTime(*aura),
            Compiled::SpellTimeToReady(spell) => Duration::SpellTimeToReady(*spell),
            other => Duration::Other(Box::new(other.clone())),
        }
    }
}

impl Int {
    fn asks_cost(&self) -> bool {
        matches!(self, Int::Other(value) if compiled_asks_cost(value))
    }

    fn lower(value: &Compiled) -> Int {
        match value {
            Compiled::Const(constant) => Int::Const(constant.int),
            Compiled::AuraNumStacks(aura) => Int::AuraNumStacks(*aura),
            Compiled::CurrentComboPoints => Int::CurrentComboPoints,
            Compiled::NumberTargets => Int::NumberTargets,
            other => Int::Other(Box::new(other.clone())),
        }
    }
}

impl Float {
    fn asks_cost(&self) -> bool {
        matches!(self, Float::Other(value) if compiled_asks_cost(value))
    }

    fn lower(value: &Compiled) -> Float {
        match value {
            Compiled::Const(constant) => Float::Const(constant.float),
            Compiled::CurrentManaPercent => Float::CurrentManaPercent,
            Compiled::CurrentMana => Float::CurrentMana,
            Compiled::CurrentRage => Float::CurrentRage,
            Compiled::CurrentEnergy => Float::CurrentEnergy,
            other => Float::Other(Box::new(other.clone())),
        }
    }
}

impl<A: Agent> Fight<A> {
    /// Evaluate a lowered condition as [`Fight::get_bool`] evaluates its compiled form.
    pub(crate) fn condition(&mut self, value: &Cond) -> bool {
        match value {
            Cond::Const(value) => *value,
            Cond::AuraIsActive(aura) => self.aura(*aura).active,
            Cond::DotIsActive(dot) => self.dot_active(*dot),
            // Go `APLValueIsExecutePhase`: the encounter's execute phase is at or below it.
            Cond::IsExecutePhase(threshold) => self.execute_phase <= *threshold,
            // Go `APLValueSpellCanCast`: `CanCastOrQueue`, with its cost check's side effects.
            Cond::SpellCanCast(spell) => self.can_cast_or_queue(*spell),
            Cond::And(values) => values.iter().all(|value| self.condition(value)),
            Cond::Or(values) => values.iter().any(|value| self.condition(value)),
            Cond::Not(value) => !self.condition(value),
            Cond::DurationConst(op, lhs, rhs) => {
                let lhs = self.lowered_duration(lhs);
                compare(*op, lhs, *rhs)
            }
            Cond::IntConst(op, lhs, rhs) => {
                let lhs = self.lowered_int(lhs);
                compare(*op, lhs, *rhs)
            }
            Cond::FloatConst(op, lhs, rhs) => {
                let lhs = self.lowered_float(lhs);
                compare(*op, lhs, *rhs)
            }
            Cond::Duration(op, operands) => {
                let lhs = self.lowered_duration(&operands.0);
                let rhs = self.lowered_duration(&operands.1);
                compare(*op, lhs, rhs)
            }
            Cond::Int(op, operands) => {
                let lhs = self.lowered_int(&operands.0);
                let rhs = self.lowered_int(&operands.1);
                compare(*op, lhs, rhs)
            }
            Cond::Float(op, operands) => {
                let lhs = self.lowered_float(&operands.0);
                let rhs = self.lowered_float(&operands.1);
                compare(*op, lhs, rhs)
            }
            Cond::Other(value) => self.get_bool(value),
        }
    }

    #[inline]
    fn lowered_duration(&mut self, value: &Duration) -> i64 {
        match value {
            Duration::Const(value) => *value,
            Duration::RemainingTime => self.duration - self.now,
            Duration::CurrentTime => self.now,
            Duration::AuraRemainingTime(aura) => self.aura_remaining_time(*aura),
            Duration::SpellTimeToReady(spell) => self.spell_time_to_ready(*spell),
            Duration::Other(value) => self.get_duration(value),
        }
    }

    #[inline]
    fn lowered_int(&mut self, value: &Int) -> i32 {
        match value {
            Int::Const(value) => *value,
            Int::AuraNumStacks(aura) => self.aura(*aura).stacks,
            Int::CurrentComboPoints => self.energy_bar().combo_points,
            Int::NumberTargets => self.targets.len() as i32,
            Int::Other(value) => self.get_int(value),
        }
    }

    #[inline]
    fn lowered_float(&mut self, value: &Float) -> f64 {
        match value {
            Float::Const(value) => *value,
            Float::CurrentManaPercent => self.player.mana / self.player.powers.max_mana,
            Float::CurrentMana => self.player.mana,
            Float::CurrentRage => self.current_rage(),
            Float::CurrentEnergy => self.energy_bar().current,
            Float::Other(value) => self.get_float(value),
        }
    }
}

/// A value read while evaluating, and how it moves while the clock runs with nothing it reads
/// changing.
#[derive(Clone, Copy)]
enum Motion {
    Fixed,
    /// Falls one nanosecond per nanosecond; `clamped` stops it at zero.
    Falling {
        clamped: bool,
    },
    /// Rises one nanosecond per nanosecond.
    Rising,
    /// No promise.
    Unknown,
}

/// When a comparison of a moving value, now `value`, with a fixed `other` can next change.
fn crossing(now: i64, value: i64, other: i64, motion: Motion) -> i64 {
    match motion {
        Motion::Fixed => NEVER_EXPIRES,
        Motion::Unknown => now,
        Motion::Falling { clamped } => {
            if clamped && other < 0 {
                NEVER_EXPIRES
            } else if value > other {
                now.saturating_add(value - other)
            } else if value == other {
                if clamped && other == 0 {
                    NEVER_EXPIRES
                } else {
                    now + 1
                }
            } else {
                NEVER_EXPIRES
            }
        }
        Motion::Rising => {
            if value < other {
                now.saturating_add(other - value)
            } else if value == other {
                now + 1
            } else {
                NEVER_EXPIRES
            }
        }
    }
}

/// The energy below which `energy op constant` keeps the value it has now, as energy rises from
/// `energy`.
fn energy_crossing(op: CompareOp, energy: f64, constant: f64) -> f64 {
    let at = match op {
        // The value changes once energy reaches the constant.
        CompareOp::Lt | CompareOp::Ge => constant,
        // The value changes once energy passes it.
        CompareOp::Le | CompareOp::Gt => constant.next_up(),
        CompareOp::Eq | CompareOp::Ne if energy < constant => constant,
        CompareOp::Eq | CompareOp::Ne => constant.next_up(),
    };
    if energy < at {
        at
    } else {
        f64::INFINITY
    }
}

impl<A: Agent> Fight<A> {
    /// [`Fight::condition`], which it evaluates in the same order with the same reads, and
    /// also how long its answer holds: until the clock reaches the returned time, while nothing
    /// in the returned [`reads`] changes.
    pub(crate) fn condition_stable(&mut self, value: &Cond) -> (bool, i64, u8) {
        match value {
            Cond::Const(value) => (*value, NEVER_EXPIRES, 0),
            Cond::AuraIsActive(aura) => (self.aura(*aura).active, NEVER_EXPIRES, reads::AURAS),
            Cond::DotIsActive(dot) => (self.dot_active(*dot), NEVER_EXPIRES, reads::AURAS),
            Cond::IsExecutePhase(threshold) => (
                self.execute_phase <= *threshold,
                NEVER_EXPIRES,
                reads::PHASE,
            ),
            Cond::SpellCanCast(spell) => (self.can_cast_or_queue(*spell), self.now, reads::ALL),
            // A false `and` stays false while its first false operand does; a true one holds
            // while every operand does.
            Cond::And(values) => {
                let (mut until, mut read) = (NEVER_EXPIRES, 0);
                for value in values.iter() {
                    let (holds, holds_until, reads) = self.condition_stable(value);
                    if !holds {
                        return (false, holds_until, reads);
                    }
                    until = until.min(holds_until);
                    read |= reads;
                }
                (true, until, read)
            }
            Cond::Or(values) => {
                let (mut until, mut read) = (NEVER_EXPIRES, 0);
                for value in values.iter() {
                    let (holds, holds_until, reads) = self.condition_stable(value);
                    if holds {
                        return (true, holds_until, reads);
                    }
                    until = until.min(holds_until);
                    read |= reads;
                }
                (false, until, read)
            }
            Cond::Not(value) => {
                let (holds, until, reads) = self.condition_stable(value);
                (!holds, until, reads)
            }
            Cond::DurationConst(op, lhs, rhs) => {
                let (value, motion, reads) = self.duration_stable(lhs);
                (
                    compare(*op, value, *rhs),
                    crossing(self.now, value, *rhs, motion),
                    reads,
                )
            }
            Cond::Duration(op, operands) => {
                let (lhs, lhs_motion, lhs_reads) = self.duration_stable(&operands.0);
                let (rhs, rhs_motion, rhs_reads) = self.duration_stable(&operands.1);
                let until = match (lhs_motion, rhs_motion) {
                    (motion, Motion::Fixed) => crossing(self.now, lhs, rhs, motion),
                    (Motion::Fixed, motion) => crossing(self.now, rhs, lhs, motion),
                    _ => self.now,
                };
                (compare(*op, lhs, rhs), until, lhs_reads | rhs_reads)
            }
            Cond::IntConst(op, lhs, rhs) => {
                let (value, fixed, reads) = self.int_stable(lhs);
                let until = if fixed { NEVER_EXPIRES } else { self.now };
                (compare(*op, value, *rhs), until, reads)
            }
            Cond::Int(op, operands) => {
                let (lhs, lhs_fixed, lhs_reads) = self.int_stable(&operands.0);
                let (rhs, rhs_fixed, rhs_reads) = self.int_stable(&operands.1);
                let until = if lhs_fixed && rhs_fixed {
                    NEVER_EXPIRES
                } else {
                    self.now
                };
                (compare(*op, lhs, rhs), until, lhs_reads | rhs_reads)
            }
            Cond::FloatConst(op, lhs, rhs) => {
                let (value, fixed, reads) = self.float_stable(lhs);
                let until = if fixed { NEVER_EXPIRES } else { self.now };
                (compare(*op, value, *rhs), until, reads)
            }
            Cond::Float(op, operands) => {
                let (lhs, lhs_fixed, lhs_reads) = self.float_stable(&operands.0);
                let (rhs, rhs_fixed, rhs_reads) = self.float_stable(&operands.1);
                let until = if lhs_fixed && rhs_fixed {
                    NEVER_EXPIRES
                } else {
                    self.now
                };
                (compare(*op, lhs, rhs), until, lhs_reads | rhs_reads)
            }
            Cond::Other(value) => (self.get_bool(value), self.now, reads::ALL),
        }
    }

    /// The energy below which a condition keeps the value it has now while energy only rises and
    /// nothing else changes, evaluated in [`Fight::condition`]'s order like
    /// [`Fight::condition_stable`]. A comparison of the current energy with a constant changes
    /// value at that constant, one of the current energy with another value is not followed,
    /// and no other node reads energy.
    pub(crate) fn condition_energy_limit(&mut self, value: &Cond) -> f64 {
        match value {
            // A false `and` keeps its value while its first false operand does; a true one while
            // every operand does. An `or` the other way round.
            Cond::And(values) | Cond::Or(values) => {
                let stops_on = matches!(value, Cond::Or(_));
                let mut limit = f64::INFINITY;
                for value in values.iter() {
                    let holds = self.condition(value);
                    let part = self.condition_energy_limit(value);
                    if holds == stops_on {
                        return part;
                    }
                    limit = limit.min(part);
                }
                limit
            }
            Cond::Not(value) => self.condition_energy_limit(value),
            Cond::FloatConst(op, Float::CurrentEnergy, constant) => {
                energy_crossing(*op, self.energy_bar().current, *constant)
            }
            Cond::Float(_, operands)
                if matches!(operands.0, Float::CurrentEnergy)
                    || matches!(operands.1, Float::CurrentEnergy) =>
            {
                self.energy_bar().current
            }
            _ => f64::INFINITY,
        }
    }

    fn duration_stable(&mut self, value: &Duration) -> (i64, Motion, u8) {
        match value {
            Duration::Const(value) => (*value, Motion::Fixed, 0),
            Duration::RemainingTime => (
                self.duration - self.now,
                Motion::Falling { clamped: false },
                0,
            ),
            Duration::CurrentTime => (self.now, Motion::Rising, 0),
            // Falls to the aura's expiry, which changes the aura.
            Duration::AuraRemainingTime(aura) => {
                let state = self.aura(*aura);
                let motion = if state.active && state.expires != NEVER_EXPIRES {
                    Motion::Falling { clamped: false }
                } else {
                    Motion::Fixed
                };
                (self.aura_remaining_time(*aura), motion, reads::AURAS)
            }
            Duration::SpellTimeToReady(spell) => (
                self.spell_time_to_ready(*spell),
                Motion::Falling { clamped: true },
                reads::TIMERS,
            ),
            Duration::Other(value) => (self.get_duration(value), Motion::Unknown, reads::ALL),
        }
    }

    /// An int read, whether it holds still with the clock, and what it reads.
    fn int_stable(&mut self, value: &Int) -> (i32, bool, u8) {
        match value {
            Int::Const(value) => (*value, true, 0),
            Int::AuraNumStacks(aura) => (self.aura(*aura).stacks, true, reads::AURAS),
            Int::CurrentComboPoints => (self.energy_bar().combo_points, true, reads::RESOURCES),
            Int::NumberTargets => (self.targets.len() as i32, true, 0),
            Int::Other(value) => (self.get_int(value), false, reads::ALL),
        }
    }

    /// A float read, whether it holds still with the clock, and what it reads.
    fn float_stable(&mut self, value: &Float) -> (f64, bool, u8) {
        match value {
            Float::Const(value) => (*value, true, 0),
            // The maximum moves with stats, which only auras change.
            Float::CurrentManaPercent => (
                self.player.mana / self.player.powers.max_mana,
                true,
                reads::RESOURCES | reads::AURAS,
            ),
            Float::CurrentMana => (self.player.mana, true, reads::RESOURCES),
            Float::CurrentRage => (self.current_rage(), true, reads::RESOURCES),
            Float::CurrentEnergy => (self.energy_bar().current, true, reads::RESOURCES),
            Float::Other(value) => (self.get_float(value), false, reads::ALL),
        }
    }
}
