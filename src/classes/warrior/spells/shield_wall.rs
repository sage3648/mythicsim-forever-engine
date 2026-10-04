//! Shield Wall (871), from Go sim/warrior/shield_wall.go: in Defensive Stance with a shield, an
//! aura whose damage taken multiplier `pseudo_stat_auras` carries. A tank autocasts it as a
//! survival cooldown once health falls below the share Go states and the defensive health
//! threshold; a DPS warrior never does.

use crate::core::fight::AuraRef;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShieldWall {
    pub(crate) aura: AuraRef,
    /// Whether the major cooldown's `ShouldActivate` can pass: not for a DPS warrior.
    pub(crate) autocast: bool,
    /// The health share below which a tank autocasts it, a Go literal.
    pub(crate) health_percent: f64,
    /// The `ExtraCastCondition`'s shield.
    pub(crate) can_block: bool,
}
