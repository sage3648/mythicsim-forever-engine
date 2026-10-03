//! Go cast.go, spell.go, spell_queueing.go, gcd.go, mana.go and major_cooldown.go.

use crate::{
    contracts::prepared_v2::CastKind,
    core::time::{go_string, round, NS_PER_MILLISECOND, NS_PER_SECOND, STARTING_CD_TIME},
};

use super::{log::action_string, Action, Agent, Fight, Side, SpellBehavior, SpellId, PRIORITY_GCD};

/// Go `MaxSpellQueueWindow`.
const MAX_SPELL_QUEUE_WINDOW: i64 = 400 * NS_PER_MILLISECOND;

impl<A: Agent> Fight<A> {
    /// Go `ApplyCastSpeed`.
    pub(crate) fn apply_cast_speed(&self, duration: i64) -> i64 {
        (duration as f64 * self.config.cast_speed) as i64
    }

    /// Go `ApplyCastSpeedForSpell`.
    pub(crate) fn apply_cast_speed_for_spell(&self, duration: i64, spell: SpellId) -> i64 {
        (duration as f64
            * self.config.cast_speed
            * self.spells[spell].cast_time_multiplier.max(0.0)) as i64
    }

    /// Go `SpellCost.GetCurrentCost`, with Go's int32 percentage arithmetic.
    pub(crate) fn current_cost(&self, spell: SpellId) -> f64 {
        let Some(cost) = self.spells[spell].cost else {
            return 0.0;
        };
        let mut value = (cost.base + cost.flat_modifier).max(0);
        value = (value * self.player.spell_cost_percent_modifier / 100).max(0);
        (f64::from(value) * cost.percent_modifier * cost.additive_percent_modifier).max(0.0)
    }

    fn timer_ready(&self, timer: Option<(usize, i64)>) -> bool {
        timer.is_none_or(|(id, _)| self.timers[id] <= self.now)
    }

    /// Go `BothTimersReady` for a spell's cooldown and shared cooldown.
    pub(crate) fn spell_ready(&self, spell: SpellId) -> bool {
        self.timer_ready(self.spells[spell].cd) && self.timer_ready(self.spells[spell].shared_cd)
    }

    /// Go `Spell.ReadyAt` through `BothTimersReadyAt`: the first timer's value replaces the
    /// zero default, so an unused cooldown reads as Go's negative starting time.
    pub(crate) fn spell_ready_at(&self, spell: SpellId) -> i64 {
        let mut ready = 0;
        if let Some((timer, _)) = self.spells[spell].cd {
            ready = self.timers[timer];
        }
        if let Some((timer, _)) = self.spells[spell].shared_cd {
            ready = ready.max(self.timers[timer]);
        }
        ready
    }

    /// Go `MaxTimeToReady`.
    fn spell_time_to_ready(&self, spell: SpellId) -> i64 {
        let remaining = |timer: Option<(usize, i64)>| {
            timer.map_or(0, |(id, _)| (self.timers[id] - self.now).max(0))
        };
        remaining(self.spells[spell].cd).max(remaining(self.spells[spell].shared_cd))
    }

    fn gcd_ready(&self) -> bool {
        self.player.gcd <= self.now
    }

    fn extra_cast_condition(&self, spell: SpellId) -> bool {
        match &self.spells[spell].behavior {
            SpellBehavior::Class(behavior) if self.spells[spell].has_extra_cast_condition => {
                A::extra_cast_condition(self, spell, *behavior)
            }
            _ => true,
        }
    }

    /// Go `ManaCost.MeetsRequirement`, including its out-of-mana bookkeeping.
    fn meets_cost(&mut self, spell: SpellId) -> bool {
        if self.spells[spell].cost.is_none() {
            return true;
        }
        let cost = self.current_cost(spell);
        self.spells[spell].cur_cast.cost = cost;
        let meets = self.player.mana >= cost;
        if cost > 0.0 {
            if meets {
                if self.player.waiting_for_mana != 0.0 {
                    let duration = self.now - self.player.waiting_for_mana_start;
                    self.add_oom_time(duration);
                    self.player.waiting_for_mana_start = 0;
                    self.player.waiting_for_mana = 0.0;
                }
            } else if self.player.waiting_for_mana != 0.0 {
                self.player.waiting_for_mana = self.player.waiting_for_mana.min(cost);
            } else {
                self.player.waiting_for_mana_start = self.now;
                self.player.waiting_for_mana = cost;
                self.mark_oom();
            }
        }
        meets
    }

