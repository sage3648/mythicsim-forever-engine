//! Go sim/core/racials.go.

use super::env::Environment;
use super::stats::Stat;
use super::Refusal;

/// Go `applyRaceEffects`.
pub(crate) fn apply_race_effects(env: &mut Environment) -> Result<(), Refusal> {
    let unit = env.player;
    let race = env.sim.character(unit).race.clone();
    match race.as_str() {
        "RaceHuman" => {
            env.sim.unit_mut(unit).sdm.multiply_stat(Stat::Spirit, 1.05);
            super::aura_helpers::apply_weapon_specialization(
                env,
                "Sword Specialization",
                20597,
                2,
                &["WeaponTypeSword"],
            );
            Ok(())
        }
        other => Err(Refusal::new(
            "race",
            format!("{other} racials are not prepared yet"),
        )),
    }
}
