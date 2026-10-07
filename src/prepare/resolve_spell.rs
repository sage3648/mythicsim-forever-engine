//! Go sim/core/spelldata/resolve_spell.go: what the client states about a spell, as the fields
//! core registers it through.
//!
//! The effect-side half of the file (`HitsAnEnemy`, `HitsAnArea`, `TargetsAnEnemy`) lives with the
//! effect accessors in `spelldata/effect.rs`.

use std::rc::Rc;

use crate::data::spells::Spell;

use super::dbcenums;
use super::sim::{Cooldown, Sim, UnitId};
use super::spell::{school, CastConfig, ProcMask, SpellConfig, SpellFlag};

/// Go `SpellOpt`: an addition to the resolved config for what the client does not state: the
/// proc mask, the metrics the sim keeps and the flags a row cannot justify.
pub(crate) type SpellOpt = Rc<dyn Fn(&mut SpellConfig, &Spell)>;

/// `SpellCategories.StartRecoveryCategory` of the global cooldown. It is the only category the
/// store carries: 1763 rows state 133 and the rest state nothing, so a row outside it spends no
/// GCD.
const GLOBAL_COOLDOWN_CATEGORY: i16 = 133;

/// `ImplicitTarget_0` values that name a friendly unit.
const HELPFUL_TARGETS: [i32; 12] = [
    dbcenums::TARGET_UNIT_CASTER,
    dbcenums::TARGET_UNIT_PET,
    dbcenums::TARGET_UNIT_CASTER_AREA_PARTY,
    dbcenums::TARGET_UNIT_TARGET_ALLY,
    dbcenums::TARGET_UNIT_SRC_AREA_ALLY,
    dbcenums::TARGET_UNIT_DEST_AREA_ALLY,
    dbcenums::TARGET_UNIT_SRC_AREA_PARTY,
    dbcenums::TARGET_UNIT_DEST_AREA_PARTY,
    dbcenums::TARGET_UNIT_TARGET_PARTY,
    dbcenums::TARGET_UNIT_TARGET_CHAINHEAL_ALLY,
    dbcenums::TARGET_UNIT_CASTER_AREA_RAID,
    dbcenums::TARGET_UNIT_TARGET_RAID,
];

/// Go `SpellConfig` of spelldata: what the client states about a spell. The caller adds the proc
/// mask and anything the client does not carry to the returned value before registering it: the
/// resolver fills the row's own fields and nothing else.
///
/// The unit is needed for the cooldown timers, so a config is built where the sim has a
/// character.
pub(crate) fn spell_config(
    sim: &mut Sim,
    unit: UnitId,
    s: &'static Spell,
    opts: &[SpellOpt],
) -> SpellConfig {
    let mut config = SpellConfig {
        action_id: crate::contracts::prepared_v2::ActionId {
            spell_id: s.id,
            ..Default::default()
        },
        rank: rank_of(s),
        spell_school: s.spell_school(),
        defense_type: s.defense_type_core(),
        class_flags: s.class_flags,
        flags: row_flags(s),
        missile_speed: f64::from(s.speed),
        min_range: f64::from(s.min_range),
        max_range: f64::from(s.max_range),
        cast: cast_config(sim, unit, s),
        has_cast_requirement: has_cast_requirement(s),
        ..SpellConfig::default()
    };
    apply_cost(&mut config, s);
    if s.is_bleed() {
        config.damage_multiplier = 1.0;
        config.threat_multiplier = 1.0;
    }

    // The row first, the caller's options on top, so an option sees what the row filled.
    for opt in opts {
        opt(&mut config, s);
    }
    config
}

/// Go `Melee`: the proc mask, the melee metrics bucket and the multipliers an ability needs,
/// for a physical spell.
#[allow(dead_code)]
pub(crate) fn melee(mask: ProcMask) -> SpellOpt {
    Rc::new(move |config, _| {
        config.proc_mask = mask;
        config.flags |= SpellFlag::MELEE_METRICS | SpellFlag::APL;
        config.damage_multiplier = 1.0;
        config.threat_multiplier = 1.0;
        config.cast.ignore_haste = true;
    })
}

/// Go `Magic`: the same for a spell that scales with spell power, whose share of it the row
/// states on the effect that deals the damage, or heals where the spell has no damaging effect.
pub(crate) fn magic(mask: ProcMask) -> SpellOpt {
    Rc::new(move |config, s| {
        config.proc_mask = mask;
        config.flags |= SpellFlag::APL;
        config.damage_multiplier = 1.0;
        config.threat_multiplier = 1.0;
        config.bonus_coefficient = spell_power_coeff(s);
    })
}

/// Go `Proc`: a spell another spell or an aura casts: it is out of the rotation `Melee` and
/// `Magic` put it in, it does not feed on-cast effects, and it has no cast, cost, cooldown or
/// form requirement of its own.
pub(crate) fn proc() -> SpellOpt {
    Rc::new(|config, _| {
        config.flags |= SpellFlag::PASSIVE_SPELL | SpellFlag::NO_ON_CAST_COMPLETE;
        config.flags = SpellFlag(config.flags.0 & !SpellFlag::APL.0);
        config.cast = CastConfig::default();
        config.has_cast_requirement = false;
        config.cost = Default::default();
    })
}

/// Go `Flags`: flags the client does not state, such as the sim's own metrics and rotation
/// flags.
#[allow(dead_code)]
pub(crate) fn flags(flags: SpellFlag) -> SpellOpt {
    Rc::new(move |config, _| config.flags |= flags)
}