    fn add_oom_time(&mut self, duration: i64) {
        if duration > 0 {
            self.player.oom_time += duration;
            self.mark_oom();
        }
    }

    fn mark_oom(&mut self) {
        if !self.player.went_oom {
            self.player.went_oom = true;
            self.player.first_oom = self.now;
        }
    }

    fn cost_failure(&self, spell: SpellId) -> String {
        format!(
            "not enough mana (Current Mana = {:.3}, Mana Cost = {:.3})",
            self.player.mana, self.spells[spell].cur_cast.cost
        )
    }

    /// Go `Spell.CanCompleteCast`. Cost checks have side effects, as in Go.
    pub(crate) fn can_complete_cast(&mut self, spell: SpellId, log_failure: bool) -> bool {
        if !self.extra_cast_condition(spell) {
            if log_failure {
                self.cast_failure(spell, |_| "extra spell condition".into());
            }
            return false;
        }
        if !self.meets_cost(spell) {
            if log_failure {
                self.cast_failure(spell, |fight| fight.cost_failure(spell));
            }
            return false;
        }
        true
    }

    /// Go `Spell.CanCast`.
    pub(crate) fn can_cast(&mut self, spell: SpellId) -> bool {
        if !self.can_complete_cast(spell, false) {
            return false;
        }
        let state = &self.spells[spell];
        if state.flags.swapped {
            return false;
        }
        if self.player.hardcast.expires > self.now {
            return false;
        }
        let needs_gcd = state.default_cast.gcd > 0 || (state.flags.mcd && self.in_sequence());
        if needs_gcd && !self.gcd_ready() {
            return false;
        }
        self.spell_ready(spell)
    }

    fn in_sequence(&self) -> bool {
        false
    }

    /// Go `Spell.CanQueue`.
    fn can_queue(&mut self, spell: SpellId) -> bool {
        if !self.can_complete_cast(spell, false) {
            return false;
        }
        let state = &self.spells[spell];
        if state.flags.swapped {
            return false;
        }
        if state.flags.channeled && self.player.hardcast.expires > self.now + MAX_SPELL_QUEUE_WINDOW
        {
            return false;
        }
        if state.default_cast.gcd > 0
            && (self.player.gcd - self.now).max(0) > MAX_SPELL_QUEUE_WINDOW
        {
            return false;
        }
        self.spell_time_to_ready(spell) <= MAX_SPELL_QUEUE_WINDOW
    }

    /// Go `Unit.CanQueueSpell` and `Spell.CanQueue`.
    pub(crate) fn can_cast_or_queue(&mut self, spell: SpellId) -> bool {
        let can_queue_spell = self
            .player
            .queued
            .is_none_or(|queued| queued.initiated_at != self.now);
        can_queue_spell && self.can_queue(spell)
    }

    /// Go `Spell.CastOrQueue`.
    pub(crate) fn cast_or_queue(&mut self, spell: SpellId, target: Side) {
        if self.can_cast(spell) {
            self.cast(spell, target);
        } else if self.can_queue(spell) {
            let mut queue_time = self.player.hardcast.expires.max(self.spell_ready_at(spell));
            let state = &self.spells[spell];
            if state.default_cast.gcd > 0 || state.flags.mcd {
                queue_time = queue_time.max(self.player.gcd);
            }
            self.queue_spell(spell, target, queue_time);
        } else {
            self.cast(spell, target);
        }
    }

