//! Small constructors the Warrior's registrations share: Go's aura arrays, the fear immunity
//! helper and the spell config literals every ability repeats.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::character::constants::MAX_MELEE_RANGE;
use crate::prepare::sim::{AuraCallback, AuraId, Cooldown, Duration, Sim, UnitId, UnitType};
use crate::prepare::spell::{Cast, CastConfig, CostOptions, LabeledAuraArrays};
use crate::prepare::spelldata::Spell as Row;

/// `core.ActionID{SpellID: id}`.
pub(super) fn spell_action(id: i32) -> ActionId {
    ActionId {
        spell_id: id,
        ..ActionId::default()
    }
}

/// Go `ActionID.WithTag`.
pub(super) fn with_tag(action: &ActionId, tag: i32) -> ActionId {
    ActionId {
        tag,
        ..action.clone()
    }
}

/// Go `cooldownOf`: the recovery the ability waits out, which a shared category may state.
pub(super) fn cooldown_of(row: &Row) -> Duration {
    row.cooldown().max(row.category_cooldown())
}

/// `core.RageCostOptions{Cost: int32(row.Cost())}`.
pub(super) fn rage_cost(row: &Row) -> CostOptions {
    CostOptions {
        rage_cost: row.cost() as i32,
        ..CostOptions::default()
    }
}

/// `core.RageCostOptions{Cost: int32(row.Cost()), Refund: row.MissRefund()}`.
pub(super) fn rage_cost_with_refund(row: &Row) -> CostOptions {
    CostOptions {
        rage_cost: row.cost() as i32,
        rage_refund: row.miss_refund(),
        ..CostOptions::default()
    }
}

/// `core.CastConfig{DefaultCast: core.Cast{GCD: gcd}, IgnoreHaste: true}`.
pub(super) fn gcd_cast(gcd: Duration) -> CastConfig {
    CastConfig {
        default_cast: Cast {
            gcd,
            ..Cast::default()
        },
        ignore_haste: true,
        ..CastConfig::default()
    }
}

/// `core.CastConfig{DefaultCast: core.Cast{NonEmpty: true}, IgnoreHaste: ...}`.
pub(super) fn non_empty_cast(ignore_haste: bool) -> CastConfig {
    CastConfig {
        default_cast: Cast {
            non_empty: true,
            ..Cast::default()
        },
        ignore_haste,
        ..CastConfig::default()
    }
}

/// `core.Cooldown{Timer: warrior.NewTimer(), Duration: duration}`.
pub(super) fn new_cooldown(sim: &mut Sim, unit: UnitId, duration: Duration) -> Cooldown {
    Cooldown {
        timer: Some(sim.new_timer(unit)),
        duration,
    }
}

/// `core.MaxMeleeRange`.
pub(super) const MELEE_RANGE: f64 = MAX_MELEE_RANGE;

/// Go `AuraArray`: the aura on each unit, by unit index.
pub(super) type AuraArray = Vec<Option<AuraId>>;

/// Go `Unit.NewEnemyAuraArray`: one aura on each enemy, made in unit order.
pub(super) fn new_enemy_aura_array(
    sim: &mut Sim,
    mut make_aura: impl FnMut(&mut Sim, UnitId) -> AuraId,
) -> AuraArray {
    new_aura_array(sim, true, &mut make_aura)
}

/// Go `Unit.NewAllyAuraArray`: one aura on each unit that is not an enemy.
pub(super) fn new_ally_aura_array(
    sim: &mut Sim,
    mut make_aura: impl FnMut(&mut Sim, UnitId) -> AuraId,
) -> AuraArray {
    new_aura_array(sim, false, &mut make_aura)
}

fn new_aura_array(
    sim: &mut Sim,
    enemies: bool,
    make_aura: &mut dyn FnMut(&mut Sim, UnitId) -> AuraId,
) -> AuraArray {
    let units = sim.all_units();
    let mut auras: AuraArray = vec![None; units.len()];
    for target in units {
        if (sim.unit(target).unit_type == UnitType::Enemy) == enemies {
            let index = sim.unit(target).unit_index as usize;
            auras[index] = Some(make_aura(sim, target));
        }
    }
    auras
}

/// Go `AuraArray.ToMap`: the array under the label of its first aura, nothing if it is empty.
pub(super) fn aura_array_to_map(sim: &Sim, auras: &AuraArray) -> LabeledAuraArrays {
    let mut map = LabeledAuraArrays::new();
    if let Some(first) = auras.iter().flatten().next() {
        map.insert(sim.aura(*first).label.clone(), auras.clone());
    }
    map
}

/// Go `AuraArray.Get`.
pub(super) fn aura_of(auras: &AuraArray, sim: &Sim, target: UnitId) -> Option<AuraId> {
    auras
        .get(sim.unit(target).unit_index as usize)
        .copied()
        .flatten()
}

/// Go `Aura.AttachFearImmunity`: the aura re-derives the unit's fear immunity when it is gained
/// and when it expires, and breaks the fears that are up when it is gained.
pub(super) fn attach_fear_immunity(sim: &mut Sim, aura: AuraId) -> AuraId {
    sim.apply_on_gain(aura, noop());
    sim.apply_on_expire(aura, noop());
    aura
}

/// A lifecycle callback whose Go body only changes state a fight reads: preparation keeps that
/// it is set.
pub(super) fn noop() -> AuraCallback {
    Rc::new(|_: &mut Sim, _| {})
}

/// `core.CastConfig{DefaultCast: core.Cast{GCD: gcd}, IgnoreHaste: ..., CD: cd}`.
pub(super) fn cast_config(gcd: Duration, ignore_haste: bool, cd: Cooldown) -> CastConfig {
    CastConfig {
        default_cast: Cast {
            gcd,
            ..Cast::default()
        },
        ignore_haste,
        cd,
        ..CastConfig::default()
    }
}

/// Go `ActionID.String`, for the actions Warrior labels are built from.
pub(super) fn action_id_string(action: &ActionId) -> String {
    let mut out = String::from("{");
    if action.spell_id != 0 {
        out.push_str(&format!("SpellID: {}", action.spell_id));
    } else if action.item_id != 0 {
        out.push_str(&format!("ItemID: {}", action.item_id));
    }
    if action.tag != 0 {
        out.push_str(&format!(", Tag: {}", action.tag));
    }
    out.push('}');
    out
}
