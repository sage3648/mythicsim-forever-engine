//! Rip (9896), from Go sim/druid/rip.go: a finisher whose bleed ticks a flat amount per combo
//! point plus an attack power share per point, up to its cap. The bleed snapshots the flat
//! amount but reads the share from the attack power each tick has, and ticks on the current
//! attacker multiplier, as Go `SnapshotPhysical` and `SnapshotAttackPowerShare` do. Applying
//! it logs its projected power at five combo points.

use crate::core::fight::{melee::PhysicalOutcome, DotId, Fight, Side, SpellId};

use super::super::agent::DruidAgent;

#[derive(Clone, Debug)]
pub(crate) struct Rip {
    pub(crate) tick_base: f64,
    pub(crate) tick_per_combo_point: f64,
    pub(crate) share_per_combo_point: f64,
    pub(crate) share_max_points: f64,
    pub(crate) tick_can_crit: bool,
    pub(crate) expected_combo_points: f64,
    pub(crate) short_name: String,
}

/// The bleed's stored amounts: Go `SnapshotBaseDamage`, `attackPowerShare` and
/// `snapshotAttackPower`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Snapshot {
    base: f64,
    share: f64,
    attack_power: f64,
}

impl Rip {
    /// `ExtraCastCondition`: at least one combo point.
    pub(crate) fn can_cast(&self, fight: &Fight<DruidAgent>) -> bool {
        fight.energy_bar().combo_points > 0
    }

    fn flat_damage(&self, combo_points: f64) -> f64 {
        self.tick_base + self.tick_per_combo_point * combo_points
    }

    fn share(&self, combo_points: f64) -> f64 {
        self.share_per_combo_point * combo_points.min(self.share_max_points)
    }

    /// `ApplyEffects`: the bleed and the combo points spent on a landed outcome, a refund
    /// otherwise, then the outcome.
    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side) {
        let result = fight.calc_physical_outcome(
            spell,
            target,
            PhysicalOutcome::MeleeSpecialHit { count: false },
        );
        if result.landed() {
            let dot = fight.spells[spell].dot.expect("Rip has a dot");
            // Go Dot.Apply: the running copy ends, the dot snapshots, then it starts again.
            let aura = fight.dots[dot].aura;
            fight.deactivate_aura(aura);
            self.snapshot(fight, spell, target);
            fight.apply_dot(dot);
            let (_, combo) = fight.spells[spell].energy_metrics.expect("energy cost");
            fight.spend_combo_points(combo);
        } else {
            fight.issue_refund(spell);
        }
        fight.deal_damage(spell, result, false);
    }

    /// `OnSnapshot`, then `UpdateBleedPower`'s log of the projected tick.
    fn snapshot(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side) {
        let combo_points = f64::from(fight.energy_bar().combo_points);
        let share = self.share(combo_points);
        let attack_power = fight.melee_attack_power();
        fight.agent.rip_snapshot = Snapshot {
            base: self.flat_damage(combo_points) + share * attack_power,
            share,
            attack_power: share * attack_power,
        };
        let power = self.expected_tick(fight, spell, target);
        if fight.log.is_some() {
            let line = format!("{} Snapshot Power: {power:.1}", self.short_name);
            fight.player_log(&line);
        }
    }

    /// `ExpectedTickDamage` without the snapshot: a tick at the projected combo points on the
    /// current stats, always hitting, scaled by the average crit.
    fn expected_tick(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side) -> f64 {
        let points = self.expected_combo_points;
        let tick = self.flat_damage(points) + self.share(points) * fight.melee_attack_power();
        let dot = fight.spells[spell].dot.expect("Rip has a dot");
        let attacker =
            fight.attacker_multiplier(spell, true) * fight.dots[dot].periodic_damage_multiplier;
        let result = fight.expected_physical_periodic(spell, target, tick, attacker);
        let crit_chance = fight.spell_physical_crit_chance(spell);
        result * (1.0 + crit_chance * (fight.crit_multiplier(spell) - 1.0))
    }

    /// A bleed tick on the stored amounts and the current attack power.
    pub(crate) fn tick(&self, fight: &mut Fight<DruidAgent>, dot: DotId) {
        let state = &fight.dots[dot];
        let (spell, side, multiplier) = (state.spell, state.side, state.periodic_damage_multiplier);
        let snapshot = fight.agent.rip_snapshot;
        let mut base = snapshot.base;
        if snapshot.share != 0.0 {
            base += snapshot.share * fight.melee_attack_power() - snapshot.attack_power;
        }
        let attacker = fight.attacker_multiplier(spell, true) * multiplier;
        let result = fight.calc_physical_periodic(spell, side, base, attacker, self.tick_can_crit);
        fight.deal_damage(spell, result, true);
    }
}
