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

    /// Go `ripFlatDamage`, whose multiply and add the arm64 build fuses.
    fn flat_damage(&self, combo_points: f64) -> f64 {
        self.tick_per_combo_point
            .mul_add(combo_points, self.tick_base)
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
            let dot = fight.dot_on(dot, target);
            // Go Dot.Apply: the running copy ends, the dot snapshots, then it starts again.
            let aura = fight.dots[dot].aura;
            fight.deactivate_aura(aura);
            self.snapshot(fight, spell, target, dot);
            fight.apply_dot(dot);
            let (_, combo) = fight.spells[spell].energy_metrics.expect("energy cost");
            fight.spend_combo_points(combo);
        } else {
            fight.issue_refund(spell);
        }
        fight.deal_damage(spell, result, false);
    }

    /// `OnSnapshot`, then `UpdateBleedPower`'s log of the projected tick.
    fn snapshot(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side, dot: DotId) {
        let combo_points = f64::from(fight.energy_bar().combo_points);
        let share = self.share(combo_points);
        let attack_power = fight.melee_attack_power();
        let snapshot = Snapshot {
            base: share.mul_add(attack_power, self.flat_damage(combo_points)),
            share,
            attack_power: share * attack_power,
        };
        fight.agent.rip_snapshot.set(dot, snapshot);
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
        let tick = self
            .share(points)
            .mul_add(fight.melee_attack_power(), self.flat_damage(points));
        let dot = fight.spells[spell].dot.expect("Rip has a dot");
        let attacker =
            fight.attacker_multiplier(spell, true) * fight.dots[dot].periodic_damage_multiplier;
        let result = fight.expected_physical_periodic(spell, target, tick, attacker);
        let crit_chance = fight.spell_physical_crit_chance(spell);
        result * crit_chance.mul_add(fight.crit_multiplier(spell) - 1.0, 1.0)
    }

    /// A bleed tick on the stored amounts and the current attack power.
    pub(crate) fn tick(&self, fight: &mut Fight<DruidAgent>, dot: DotId) {
        let result = self.tick_calc(fight, dot, self.tick_can_crit);
        let spell = fight.dots[dot].spell;
        fight.deal_damage(spell, result, true);
    }

    /// Go `Dot.CalcSnapshotDamage` of the bleed, its crit rolled only where it can crit.
    fn tick_calc(
        &self,
        fight: &mut Fight<DruidAgent>,
        dot: DotId,
        can_crit: bool,
    ) -> crate::core::fight::SpellResult {
        let state = &fight.dots[dot];
        let (spell, side, multiplier) = (state.spell, state.side, state.periodic_damage_multiplier);
        let snapshot = fight.agent.rip_snapshot.get(dot);
        let mut base = snapshot.base;
        if snapshot.share != 0.0 {
            base += snapshot
                .share
                .mul_add(fight.melee_attack_power(), -snapshot.attack_power);
        }
        let attacker = fight.attacker_multiplier(spell, true) * multiplier;
        fight.calc_physical_periodic(spell, side, base, attacker, can_crit)
    }

    /// Go rip.go `ExpectedTickDamage`: the snapshot's tick with `OutcomeTick`, or a tick cast
    /// now of the combo points it projects.
    pub(crate) fn expected_tick_damage(
        &self,
        fight: &mut Fight<DruidAgent>,
        spell: SpellId,
        target: Side,
        snapshot: bool,
    ) -> f64 {
        if snapshot {
            let dot = fight.spells[spell].dot.expect("Rip has a dot");
            let dot = fight.dot_on(dot, target);
            self.tick_calc(fight, dot, false).damage
        } else {
            self.expected_tick(fight, spell, target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rip(share_per_combo_point: f64) -> Rip {
        Rip {
            tick_base: 4.0,
            tick_per_combo_point: 5.5,
            share_per_combo_point,
            share_max_points: 4.0,
            tick_can_crit: false,
            expected_combo_points: 5.0,
            short_name: "Rip".to_string(),
        }
    }

    /// The tick a Tauren druid's combat log read (levels 22 to 25, client 1.60.1.70862, 7 November
    /// 2026): floor(4 + 5.5 x points + k x attack power x points), k from 0.0113 to 0.0115, over
    /// 17 ticks at 3 and 4 combo points and 192 to 239 attack power. The earlier 0.0125 reads 37
    /// at 4 points and 239 attack power, where the game read 36.
    #[test]
    fn a_tick_reads_the_measured_floor() {
        let reading = |rip: &Rip, points: f64, attack_power: f64| {
            rip.share(points)
                .mul_add(attack_power, rip.flat_damage(points))
                .floor()
        };
        for share in [0.0113, 0.0114, 0.0115] {
            let rip = rip(share);
            assert_eq!(reading(&rip, 3.0, 192.0), 27.0, "k {share}");
            assert_eq!(reading(&rip, 4.0, 239.0), 36.0, "k {share}");
        }
        assert_eq!(reading(&rip(0.0125), 4.0, 239.0), 37.0);
    }

    /// The share counts up to four combo points, so a fifth adds flat damage only.
    #[test]
    fn the_share_stops_at_four_combo_points() {
        let rip = rip(0.0114);
        assert!((rip.share(4.0) - 0.0456).abs() < 1e-12);
        assert_eq!(rip.share(5.0), rip.share(4.0));
    }
}
