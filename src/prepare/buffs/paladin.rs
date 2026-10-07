//! Go sim/core/buffs/paladin.go: the paladin's auras and judgements, which share categories with
//! the generated party buffs and raid debuffs.
//!
//! The paladin class registers one aura per rank through these; the raid config applies the top
//! rank, carried by the `*_max_rank` values with rank 0.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;

use super::super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use super::super::dbcenums;
use super::super::env::Environment;
use super::super::sim::{
    school_array_index, AuraConfig, AuraId, EffectId, Sim, UnitId,
};
use super::super::spell::{school, ProcMask};
use super::super::spelldata::must_find;
use super::super::stats::SchoolIndex;
use super::super::Refusal;
use super::generated::{
    CONCENTRATION_AURA, DEVOTION_AURA, FIRE_RESISTANCE_AURA, FROST_RESISTANCE_AURA,
    JUDGEMENT_OF_LIGHT, JUDGEMENT_OF_WISDOM, RETRIBUTION_AURA, SHADOW_RESISTANCE_AURA,
};
use super::{amount, aura_duration, Meta};

/// Go `PaladinAuraRank`: one rank of a paladin aura, the spell the caster used and the number its
/// row states. Retribution Aura's damage shield deals `value`; every other aura reads its amount
/// off the spell.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PaladinAuraRank {
    pub spell_id: i32,
    pub rank: i32,
    pub value: f64,
}

/// Go `RetributionAuraMaxRank`.
pub(crate) fn retribution_aura_max_rank() -> PaladinAuraRank {
    PaladinAuraRank {
        spell_id: 10301,
        rank: 0,
        value: RETRIBUTION_AURA.value(0),
    }
}

/// Go `paladinRankName`: a name with the rank the paladin cast; the max rank the raid config
/// applies carries none.
pub(crate) fn rank_name(name: &str, rank: i32) -> String {
    if rank > 0 {
        format!("{name} Rank {rank}")
    } else {
        name.to_string()
    }
}

/// Go `paladinAuraMeta`: a rank the paladin casts is the party-buff row on the rank's own spell,
/// labelled with the rank.
fn aura_meta(base: &Meta, rank: &PaladinAuraRank) -> Meta {
    Meta {
        rank: rank.rank,
        spell: rank.spell_id,
        ..*base
    }
}

fn ranked_buff(
    env: &mut Environment,
    unit: UnitId,
    base: &Meta,
    is_player: bool,
    rank: &PaladinAuraRank,
) -> Result<AuraId, Refusal> {
    aura_meta(base, rank).aura(env, unit, is_player, 0, 0.0)
}

/// Go `DevotionAuraBuff`.
#[allow(dead_code)]
pub(crate) fn devotion_aura_buff(
    env: &mut Environment,
    unit: UnitId,
    is_player: bool,
    rank: &PaladinAuraRank,
) -> Result<AuraId, Refusal> {
    ranked_buff(env, unit, &DEVOTION_AURA, is_player, rank)
}

/// Go `ConcentrationAura`.
#[allow(dead_code)]
pub(crate) fn concentration_aura(
    env: &mut Environment,
    unit: UnitId,
    is_player: bool,
    rank: &PaladinAuraRank,
) -> Result<AuraId, Refusal> {
    ranked_buff(env, unit, &CONCENTRATION_AURA, is_player, rank)
}

/// Go `FireResistanceAura`.
#[allow(dead_code)]
pub(crate) fn fire_resistance_aura(
    env: &mut Environment,
    unit: UnitId,
    is_player: bool,
    rank: &PaladinAuraRank,
) -> Result<AuraId, Refusal> {
    ranked_buff(env, unit, &FIRE_RESISTANCE_AURA, is_player, rank)
}

/// Go `FrostResistanceAura`.
#[allow(dead_code)]
pub(crate) fn frost_resistance_aura(
    env: &mut Environment,
    unit: UnitId,
    is_player: bool,
    rank: &PaladinAuraRank,
) -> Result<AuraId, Refusal> {
    ranked_buff(env, unit, &FROST_RESISTANCE_AURA, is_player, rank)
}

/// Go `ShadowResistanceAura`.
#[allow(dead_code)]
pub(crate) fn shadow_resistance_aura(
    env: &mut Environment,
    unit: UnitId,
    is_player: bool,
    rank: &PaladinAuraRank,
) -> Result<AuraId, Refusal> {
    ranked_buff(env, unit, &SHADOW_RESISTANCE_AURA, is_player, rank)
}

