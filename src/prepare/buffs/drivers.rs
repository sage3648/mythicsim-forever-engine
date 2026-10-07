//! Go sim/core/buffs/drivers.go and flametongue_totem.go: the buffs the generated tables hand to a
//! driver.
//!
//! A driver adds what the client does not state: a cooldown another player casts, a totem that
//! twists, a damage shield or a judgement's heal. What a fight does with them (rolling a
//! cooldown, spending a totem's charges, the handlers of a proc trigger) is not part of
//! preparation, which registers the same auras, spells, timers, exclusive effects and callbacks.

use std::rc::Rc;

use crate::contracts::request::Message;

use super::super::character::cooldown_type;
use super::super::env::Environment;
use super::super::periodic_action::PeriodicActionOptions;
use super::super::sim::{AuraId, Sim, UnitId, SECOND};
use super::super::spell::GCD_DEFAULT;
use super::super::stats::Stat;
use super::super::Refusal;
use super::generated::{
    ATIESH_DRUID, ATIESH_MAGE, ATIESH_PRIEST, ATIESH_WARLOCK, BATTLE_SHOUT, GRACE_OF_AIR_TOTEM,
    GREATER_BLESSING_OF_LIGHT, INNERVATES, JUDGEMENT_OF_LIGHT, JUDGEMENT_OF_WISDOM,
    MANA_TIDE_TOTEMS, POWER_INFUSIONS, SUNDER_ARMOR,
};
use super::paladin;
use super::support::{
    add_generated_flat_bonus, apply_fixed_shout_aura, new_generated_external_cd,
    GeneratedExternalCd,
};
use super::{permanent, permanent_with_count};

/// Spell 29166 states 100% mana regen while casting (aura 134) and +400% of it (aura 110);
/// neither is a stat or a pseudo-stat, so the regen is the driver's.
const INNERVATE_SPIRIT_REGEN_MULTIPLIER: f64 = 5.0;

/// Three pieces of Battlegear of Wrath are worth 30 more attack power on Battle Shout: item set
/// 218's ItemSetSpell at three pieces is 23563, which adds a flat 30 to every effect of the
/// Battle Shout family. The resolver reads no ItemSetSpell, so the amount is stated here.
pub(crate) const BATTLE_SHOUT_T2_BONUS: f64 = 30.0;

/// Go `AirTotemCategory`: since build 70009 Tranquil Air, Windfury and Grace of Air totems no
/// longer stack, even from different shamans in the group. One air totem stands: a totem the
/// shaman casts replaces the one the party buffs assume, and of the two party buffs Windfury
/// holds.
pub(crate) const AIR_TOTEM_CATEGORY: &str = "AirTotem";

/// Go `AirTotemPartyGraceOfAir`, `AirTotemPartyWindfury`, `AirTotemCastGraceOfAir` and
/// `AirTotemCastWindfury`: what each bids in the category.
pub(crate) const AIR_TOTEM_PARTY_GRACE_OF_AIR: f64 = 1.0;
#[allow(dead_code)]
pub(crate) const AIR_TOTEM_PARTY_WINDFURY: f64 = 2.0;
#[allow(dead_code)]
pub(crate) const AIR_TOTEM_CAST_GRACE_OF_AIR: f64 = 3.0;
#[allow(dead_code)]
pub(crate) const AIR_TOTEM_CAST_WINDFURY: f64 = 4.0;

fn not_yet(name: &str) -> Result<(), Refusal> {
    Err(Refusal::new("buff", format!("{name} is not prepared yet")))
}

/// Go `driveBattleShout`: the party's Battle Shout is the external caster's copy, which chains
/// behind the player's own shout rather than being up from the start. Its improved state says
/// that warrior shouted with the set on.
pub(crate) fn drive_battle_shout(
    env: &mut Environment,
    unit: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    let aura = BATTLE_SHOUT.aura(env, unit, false, 0, 0.0)?;
    // TristateEffectImproved.
    if party.enum_number("battle_shout") == 2 {
        add_generated_flat_bonus(
            &mut env.sim,
            aura,
            Stat::AttackPower,
            BATTLE_SHOUT.value(0),
            BATTLE_SHOUT_T2_BONUS,
        );
    }
    apply_fixed_shout_aura(&mut env.sim, unit, aura, BATTLE_SHOUT.category);
    Ok(())
}

