//! Translated by tools/rust_buffs.py from the reference's sim/core/buffs/buffs_auto_gen.go and
//! debuffs_auto_gen.go at cd7d44aec711bcc8f20ea12d3ed83cea2126ac66. Do not edit; run `python3 tools/rust_buffs.py write`.

use crate::contracts::request::Message;

use super::super::dbcenums;
use super::super::env::Environment;
use super::super::sim::UnitId;
use super::super::Refusal;
use super::{permanent, tristate, Meta, MetaKind};

pub(crate) static BLOOD_PACT: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Blood Pact",
    spell: 11767,
    ..Meta::DEFAULT
};

pub(crate) static BATTLE_SHOUT: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Battle Shout",
    spell: 25289,
    category: "BattleShout",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static DEVOTION_AURA: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Devotion Aura",
    spell: 10293,
    category: "DevotionAura",
    shared_category: "PaladinAura",
    single_aura: true,
    skip_auras: &[dbcenums::A_MOD_HEALING_PCT],
    ..Meta::DEFAULT
};

pub(crate) static LEADER_OF_THE_PACK: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Leader of the Pack",
    spell: 24932,
    category: "DruidCritAura",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static MANA_SPRING_TOTEM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Mana Spring Totem",
    spell: 10494,
    category: "ManaSpringTotem",
    talent: Some((16187, 5)),
    talent_effect: 1,
    ..Meta::DEFAULT
};

pub(crate) static MANA_TIDE_TOTEMS: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Mana Tide Totem",
    spell: 17360,
    cast: Some(17359),
    category: "ManaTideTotem",
    ..Meta::DEFAULT
};

pub(crate) static MOONKIN_AURA: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Moonkin Aura",
    spell: 24907,
    category: "DruidCritAura",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static RETRIBUTION_AURA: Meta = Meta {
    kind: MetaKind::DamageShield,
    label: "Retribution Aura",
    spell: 10301,
    category: "RetributionAura",
    shared_category: "PaladinAura",
    single_aura: true,
    skip_auras: &[dbcenums::A_MOD_HEALING_PCT],
    ..Meta::DEFAULT
};

pub(crate) static CONCENTRATION_AURA: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Concentration Aura",
    spell: 19746,
    category: "ConcentrationAura",
    shared_category: "PaladinAura",
    single_aura: true,
    skip_auras: &[dbcenums::A_MOD_HEALING_PCT],
    ..Meta::DEFAULT
};

pub(crate) static TRUESHOT_AURA: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Trueshot Aura",
    spell: 20905,
    ..Meta::DEFAULT
};

pub(crate) static ATIESH_MAGE: Meta = Meta {
    kind: MetaKind::ItemCountBuff,
    label: "Atiesh - Mage",
    spell: 28142,
    ..Meta::DEFAULT
};

pub(crate) static ATIESH_WARLOCK: Meta = Meta {
    kind: MetaKind::ItemCountBuff,
    label: "Atiesh - Warlock",
    spell: 28143,
    ..Meta::DEFAULT
};

pub(crate) static STRENGTH_OF_EARTH_TOTEM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Strength of Earth Totem",
    spell: 25362,
    category: "StrengthOfEarthTotem",
    ..Meta::DEFAULT
};

pub(crate) static GRACE_OF_AIR_TOTEM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Grace of Air Totem",
    spell: 25360,
    category: "GraceOfAirTotem",
    ..Meta::DEFAULT
};

pub(crate) static WINDFURY_TOTEM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Windfury Totem",
    spell: 10610,
    category: "WindfuryTotem",
    ..Meta::DEFAULT
};

pub(crate) static ATIESH_DRUID: Meta = Meta {
    kind: MetaKind::ItemCountBuff,
    label: "Atiesh - Druid",
    spell: 28145,
    ..Meta::DEFAULT
};

pub(crate) static ATIESH_PRIEST: Meta = Meta {
    kind: MetaKind::ItemCountBuff,
    label: "Atiesh - Priest",
    spell: 28144,
    ..Meta::DEFAULT
};

pub(crate) static FLAMETONGUE_TOTEM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Flametongue Totem",
    spell: 15036,
    category: "FlametongueTotem",
    ..Meta::DEFAULT
};

pub(crate) static ARCANE_BRILLIANCE: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Arcane Brilliance",
    spell: 23028,
    category: "StatBuff",
    ..Meta::DEFAULT
};

pub(crate) static PRAYER_OF_SPIRIT: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Prayer of Spirit",
    spell: 27681,
    category: "StatBuff",
    ..Meta::DEFAULT
};

