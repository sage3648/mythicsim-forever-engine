//! Go spell_mod.go dynamic modifiers: activation adds the value to every affected spell,
//! deactivation subtracts it, and a value update deactivates, replaces and reactivates.
//! The value survives deactivation, as in Go, and the float operations happen in Go's
//! order, which matters because spell fields are not reset between iterations.

use super::{Agent, Fight, SpellId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModKind {
    /// Go `SpellMod_BonusCrit_Percent`.
    BonusCritPercent,
    /// Go `SpellMod_PowerCost_Pct_Add`.
    PowerCostPercentAdd,
    /// Go `SpellMod_DotTickLength_Flat`, in nanoseconds.
    DotTickLengthFlat,
}

#[derive(Clone, Debug)]
pub(crate) struct SpellMod {
    kind: ModKind,
    float_value: f64,
    time_value: i64,
    active: bool,
    affected: Vec<SpellId>,
}

pub(crate) type ModId = usize;

impl<A: Agent> Fight<A> {
    /// Go `AddDynamicMod`: inactive until its aura activates it.
    pub(crate) fn register_mod(
        &mut self,
        kind: ModKind,
        float_value: f64,
        time_value: i64,
        affected: Vec<SpellId>,
    ) -> ModId {
        self.mods.push(SpellMod {
            kind,
            float_value,
            time_value,
            active: false,
            affected,
        });
        self.mods.len() - 1
    }

    fn apply_mod(&mut self, id: ModId, sign: f64) {
        let modifier = self.mods[id].clone();
        for &spell in &modifier.affected {
            let state = &mut self.spells[spell];
            match modifier.kind {
                ModKind::BonusCritPercent => {
                    if sign > 0.0 {
                        state.bonus_crit_percent += modifier.float_value;
                    } else {
                        state.bonus_crit_percent -= modifier.float_value;
                    }
                }
                ModKind::PowerCostPercentAdd => {
                    if let Some(cost) = state.cost.as_mut() {
                        if sign > 0.0 {
                            cost.additive_percent_modifier += modifier.float_value;
                        } else {
                            cost.additive_percent_modifier -= modifier.float_value;
                        }
                    }
                }
                ModKind::DotTickLengthFlat => {
                    if let Some(dot) = state.dot {
                        if sign > 0.0 {
                            self.dots[dot].base_tick_length += modifier.time_value;
                        } else {
                            self.dots[dot].base_tick_length -= modifier.time_value;
                        }
                    }
                }
            }
        }
    }

    /// Go `SpellMod.Activate`.
    pub(crate) fn activate_mod(&mut self, id: ModId) {
        if self.mods[id].active {
            return;
        }
        self.apply_mod(id, 1.0);
        self.mods[id].active = true;
    }

    /// Go `SpellMod.Deactivate`.
    pub(crate) fn deactivate_mod(&mut self, id: ModId) {
        if !self.mods[id].active {
            return;
        }
        self.apply_mod(id, -1.0);
        self.mods[id].active = false;
    }

    /// Go `SpellMod.UpdateFloatValue`.
    pub(crate) fn update_mod_value(&mut self, id: ModId, value: f64) {
        if self.mods[id].active {
            self.deactivate_mod(id);
            self.mods[id].float_value = value;
            self.activate_mod(id);
        } else {
            self.mods[id].float_value = value;
        }
    }

    /// Spells a class-masked modifier applies to, skipping `SpellFlagNoSpellMods`.
    pub(crate) fn spells_with_class(&self, classes: &[&str]) -> Vec<SpellId> {
        (0..self.spells.len())
            .filter(|&spell| {
                let state = &self.spells[spell];
                !state.flags.no_spell_mods
                    && state
                        .class_spell
                        .as_deref()
                        .is_some_and(|class| classes.contains(&class))
            })
            .collect()
    }
}