/// Go `RetributionAuraSpellPowerCoefficient`: Retribution Aura scales with the casting paladin's
/// Holy spell power in Forever even though its client row carries no coefficient. It is the 1.5 s
/// cast-time floor over 3.5, the AoE divisor because the shield hits every attacker, and the 0.95
/// penalty for the aura effect: `1.5 / 3.5 / 3 * 0.95`, which Go evaluates exactly as 19/140.
const RETRIBUTION_AURA_SPELL_POWER_COEFFICIENT: f64 = 19.0 / 140.0;

/// Go `RetributionAuraBuff`: the aura on the unit the shield protects. The self-cast variant reads
/// the paladin's own Holy spell power through the proc spell; the external (party-buff) variant
/// cannot see the providing paladin, so `external_spell_power` stands in for it and the
/// recipient's own stats stay out of the damage.
pub(crate) fn retribution_aura_buff(
    env: &mut Environment,
    unit: UnitId,
    is_player: bool,
    rank: &PaladinAuraRank,
    external_spell_power: f64,
) -> AuraId {
    let meta = aura_meta(&RETRIBUTION_AURA, rank);
    let label = meta.label_for(is_player);
    if let Some(aura) = env.sim.get_aura(unit, &label) {
        return aura;
    }
    let (damage, coefficient) = if is_player {
        (rank.value, RETRIBUTION_AURA_SPELL_POWER_COEFFICIENT)
    } else {
        // paladin.go:82 is a fused multiply-add on arm64.
        (
            RETRIBUTION_AURA_SPELL_POWER_COEFFICIENT.mul_add(external_spell_power, rank.value),
            0.0,
        )
    };
    let aura = super::support::new_damage_shield(
        &mut env.sim,
        unit,
        super::support::DamageShield {
            label,
            action_id: meta.action_id(is_player),
            duration: super::super::sim::NEVER_EXPIRES,
            category: meta.category,
            single_aura: meta.single_aura,
            school: school::HOLY,
            damage,
            bonus_coefficient: coefficient,
        },
    );
    super::support::join_shared_category(&mut env.sim, aura, meta.shared_category, is_player);
    aura
}

/// Go `JudgementRank`: one rank of a judgement debuff, the spell the target shows and the number
/// its row states. The paladin registers a rank per row; the debuff panel applies the max rank,
/// carried by the `*_max_rank` values with rank 0.
#[derive(Clone, Copy, Debug)]
pub(crate) struct JudgementRank {
    pub spell_id: i32,
    pub rank: i32,
    pub value: f64,
}

/// The raid's Judgement of the Crusader, which no generated row reads: the client states its bonus
/// for the holy school alone, which the manifest has no pseudo-stat for.
const JUDGEMENT_OF_THE_CRUSADER_SPELL: i32 = 20303;

/// The top ranks' heal and mana. The judgement states neither: its trigger is a dummy, and the
/// spells the paladin's own ranks pair with it by hand carry the amounts.
const JUDGEMENT_OF_LIGHT_HEAL_ID: i32 = 20343;
const JUDGEMENT_OF_WISDOM_MANA_ID: i32 = 20353;

/// Go `JudgementOfTheCrusaderMaxRank`.
pub(crate) fn judgement_of_the_crusader_max_rank() -> JudgementRank {
    let spell = must_find(JUDGEMENT_OF_THE_CRUSADER_SPELL);
    JudgementRank {
        spell_id: spell.id,
        rank: 0,
        value: amount(spell.effect(dbcenums::A_MOD_DAMAGE_TAKEN, i32::from(school::HOLY))),
    }
}

/// Go `JudgementOfLightMaxRank`.
pub(crate) fn judgement_of_light_max_rank() -> JudgementRank {
    JudgementRank {
        spell_id: JUDGEMENT_OF_LIGHT.spell,
        rank: 0,
        value: amount(must_find(JUDGEMENT_OF_LIGHT_HEAL_ID).heal_effect()),
    }
}

/// Go `JudgementOfWisdomMaxRank`.
pub(crate) fn judgement_of_wisdom_max_rank() -> JudgementRank {
    JudgementRank {
        spell_id: JUDGEMENT_OF_WISDOM.spell,
        rank: 0,
        value: amount(must_find(JUDGEMENT_OF_WISDOM_MANA_ID).energize_effect()),
    }
}

/// Go `JudgementAuraTag`: every judgement debuff a paladin puts up carries the tag, so an effect
/// that refreshes "all Judgement effects on the target" can find them.
#[allow(dead_code)]
pub(crate) const JUDGEMENT_AURA_TAG: &str = "JudgementAura";

/// The client says the judgements proc on a chance without stating it; the sim uses 50% until
/// in-game testing says otherwise.
pub(crate) const JUDGEMENT_PROC_CHANCE: f64 = 0.5;