pub(crate) static GIFT_OF_THE_WILD: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Gift of the Wild",
    spell: 21850,
    ..Meta::DEFAULT
};

pub(crate) static THORNS: Meta = Meta {
    kind: MetaKind::DamageShield,
    label: "Thorns",
    spell: 9910,
    category: "Thorns",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static PRAYER_OF_FORTITUDE: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Prayer of Fortitude",
    spell: 21564,
    ..Meta::DEFAULT
};

pub(crate) static PRAYER_OF_SHADOW_PROTECTION: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Prayer of Shadow Protection",
    spell: 27683,
    ..Meta::DEFAULT
};

pub(crate) static FIRE_RESISTANCE_AURA: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Fire Resistance Aura",
    spell: 19900,
    category: "FireResistanceAura",
    shared_category: "PaladinAura",
    single_aura: true,
    skip_auras: &[dbcenums::A_MOD_HEALING_PCT],
    ..Meta::DEFAULT
};

pub(crate) static FROST_RESISTANCE_AURA: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Frost Resistance Aura",
    spell: 19898,
    category: "FrostResistanceAura",
    shared_category: "PaladinAura",
    single_aura: true,
    skip_auras: &[dbcenums::A_MOD_HEALING_PCT],
    ..Meta::DEFAULT
};

pub(crate) static SHADOW_RESISTANCE_AURA: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Shadow Resistance Aura",
    spell: 19896,
    category: "ShadowResistanceAura",
    shared_category: "PaladinAura",
    single_aura: true,
    skip_auras: &[dbcenums::A_MOD_HEALING_PCT],
    ..Meta::DEFAULT
};

pub(crate) static FIRE_RESISTANCE_TOTEM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Fire Resistance Totem",
    spell: 10535,
    ..Meta::DEFAULT
};

pub(crate) static FROST_RESISTANCE_TOTEM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Frost Resistance Totem",
    spell: 10477,
    ..Meta::DEFAULT
};

pub(crate) static NATURE_RESISTANCE_TOTEM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Nature Resistance Totem",
    spell: 10599,
    ..Meta::DEFAULT
};

pub(crate) static ASPECT_OF_THE_WILD: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Aspect of the Wild",
    spell: 20190,
    ..Meta::DEFAULT
};

pub(crate) static GREATER_BLESSING_OF_KINGS: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Greater Blessing of Kings",
    spell: 25898,
    ..Meta::DEFAULT
};

pub(crate) static GREATER_BLESSING_OF_MIGHT: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Greater Blessing of Might",
    spell: 25916,
    ..Meta::DEFAULT
};

pub(crate) static GREATER_BLESSING_OF_WISDOM: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Greater Blessing of Wisdom",
    spell: 25918,
    ..Meta::DEFAULT
};

pub(crate) static GREATER_BLESSING_OF_SALVATION: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Greater Blessing of Salvation",
    spell: 25895,
    ..Meta::DEFAULT
};

pub(crate) static GREATER_BLESSING_OF_LIGHT: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Greater Blessing of Light",
    spell: 25890,
    category: "BlessingOfLight",
    ..Meta::DEFAULT
};

pub(crate) static INNERVATES: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Innervates",
    spell: 29166,
    category: "Innervate",
    ..Meta::DEFAULT
};

pub(crate) static POWER_INFUSIONS: Meta = Meta {
    kind: MetaKind::Buff,
    label: "Power Infusions",
    spell: 10060,
    category: "PowerInfusion",
    ..Meta::DEFAULT
};

pub(crate) static HUNTERS_MARK: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Hunter's Mark",
    spell: 14325,
    category: "HuntersMark",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static JUDGEMENT_OF_LIGHT: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Judgement of Light",
    spell: 20346,
    ..Meta::DEFAULT
};

pub(crate) static JUDGEMENT_OF_WISDOM: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Judgement of Wisdom",
    spell: 20355,
    ..Meta::DEFAULT
};

pub(crate) static CURSE_OF_ELEMENTS: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Curse of the Elements",
    spell: 1311680,
    category: "CurseOfElements",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static CURSE_OF_RECKLESSNESS: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Curse of Recklessness",
    spell: 11717,
    category: "MinorArmorReduction",
    per_stat: true,
    ..Meta::DEFAULT
};

pub(crate) static FAERIE_FIRE: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Faerie Fire",
    spell: 9907,
    category: "MinorArmorReduction",
    per_stat: true,
    ..Meta::DEFAULT
};