    /// Go `Unit.QueueSpell` and `QueuedSpell.InitiateQueue`.
    fn queue_spell(&mut self, spell: SpellId, target: Side, queue_at: i64) {
        self.cancel_queued_spell();
        let fire_at = queue_at + 1;
        let action = self.schedule(fire_at, PRIORITY_GCD, Action::QueuedCast);
        self.player.queued = Some(super::QueuedSpell {
            spell,
            target,
            action: Some(action),
            initiated_at: self.now,
        });
        if self.log.is_some() {
            let line = format!(
                "Queueing up {} to cast at {}.",
                action_string(&self.spells[spell].id),
                go_string(fire_at)
            );
            self.player_log(&line);
        }
    }

    /// Go `QueuedSpell.Cancel`: the queue initiation time becomes -NeverExpires.
    pub(crate) fn cancel_queued_spell(&mut self) {
        if let Some(queued) = self.player.queued.as_mut() {
            if let Some(action) = queued.action.take() {
                self.queue.cancel(action);
            }
            queued.initiated_at = -crate::core::time::NEVER_EXPIRES;
        }
    }

    /// Go `castFailureHelper`. The reason is only formatted when logging.
    fn cast_failure(&mut self, spell: SpellId, reason: impl FnOnce(&Self) -> String) -> bool {
        if self.now >= 0 && !self.spells[spell].flags.no_logs && self.log.is_some() {
            let reason = reason(self);
            let line = format!(
                "{} failed to cast: {reason}",
                action_string(&self.spells[spell].id)
            );
            self.player_log(&line);
        }
        false
    }

    /// Go `Spell.Cast`.
    pub(crate) fn cast(&mut self, spell: SpellId, target: Side) -> bool {
        if self.spells[spell].default_cast.effective_time() > 0 {
            self.cancel_queued_spell();
        }
        match self.spells[spell].cast_kind {
            CastKind::Full => self.cast_full(spell, target),
            CastKind::Simple => self.cast_simple(spell, target),
            CastKind::AutosOrProcs => {
                self.log_instant_cast(spell);
                self.apply_effects(spell, target);
                if !self.spells[spell].flags.no_on_cast_complete {
                    self.on_cast_complete(spell);
                }
                true
            }
        }
    }

    fn log_instant_cast(&mut self, spell: SpellId) {
        if self.log.is_some() && !self.spells[spell].flags.no_logs {
            let id = action_string(&self.spells[spell].id);
            self.player_log(&format!(
                "Casting {id} (Cost = 0.000, Cast Time = 0s, GCD = 0s, Effective Time = 0s)"
            ));
            self.player_log(&format!("Completed cast {id}"));
        }
    }

    /// Go `makeCastFuncSimple`.
    fn cast_simple(&mut self, spell: SpellId, target: Side) -> bool {
        if self.spells[spell].flags.swapped {
            return self.cast_failure(spell, |_| "spell attached to an un-equipped item".into());
        }
        if !self.extra_cast_condition(spell) {
            return self.cast_failure(spell, |_| "extra spell condition".into());
        }
        if let Some((timer, _)) = self.spells[spell].cd {
            if self.timers[timer] > self.now {
                return self.cast_failure(spell, |fight| {
                    format!(
                        "still on cooldown for {}, curTime = {}",
                        go_string((fight.timers[timer] - fight.now).max(0)),
                        go_string(fight.now)
                    )
                });
            }
        }
        if let Some((timer, _)) = self.spells[spell].shared_cd {
            if self.timers[timer] > self.now {
                return self.cast_failure(spell, |fight| {
                    format!(
                        "still on shared cooldown for {}, curTime = {}",
                        go_string((fight.timers[timer] - fight.now).max(0)),
                        go_string(fight.now)
                    )
                });
            }
        }
        self.log_instant_cast(spell);
        self.trigger_cooldowns(spell);
        self.apply_effects(spell, target);
        if !self.spells[spell].flags.no_on_cast_complete {
            self.on_cast_complete(spell);
        }
        true
    }

    /// Go `triggerCooldown` and the shared cooldown set.
    fn trigger_cooldowns(&mut self, spell: SpellId) {
        let state = &self.spells[spell];
        let multiplier = state.cd_multiplier;
        if let Some((timer, duration)) = state.cd {
            let cd = (duration as f64 * multiplier) as i64;
            if cd > 0 {
                self.timers[timer] = self.now + cd;
            }
        }
        if let Some((timer, duration)) = state.shared_cd {
            self.timers[timer] = self.now + (duration as f64 * multiplier) as i64;
        }
    }