/// Go `JudgementOfTheCrusaderAura`: raises the Holy damage the target takes by a flat amount.
/// Every rank and every paladin share one exclusive category, so the strongest active one is the
/// one that counts.
pub(crate) fn judgement_of_the_crusader_aura(
    sim: &mut Sim,
    target: UnitId,
    rank: &JudgementRank,
) -> AuraId {
    let bonus = rank.value;
    let label = rank_name("Judgement of the Crusader", rank.rank);
    if let Some(aura) = sim.get_aura(target, &label) {
        return aura;
    }
    let aura = sim.get_or_register_aura(
        target,
        AuraConfig {
            label,
            action_id: Some(ActionId::spell(rank.spell_id)),
            tag: JUDGEMENT_AURA_TAG.to_string(),
            duration: aura_duration(must_find(JUDGEMENT_OF_THE_CRUSADER_SPELL)),
            ..AuraConfig::default()
        },
    );
    let holy = school_array_index(SchoolIndex::Holy);
    sim.new_exclusive_effect(
        aura,
        "Judgement of the Crusader",
        true,
        bonus,
        Some(Rc::new(move |sim: &mut Sim, _: EffectId| {
            sim.unit_mut(target).pseudo_stats.school_bonus_spell_damage[holy] += bonus;
        })),
        Some(Rc::new(move |sim: &mut Sim, _: EffectId| {
            sim.unit_mut(target).pseudo_stats.school_bonus_spell_damage[holy] -= bonus;
        })),
    );
    aura
}

/// Go `JudgementOfLightRankAura`: the paladin's own Judgement of Light at one rank, the bare
/// 40-second debuff plus the heal it grants whoever strikes the target.
#[allow(dead_code)]
pub(crate) fn judgement_of_light_rank_aura(
    sim: &mut Sim,
    target: UnitId,
    rank: &JudgementRank,
) -> AuraId {
    let label = rank_name("Judgement of Light", rank.rank);
    if let Some(aura) = sim.get_aura(target, &label) {
        return aura;
    }
    let aura = sim.get_or_register_aura(
        target,
        AuraConfig {
            label,
            action_id: Some(ActionId::spell(rank.spell_id)),
            tag: JUDGEMENT_AURA_TAG.to_string(),
            duration: JUDGEMENT_OF_LIGHT.duration(0),
            ..AuraConfig::default()
        },
    );
    attach_judgement_of_light_heal(sim, aura)
}

/// Go `AttachJudgementOfLightHeal`: whoever lands a melee hit on the judged target has a chance
/// to be healed for the rank's amount. The health metrics Go creates for the heal are not part of
/// the prepared state.
pub(crate) fn attach_judgement_of_light_heal(sim: &mut Sim, aura: AuraId) -> AuraId {
    let name = format!("{} - Heal", sim.aura(aura).label);
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name,
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            proc_mask: ProcMask::MELEE,
            outcome: HitOutcome::LANDED,
            ..ProcTrigger::default()
        },
    )
}

/// Go `JudgementOfWisdomRankAura`: the paladin's own Judgement of Wisdom at one rank.
#[allow(dead_code)]
pub(crate) fn judgement_of_wisdom_rank_aura(
    sim: &mut Sim,
    target: UnitId,
    rank: &JudgementRank,
) -> AuraId {
    let label = rank_name("Judgement of Wisdom", rank.rank);
    if let Some(aura) = sim.get_aura(target, &label) {
        return aura;
    }
    let aura = sim.get_or_register_aura(
        target,
        AuraConfig {
            label,
            action_id: Some(ActionId::spell(rank.spell_id)),
            tag: JUDGEMENT_AURA_TAG.to_string(),
            duration: JUDGEMENT_OF_WISDOM.duration(0),
            ..AuraConfig::default()
        },
    );
    attach_judgement_of_wisdom_mana(sim, aura, rank)
}

/// Go `AttachJudgementOfWisdomMana`: whoever lands an attack or spell on the judged target has a
/// chance to regain the rank's mana. Melee claim it returns mana on a miss as well.
pub(crate) fn attach_judgement_of_wisdom_mana(
    sim: &mut Sim,
    aura: AuraId,
    rank: &JudgementRank,
) -> AuraId {
    let action_id = ActionId::spell(rank.spell_id);
    let name = sim.aura(aura).label.clone();
    sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name,
            action_id: action_id.clone(),
            metrics_action_id: action_id,
            proc_chance: JUDGEMENT_PROC_CHANCE,
            proc_mask: ProcMask::DIRECT,
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            ..ProcTrigger::default()
        },
    )
}