/// Go `Tag`: splits one spell id into several actions, for a spell the sim registers more than
/// once.
#[allow(dead_code)]
pub(crate) fn tag(tag: i32) -> SpellOpt {
    Rc::new(move |config, _| config.action_id.tag = tag)
}

/// Go `CastRequirement`'s zero test: whether the row states a form, caster form, shapeshift or
/// caster aura requirement.
fn has_cast_requirement(s: &Spell) -> bool {
    s.stance_mask != 0
        || s.stance_exclude != 0
        || s.castable_in_caster_form()
        || s.not_shapeshifted()
        || s.caster_aura != 0
        || s.exclude_caster_aura != 0
}

fn spell_power_coeff(s: &Spell) -> f64 {
    let damage = s.damage_effect();
    if !damage.is_nil() {
        return damage.coeff();
    }
    s.heal_effect().coeff()
}

/// `Spell.NameSubtext_lang`, which is "Rank 4" on a ranked spell: Go `rankOf`.
fn rank_of(s: &Spell) -> i32 {
    s.rank_number()
}

/// The flags the row's attributes and targets state. Everything else is the caller's: a flag
/// the client does not carry is not invented here.
fn row_flags(s: &Spell) -> SpellFlag {
    let mut flags = SpellFlag::NONE;
    if s.is_passive() {
        flags |= SpellFlag::PASSIVE_SPELL;
    }
    if s.is_channeled() {
        flags |= SpellFlag::CHANNELED;
    }
    if s.suppresses_weapon_procs() {
        flags |= SpellFlag::SUPPRESS_WEAPON_PROCS;
    }
    if s.pushed_back() {
        flags |= SpellFlag::PUSHBACK;
    }
    // Helpful decides who the APL casts the spell on, so it follows the first effect's target.
    if HELPFUL_TARGETS.contains(&s.effect_n(1).target[0]) {
        flags |= SpellFlag::HELPFUL;
    }
    flags
}

/// Go `Cast`: the cast time and global cooldown the row states and whether haste shortens them,
/// without the cooldowns: for a caller that runs the spell on cooldowns of its own.
pub(crate) fn cast(s: &Spell) -> CastConfig {
    // Haste shortens a cast and the GCD it spends, which is the school's business rather than
    // the hit table's.
    let mut cast = CastConfig {
        default_cast: super::spell::Cast {
            cast_time: s.cast_time(),
            ..Default::default()
        },
        ignore_haste: s.spell_school() == school::PHYSICAL,
        ..CastConfig::default()
    };
    if s.start_recovery_category == GLOBAL_COOLDOWN_CATEGORY {
        cast.default_cast.gcd = s.gcd();
    }
    cast
}

fn cast_config(sim: &mut Sim, unit: UnitId, s: &Spell) -> CastConfig {
    let mut cast = cast(s);
    if s.cooldown_ms > 0 {
        cast.cd = Cooldown {
            timer: Some(sim.new_timer(unit)),
            duration: s.cooldown(),
        };
        if s.category_cooldown_ms > 0 {
            cast.shared_cd = Cooldown {
                timer: Some(category_timer(sim, unit, s)),
                duration: s.category_cooldown(),
            };
        }
    } else if s.category_cooldown_ms > 0 {
        cast.cd = Cooldown {
            timer: Some(category_timer(sim, unit, s)),
            duration: s.category_cooldown(),
        };
    }
    cast
}

/// The timer a category cooldown runs off. A category is a set of spells that share one
/// cooldown, so the timer is the unit's for that category; 16 rows state a category cooldown
/// without a category to share it with, and that is the spell's own recovery time.
fn category_timer(sim: &mut Sim, unit: UnitId, s: &Spell) -> super::sim::TimerId {
    if s.category == 0 {
        return sim.new_timer(unit);
    }
    sim.category_timer(unit, i32::from(s.category))
}

/// Go `applyCost`: the cost out of the first bar the row states, and `NonEmpty` where the cast
/// would otherwise read as an empty one. Only the first bar: the sim has one cost per spell. A
/// bar the sim does not model resolves to no cost at all.
fn apply_cost(config: &mut SpellConfig, s: &Spell) {
    let Some(first) = s.powers.first() else {
        return;
    };
    let power_type = first.power_type;

    // Truncated rather than rounded: Retaliation states one rage-tenth and so costs nothing.
    let cost = s.power_cost(power_type) as i32;
    let cost_pct = f64::from(first.cost_pct);

    match power_type {
        dbcenums::POWER_MANA => {
            let mana = s.mana_cost();
            config.cost.mana_flat_cost = mana.flat_cost;
            config.cost.mana_base_cost_percent = mana.base_cost_percent;
        }
        dbcenums::POWER_RAGE => {
            config.cost.rage_cost = cost;
            config.cost.rage_refund = s.miss_refund();
        }
        dbcenums::POWER_ENERGY => {
            config.cost.energy_cost = cost;
            config.cost.energy_refund = s.miss_refund();
        }
        dbcenums::POWER_FOCUS => {
            config.cost.focus_cost = cost;
            config.cost.focus_refund = s.miss_refund();
        }
        _ => return,
    }

    let spends = cost > 0 || cost_pct > 0.0;
    if spends && config.cast.default_cast.gcd == 0 && config.cast.default_cast.cast_time == 0 {
        config.cast.default_cast.non_empty = true;
    }
}
