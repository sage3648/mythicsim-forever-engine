//! Serpent Sting (1978 to 25295), from Go sim/hunter/serpent_sting.go: a ranged hit roll
//! without a hit count, then after travel the zero-damage outcome reaches hit listeners and a
//! landed sting applies its dot. The dot snapshots its client tick base plus a share of ranged
//! attack power, a Go literal, and its spell power share; each tick swaps both shares for the
//! caster's current values (Go `Dot.tickOnCurrentStats`).

use crate::core::fight::{
    melee::PhysicalOutcome, Agent, DotId, Fight, Outcome, Side, SpellId, SpellResult,
};

/// The bound Serpent Sting: its parameters and the attack power its dot snapshotted.
#[derive(Clone, Debug)]
pub(crate) struct SerpentSting {
    pub(crate) dot: DotId,
    tick_base: f64,
    attack_power_share: f64,
    outcome: Outcome,
    snapshot_attack_power: f64,
}

/// The tick outcome the exporter names: spelldata `TickOutcome`.
pub(crate) fn tick_outcome(name: &str) -> Option<Outcome> {
    match name {
        "physical_crit" => Some(Outcome::TickPhysicalCrit),
        "magic_crit" => Some(Outcome::TickMagicCrit),
        "plain" => Some(Outcome::Tick),
        _ => None,
    }
}

impl SerpentSting {
    pub(crate) fn bind<A: Agent>(
        fight: &Fight<A>,
        spell: SpellId,
        tick_base: f64,
        attack_power_share: f64,
        outcome: &str,
    ) -> Result<Self, String> {
        Ok(SerpentSting {
            dot: fight.spells[spell].dot.ok_or("Serpent Sting has no dot")?,
            tick_base,
            attack_power_share,
            outcome: tick_outcome(outcome)
                .ok_or_else(|| format!("Serpent Sting tick outcome {outcome} is unsupported"))?,
            snapshot_attack_power: 0.0,
        })
    }
}

/// The cast's `ApplyEffects`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result =
        fight.calc_physical_outcome(spell, target, PhysicalOutcome::RangedHit { count: false });
    fight.class_after_travel(spell, result);
}

/// The travel callback: the outcome, then the dot when the sting landed. Returns the attack
/// power the dot's share snapshotted.
pub(crate) fn on_travel<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    result: SpellResult,
    sting: &SerpentSting,
) -> Option<f64> {
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        return None;
    }
    // Go OnSnapshot: Snapshot on the base with the attack power share, then
    // SnapshotAttackPowerShare.
    let attack_power = fight.ranged_attack_power();
    // Go fuses each multiply and add or subtract here on the reference arm64 build.
    fight.dots[sting.dot].tick_base = Some(
        sting
            .attack_power_share
            .mul_add(attack_power, sting.tick_base),
    );
    fight.apply_dot(sting.dot);
    Some(sting.attack_power_share * attack_power)
}

impl SerpentSting {
    /// Record the attack power share the latest snapshot took.
    pub(crate) fn snapshotted(&mut self, attack_power: f64) {
        self.snapshot_attack_power = attack_power;
    }

    /// Go `Dot.CalcAndDealPeriodicSnapshotDamage` on the caster's current stats.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>, dot: DotId) {
        let state = &fight.dots[dot];
        let (spell, side) = (state.spell, state.side);
        let mut base = state.snapshot_base;
        if state.reads_spell_power {
            base += state
                .bonus_coefficient
                .mul_add(fight.bonus_damage(spell), -state.snapshot_spell_power);
        }
        if self.attack_power_share != 0.0 {
            base += self
                .attack_power_share
                .mul_add(fight.ranged_attack_power(), -self.snapshot_attack_power);
        }
        let attacker =
            fight.attacker_multiplier(spell, true) * fight.dots[dot].periodic_damage_multiplier;
        let result = fight.calc_tick_damage(spell, side, base, attacker, self.outcome);
        fight.deal_damage(spell, result, true);
    }
}