pub(crate) static EXPOSE_ARMOR: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Expose Armor",
    spell: 11198,
    category: "MajorArmorReduction",
    single_aura: true,
    full_combo_points: true,
    ..Meta::DEFAULT
};

pub(crate) static SUNDER_ARMOR: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Sunder Armor",
    spell: 11597,
    category: "MajorArmorReduction",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static GIFT_OF_ARTHAS: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Gift of Arthas",
    spell: 11374,
    category: "GiftOfArthasAura",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static DEMORALIZING_ROAR: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Demoralizing Roar",
    spell: 9898,
    category: "Demoralizing",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static DEMORALIZING_SHOUT: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Demoralizing Shout",
    spell: 11556,
    category: "Demoralizing",
    single_aura: true,
    ..Meta::DEFAULT
};

pub(crate) static THUNDER_CLAP: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Thunder Clap",
    spell: 11581,
    category: "AtkSpdReduction",
    ..Meta::DEFAULT
};

pub(crate) static INSECT_SWARM: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Insect Swarm",
    spell: 24977,
    ..Meta::DEFAULT
};

pub(crate) static SCORPID_STING: Meta = Meta {
    kind: MetaKind::Debuff,
    label: "Scorpid Sting",
    spell: 3043,
    ..Meta::DEFAULT
};

pub(crate) fn apply_generated_buffs(
    env: &mut Environment,
    unit: UnitId,
    raid: &Message,
    party: &Message,
    individual: &Message,
) -> Result<(), Refusal> {
    if party.bool("blood_pact") {
        permanent(env, unit, &BLOOD_PACT, false, 0)?;
    }
    if party.enum_number("battle_shout") != 0 {
        super::drivers::drive_battle_shout(env, unit, party)?;
    }
    if party.bool("devotion_aura") {
        permanent(env, unit, &DEVOTION_AURA, false, 0)?;
    }
    if party.bool("leader_of_the_pack") {
        permanent(env, unit, &LEADER_OF_THE_PACK, false, 0)?;
    }
    if party.enum_number("mana_spring_totem") != 0 {
        permanent(
            env,
            unit,
            &MANA_SPRING_TOTEM,
            false,
            tristate(party.enum_number("mana_spring_totem"), 0, 5),
        )?;
    }
    if party.int("mana_tide_totems") > 0 {
        super::drivers::drive_mana_tide_totems(env, unit, party)?;
    }
    if party.bool("moonkin_aura") {
        permanent(env, unit, &MOONKIN_AURA, false, 0)?;
    }
    if party.bool("retribution_aura") {
        super::drivers::drive_retribution_aura(env, unit, party)?;
    }
    if party.bool("concentration_aura") {
        permanent(env, unit, &CONCENTRATION_AURA, false, 0)?;
    }
    if party.bool("trueshot_aura") {
        permanent(env, unit, &TRUESHOT_AURA, false, 0)?;
    }
    if party.int("atiesh_mage") > 0 {
        super::drivers::drive_atiesh_mage(env, unit, party)?;
    }
    if party.int("atiesh_warlock") > 0 {
        super::drivers::drive_atiesh_warlock(env, unit, party)?;
    }
    if party.bool("strength_of_earth_totem") {
        permanent(env, unit, &STRENGTH_OF_EARTH_TOTEM, false, 0)?;
    }
    if party.bool("grace_of_air_totem") {
        super::drivers::drive_grace_of_air_totem(env, unit, party)?;
    }
    if party.bool("windfury_totem") {
        super::drivers::drive_windfury_totem(env, unit, party)?;
    }
    if party.int("atiesh_druid") > 0 {
        super::drivers::drive_atiesh_druid(env, unit, party)?;
    }
    if party.int("atiesh_priest") > 0 {
        super::drivers::drive_atiesh_priest(env, unit, party)?;
    }
    if party.bool("flametongue_totem") {
        super::drivers::drive_flametongue_totem(env, unit, party)?;
    }
    if raid.bool("arcane_brilliance") {
        permanent(env, unit, &ARCANE_BRILLIANCE, false, 0)?;
    }
    if raid.bool("prayer_of_spirit") {
        permanent(env, unit, &PRAYER_OF_SPIRIT, false, 0)?;
    }
    if raid.bool("gift_of_the_wild") {
        permanent(env, unit, &GIFT_OF_THE_WILD, false, 0)?;
    }
    if raid.bool("thorns") {
        permanent(env, unit, &THORNS, false, 0)?;
    }
    if raid.bool("prayer_of_fortitude") {
        permanent(env, unit, &PRAYER_OF_FORTITUDE, false, 0)?;
    }
    if raid.bool("prayer_of_shadow_protection") {
        permanent(env, unit, &PRAYER_OF_SHADOW_PROTECTION, false, 0)?;
    }
    if raid.bool("fire_resistance_aura") {
        permanent(env, unit, &FIRE_RESISTANCE_AURA, false, 0)?;
    }
    if raid.bool("frost_resistance_aura") {
        permanent(env, unit, &FROST_RESISTANCE_AURA, false, 0)?;
    }
    if raid.bool("shadow_resistance_aura") {
        permanent(env, unit, &SHADOW_RESISTANCE_AURA, false, 0)?;
    }
    if raid.bool("fire_resistance_totem") {
        permanent(env, unit, &FIRE_RESISTANCE_TOTEM, false, 0)?;
    }
    if raid.bool("frost_resistance_totem") {
        permanent(env, unit, &FROST_RESISTANCE_TOTEM, false, 0)?;
    }
    if raid.bool("nature_resistance_totem") {
        permanent(env, unit, &NATURE_RESISTANCE_TOTEM, false, 0)?;
    }
    if raid.bool("aspect_of_the_wild") {
        permanent(env, unit, &ASPECT_OF_THE_WILD, false, 0)?;
    }
    if individual.bool("greater_blessing_of_kings") {
        permanent(env, unit, &GREATER_BLESSING_OF_KINGS, false, 0)?;
    }
    if individual.bool("greater_blessing_of_might") {
        permanent(env, unit, &GREATER_BLESSING_OF_MIGHT, false, 0)?;
    }
    if individual.bool("greater_blessing_of_wisdom") {
        permanent(env, unit, &GREATER_BLESSING_OF_WISDOM, false, 0)?;
    }
    if individual.bool("greater_blessing_of_salvation") {
        permanent(env, unit, &GREATER_BLESSING_OF_SALVATION, false, 0)?;
    }
    if individual.bool("greater_blessing_of_light") {
        super::drivers::drive_greater_blessing_of_light(env, unit, individual)?;
    }
    if individual.int("innervates") > 0 {
        super::drivers::drive_innervates(env, unit, individual)?;
    }
    if individual.int("power_infusions") > 0 {
        super::drivers::drive_power_infusions(env, unit, individual)?;
    }
    let _ = raid;
    Ok(())
}