/// Go `driveInnervates`: a druid innervates a character who is nearly out of mana, so that every
/// other mana cooldown is spent first. The aura forces full spirit regen while it is up. Its mana
/// is regen (29166 has no energize effect), so it lands in the regen metrics and makes no threat.
pub(crate) fn drive_innervates(
    env: &mut Environment,
    unit: UnitId,
    individual: &Message,
) -> Result<(), Refusal> {
    let aura = INNERVATES.aura(env, unit, false, 0, 0.0)?;
    attach_innervate_regen(&mut env.sim, unit, aura);
    // The mana threshold the cooldown waits for is read after finalize and only by a fight.
    new_generated_external_cd(
        &mut env.sim,
        unit,
        aura,
        GeneratedExternalCd {
            num_sources: individual.i32("innervates"),
            cooldown_type: cooldown_type::MANA,
        },
    );
    Ok(())
}

/// Go `AttachInnervateRegen`: shared by player casts and external cooldowns. Actual mana still
/// arrives on the normal regen ticks; reporting splits its bonus out, which a fight does.
pub(crate) fn attach_innervate_regen(sim: &mut Sim, unit: UnitId, aura: AuraId) -> AuraId {
    sim.apply_on_gain(
        aura,
        Rc::new(move |sim: &mut Sim, _| {
            let pseudo = &mut sim.unit_mut(unit).pseudo_stats;
            pseudo.force_full_spirit_regen = true;
            pseudo.spirit_regen_multiplier *= INNERVATE_SPIRIT_REGEN_MULTIPLIER;
        }),
    );
    sim.apply_on_expire(
        aura,
        Rc::new(move |sim: &mut Sim, _| {
            let pseudo = &mut sim.unit_mut(unit).pseudo_stats;
            pseudo.force_full_spirit_regen = false;
            pseudo.spirit_regen_multiplier /= INNERVATE_SPIRIT_REGEN_MULTIPLIER;
        }),
    );
    aura
}

/// Go `drivePowerInfusions`: the priest has nothing to hold Power Infusion for, so it goes out on
/// cooldown.
pub(crate) fn drive_power_infusions(
    env: &mut Environment,
    unit: UnitId,
    individual: &Message,
) -> Result<(), Refusal> {
    let aura = POWER_INFUSIONS.aura(env, unit, false, 0, 0.0)?;
    new_generated_external_cd(
        &mut env.sim,
        unit,
        aura,
        GeneratedExternalCd {
            num_sources: individual.i32("power_infusions"),
            cooldown_type: cooldown_type::DPS,
        },
    );
    Ok(())
}

/// Go `driveManaTideTotems`: a restoration shaman drops Mana Tide once the party has mana to
/// refill, which is 40 seconds in, or halfway through a fight shorter than that.
pub(crate) fn drive_mana_tide_totems(
    env: &mut Environment,
    unit: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    // The initial delay is read after finalize and only by a fight.
    let aura = MANA_TIDE_TOTEMS.aura(env, unit, false, 0, 0.0)?;
    new_generated_external_cd(
        &mut env.sim,
        unit,
        aura,
        GeneratedExternalCd {
            num_sources: party.i32("mana_tide_totems"),
            cooldown_type: cooldown_type::MANA,
        },
    );
    Ok(())
}

/// Go `driveWindfuryTotem`: needs `spelldata.ProcTrigger`, which is not ported yet.
pub(crate) fn drive_windfury_totem(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Windfury Totem")
}

/// Go `driveGraceOfAirTotem`: a shaman twisting totems keeps Grace of Air up for 9 seconds out of
/// every 10, because the air slot is holding another totem the rest of the time; a shaman who is
/// not twisting leaves it standing.
pub(crate) fn drive_grace_of_air_totem(
    env: &mut Environment,
    unit: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    let aura = GRACE_OF_AIR_TOTEM.aura(env, unit, false, 0, 0.0)?;
    env.sim.new_exclusive_effect(
        aura,
        AIR_TOTEM_CATEGORY,
        true,
        AIR_TOTEM_PARTY_GRACE_OF_AIR,
        None,
        None,
    );
    if !party.bool("totem_twisting") {
        env.sim.make_permanent(aura);
        return Ok(());
    }
    // The first cast lands a totem cycle into the fight, because the shaman spends the opening
    // one on the totem being twisted with.
    env.sim.aura_mut(aura).duration = 9 * SECOND;
    env.sim.apply_on_reset(
        aura,
        Rc::new(|sim: &mut Sim, _| {
            sim.start_periodic_action(PeriodicActionOptions {
                period: 10 * SECOND,
                ..PeriodicActionOptions::default()
            });
        }),
    );
    Ok(())
}

