//! Rake (9904), from Go sim/druid/rake.go: a flat hit on the weapon special table that gives a
//! combo point and applies a bleed of a flat tick when it lands, a refund when it does not.
//! Applying the bleed logs its projected power.

use crate::core::fight::{melee::PhysicalOutcome, DotId, Fight, Side, SpellId};

use super::super::agent::DruidAgent;

#[derive(Clone, Debug)]
pub(crate) struct Rake {
    pub(crate) flat_damage: f64,
    pub(crate) tick_base: f64,
    pub(crate) tick_can_crit: bool,
    pub(crate) short_name: String,
}

impl Rake {
    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side) {
        let result = fight.calc_physical_damage(
            spell,
            target,
            self.flat_damage,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
        if result.landed() {
            let (_, combo) = fight.spells[spell].energy_metrics.expect("energy cost");
            fight.add_combo_points(1, combo);
            // Go Dot.Apply: the running copy ends, the dot snapshots, then it starts again.
            let dot = fight.spells[spell].dot.expect("Rake has a dot");
            let aura = fight.dots[dot].aura;
            fight.deactivate_aura(aura);
            fight.agent.rake_snapshot = self.tick_base;
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
        let result = fight.expected_physical_periodic(spell, target, self.tick_base, attacker);
        let crit_chance = fight.spell_physical_crit_chance(spell);
        // Go's arm64 build fuses the average crit into the 1.
        result * crit_chance.mul_add(fight.crit_multiplier(spell) - 1.0, 1.0)
    }

    /// A bleed tick on the stored amount and the current attacker multiplier.
    pub(crate) fn tick(&self, fight: &mut Fight<DruidAgent>, dot: DotId) {
        let state = &fight.dots[dot];
        let (spell, side, multiplier) = (state.spell, state.side, state.periodic_damage_multiplier);
        let base = fight.agent.rake_snapshot;
        let attacker = fight.attacker_multiplier(spell, true) * multiplier;
        let result = fight.calc_physical_periodic(spell, side, base, attacker, self.tick_can_crit);
        fight.deal_damage(spell, result, true);
    }
}