pub(crate) fn apply_generated_debuffs(
    env: &mut Environment,
    target: UnitId,
    debuffs: &Message,
    raid: &Message,
) -> Result<(), Refusal> {
    if debuffs.bool("hunters_mark") {
        permanent(env, target, &HUNTERS_MARK, false, 0)?;
    }
    if debuffs.bool("judgement_of_light") {
        super::drivers::drive_judgement_of_light(env, target, debuffs, raid)?;
    }
    if debuffs.bool("judgement_of_wisdom") {
        super::drivers::drive_judgement_of_wisdom(env, target, debuffs, raid)?;
    }
    if debuffs.bool("curse_of_elements") {
        permanent(env, target, &CURSE_OF_ELEMENTS, false, 0)?;
    }
    if debuffs.bool("curse_of_recklessness") {
        permanent(env, target, &CURSE_OF_RECKLESSNESS, false, 0)?;
    }
    if debuffs.bool("faerie_fire") {
        permanent(env, target, &FAERIE_FIRE, false, 0)?;
    }
    if debuffs.bool("expose_armor") {
        permanent(env, target, &EXPOSE_ARMOR, false, 0)?;
    }
    if debuffs.bool("sunder_armor") {
        super::drivers::drive_sunder_armor(env, target, debuffs, raid)?;
    }
    if debuffs.bool("gift_of_arthas") {
        permanent(env, target, &GIFT_OF_ARTHAS, false, 0)?;
    }
    if debuffs.bool("demoralizing_roar") {
        permanent(env, target, &DEMORALIZING_ROAR, false, 0)?;
    }
    if debuffs.bool("demoralizing_shout") {
        permanent(env, target, &DEMORALIZING_SHOUT, false, 0)?;
    }
    if debuffs.bool("thunder_clap") {
        permanent(env, target, &THUNDER_CLAP, false, 0)?;
    }
    if debuffs.bool("insect_swarm") {
        permanent(env, target, &INSECT_SWARM, false, 0)?;
    }
    if debuffs.bool("scorpid_sting") {
        permanent(env, target, &SCORPID_STING, false, 0)?;
    }
    let _ = raid;
    Ok(())
}