    /// Go `makeCastFunc`.
    fn cast_full(&mut self, spell: SpellId, target: Side) -> bool {
        self.spells[spell].cur_cast = self.spells[spell].default_cast;
        if self.spells[spell].flags.swapped {
            return self.cast_failure(spell, |_| "spell attached to an un-equipped item".into());
        }
        if !self.extra_cast_condition(spell) {
            return self.cast_failure(spell, |_| "extra spell condition".into());
        }
        if self.spells[spell].cost.is_some() && !self.meets_cost(spell) {
            return self.cast_failure(spell, |fight| fight.cost_failure(spell));
        }
        // Go hastes both the GCD and the cast time unless the spell ignores haste. A spell
        // with no GCD and no cast time ignores it implicitly.
        let cur = self.spells[spell].cur_cast;
        let default_cast = self.spells[spell].default_cast;
        let implicit = default_cast.gcd <= 0 && default_cast.cast_time == 0;
        if !self.spells[spell].ignore_haste && !implicit {
            self.spells[spell].cur_cast.gcd =
                round(self.apply_cast_speed(cur.gcd).max(0), NS_PER_MILLISECOND);
            self.spells[spell].cur_cast.cast_time = round(
                self.apply_cast_speed_for_spell(cur.cast_time, spell),
                NS_PER_MILLISECOND,
            );
        }
        if let Some((timer, _)) = self.spells[spell].cd {
            if self.timers[timer] > self.now {
                return self.cast_failure(spell, |fight| {
                    format!(
                        "still on cooldown for {}, curTime = {}",
                        go_string((fight.timers[timer] - fight.now).max(0)),
                        go_string(fight.now)
                    )
                });
            }
        }
        if let Some((timer, _)) = self.spells[spell].shared_cd {
            if self.timers[timer] > self.now {
                return self.cast_failure(spell, |fight| {
                    format!(
                        "still on shared cooldown for {}, curTime = {}",
                        go_string((fight.timers[timer] - fight.now).max(0)),
                        go_string(fight.now)
                    )
                });
            }
        }
        if self.spells[spell].cur_cast.gcd > 0 && !self.gcd_ready() {
            return self.cast_failure(spell, |fight| {
                format!(
                    "GCD on cooldown for {}, curTime = {}",
                    go_string((fight.player.gcd - fight.now).max(0)),
                    go_string(fight.now)
                )
            });
        }
        if self.player.hardcast.expires > self.now {
            return self.cast_failure(spell, |fight| {
                let hardcast = fight.player.hardcast;
                let id = hardcast
                    .spell
                    .map(|s| action_string(&fight.spells[s].id))
                    .unwrap_or_else(|| "{}".into());
                format!(
                    "casting/channeling {id} for {}, curTime = {}",
                    go_string(hardcast.expires - fight.now),
                    go_string(fight.now)
                )
            });
        }

        let cur = self.spells[spell].cur_cast;
        let channeled = self.spells[spell].flags.channeled;
        let effective = cur.effective_time();
        if effective != 0 {
            if !channeled {
                self.spells[spell].metrics[target.index()].total_cast_time += effective;
            }
            let ready = (self.now + effective).max(self.player.gcd);
            self.set_gcd_timer(ready);
        }

        if cur.cast_time > 0 {
            if self.log.is_some() && !self.spells[spell].flags.no_logs {
                self.log_casting(spell);
            }
            self.player.hardcast = super::Hardcast {
                expires: self.now + cur.cast_time,
                spell: Some(spell),
                target,
            };
            self.new_hardcast_action();
            return true;
        }

        if self.log.is_some() && !self.spells[spell].flags.no_logs {
            self.log_casting(spell);
            let line = format!("Completed cast {}", action_string(&self.spells[spell].id));
            self.player_log(&line);
        }
        self.spend_cost(spell);
        self.trigger_cooldowns(spell);
        self.apply_effects(spell, target);
        if !self.spells[spell].flags.no_on_cast_complete {
            self.on_cast_complete(spell);
        }
        true
    }

