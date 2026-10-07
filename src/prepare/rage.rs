//! Go sim/core/rage.go: the rage bar a class enables, as far as construction and a reset
//! leave it.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;

use super::sim::{AuraConfig, EventCallbacks, PowerBar, Sim, UnitId, NEVER_EXPIRES};
use super::spell::SpellConfig;

/// Go `ThreatPerRageGained`.
pub(crate) const THREAT_PER_RAGE_GAINED: f64 = 5.0;
/// Go `CritRageMultiplier`.
pub(crate) const CRIT_RAGE_MULTIPLIER: f64 = 1.75;
/// Go `BaseRageHitFactor`.
pub(crate) const BASE_RAGE_HIT_FACTOR: f64 = 3.46;
/// Go `TwoHandRageHitFactor`.
pub(crate) const TWO_HAND_RAGE_HIT_FACTOR: f64 = 4.5;
/// Go `TwoHandRageMultiplier`.
pub(crate) const TWO_HAND_RAGE_MULTIPLIER: f64 = TWO_HAND_RAGE_HIT_FACTOR / BASE_RAGE_HIT_FACTOR;

/// Go `RageBarOptions`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RageBarOptions {
    pub max_rage: f64,
    pub starting_rage: f64,
    pub base_rage_multiplier: f64,
}

impl Sim {
    /// Go `Unit.EnableRageBar`: the `RageBar` aura, the spell that holds the rage gain's
    /// metrics and the bar.
    pub(crate) fn enable_rage_bar(&mut self, unit: UnitId, options: RageBarOptions) {
        self.unit_mut(unit).current_power_bar = PowerBar::Rage;
        self.register_aura(
            unit,
            AuraConfig {
                label: "RageBar".to_string(),
                duration: NEVER_EXPIRES,
                on_reset: Some(Rc::new(|sim: &mut Sim, aura| sim.activate(aura))),
                events: EventCallbacks {
                    on_spell_hit_dealt: true,
                    on_spell_hit_taken: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        self.register_spell(
            unit,
            SpellConfig {
                action_id: ActionId {
                    other_id: "OtherActionRageGain".to_string(),
                    ..ActionId::default()
                },
                ..Default::default()
            },
        );
        let max_rage = options.max_rage.max(100.0);
        let bar = &mut self.unit_mut(unit).rage_bar;
        bar.enabled = true;
        bar.max_rage = max_rage;
        bar.starting_rage = f64::max(0.0, f64::min(options.starting_rage, max_rage));
        bar.current_rage = 0.0;
        bar.off_hand_rage_multiplier = 1.0;
        bar.base_rage_multiplier = options.base_rage_multiplier;
    }

    /// Go `Unit.HasRageBar`.
    pub(crate) fn has_rage_bar(&self, unit: UnitId) -> bool {
        self.unit(unit).rage_bar.enabled
    }

    /// Go `rageBar.reset`.
    pub(crate) fn reset_rage_bar(&mut self, unit: UnitId) {
        let bar = &mut self.unit_mut(unit).rage_bar;
        if !bar.enabled {
            return;
        }
        bar.current_rage = bar.starting_rage;
    }
}
