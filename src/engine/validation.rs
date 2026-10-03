//! Validate wire compatibility, supported build scope and prepared parameters.

use crate::{classes::mage::specs::frost, contracts::Request, core::time::SECOND, SOURCE_REVISION};

impl Request {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.source_revision != SOURCE_REVISION {
            return Err("unsupported schema or source revision".into());
        }
        frost::validate_build(self)?;
        if self.scenario_id.is_empty() || self.scenario_id.len() > 200 {
            return Err("scenario_id must contain 1 to 200 bytes".into());
        }
        if !(1..=1_000_000).contains(&self.iterations)
            || self.seed == 0
            || self.seed > i64::MAX as u64 - u64::from(self.iterations)
            || !(SECOND..=600 * SECOND).contains(&self.duration_ns)
            || !(10_000_000..=SECOND).contains(&self.reaction_ns)
            || !(SECOND..=10 * SECOND).contains(&self.spell.cast_ns)
            || !(SECOND..=10 * SECOND).contains(&self.spell.gcd_ns)
            || self.spell.travel_ns > 10 * SECOND
        {
            return Err("iterations, seed, duration, reaction or spell timing is outside the prototype limits".into());
        }
        for (name, value) in [
            ("max_mana", self.caster.max_mana),
            ("spell_power", self.caster.spell_power),
            ("hit_percent", self.caster.hit_percent),
            ("crit_percent", self.caster.crit_percent),
            ("spell_penetration", self.caster.spell_penetration),
            (
                "regen_casting_per_second",
                self.caster.regen_casting_per_second,
            ),
            ("regen_idle_per_second", self.caster.regen_idle_per_second),
            ("frost_resistance", self.target.frost_resistance),
            ("min_damage", self.spell.min_damage),
            ("max_damage", self.spell.max_damage),
            ("coefficient", self.spell.coefficient),
            ("damage_multiplier", self.spell.damage_multiplier),
            ("crit_multiplier", self.spell.crit_multiplier),
            ("mana_cost", self.spell.mana_cost),
        ] {
            if !value.is_finite() || !(0.0..=1_000_000.0).contains(&value) {
                return Err(format!(
                    "{name} must be finite, nonnegative and at most 1000000"
                ));
            }
        }
        if self.caster.max_mana == 0.0
            || self.spell.mana_cost > self.caster.max_mana
            || self.caster.crit_percent > 100.0
            || self.caster.hit_percent > 100.0
            || self.spell.min_damage > self.spell.max_damage
            || self.spell.crit_multiplier < 1.0
            || self.spell.damage_multiplier == 0.0
        {
            return Err("inconsistent mana, damage or chance parameters".into());
        }
        Ok(())
    }
}