    fn log_casting(&mut self, spell: SpellId) {
        let cur = self.spells[spell].cur_cast;
        let line = format!(
            "Casting {} (Cost = {:.3}, Cast Time = {}, GCD = {}, Effective Time = {})",
            action_string(&self.spells[spell].id),
            cur.cost.max(0.0),
            go_string(cur.cast_time),
            go_string(cur.gcd_time().max(0)),
            go_string(cur.effective_time())
        );
        self.player_log(&line);
    }

    /// The hardcast `OnComplete` closure from `makeCastFunc`.
    fn complete_hardcast(&mut self, spell: SpellId, target: Side) {
        if self.log.is_some() && !self.spells[spell].flags.no_logs {
            let line = format!("Completed cast {}", action_string(&self.spells[spell].id));
            self.player_log(&line);
        }
        if !self.can_complete_cast(spell, true) {
            return;
        }
        self.spend_cost(spell);
        self.trigger_cooldowns(spell);
        self.apply_effects(spell, target);
        if !self.spells[spell].flags.no_on_cast_complete {
            self.on_cast_complete(spell);
        }
    }

    /// Run a due hardcast's completion, as both the rotation and hardcast actions do.
    pub(crate) fn complete_due_hardcast(&mut self) {
        let hardcast = self.player.hardcast;
        if hardcast.expires != STARTING_CD_TIME && hardcast.expires <= self.now {
            self.player.hardcast.expires = STARTING_CD_TIME;
            if let Some(spell) = hardcast.spell {
                self.complete_hardcast(spell, hardcast.target);
            }
        }
    }

    /// Go `ManaCost.SpendCost`.
    fn spend_cost(&mut self, spell: SpellId) {
        if self.spells[spell].cost.is_none() {
            return;
        }
        let cost = self.spells[spell].cur_cast.cost;
        if cost > 0.0 {
            let metrics = self.spells[spell].mana_metrics.expect("mana metrics");
            self.spend_mana(cost, metrics);
            self.player.five_second_rule_refresh =
                (self.now + 5 * NS_PER_SECOND).max(self.player.hardcast.expires);
        }
    }

    /// Go `Spell.applyEffects`.
    pub(crate) fn apply_effects(&mut self, spell: SpellId, target: Side) {
        self.spells[spell].metrics[target.index()].casts += 1;
        match self.spells[spell].behavior.clone() {
            SpellBehavior::Class(behavior) => A::apply_effects(self, spell, target, behavior),
            SpellBehavior::PotionMana {
                label,
                min,
                spread,
                stone_multiplier,
                ..
            } => {
                let gain = (min + self.random(&label) * spread) * stone_multiplier;
                let metrics = self.item_metrics(spell);
                self.execute_mana_gain(gain, metrics);
            }
            SpellBehavior::ConjuredMana {
                label, min, spread, ..
            } => {
                // Go's TernaryFloat64 evaluates both arguments, so the roll always happens.
                let rolled = self.random(&label) * spread;
                let gain = min + if spread > 1.0 { rolled } else { spread };
                let metrics = self.item_metrics(spell);
                self.execute_mana_gain(gain, metrics);
            }
            SpellBehavior::EnergizeOnUse {
                average, variance, ..
            } => {
                let gain = if variance == 0.0 {
                    average
                } else {
                    let (low, high) = (
                        average * (1.0 - variance / 2.0),
                        average * (1.0 + variance / 2.0),
                    );
                    low + (high - low) * self.random("Damage Roll")
                };
                let metrics = self.item_metrics(spell);
                self.add_mana(gain, metrics);
            }
            SpellBehavior::None => panic!("spell {} has no behavior", self.spells[spell].id),
        }
    }

    /// Go `ExecuteResourceGain` for mana: only a positive amount is gained.
    fn execute_mana_gain(&mut self, amount: f64, metrics: usize) {
        if amount > 0.0 {
            self.add_mana(amount, metrics);
        }
    }

