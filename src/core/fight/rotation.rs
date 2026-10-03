//! Go apl.go: compile the parsed rotation and run `DoNextAction`.

use crate::rotation::{Action as ParsedAction, CompareOp, Const, Rotation, Value, ValueType};

use super::{Agent, AuraRef, Fight, Side, SpellId};

/// A compiled APL value with Go's coercion applied.
#[derive(Clone, Debug)]
pub(crate) enum Compiled {
    Const(Const),
    Compare {
        op: CompareOp,
        lhs: Box<Compiled>,
        rhs: Box<Compiled>,
    },
    And(Vec<Compiled>),
    CurrentManaPercent,
    RemainingTime,
    AuraIsActive(AuraRef),
    /// Go `APLValueCoerced`.
    Coerced {
        to: ValueType,
        inner: Box<Compiled>,
    },
}

impl Compiled {
    fn value_type(&self) -> ValueType {
        match self {
            Compiled::Const(constant) => constant.value_type,
            Compiled::Compare { .. } | Compiled::And(_) | Compiled::AuraIsActive(_) => {
                ValueType::Bool
            }
            Compiled::CurrentManaPercent => ValueType::Float,
            Compiled::RemainingTime => ValueType::Duration,
            Compiled::Coerced { to, .. } => *to,
        }
    }

