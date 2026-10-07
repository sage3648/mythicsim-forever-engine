//! Go sim/core/energy.go: the energy bar a class enables, as far as construction and a reset
//! leave it.

use super::sim::{PowerBar, Sim, UnitId, MILLISECOND};

/// Go `EnergyBarOptions`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct EnergyBarOptions {
    pub max_combo_points: i32,
    pub max_energy: f64,
    pub has_no_regen: bool,
}

impl Sim {
    /// Go `Unit.EnableEnergyBar`.
    pub(crate) fn enable_energy_bar(&mut self, unit: UnitId, options: EnergyBarOptions) {
        let u = self.unit_mut(unit);
        u.current_power_bar = PowerBar::Energy;
        u.energy_bar.enabled = true;
        u.energy_bar.max_energy = options.max_energy.max(10.0);
        u.energy_bar.current_energy = 0.0;
        u.energy_bar.max_combo_points = options.max_combo_points;
        // Energy refills smoothly, 1 every 100 ms, not in Classic's 2020 ms ticks of 20.2.
        u.energy_bar.tick_duration = 100 * MILLISECOND;
        u.energy_bar.energy_per_tick = 1.0;
        u.energy_bar.energy_regen_multiplier = 1.0;
        u.energy_bar.has_no_regen = options.has_no_regen;
    }

    /// Go `Unit.HasEnergyBar`.
    pub(crate) fn has_energy_bar(&self, unit: UnitId) -> bool {
        self.unit(unit).energy_bar.enabled
    }

    /// Go `energyBar.reset`: a full bar and a plain regeneration rate. The tick it schedules
    /// only exists in a fight.
    pub(crate) fn reset_energy_bar(&mut self, unit: UnitId) {
        let bar = &mut self.unit_mut(unit).energy_bar;
        if !bar.enabled {
            return;
        }
        bar.current_energy = bar.max_energy;
        bar.energy_regen_multiplier = 1.0;
    }
}
