//! Go apl.go: compile the parsed rotation and run `DoNextAction`.

use crate::{
    contracts::prepared_v2::ActionId,
    rotation::{
        compile_condition, Action as ParsedAction, CompareOp, CompiledCondition, FoundAura, Lookup,
        MissingAura, Rotation, ValueType,
    },
};

use super::{cast::MAX_SPELL_QUEUE_WINDOW, Agent, AuraRef, Fight, Side, SpellId};

pub(crate) type Compiled = crate::rotation::Compiled<AuraRef>;

#[derive(Clone, Copy, Debug)]
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

impl<A: Agent> Fight<A> {
    /// Go `GetAPLSpell`: the first APL-flagged spell with the action ID, otherwise the first
    /// registered one.
    pub(crate) fn apl_spell(&self, id: &ActionId) -> Option<SpellId> {
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
                ParsedAction::AutocastOtherCooldowns => None,
            })
            .collect();
        prepull.sort_by_key(|(do_at, _)| *do_at);
        prepull
    }

    /// Go `newAPLRotation` for the supported subset. Conditions compile as the pinned
    /// reference does; coverage rejects rotations where community #622 would differ.
    pub(crate) fn compile_rotation(&self, rotation: &Rotation) -> Vec<Item> {
        let tracker = &self.trackers[Side::Player.index()];
        let aura = |id: &ActionId| {
            tracker.find_by_id(id).map(|index| FoundAura {
                aura: AuraRef {
                    side: Side::Player,
                    index,
                },
                max_stacks: tracker.auras[index].max_stacks,
            })
        };
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

    /// Whether the dot of a compiled spell is active on its unit.
    fn dot_active(&self, spell: SpellId) -> bool {
        let dot = self.spells[spell].dot.expect("compiled dots have a dot");
        self.aura(self.dots[dot].aura).active
    }

    /// Go `APLAction.IsReady`: the condition, then the action's readiness.
    fn item_ready(&mut self, item: usize) -> Option<Ready> {
        if let Some(condition) = &self.rotation[item].condition {
            if !self.get_bool(condition) {
                return None;
            }
        }
        match self.rotation[item].action {
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
