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
    /// A stance cast: castable only outside its stance.
    Change(Stance),
    /// retaliation.go: Battle Stance.
    Battle,
    /// shield_wall.go: Defensive Stance and a shield, which the gate keeps out of scope.
    Defensive,
}

impl StanceLock {
    /// The spell's `ExtraCastCondition` in the current stance.
    pub(crate) fn allows(self, stance: Stance) -> bool {
        match self {
            StanceLock::Change(to) => stance != to,
            StanceLock::Battle => stance == Stance::Battle,
            StanceLock::Defensive => stance == Stance::Defensive,
        }
    }
}
