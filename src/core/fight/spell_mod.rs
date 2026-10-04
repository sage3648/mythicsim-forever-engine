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
    /// Go `SpellMod_DamageDone_Flat`: adds to `DamageMultiplierAdditive`.
    DamageDoneFlat,
    /// Go `SpellMod_DirectDamageDone_Flat`: adds to `DirectDamageMultiplierAdditive`.
    DirectDamageDoneFlat,
    /// Go `SpellMod_CastTime_Pct`: adds to `CastTimeMultiplier`.
    CastTimePercent,
    /// Go `SpellMod_ThreatMultiplier_Pct`: multiplies `ThreatMultiplier` by one plus the value.
    ThreatMultiplierPercent,
    /// Go `SpellMod_DamageDone_Pct`: multiplies `DamageMultiplier` by one plus the value.
    DamageDonePercent,
    /// Go `SpellMod_PowerCost_Pct`: multiplies the cost's `PercentModifier`.
    PowerCostPercent,
    /// Go `SpellMod_DotDamageDone_Pct`: multiplies the dot's `PeriodicDamageMultiplier`.
    DotDamageDonePercent,
    /// Go `SpellMod_CritMultiplier_Flat`: adds to `CritMultiplierAdditive`.
    CritMultiplierFlat,
    /// Go `SpellMod_CastTime_Flat`: adds to the default cast time, in nanoseconds.
    CastTimeFlat,
    /// Go `SpellMod_GlobalCooldown_Flat`: adds to the default GCD, in nanoseconds.
    GlobalCooldownFlat,
    /// Go `SpellMod_PowerCost_Flat`: adds its integer value, held in the float value, to the
    /// cost's `FlatModifier`.
    PowerCostFlat,
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
        let modifier = &self.mods[id];
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
                ModKind::PowerCostFlat => {
                    if let Some(cost) = state.cost.as_mut() {
                        if sign > 0.0 {
                            cost.flat_modifier += modifier.float_value as i32;
                        } else {
                            cost.flat_modifier -= modifier.float_value as i32;
                        }
                    }
                }
                ModKind::DamageDoneFlat => {
                    if sign > 0.0 {
                        state.damage_multiplier_additive += modifier.float_value;
                    } else {
                        state.damage_multiplier_additive -= modifier.float_value;
                    }
                }
                ModKind::DirectDamageDoneFlat => {
                    if sign > 0.0 {
                        state.direct_damage_multiplier_additive += modifier.float_value;
                    } else {
                        state.direct_damage_multiplier_additive -= modifier.float_value;
                    }
                }
                ModKind::ThreatMultiplierPercent => {
                    if sign > 0.0 {
                        state.threat_multiplier *= 1.0 + modifier.float_value;
                    } else {
                        state.threat_multiplier /= 1.0 + modifier.float_value;
                    }
                }
                ModKind::DamageDonePercent => {
                    if sign > 0.0 {
                        state.damage_multiplier *= 1.0 + modifier.float_value;
                    } else {
                        state.damage_multiplier /= 1.0 + modifier.float_value;
                    }
                }
                ModKind::PowerCostPercent => {
                    if let Some(cost) = state.cost.as_mut() {
                        if sign > 0.0 {
                            cost.percent_modifier *= 1.0 + modifier.float_value;
                        } else {
                            cost.percent_modifier /= 1.0 + modifier.float_value;
                        }
                    }
                }
                ModKind::DotDamageDonePercent => {
                    if let Some(dot) = state.dot {
                        let value = &mut self.dots[dot].periodic_damage_multiplier;
                        if sign > 0.0 {
                            *value *= 1.0 + modifier.float_value;
                        } else {
                            *value /= 1.0 + modifier.float_value;
                        }
                    }
                }
                ModKind::CastTimePercent => {
                    if sign > 0.0 {
                        state.cast_time_multiplier += modifier.float_value;
                    } else {
                        state.cast_time_multiplier -= modifier.float_value;
                    }
                }
                ModKind::CritMultiplierFlat => {
                    if sign > 0.0 {
                        state.crit_multiplier_additive += modifier.float_value;
                    } else {
                        state.crit_multiplier_additive -= modifier.float_value;
                    }
                }
                ModKind::CastTimeFlat => {
                    if sign > 0.0 {
                        state.default_cast.cast_time += modifier.time_value;
                    } else {
                        state.default_cast.cast_time -= modifier.time_value;
                    }
                }
                ModKind::GlobalCooldownFlat => {
                    if sign > 0.0 {
                        state.default_cast.gcd += modifier.time_value;
                    } else {
                        state.default_cast.gcd -= modifier.time_value;
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

    /// Go `onResetDamageDoneAdd` and `onResetDirectDamageDoneAdd`, reset effects of every
    /// damage-done modifier: round the additive multiplier of its spells to four decimals,
    /// so add and subtract residue does not carry into the next iteration.
    pub(crate) fn reset_mods(&mut self) {
        let round = |value: &mut f64| *value = (*value * 10000.0).round() / 10000.0;
        for modifier in &self.mods {
            for &spell in &modifier.affected {
                let state = &mut self.spells[spell];
                match modifier.kind {
                    ModKind::DamageDoneFlat => round(&mut state.damage_multiplier_additive),
                    ModKind::DirectDamageDoneFlat => {
                        round(&mut state.direct_damage_multiplier_additive)
                    }
                    _ => {}
                }
            }
        }
    }

    /// Spells a class- and school-masked modifier applies to: Go `shouldApply` with a
    /// `School`, which matches any shared school bit.
    pub(crate) fn spells_with_class_and_school(
        &self,
        classes: &[&str],
        school: u8,
    ) -> Vec<SpellId> {
        self.spells_with_class(classes)
            .into_iter()
            .filter(|&spell| self.spells[spell].school & school != 0)
            .collect()
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
