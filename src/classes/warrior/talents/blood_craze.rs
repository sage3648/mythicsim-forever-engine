//! Blood Craze (16487, hot 16488), from Go sim/warrior/talents_fury.go `registerBloodCraze`:
//! a crit or a hit taken above a share of maximum health, or a landed Bloodthirst, applies a
//! hot on the warrior a spell batch window later. Each tick heals a share of maximum health
//! over the hot's ticks.

use crate::{
    contracts::prepared_v2::HealModifiers,
    core::fight::{healing::Healing, Agent, DotId, Fight, Side, SpellResult, PRIORITY_DOT},
};

/// The class periodic tag of the delayed handler.
pub(crate) const PERIODIC_TAG: u32 = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) struct BloodCraze {
    pub(crate) dot: DotId,
    pub(crate) heal: HealModifiers,
    pub(crate) health_fraction: f64,
    pub(crate) hit_threshold: f64,
}

fn delay<A: Agent>(fight: &mut Fight<A>) {
    fight.start_class_periodic(
        PERIODIC_TAG,
        crate::core::fight::SPELL_BATCH_WINDOW,
        1,
        PRIORITY_DOT,
    );
}

/// "Blood Craze - Damage Taken": a landed hit taken that dealt damage and crit or exceeded
/// the threshold share of maximum health.
pub(crate) fn on_hit_taken<A: Agent>(
    fight: &mut Fight<A>,
    params: BloodCraze,
    result: &SpellResult,
) {
    if !result.landed() || result.damage == 0.0 {
        return;
    }
    if result.crit() || result.damage > fight.player_max_health() * params.hit_threshold {
        delay(fight);
    }
}

/// "Blood Craze - Bloodthirst": a landed Bloodthirst that dealt damage.
pub(crate) fn on_bloodthirst<A: Agent>(fight: &mut Fight<A>, result: &SpellResult) {
    if result.landed() && result.damage != 0.0 {
        delay(fight);
    }
}

/// The delayed handler: Go `SelfHot().Apply`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, params: BloodCraze) {
    fight.apply_dot(params.dot);
}

/// A tick: maximum health times the share, over the expected tick count. Go reads the
/// warrior's live healing power, which a stat aura such as an on-use trinket's changes, plus
/// the bonus healing taken the export folds into the reset value.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, params: BloodCraze) {
    let ticks = f64::from(fight.dots[dot].base_tick_count);
    let base = fight.player_max_health() * params.health_fraction / ticks;
    let heal = params.heal;
    let bonus_healing_taken = heal.healing_power - fight.config.powers.healing_power;
    let healing = Healing {
        dealt_multiplier: heal.healing_dealt_multiplier,
        taken_multiplier: heal.healing_taken_multiplier,
        table_multiplier: heal.table_healing_dealt_multiplier,
        healing_power: fight.unit(Side::Player).powers.healing_power + bonus_healing_taken,
    };
    fight.periodic_self_healing_tick(dot, base, healing, heal.periodic_healing_dealt_multiplier);
}
