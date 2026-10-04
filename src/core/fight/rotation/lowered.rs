//! Rotation conditions lowered for evaluation. Compilation resolves the operand types of
//! the common nodes and fuses a comparison with a constant right operand into one node, so
//! evaluating the usual condition walks few nodes and needs no type dispatch. Every other
//! node keeps its compiled form and goes through the general getters. Nodes are evaluated
//! in the compiled order, so reads with side effects happen exactly as before.

use super::{compare, Compiled};
use crate::{
    core::fight::{Agent, AuraRef, Fight, SpellId},
    rotation::{CompareOp, ValueType},
};

/// A boolean condition.
#[derive(Debug)]
pub(crate) enum Cond {
    Const(bool),
    AuraIsActive(AuraRef),
    DotIsActive(SpellId),
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
    /// Lower a compiled condition. Each node reads exactly what its compiled form reads.
    pub(crate) fn lower(value: &Compiled) -> Cond {
        match value {
            // The getters read a constant's boolean whatever its type.
            Compiled::Const(constant) => Cond::Const(constant.boolean),
            Compiled::AuraIsActive(aura) => Cond::AuraIsActive(*aura),
            Compiled::DotIsActive(spell) => Cond::DotIsActive(*spell),
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

impl Duration {
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
            Cond::DotIsActive(spell) => self.dot_active(*spell),
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
            Int::NumberTargets => 1,
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
