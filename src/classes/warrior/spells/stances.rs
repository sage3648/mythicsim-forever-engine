//! Warrior stances, from Go sim/warrior/stances.go, and the stance checks of the spells'
//! `ExtraCastCondition`s. The warrior starts every fight in its default stance, whose aura
//! is permanent; the gate rejects rotations that would change stance.

/// Go `Stance`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Stance {
    #[default]
    None,
    Battle,
    Defensive,
    Berserker,
}

impl Stance {
    /// The exporter's stance name.
    pub(crate) fn from_name(name: &str) -> Stance {
        match name {
            "battle" => Stance::Battle,
            "defensive" => Stance::Defensive,
            "berserker" => Stance::Berserker,
            _ => Stance::None,
        }
    }
}

/// What a stance-locked Warrior spell needs before it may be cast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StanceLock {
    /// retaliation.go: Battle Stance.
    Battle,
    /// shield_wall.go: Defensive Stance and a shield, which the gate keeps out of scope.
    Defensive,
}

impl StanceLock {
    /// The spell's `ExtraCastCondition` in the current stance.
    pub(crate) fn allows(self, stance: Stance) -> bool {
        match self {
            StanceLock::Battle => stance == Stance::Battle,
            StanceLock::Defensive => stance == Stance::Defensive,
        }
    }
}

/// A stance cast's aura and its rage metrics.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StanceCast {
    pub(crate) stance: Stance,
    pub(crate) aura: crate::core::fight::AuraRef,
    /// Go `NewRageMetrics(actionID)` of the stance's cast.
    pub(crate) metrics: usize,
}

/// A stance cast's `ApplyEffects`: the aura activates, which deactivates the old stance's
/// through their exclusive category, rage above what Tactical Mastery keeps is spent, and the
/// warrior is in the new stance.
pub(crate) fn change<A: crate::core::fight::Agent>(
    fight: &mut crate::core::fight::Fight<A>,
    cast: StanceCast,
    max_retained_rage: f64,
) -> Stance {
    fight.activate_aura(cast.aura);
    let rage = fight.current_rage();
    if rage > max_retained_rage {
        fight.spend_rage(rage - max_retained_rage, cast.metrics);
    }
    cast.stance
}
