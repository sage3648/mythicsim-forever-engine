//! Go sim/warrior/items.go: the item effects the Warrior package registers, and the Warrior's
//! Eureka! spells (eureka.go).

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::dbcenums;
use crate::prepare::racials::EurekaSpells;
use crate::prepare::sim::{AuraConfig, Cooldown, Duration, Sim, UnitId, SECOND};
use crate::prepare::spell::{
    school, Cast, CastConfig, DotConfig, ProcMask, SpellConfig, SpellFlag,
};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::helpers::spell_action;
use super::masks;

const MINUTE: Duration = 60 * SECOND;

/// Go `Warrior.EurekaSpells`: Intercept, Pummel, Revenge, Shield Bash and Spearing Strike are
/// among the abilities, Rend off both effects, and Deep Wounds, Sunder Armor, Sweeping Strikes,
/// Concussion Blow and Victory Rush are on none.
pub(super) fn eureka_spells() -> EurekaSpells {
    let abilities = masks::BLOODTHIRST
        | masks::CLEAVE
        | masks::EXECUTE
        | masks::HAMSTRING
        | masks::HEROIC_STRIKE
        | masks::MOCKING_BLOW
        | masks::MORTAL_STRIKE
        | masks::OVERPOWER
        | masks::SHIELD_SLAM
        | masks::SLAM
        | masks::THUNDER_CLAP
        | masks::WHIRLWIND
        | masks::WHIRLWIND_OH
        | masks::INTERCEPT
        | masks::PUMMEL
        | masks::REVENGE
        | masks::SHIELD_BASH
        | masks::SPEARING_STRIKE;
    EurekaSpells {
        cost: abilities,
        damage: abilities,
        tick: 0,
    }
}

/// Go's `core.NewItemEffect` calls in sim/warrior/items.go `init`.
pub(super) fn apply_item_effect(sim: &mut Sim, unit: UnitId, item: i32) -> bool {
    match item {
        // Marshal's Plate Gauntlets, General's Plate Gauntlets, Rage of Mugamba and
        // Knight-Lieutenant's Plate Gauntlets.
        16484 | 16548 | 16406 => hamstring_cost_reduction(sim, unit, item, 3),
        19577 => hamstring_cost_reduction(sim, unit, item, 2),
        19951 => grilek_charm_of_might(sim, unit),
        20130 => diamond_flask(sim, unit),
        _ => return false,
    }
    true
}

fn hamstring_cost_reduction(sim: &mut Sim, unit: UnitId, item: i32, rage: i32) {
    let aura = sim.register_aura(
        unit,
        AuraConfig {
            label: "Hamstring Rage Reduction".to_string(),
            action_id: Some(ActionId::item(item)),
            ..AuraConfig::default()
        },
    );
    sim.make_permanent(aura);
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            class_mask: masks::HAMSTRING,
            kind: SpellModType::PowerCostFlat,
            int_value: -rage,
            ..SpellModConfig::default()
        },
    );
}

/// Gri'lek's Charm of Might: 30 rage, 3 min cooldown.
fn grilek_charm_of_might(sim: &mut Sim, unit: UnitId) {
    let timer = sim.new_timer(unit);
    let spell = sim.register_spell(
        unit,
        SpellConfig {
            action_id: ActionId::item(19951),
            spell_school: school::PHYSICAL,
            proc_mask: ProcMask::EMPTY,
            flags: SpellFlag::NO_ON_CAST_COMPLETE,
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(timer),
                    duration: 3 * MINUTE,
                },
                ..CastConfig::default()
            },
            ..SpellConfig::default()
        },
    );
    super::spells::add_cooldown(sim, unit, spell, cooldown_type::DPS);
}

/// Diamond Flask: a 5 second channel, a self hot of five ticks a second whose last tick
/// activates a Strength aura. The heal is left out.
fn diamond_flask(sim: &mut Sim, unit: UnitId) {
    let mut stats = Stats::default();
    stats[Stat::Strength] = 20.0;
    sim.new_temporary_stats_aura(unit, "Diamond Flask", &spell_action(1318070), stats, MINUTE);
    let timer = sim.new_timer(unit);
    let spell = sim.register_spell(
        unit,
        SpellConfig {
            action_id: ActionId::item(20130),
            proc_mask: ProcMask::EMPTY,
            flags: SpellFlag::NO_ON_CAST_COMPLETE | SpellFlag::CHANNELED | SpellFlag::HELPFUL,
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(timer),
                    duration: 6 * MINUTE,
                },
                ..CastConfig::default()
            },
            hot: DotConfig {
                self_only: true,
                aura: AuraConfig {
                    label: "CHUG! CHUG! CHUG! CHUG!".to_string(),
                    ..AuraConfig::default()
                },
                number_of_ticks: 5,
                tick_length: SECOND,
                ..DotConfig::default()
            },
            ..SpellConfig::default()
        },
    );
    super::spells::add_cooldown(sim, unit, spell, cooldown_type::DPS);
}

#[allow(dead_code)]
fn unused(_: &Cast, _: i32) {
    let _ = dbcenums::A_DUMMY;
}

/// Item sets: the registry takes them from [`ITEM_SETS`] once the shared registry lands.
#[allow(dead_code)]
pub(crate) fn no_sets() {}
