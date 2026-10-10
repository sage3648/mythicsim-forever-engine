//! Go rage.go: the rage bar, rage from landed white hits, rage costs, refunds and the threat
//! rage gains generate at the end of each fight.

use super::{
    log::action_string, melee::Hand, Agent, Fight, ResourceKind, Side, SpellId, SpellResult,
};

/// Go `DamageTakenRageFactor`.
const DAMAGE_TAKEN_RAGE_FACTOR: f64 = 10.0;

/// Go `rageBar`, with the per-hit rage the exporter resolved from the weapons.
#[derive(Clone, Debug)]
pub(crate) struct RageBar {
    pub(crate) max: f64,
    pub(crate) starting: f64,
    pub(crate) current: f64,
    /// The rage a landed main hand or off hand white hit gives before a crit's multiplier.
    pub(crate) main_hand_rage: f64,
    pub(crate) off_hand_rage: f64,
    pub(crate) crit_multiplier: f64,
    pub(crate) threat_per_rage: f64,
    pub(crate) damage_taken_metrics: usize,
    pub(crate) refund_metrics: usize,
    /// Go `rageBar.damageTakenRageMultiplier`, reset each fight.
    pub(crate) damage_taken_multiplier: f64,
    /// Go `OtherActionRageGain`, the spell that holds rage gain threat.
    pub(crate) gain_spell: Option<SpellId>,
    /// Go `GetCurrentPowerBar() == RageBar`: a druid out of Bear Form gains no rage from hits.
    pub(crate) in_use: bool,
}

impl<A: Agent> Fight<A> {
    /// The player's rage bar. Only a player with one casts rage spells.
    pub(crate) fn rage_bar(&self) -> &RageBar {
        self.rage.as_ref().expect("the player has a rage bar")
    }

    /// Go `CurrentRage`.
    pub(crate) fn current_rage(&self) -> f64 {
        self.rage_bar().current
    }

    /// Go `MaximumRage`.
    pub(crate) fn maximum_rage(&self) -> f64 {
        self.rage_bar().max
    }

    /// Go `SetCurrentPowerBar`: whether the rage bar is the unit's current power bar.
    pub(crate) fn set_rage_bar_in_use(&mut self, in_use: bool) {
        if let Some(bar) = self.rage.as_mut() {
            bar.in_use = in_use;
        }
    }

    /// Go `rageBar.reset`.
    pub(crate) fn reset_rage(&mut self) {
        if let Some(bar) = self.rage.as_mut() {
            bar.current = bar.starting;
            bar.damage_taken_multiplier = 1.0;
        }
    }

    /// Go `rageBar.MultiplyDamageTakenRageGen`.
    pub(crate) fn multiply_damage_taken_rage(&mut self, multiplier: f64) {
        if let Some(bar) = self.rage.as_mut() {
            bar.damage_taken_multiplier *= multiplier;
        }
    }

    /// Go `Unit.NewRageMetrics`: every call registers a new metric.
    pub(crate) fn new_rage_metrics(
        &mut self,
        id: crate::contracts::prepared_v2::ActionId,
    ) -> usize {
        self.new_resource_metrics(id, ResourceKind::Rage)
    }

    /// Go `rageBar.AddRage`, which lets the rotation react.
    pub(crate) fn add_rage(&mut self, amount: f64, metrics: usize) {
        assert!(amount >= 0.0, "Trying to add negative rage!");
        let bar = self.rage_bar();
        let (old, max) = (bar.current, bar.max);
        let new = (old + amount).min(max);
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain += amount;
        resource.actual_gain += new - old;
        if self.log.is_some() {
            let line = format!(
                "Gained {amount:.3} rage from {} ({old:.3} --> {new:.3}) of {max:.0} total.",
                action_string(&self.resources[metrics].id)
            );
            self.player_log(&line);
        }
        self.rage
            .as_mut()
            .expect("the player has a rage bar")
            .current = new;
        self.changed(super::reads::RESOURCES);
        self.react_to_event(Side::Player);
    }

    /// Go `rageBar.SpendRage`.
    pub(crate) fn spend_rage(&mut self, amount: f64, metrics: usize) {
        assert!(amount >= 0.0, "Trying to spend negative rage!");
        let bar = self.rage_bar();
        let (old, max) = (bar.current, bar.max);
        let new = old - amount;
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain += -amount;
        resource.actual_gain += -amount;
        if self.log.is_some() {
            let line = format!(
                "Spent {amount:.3} rage from {} ({old:.3} --> {new:.3}) of {max:.0} total.",
                action_string(&self.resources[metrics].id)
            );
            self.player_log(&line);
        }
        self.rage
            .as_mut()
            .expect("the player has a rage bar")
            .current = new;
        self.changed(super::reads::RESOURCES);
    }