    fn const_bool(&self) -> Option<bool> {
        match self {
            Compiled::Const(constant) if constant.value_type == ValueType::Bool => {
                Some(constant.boolean)
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Act {
    Cast(SpellId),
    Autocast,
}

/// A ready action. Go keeps the cooldown found by `IsReady` for `Execute`.
enum Ready {
    Cast(SpellId),
    Autocast(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Item {
    condition: Option<Compiled>,
    action: Act,
}

/// Go `coerceTo`: a constant changes type in place; anything else is wrapped.
fn coerce(value: Compiled, to: ValueType) -> Compiled {
    if value.value_type() == to {
        value
    } else if let Compiled::Const(mut constant) = value {
        constant.value_type = to;
        Compiled::Const(constant)
    } else {
        Compiled::Coerced {
            to,
            inner: Box::new(value),
        }
    }
}

impl<A: Agent> Fight<A> {
    /// Go `newAPLRotation` for the supported subset.
    pub(crate) fn compile_rotation(&self, rotation: &Rotation) -> Result<Vec<Item>, String> {
        let mut items = Vec::new();
        for item in &rotation.priority_list {
            let action = match &item.action {
                ParsedAction::CastSpell(id) => {
                    // Go GetAPLCastSpell: an unknown spell drops the action.
                    let apl = self.spells.iter().position(|s| &s.id == id && s.flags.apl);
                    match apl.or_else(|| self.spells.iter().position(|s| &s.id == id)) {
                        Some(spell) => Act::Cast(spell),
                        None => continue,
                    }
                }
                ParsedAction::AutocastOtherCooldowns => Act::Autocast,
            };
            let condition = match &item.condition {
                Some(value) => self
                    .compile_value(value)?
                    .map(|value| coerce(value, ValueType::Bool)),
                None => None,
            };
            match condition.as_ref().and_then(Compiled::const_bool) {
                // A constant false condition prunes the action; its spells already left the
                // major cooldowns in Go's export.
                Some(false) => continue,
                Some(true) => items.push(Item {
                    condition: None,
                    action,
                }),
                None => items.push(Item { condition, action }),
            }
        }
        Ok(items)
    }

    fn compile_value(&self, value: &Value) -> Result<Option<Compiled>, String> {
        Ok(Some(match value {
            Value::Const(constant) => Compiled::Const(constant.clone()),
            Value::CurrentManaPercent => Compiled::CurrentManaPercent,
            Value::RemainingTime => Compiled::RemainingTime,
            Value::AuraIsKnown(id) => {
                let known = self.trackers[Side::Player.index()].find_by_id(id).is_some();
                Compiled::Const(crate::rotation::parse_const(if known {
                    "true"
                } else {
                    "false"
                })?)
            }
            Value::AuraIsActive(id) => match self.trackers[Side::Player.index()].find_by_id(id) {
                Some(index) => Compiled::AuraIsActive(AuraRef {
                    side: Side::Player,
                    index,
                }),
                None => {
                    return Err(format!(
                        "auraIsActive names {id}, which the character lacks"
                    ))
                }
            },
            Value::Compare { op, lhs, rhs } => {
                let (Some(lhs), Some(rhs)) = (self.compile_value(lhs)?, self.compile_value(rhs)?)
                else {
                    return Ok(None);
                };
                let to = lhs.value_type().max(rhs.value_type());
                Compiled::Compare {
                    op: *op,
                    lhs: Box::new(coerce(lhs, to)),
                    rhs: Box::new(coerce(rhs, to)),
                }
            }
            Value::And(values) => {
                let mut compiled = Vec::new();
                for value in values {
                    if let Some(value) = self.compile_value(value)? {
                        compiled.push(coerce(value, ValueType::Bool));
                    }
                }
                match compiled.len() {
                    0 => return Ok(None),
                    1 => compiled.pop().expect("one value"),
                    _ => {
                        if let Some(false_value) =
                            compiled.iter().find(|v| v.const_bool() == Some(false))
                        {
                            false_value.clone()
                        } else {
                            Compiled::And(compiled)
                        }
                    }
                }
            }
        }))
    }

    fn get_bool(&self, value: &Compiled) -> bool {
        match value {
            Compiled::Const(constant) => constant.boolean,
            Compiled::AuraIsActive(aura) => self.aura(*aura).active,
            Compiled::And(values) => values.iter().all(|value| self.get_bool(value)),
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
            Compiled::RemainingTime => self.duration - self.now,
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

    /// Go `APLAction.IsReady`: the condition, then the action's readiness.
    fn item_ready(&mut self, item: usize) -> Option<Ready> {
        let (condition, action) = {
            let entry = &self.rotation[item];
            (entry.condition.clone(), entry.action.clone())
        };
        if let Some(condition) = &condition {
            if !self.get_bool(condition) {
                return None;
            }
        }
        match action {
            Act::Cast(spell) => {
                let ready = self.can_cast_or_queue(spell) && {
                    let flags = self.spells[spell].flags;
                    !flags.mcd || flags.reactive || self.player.gcd <= self.now
                };
                ready.then_some(Ready::Cast(spell))
            }
            Act::Autocast => self.autocast_ready().map(Ready::Autocast),
        }
    }

    /// Go `APLRotation.DoNextAction`.
    pub(crate) fn do_next_action(&mut self) {
        if self.now < 0 || self.in_rotation {
            return;
        }
        if let Some(dot) = self.player.channeled_dot {
            // With no interrupt condition, a channel only ends here once its ticks are spent.
            if self.dots[dot].remaining_ticks == 0 {
                let aura = self.dots[dot].aura;
                self.deactivate_aura(aura);
            }
            return;
        }
        if self.player.rotation_timer > self.now {
            return;
        }
        self.in_rotation = true;
        let mut executed = 0;
        loop {
            let mut next = None;
            for item in 0..self.rotation.len() {
                if let Some(action) = self.item_ready(item) {
                    next = Some(action);
                    break;
                }
            }
            let Some(action) = next else { break };
            assert!(executed <= 1000, "infinite rotation loop");
            match action {
                Ready::Cast(spell) => self.cast_or_queue(spell, Side::Target),
                Ready::Autocast(cooldown) => self.autocast(cooldown),
            }
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

    /// Go `APLRotation.reset`.
    pub(crate) fn rotation_reset(&mut self, side: Side) {
        if side == Side::Player {
            self.in_rotation = false;
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