    /// The mana metrics registered for an item or class spell action.
    pub(crate) fn item_metrics(&mut self, spell: SpellId) -> usize {
        if let Some(metrics) = self.spells[spell].mana_metrics {
            return metrics;
        }
        let id = self.spells[spell].id.clone();
        self.resources.push(super::ResourceMetrics {
            id,
            events: 0,
            gain: 0.0,
            actual_gain: 0.0,
            previous_events: 0,
            previous_actual_gain: 0.0,
            is_mana_regen: false,
        });
        let index = self.resources.len() - 1;
        self.spells[spell].mana_metrics = Some(index);
        index
    }

    /// Go `Unit.SetGCDTimer`.
    pub(crate) fn set_gcd_timer(&mut self, ready: i64) {
        self.player.gcd = ready;
        self.set_rotation_timer(ready);
    }

    /// Go `Unit.SetRotationTimer`.
    pub(crate) fn set_rotation_timer(&mut self, ready: i64) {
        self.player.rotation_timer = ready;
        if let Some(action) = self.player.rotation_action.take() {
            self.queue.cancel(action);
        }
        self.player.rotation_action = Some(self.schedule(ready, PRIORITY_GCD, Action::Rotation));
    }

    /// Go `Unit.WaitUntil`.
    pub(crate) fn wait_until(&mut self, ready: i64) {
        assert!(ready >= self.now, "cannot wait negative time");
        self.set_rotation_timer(ready);
        if self.log.is_some() && ready > self.now {
            let line = format!(
                "Pausing rotation for {} due to resources / CDs.",
                go_string(ready - self.now)
            );
            self.player_log(&line);
        }
    }

    /// Go `Unit.newHardcastAction`.
    fn new_hardcast_action(&mut self) {
        if let Some(action) = self.player.hardcast_action.take() {
            self.queue.cancel(action);
        }
        let expires = self.player.hardcast.expires;
        self.player.hardcast_action = Some(self.schedule(expires, PRIORITY_GCD, Action::Hardcast));
    }

    /// Go `Unit.AddMana`.
    pub(crate) fn add_mana(&mut self, amount: f64, metrics: usize) {
        assert!(amount >= 0.0, "negative mana gain");
        let old = self.player.mana;
        let new = (old + amount).min(self.config.max_mana);
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain += amount;
        resource.actual_gain += new - old;
        if self.log.is_some() {
            let line = format!(
                "Gained {amount:.3} mana from {} ({old:.3} --> {new:.3}) of {:.0} total.",
                action_string(&self.resources[metrics].id),
                self.config.max_mana
            );
            self.player_log(&line);
        }
        self.player.mana = new;
        self.player.mana_gained += new - old;
    }

    /// Go `Unit.SpendMana`.
    fn spend_mana(&mut self, amount: f64, metrics: usize) {
        let new = self.player.mana - amount;
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain -= amount;
        resource.actual_gain -= amount;
        if self.log.is_some() {
            let line = format!(
                "Spent {amount:.3} mana from {} ({:.3} --> {new:.3}) of {:.0} total.",
                action_string(&self.resources[metrics].id),
                self.player.mana,
                self.config.max_mana
            );
            self.player_log(&line);
        }
        self.player.mana = new;
        self.player.mana_spent += amount;
    }

    /// Go `Unit.ManaTick`. Spirit regeneration attribution is not modeled.
    pub(crate) fn mana_tick(&mut self) {
        let casting = self.now < self.player.five_second_rule_refresh;
        let (regen, metrics) = if casting {
            (self.player.mana_tick_casting, self.mana_regen_casting)
        } else {
            (
                self.player.mana_tick_not_casting,
                self.mana_regen_not_casting,
            )
        };
        self.add_mana(regen.max(0.0), metrics);
    }

    /// Go `majorCooldownManager.reset`: copies in initial order, then a stable sort.
    pub(crate) fn reset_cooldown_manager(&mut self) {
        for cooldown in &mut self.major_cooldowns {
            cooldown.uses = 0;
        }
        self.cooldown_order = (0..self.major_cooldowns.len()).collect();
        self.update_major_cooldowns();
    }