    /// Go `RageCost.IssueRefund`: the refund share of the cost paid.
    pub(crate) fn issue_rage_refund(&mut self, spell: SpellId) {
        let Some(cost) = self.spells[spell].cost else {
            return;
        };
        let paid = self.spells[spell].cur_cast.cost;
        if cost.refund > 0.0 && paid > 0.0 {
            let metrics = self.rage_bar().refund_metrics;
            self.add_rage(cost.refund * paid, metrics);
        }
    }

    /// The "RageBar" aura's OnSpellHitDealt: a landed white hit gives rage, scaled by its
    /// hand and raised on a crit, credited to the swing's cost or its own rage metrics.
    pub(crate) fn rage_bar_hit_dealt(&mut self, spell: SpellId, result: &SpellResult) {
        if !self.rage_bar().in_use || !result.landed() {
            return;
        }
        let bar = self.rage_bar();
        let mut generated = match self.spells[spell].white_hand {
            Some(Hand::Main) => bar.main_hand_rage,
            Some(Hand::Off) => bar.off_hand_rage,
            Some(Hand::Enemy | Hand::Ranged) | None => return,
        };
        if result.crit() {
            generated *= bar.crit_multiplier;
        }
        let metrics = match self.spells[spell].cost.map(|cost| cost.kind) {
            Some(_) => self.spells[spell].mana_metrics.expect("cost metrics"),
            None => match self.spells[spell].rage_metrics {
                Some(metrics) => metrics,
                None => {
                    let metrics = self.new_rage_metrics(self.spells[spell].id.clone());
                    self.spells[spell].rage_metrics = Some(metrics);
                    metrics
                }
            },
        };
        self.add_rage(generated, metrics);
    }

    /// The "RageBar" aura's OnSpellHitTaken: a landed hit gives rage from its damage before
    /// armor and resistance, against maximum health.
    pub(crate) fn rage_bar_hit_taken(&mut self, result: &SpellResult) {
        if !self.rage_bar().in_use || !result.landed() {
            return;
        }
        let (post, multiplier) = self.player_hit_resistance;
        let mut pre_armor = post;
        if multiplier > 0.0 {
            pre_armor /= multiplier;
        }
        let bar = self.rage_bar();
        let generated = pre_armor * DAMAGE_TAKEN_RAGE_FACTOR / self.player_max_health()
            * bar.damage_taken_multiplier;
        let metrics = bar.damage_taken_metrics;
        self.add_rage(generated, metrics);
    }

    /// Go `rageBar.doneIteration`: rage gained from anything but damage taken, refunds and
    /// spells that dealt damage this fight counts as casts and threat of the rage gain spell.
    pub(crate) fn rage_done_iteration(&mut self) {
        let Some(bar) = self.rage.as_ref() else {
            return;
        };
        let (gain_spell, threat_per_rage) = (bar.gain_spell, bar.threat_per_rage);
        let Some(gain_spell) = gain_spell else {
            return;
        };
        for index in 0..self.resources.len() {
            let resource = &self.resources[index];
            if resource.kind != ResourceKind::Rage
                || resource.no_threat
                || matches!(
                    resource.id.other_id.as_str(),
                    "OtherActionDamageTaken" | "OtherActionRefund"
                )
            {
                continue;
            }
            let actual = resource.actual_gain - resource.previous_actual_gain;
            if actual <= 0.0 {
                continue;
            }
            // Go GetSpell: the first registered spell with the metric's action ID.
            let id = resource.id.clone();
            let source = self.spells.iter().position(|spell| spell.id == id);
            if source.is_some_and(|source| self.spells[source].metrics[0].total_damage > 0.0) {
                continue;
            }
            let events = self.resources[index].events - self.resources[index].previous_events;
            self.spells[gain_spell].metrics[0].casts += events;
            // Go ApplyAOEThreatIgnoreMultipliers on every target, whose sum the arm64 build
            // fuses.
            for position in 0..self.targets.len() {
                let slot = Side::target(position).index();
                let threat = &mut self.spells[gain_spell].metrics[slot].total_threat;
                *threat = actual.mul_add(threat_per_rage, *threat);
            }
        }
    }
}