/// Go `driveRetributionAura`: the party's Retribution Aura is the top rank, and its damage
/// carries the providing paladin's Holy spell power, which the party states alongside it.
pub(crate) fn drive_retribution_aura(
    env: &mut Environment,
    unit: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    let aura = paladin::retribution_aura_buff(
        env,
        unit,
        false,
        &paladin::retribution_aura_max_rank(),
        party.f64("retribution_aura_spell_power"),
    );
    env.sim.make_permanent(aura);
    Ok(())
}

/// Go `driveGreaterBlessingOfLight`: the blessing does nothing on its own, the paladin's Holy
/// Light and Flash of Light read their bonus off their own rows when the target carries it.
pub(crate) fn drive_greater_blessing_of_light(
    env: &mut Environment,
    unit: UnitId,
    _individual: &Message,
) -> Result<(), Refusal> {
    permanent(env, unit, &GREATER_BLESSING_OF_LIGHT, false, 0)?;
    Ok(())
}

/// Go `driveAtieshMage`, and the three below: the staff's aura is worth its amounts once per
/// Atiesh in the party.
pub(crate) fn drive_atiesh_mage(
    env: &mut Environment,
    unit: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    let count = f64::from(party.i32("atiesh_mage"));
    permanent_with_count(env, unit, &ATIESH_MAGE, count)?;
    Ok(())
}

/// Go `driveAtieshWarlock`.
pub(crate) fn drive_atiesh_warlock(
    env: &mut Environment,
    unit: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    let count = f64::from(party.i32("atiesh_warlock"));
    permanent_with_count(env, unit, &ATIESH_WARLOCK, count)?;
    Ok(())
}

/// Go `driveAtieshDruid`.
pub(crate) fn drive_atiesh_druid(
    env: &mut Environment,
    unit: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    let count = f64::from(party.i32("atiesh_druid"));
    permanent_with_count(env, unit, &ATIESH_DRUID, count)?;
    Ok(())
}

/// Go `driveAtieshPriest`.
pub(crate) fn drive_atiesh_priest(
    env: &mut Environment,
    unit: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    let count = f64::from(party.i32("atiesh_priest"));
    permanent_with_count(env, unit, &ATIESH_PRIEST, count)?;
    Ok(())
}

/// Go `driveFlametongueTotem`: needs `spelldata.ProcTrigger`, which is not ported yet.
pub(crate) fn drive_flametongue_totem(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Flametongue Totem")
}

/// Go `driveJudgementOfLight`: the judgement the paladin leaves on the target heals whoever
/// strikes it; the client's trigger spell 5373 is a dummy, so how much and how often is the
/// driver's. The raid's copy is the top rank the paladin's own judgement states.
pub(crate) fn drive_judgement_of_light(
    env: &mut Environment,
    target: UnitId,
    _debuffs: &Message,
    _raid: &Message,
) -> Result<(), Refusal> {
    let aura = permanent(env, target, &JUDGEMENT_OF_LIGHT, false, 0)?;
    paladin::attach_judgement_of_light_heal(&mut env.sim, aura);
    Ok(())
}

/// Go `driveJudgementOfWisdom`: Judgement of Wisdom returns mana to whoever strikes the target,
/// on the same terms: 1826 is a dummy, so the amount and the chance stay with the paladin's top
/// rank.
pub(crate) fn drive_judgement_of_wisdom(
    env: &mut Environment,
    target: UnitId,
    _debuffs: &Message,
    _raid: &Message,
) -> Result<(), Refusal> {
    let aura = permanent(env, target, &JUDGEMENT_OF_WISDOM, false, 0)?;
    paladin::attach_judgement_of_wisdom_mana(
        &mut env.sim,
        aura,
        &paladin::judgement_of_wisdom_max_rank(),
    );
    Ok(())
}

/// Go `driveSunderArmor`: a stack of Sunder Armor is worth nothing until it is on the target, so
/// the raid's copy is ramped to five over the first five global cooldowns, which is how long a
/// warrior takes to stack it.
pub(crate) fn drive_sunder_armor(
    env: &mut Environment,
    target: UnitId,
    _debuffs: &Message,
    _raid: &Message,
) -> Result<(), Refusal> {
    let aura = permanent(env, target, &SUNDER_ARMOR, false, 0)?;
    super::super::debuffs::scheduled_aura(
        &mut env.sim,
        aura,
        PeriodicActionOptions {
            period: GCD_DEFAULT,
            num_ticks: 5,
            tick_immediately: true,
        },
    );
    Ok(())
}