    /// Go `UpdateMajorCooldowns`: sort by ready time, then higher priority first.
    pub(crate) fn update_major_cooldowns(&mut self) {
        if self.cooldown_order.is_empty() {
            self.cooldown_min_ready = crate::core::time::NEVER_EXPIRES;
            return;
        }
        let mut order = std::mem::take(&mut self.cooldown_order);
        order.sort_by(|&a, &b| {
            let (ra, rb) = (
                self.spell_ready_at(self.major_cooldowns[a].spell),
                self.spell_ready_at(self.major_cooldowns[b].spell),
            );
            ra.cmp(&rb).then(
                self.major_cooldowns[b]
                    .priority
                    .cmp(&self.major_cooldowns[a].priority),
            )
        });
        self.cooldown_min_ready = self.spell_ready_at(self.major_cooldowns[order[0]].spell);
        self.cooldown_order = order;
    }

    /// Go `MajorCooldown.ShouldActivate` for the non-class cooldowns.
    fn cooldown_should_activate(&self, spell: SpellId) -> bool {
        let max = self.config.max_mana;
        let mana = self.player.mana;
        let casting_regen = crate::mechanics::mana::regen_per_second_casting(self.regen_inputs());
        match &self.spells[spell].behavior {
            SpellBehavior::Class(behavior) => A::should_activate(self, spell, *behavior),
            SpellBehavior::PotionMana {
                min,
                spread,
                stone_multiplier,
                regen_window,
                ..
            } => {
                let total_regen = casting_regen * regen_window;
                let gain = (min + spread) * stone_multiplier;
                max - (mana + total_regen) >= gain
            }
            SpellBehavior::ConjuredMana {
                min,
                spread,
                selected,
                regen_window,
                ..
            } => {
                let total_regen = casting_regen * regen_window;
                max - (mana + total_regen) >= min + spread && *selected
            }
            SpellBehavior::EnergizeOnUse { whole, .. } => max - mana >= *whole,
            SpellBehavior::None => false,
        }
    }

    /// Go `shouldActivateHelper`.
    fn should_activate_helper(&mut self, cooldown: usize) -> bool {
        let spell = self.major_cooldowns[cooldown].spell;
        if !self.can_cast(spell) {
            return false;
        }
        let uses = self.major_cooldowns[cooldown].uses;
        if let Some(&timing) = self.major_cooldowns[cooldown].timings.get(uses) {
            return self.now >= timing;
        }
        self.cooldown_should_activate(spell)
    }

    /// Go `getFirstReadyMCD`.
    pub(crate) fn first_ready_cooldown(&mut self) -> Option<usize> {
        if self.now < self.cooldown_min_ready {
            return None;
        }
        for position in 0..self.cooldown_order.len() {
            let cooldown = self.cooldown_order[position];
            if !self.spell_ready(self.major_cooldowns[cooldown].spell) {
                continue;
            }
            if self.should_activate_helper(cooldown) {
                return Some(cooldown);
            }
        }
        None
    }

    /// Go `APLActionAutocastOtherCooldowns.IsReady`.
    pub(crate) fn autocast_ready(&mut self) -> Option<usize> {
        let cooldown = self.first_ready_cooldown()?;
        let spell = self.major_cooldowns[cooldown].spell;
        let explosive = self.major_cooldowns[cooldown].explosive;
        (self.gcd_ready() != explosive || self.spells[spell].flags.reactive).then_some(cooldown)
    }

    /// Go `tryActivateHelper` followed by `UpdateMajorCooldowns`.
    pub(crate) fn autocast(&mut self, cooldown: usize) {
        if self.should_activate_helper(cooldown) {
            let spell = self.major_cooldowns[cooldown].spell;
            let target = if self.spells[spell].flags.helpful {
                Side::Player
            } else {
                Side::Target
            };
            self.cast(spell, target);
            self.major_cooldowns[cooldown].uses += 1;
            if self.log.is_some() {
                let line = format!(
                    "Major cooldown used: {}",
                    action_string(&self.spells[spell].id)
                );
                self.player_log(&line);
            }
        }
        self.update_major_cooldowns();
    }
}
