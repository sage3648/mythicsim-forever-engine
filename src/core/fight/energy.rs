//! Go energy.go: the energy bar, its regeneration ticks and combo points.
//!
//! Go runs energy ticks as a simulation task, outside the pending-action queue. The step loop
//! checks tasks after weapon swings and before the next pending action, as Go `Step` does,
//! through [`Fight::due_task`] and [`Fight::advance_tasks`].

use crate::{
    contracts::prepared_v2::{ActionId, Energy},
    core::time::NEVER_EXPIRES,
};

use super::{log::action_string, Agent, Fight, ResourceKind};

/// Go `energyBar`.
#[derive(Clone, Debug)]
pub(crate) struct EnergyBar {
    pub(crate) max: f64,
    pub(crate) current: f64,
    pub(crate) max_combo_points: i32,
    pub(crate) combo_points: i32,
    pub(crate) next_tick: i64,
    pub(crate) tick_duration: i64,
    pub(crate) per_tick: f64,
    pub(crate) regen_multiplier: f64,
    pub(crate) regen_metrics: usize,
    pub(crate) refund_metrics: usize,
    pub(crate) encounter_start_metrics: usize,
}

impl<A: Agent> Fight<A> {
    /// Go `EnableEnergyBar`: the bar and its three metrics, registered in Go's order.
    pub(crate) fn enable_energy_bar(&mut self, energy: &Energy) {
        let other = |name: &str| ActionId {
            other_id: name.into(),
            ..ActionId::default()
        };
        let regen_metrics =
            self.new_resource_metrics(other("OtherActionEnergyRegen"), ResourceKind::Energy);
        let refund_metrics =
            self.new_resource_metrics(other("OtherActionRefund"), ResourceKind::Energy);
        let encounter_start_metrics =
            self.new_resource_metrics(other("OtherActionEncounterStart"), ResourceKind::Energy);
        self.energy = Some(EnergyBar {
            max: energy.max_energy.max(10.0),
            current: 0.0,
            max_combo_points: energy.max_combo_points,
            combo_points: 0,
            next_tick: 0,
            tick_duration: energy.tick_duration_ns,
            per_tick: energy.energy_per_tick,
            regen_multiplier: 1.0,
            regen_metrics,
            refund_metrics,
            encounter_start_metrics,
        });
    }

    /// The player's energy bar. Only a player with one casts energy spells.
    pub(crate) fn energy_bar(&self) -> &EnergyBar {
        self.energy.as_ref().expect("the player has an energy bar")
    }

    fn energy_bar_mut(&mut self) -> &mut EnergyBar {
        self.energy.as_mut().expect("the player has an energy bar")
    }

    /// Go `energyBar.reset` and `enable`: a full bar, no combo points and the first tick at a
    /// random point of one tick period after the prepull starts.
    pub(crate) fn reset_energy(&mut self, prepull_start: i64) {
        let Some(bar) = self.energy.as_mut() else {
            return;
        };
        bar.current = bar.max;
        bar.combo_points = 0;
        bar.regen_multiplier = 1.0;
        let duration = self.energy_bar().tick_duration;
        let roll = self.random("Energy Tick");
        let next = prepull_start + (roll * duration as f64) as i64;
        self.energy_bar_mut().next_tick = next;
        self.reschedule_task(next);
    }

    /// Go `energyBar.RunTask`.
    pub(crate) fn run_energy_task(&mut self) -> i64 {
        let bar = self.energy_bar();
        if self.now < bar.next_tick {
            return bar.next_tick;
        }
        let (amount, metrics) = (bar.per_tick * bar.regen_multiplier, bar.regen_metrics);
        self.add_energy(amount, metrics);
        let next = self.now + self.energy_bar().tick_duration;
        self.energy_bar_mut().next_tick = next;
        next
    }

    /// Go `energyBar.AddEnergy`.
    pub(crate) fn add_energy(&mut self, amount: f64, metrics: usize) {
        assert!(amount >= 0.0, "Trying to add negative energy!");
        let bar = self.energy_bar();
        let (old, max) = (bar.current, bar.max);
        let new = (old + amount).min(max);
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain += amount;
        resource.actual_gain += new - old;
        if self.log.is_some() {
            let line = format!(
                "Gained {amount:.3} energy from {} ({old:.3} --> {new:.3}) of {max:.0} total.",
                action_string(&self.resources[metrics].id)
            );
            self.player_log(&line);
        }
        self.energy_bar_mut().current = new;
    }

