//! Rake (9904), from Go sim/druid/rake.go: a flat hit plus 5% of attack power on the weapon
//! special table that gives a combo point and applies a bleed when it lands, a refund when it
//! does not. The bleed ticks a flat amount plus the same share, which it snapshots as a flat
//! amount but reads from the attack power each tick has, as Go `SnapshotPhysical` and
//! `SnapshotAttackPowerShare` do (mythicsim patch 99). Applying the bleed logs its projected
//! power.

use crate::core::fight::{melee::PhysicalOutcome, DotId, Fight, Side, SpellId};

use super::super::agent::DruidAgent;

#[derive(Clone, Debug)]
pub(crate) struct Rake {
    pub(crate) flat_damage: f64,
    pub(crate) tick_base: f64,
    /// The share of attack power on the hit and on every tick.
    pub(crate) attack_power_share: f64,
    pub(crate) tick_can_crit: bool,
    pub(crate) short_name: String,
}

/// The bleed's stored amounts: Go `SnapshotBaseDamage` and `snapshotAttackPower`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Snapshot {
    base: f64,
    attack_power: f64,
}

impl Rake {
    /// Go `rakeHitDamage`, whose multiply and add the arm64 build fuses.
    fn hit_damage(&self, attack_power: f64) -> f64 {
        self.attack_power_share
            .mul_add(attack_power, self.flat_damage)
    }

    /// Go `rakeTickDamage`, fused the same way.
    fn tick_damage(&self, attack_power: f64) -> f64 {
        self.attack_power_share
            .mul_add(attack_power, self.tick_base)
    }

    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side) {
        let attack_power = fight.melee_attack_power();
        let result = fight.calc_physical_damage(
            spell,
            target,
            self.hit_damage(attack_power),
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
        if result.landed() {
            let (_, combo) = fight.spells[spell].energy_metrics.expect("energy cost");
            fight.add_combo_points(1, combo);
            // Go Dot.Apply: the running copy ends, the dot snapshots, then it starts again.
            let dot = fight.spells[spell].dot.expect("Rake has a dot");
            let dot = fight.dot_on(dot, target);
            let aura = fight.dots[dot].aura;
            fight.deactivate_aura(aura);
            let attack_power = fight.melee_attack_power();
            fight.agent.rake_snapshot.set(
                dot,
                Snapshot {
                    base: self.tick_damage(attack_power),
                    attack_power: self.attack_power_share * attack_power,
                },
            );
            let power = self.expected_tick(fight, spell, target, dot);
            if fight.log.is_some() {
                let line = format!("{} Snapshot Power: {power:.1}", self.short_name);
                fight.player_log(&line);
            }
            fight.apply_dot(dot);
        } else {
            fight.issue_refund(spell);
        }
    }

    /// `ExpectedTickDamage` without the snapshot: a tick on the current stats, always hitting,
    /// scaled by the average crit.
    fn expected_tick(
        &self,
        fight: &mut Fight<DruidAgent>,
        spell: SpellId,
        target: Side,
        dot: DotId,
    ) -> f64 {
        let attacker =
            fight.attacker_multiplier(spell, true) * fight.dots[dot].periodic_damage_multiplier;
        let tick = self.tick_damage(fight.melee_attack_power());
        let result = fight.expected_physical_periodic(spell, target, tick, attacker);
        let crit_chance = fight.spell_physical_crit_chance(spell);
        // Go's arm64 build fuses the average crit into the 1.
        result * crit_chance.mul_add(fight.crit_multiplier(spell) - 1.0, 1.0)
    }

    /// A bleed tick on the stored amount, the share of the current attack power and the
    /// current attacker multiplier.
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
        let snapshot = fight.agent.rake_snapshot.get(dot);
        let mut base = snapshot.base;
        if self.attack_power_share != 0.0 {
            base += self
                .attack_power_share
                .mul_add(fight.melee_attack_power(), -snapshot.attack_power);
        }
        let attacker = fight.attacker_multiplier(spell, true) * multiplier;
        fight.calc_physical_periodic(spell, side, base, attacker, can_crit)
    }

    /// Go rake.go `ExpectedTickDamage`: the snapshot's tick with `OutcomeTick`, or a tick cast
    /// now.
    pub(crate) fn expected_tick_damage(
        &self,
        fight: &mut Fight<DruidAgent>,
        spell: SpellId,
        target: Side,
        snapshot: bool,
    ) -> f64 {
        let dot = fight.spells[spell].dot.expect("Rake has a dot");
        let dot = fight.dot_on(dot, target);
        if snapshot {
            self.tick_calc(fight, dot, false).damage
        } else {
            self.expected_tick(fight, spell, target, dot)
        }
    }
}
