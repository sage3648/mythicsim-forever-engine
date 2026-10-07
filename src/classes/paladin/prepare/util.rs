//! Small constructors the Paladin's registrations share: Go's `core.CastConfig` and
//! `core.ManaCostOptions` literals, the shared ability timers and the enemy aura arrays.

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::sim::{AuraId, Cooldown, Duration, Sim, TimerId, UnitId, UnitType};
use crate::prepare::spell::{Cast, CastConfig, CostOptions, LabeledAuraArrays};
use crate::prepare::spelldata::Spell as Row;

/// `core.ActionID{SpellID: id}`.
pub(super) fn spell_action(id: i32) -> ActionId {
    ActionId {
        spell_id: id,
        ..ActionId::default()
    }
}

/// `core.ActionID{SpellID: id, Tag: tag}`.
pub(super) fn tagged_action(id: i32, tag: i32) -> ActionId {
    ActionId {
        spell_id: id,
        tag,
        ..ActionId::default()
    }
}

/// `ManaCost: rank.ManaCost()`.
pub(super) fn mana_cost(row: &Row) -> CostOptions {
    let mana = row.mana_cost();
    CostOptions {
        mana_base_cost_percent: mana.base_cost_percent,
        mana_flat_cost: mana.flat_cost,
        ..CostOptions::default()
    }
}

/// Go `cooldown`: a rank's own cooldown, or the one it shares with its category.
pub(super) fn cooldown(row: &Row) -> Duration {
    row.cooldown().max(row.category_cooldown())
}

/// A cast on the global cooldown and an optional cooldown timer: `core.CastConfig{DefaultCast:
/// core.Cast{GCD: gcd, CastTime: cast_time}, CD: core.Cooldown{Timer: timer, Duration:
/// duration}}`.
pub(super) fn gcd_cast(
    gcd: Duration,
    cast_time: Duration,
    cd: Option<(TimerId, Duration)>,
) -> CastConfig {
    CastConfig {
        default_cast: Cast {
            gcd,
            cast_time,
            ..Cast::default()
        },
        cd: cd.map_or_else(Cooldown::default, |(timer, duration)| Cooldown {
            timer: Some(timer),
            duration,
        }),
        ..CastConfig::default()
    }
}

/// Go `Paladin.sharedTimer`.
pub(super) fn shared_timer(sim: &mut Sim, unit: UnitId, timer: &mut Option<TimerId>) -> TimerId {
    *timer.get_or_insert_with(|| sim.new_timer(unit))
}

/// Go `unit.NewEnemyAuraArray`: one aura per enemy, by unit index.
pub(super) fn new_enemy_aura_array(
    sim: &mut Sim,
    mut make_aura: impl FnMut(&mut Sim, UnitId) -> AuraId,
) -> Vec<Option<AuraId>> {
    let units = sim.all_units();
    let mut auras = vec![None; units.len()];
    for target in units {
        if sim.unit(target).unit_type == UnitType::Enemy {
            let index = sim.unit(target).unit_index as usize;
            auras[index] = Some(make_aura(sim, target));
        }
    }
    auras
}

/// Go `AuraArray.ToMap`: nothing when the array holds no aura, else the array under its first
/// aura's label.
pub(super) fn aura_array_to_map(sim: &Sim, auras: &[Option<AuraId>]) -> LabeledAuraArrays {
    let mut map = LabeledAuraArrays::new();
    if let Some(first) = auras.iter().flatten().next() {
        map.insert(sim.aura(*first).label.clone(), auras.to_vec());
    }
    map
}
