//! Charge (11578), from Go sim/warrior/charge.go: a cast before the pull, in Battle Stance (or
//! Defensive Stance with Vanguard), that gives rage, triples the warrior's movement speed while
//! its dash aura is up and runs it to 3.5 yards inside the spell's minimum range, so that the
//! one yard steps of the movement aura leave it in melee range. The dash aura ends when the
//! movement does.

use crate::core::fight::{movement::MovementKind, Agent, AuraRef, Fight, SpellId};

use super::stances::Stance;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Charge {
    pub(crate) aura: AuraRef,
    /// The rage the cast gives, with Improved Charge.
    pub(crate) rage: f64,
    /// Vanguard lets the cast in Defensive Stance.
    pub(crate) vanguard: bool,
    /// What the dash aura multiplies the movement speed by, a Go literal.
    pub(crate) speed_multiplier: f64,
    /// How far inside the minimum range the cast runs to, a Go literal.
    pub(crate) overshoot: f64,
    pub(crate) min_range: f64,
    /// Go `NewRageMetrics(actionID)`.
    pub(crate) metrics: usize,
}

/// The cast's `ExtraCastCondition`: before the pull, in a stance that allows it.
pub(crate) fn condition<A: Agent>(fight: &Fight<A>, params: Charge, stance: Stance) -> bool {
    fight.now < 0 && (stance == Stance::Battle || (params.vanguard && stance == Stance::Defensive))
}

/// The cast's `ApplyEffects`: the aura lasts the spell's cooldown, the rage, then the run.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, params: Charge) {
    let duration = fight.spells[spell].cd.map_or(0, |(_, duration)| duration);
    fight.aura_mut(params.aura).duration = duration;
    fight.activate_aura(params.aura);
    fight.add_rage(params.rage, params.metrics);
    fight.move_to(
        crate::core::fight::Side::Player,
        params.min_range - params.overshoot,
    );
}

/// The dash aura's `OnGain`: Go `MultiplyMovementSpeed` by its multiplier.
pub(crate) fn on_gain<A: Agent>(fight: &mut Fight<A>, params: Charge) {
    fight.multiply_movement_speed(params.speed_multiplier);
}

/// The dash aura's `OnExpire`: the multiplier undone.
pub(crate) fn on_expire<A: Agent>(fight: &mut Fight<A>, params: Charge) {
    fight.multiply_movement_speed(1.0 / params.speed_multiplier);
}

/// The movement callback the dash registers: the end of the movement ends the aura.
pub(crate) fn on_movement<A: Agent>(fight: &mut Fight<A>, params: Charge, kind: MovementKind) {
    if kind == MovementKind::End && fight.aura(params.aura).active {
        fight.deactivate_aura(params.aura);
    }
}
