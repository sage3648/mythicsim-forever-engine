//! Go focus.go: a pet's focus bar and its regeneration task.
//!
//! A pet's bar fills and starts ticking when the pet is enabled: the first tick comes at a
//! random point of one tick period, then one every period. Go runs the ticks as a simulation
//! task, like the energy ticks, and its regeneration records no metrics, only a log line.

use crate::{contracts::prepared_v2::PetFocus, core::time::NEVER_EXPIRES};

use super::{log::action_string, Agent, Fight, Side};

/// Go `focusBar`.
#[derive(Clone, Debug)]
pub(crate) struct FocusBar {
    pub(crate) max: f64,
    pub(crate) current: f64,
    pub(crate) per_tick: f64,
    pub(crate) tick_duration: i64,
    pub(crate) next_tick: i64,
}

impl FocusBar {
    pub(crate) fn new(focus: &PetFocus) -> Self {
        FocusBar {
            max: focus.max,
            current: focus.max,
            per_tick: focus.regen_per_tick,
            tick_duration: focus.tick_duration_ns,
            next_tick: NEVER_EXPIRES,
        }
    }
}

impl<A: Agent> Fight<A> {
    fn pet_focus(&self) -> Option<&FocusBar> {
        self.pet.as_ref().and_then(|pet| pet.focus.as_ref())
    }

    fn pet_focus_mut(&mut self) -> &mut FocusBar {
        self.pet
            .as_mut()
            .and_then(|pet| pet.focus.as_mut())
            .expect("the pet has a focus bar")
    }

    /// The pet's current focus.
    pub(crate) fn current_focus(&self) -> f64 {
        self.pet_focus().map_or(0.0, |bar| bar.current)
    }

    /// Go `focusBar.reset` and `enable` when the pet is enabled: a full bar, and the first tick
    /// at a random point of one period from now.
    pub(crate) fn enable_pet_focus(&mut self) {
        if self.pet_focus().is_none() {
            return;
        }
        let duration = self.pet_focus_mut().tick_duration;
        self.pet_focus_mut().current = self.pet_focus_mut().max;
        let roll = self.random("Focus Tick");
        let next = self.now + (roll * duration as f64) as i64;
        self.pet_focus_mut().next_tick = next;
        self.reschedule_task(next);
    }

    /// Go `focusBar.RunTask`.
    pub(crate) fn run_focus_task(&mut self) -> i64 {
        let now = self.now;
        let bar = self.pet_focus_mut();
        if now < bar.next_tick {
            return bar.next_tick;
        }
        let per_tick = bar.per_tick;
        self.add_focus(per_tick);
        let now = self.now;
        let bar = self.pet_focus_mut();
        bar.next_tick = now + bar.tick_duration;
        bar.next_tick
    }

    /// The pet's focus task, while it is enabled.
    pub(crate) fn pet_focus_task_due(&self) -> Option<i64> {
        self.pet_focus().map(|bar| bar.next_tick)
    }

    /// Go `focusBar.AddFocus` with the regeneration metrics, which record no event.
    fn add_focus(&mut self, amount: f64) {
        let bar = self.pet_focus_mut();
        let old = bar.current;
        let new = (old + amount).min(bar.max);
        let max = bar.max;
        if old != new && self.log.is_some() {
            let id = crate::contracts::prepared_v2::ActionId {
                other_id: "OtherActionFocusRegen".into(),
                ..Default::default()
            };
            let line = format!(
                "Gained {amount:.3} focus from {} ({old:.3} --> {new:.3}) of {max:.0} total.",
                action_string(&id)
            );
            self.unit_log(Side::Pet, &line);
        }
        self.pet_focus_mut().current = new;
    }

    /// Go `focusBar.SpendFocus`.
    pub(crate) fn spend_focus(&mut self, amount: f64, metrics: usize) {
        let bar = self.pet_focus_mut();
        let old = bar.current;
        let new = old - amount;
        let max = bar.max;
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain -= amount;
        resource.actual_gain -= amount;
        if self.log.is_some() {
            let line = format!(
                "Spent {amount:.3} focus from {} ({old:.3} --> {new:.3}) of {max:.0} total.",
                action_string(&self.resources[metrics].id)
            );
            self.unit_log(Side::Pet, &line);
        }
        self.pet_focus_mut().current = new;
    }
}