    /// Go `energyBar.SpendEnergy`.
    pub(crate) fn spend_energy(&mut self, amount: f64, metrics: usize) {
        assert!(amount >= 0.0, "Trying to spend negative energy!");
        let bar = self.energy_bar();
        let (old, max) = (bar.current, bar.max);
        let new = old - amount;
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain -= amount;
        resource.actual_gain -= amount;
        if self.log.is_some() {
            let line = format!(
                "Spent {amount:.3} energy from {} ({old:.3} --> {new:.3}) of {max:.0} total.",
                action_string(&self.resources[metrics].id)
            );
            self.player_log(&line);
        }
        self.energy_bar_mut().current = new;
    }

    /// Go `energyBar.AddComboPoints`. Go formats the int32 maximum with `%0.0f`, which prints
    /// as a bad verb.
    pub(crate) fn add_combo_points(&mut self, points: i32, metrics: usize) {
        let bar = self.energy_bar();
        let (old, max) = (bar.combo_points, bar.max_combo_points);
        let new = (old + points).min(max);
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain += f64::from(points);
        resource.actual_gain += f64::from(new - old);
        if self.log.is_some() {
            let line = format!(
                "Gained {points} combo points from {} ({old} --> {new}) of %!f(int32={max}) total.",
                action_string(&self.resources[metrics].id)
            );
            self.player_log(&line);
        }
        self.energy_bar_mut().combo_points = new;
    }

    /// Go `energyBar.SpendComboPoints`: every point.
    pub(crate) fn spend_combo_points(&mut self, metrics: usize) {
        let bar = self.energy_bar();
        let (old, max) = (bar.combo_points, bar.max_combo_points);
        let spent = old;
        let new = old - spent;
        if self.log.is_some() {
            let line = format!(
                "Spent {spent} combo points from {} ({old} --> {new}) of %!f(int32={max}) total.",
                action_string(&self.resources[metrics].id)
            );
            self.player_log(&line);
        }
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain -= f64::from(spent);
        resource.actual_gain -= f64::from(spent);
        self.energy_bar_mut().combo_points = new;
    }

    /// Go `energyBar.IsTicking`: the bar ticks once reset, within one period of its next tick.
    fn energy_ticking(&self) -> bool {
        let bar = self.energy_bar();
        bar.next_tick != 0
            && bar.next_tick - self.now <= bar.tick_duration
            && self.now <= bar.next_tick
    }

    /// Go `energyBar.ResetEnergyTick`: an immediate partial tick, then a fresh period.
    pub(crate) fn reset_energy_tick(&mut self) {
        if !self.energy_ticking() {
            return;
        }
        let bar = self.energy_bar();
        let since = (self.now - (bar.next_tick - bar.tick_duration)).max(0);
        let partial =
            bar.per_tick * bar.regen_multiplier * (since as f64 / bar.tick_duration as f64);
        let metrics = bar.regen_metrics;
        self.add_energy(partial, metrics);
        let next = self.now + self.energy_bar().tick_duration;
        self.energy_bar_mut().next_tick = next;
        self.reschedule_task(next);
    }

    /// Go `energyBar.MultiplyEnergyRegenSpeed`; a multiplier of one changes nothing.
    pub(crate) fn multiply_energy_regen_speed(&mut self, multiplier: f64) {
        if multiplier == 1.0 {
            return;
        }
        self.reset_energy_tick();
        self.energy_bar_mut().regen_multiplier *= multiplier;
    }

    /// Go `energyBar.TimeToNextEnergyTick`.
    pub(crate) fn time_to_next_energy_tick(&self) -> i64 {
        (self.energy_bar().next_tick - self.now).max(0)
    }

    /// Go `Simulation.RescheduleTask`.
    pub(crate) fn reschedule_task(&mut self, time: i64) {
        self.min_task_time = self.min_task_time.min(time);
    }

    /// Go `Simulation.Step`'s task check: whether a task is due before the next action at
    /// `next`.
    pub(crate) fn due_task(&self, next: i64) -> bool {
        next >= self.min_task_time
    }

    /// Go `advanceTasks`: the energy bar is the only task.
    pub(crate) fn advance_tasks(&mut self) {
        if self.min_task_time > self.now {
            let time = self.min_task_time;
            self.advance_to(time);
        }
        self.min_task_time = NEVER_EXPIRES;
        if self.energy.is_some() {
            let next = self.run_energy_task();
            self.min_task_time = self.min_task_time.min(next);
        }
    }
}
