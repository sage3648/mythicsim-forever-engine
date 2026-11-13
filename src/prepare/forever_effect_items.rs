//! Under Forever's rules: effect items of the live client that the reference has no effect for,
//! whose spell rows the pinned store lacks and `data/forever_client_spells.rs` restates from the
//! client and the item's tooltip. Each runs through the shared constructor its effect's shape
//! names, as a reference item of the same shape does:
//!
//! - Serpentbane Crown (274609) is a damage on-use item (`NewSpellDataDamageOnUse`): its use
//!   casts Serpentbane's Fire hit and the 10 sec Fire damage over time it carries, on the item's
//!   own cooldowns (ItemEffect 234016: 5 min, and 10 sec on the shared category 1141).
//! - Bigger Rock on Bigger Stick (279480) and Rock on Stick (279852) are chance on hit weapons
//!   with no stated rate (ItemEffect trigger type 2), registered beside the reference's
//!   (`forever_items_generated::CHANCE_ON_HIT_ITEMS`) at the rate MythicSim assumes
//!   (`shared_items::CHANCE_ON_HIT_PPM`): a physical damage over time on the unit hit, whose
//!   ticks take no armor, as every physical tick here.
//! - Toofbreaker (286430) is a chance on hit weapon with no stated rate as well, registered the
//!   same way: a Physical hit of 42 on the unit hit, on the melee table. "Deals 2 times as much
//!   damage if the target is facing you": the hit is doubled for a character in front of the
//!   target or one the target swings at (`FACING_DAMAGE_ITEMS`).
//! - Pyromancer's Baton (286650) and Sword of Dismemberment (287005) are chance on hit weapons
//!   with no stated rate too, registered the same way: Ablaze, a Fire damage over time of 30
//!   every 3 sec for 9 sec on the unit hit, which rolls the magic hit as it goes on, as Flaming
//!   Incinerator's does; and Dismember, a Physical hit of 335 to 1005 on the melee table.
//! - Lightcrest Hammer (276419) is one more chance on hit weapon with no stated rate: Smite, a
//!   Holy hit of 105 on the magic table, doubled when every target is Undead
//!   (`CREATURE_TYPE_DAMAGE_ITEMS`).
//! - Dreadfrost Saber (274603) is a spell data damage proc (`NewSpellDataDamageProc`), as Fiery
//!   Plate Gauntlets' "Adds 4 fire damage to your weapon attack" is in the reference: its equip
//!   aura (1294939) hears the wearer's landed main hand melee hits and casts a Frost hit of 1
//!   (1294940) on the unit hit, every time.
//! - Maw of Min'loth (274608) is one too, as Skullflame Shield's Drain Life (18815 casting 18817)
//!   is in the reference: its equip aura (1295064) hears the melee hits the wearer takes and, 10%
//!   of the time and at most once in 3 sec, casts a Physical leech of 40 (1295065) on the
//!   attacker. Only a character who tanks the target is struck.
//! - Heart of the Primeval (274615) is a stat on-use trinket (`NewSimpleStatActive`), as Talisman
//!   of Ephemeral Power is in the reference: its client row carries the use as an item effect
//!   (19 spell damage and healing for 30 sec, on the use spell's own cooldowns), which the shared
//!   constructor reads (`ON_USE_STAT_ITEMS`).
//!
//! Two more items carry an equip effect that acts only when the wearer is struck, which nothing
//! here applies yet (`STRUCK_EFFECTS_NOT_PREPARED`): Zealot Plate (274651) and Girdle of Extracted
//! Answers (276742). Each is a plain item on a character the effect cannot reach, and refuses a
//! character it can.
//!
//! Spiderchitin Wrap (280016) raises the Feral Combat skill a Druid's forms attack with
//! (`FERAL_COMBAT_SKILL_ITEMS`).
//!
//! Fingerbreaker Stamp (276743) has nothing registered: "Equip: Increases Strength by 38 while
//! affected by a Bleed." (ItemEffect 237304, Fingerbreaker Stamp 1314784, a periodic dummy aura
//! (226) of 38 every 1000 ms, SpellEffect 1346731) acts only while the wearer bleeds, and nothing
//! in a fight here bleeds the wearer: a target only swings (preparation refuses a target with an
//! AI, `target.rs`) and the wearer's own side casts no bleed on the wearer. The stamp is worn with
//! its 6 Defense alone, exactly, as a struck effect is on a character no target attacks.

use crate::contracts::prepared_v2::ActionId;

use super::character::{cooldown_type, MajorCooldown};
use super::env::Environment;
use super::forever_items::Registration;
use super::resolve_spell;
use super::shared_items::{
    expand_variants, spell_data_proc_damage_spell, ItemVariant, ProcKind, SpellDataProc,
};
use super::sim::{Cooldown, MILLISECOND};
use super::spelldata::{must_find, Spell};
use super::Refusal;

/// A use that deals its spell row's damage to the target, on the item's own cooldowns.
pub(crate) struct OnUseDamageItem {
    pub(crate) item_id: i32,
    /// The spell the use casts.
    pub(crate) spell_id: i32,
    /// ItemEffect `CoolDownMSec`.
    cooldown_ms: i64,
    /// ItemEffect `SpellCategoryID` and `CategoryCoolDownMSec`.
    category: i32,
    category_cooldown_ms: i64,
    /// The creature types the use deals `multiplier` times its damage to, by the request's name,
    /// as the tooltip states it.
    pub(crate) creature_types: &'static [&'static str],
    pub(crate) multiplier: f64,
}

/// The damage on-use items.
pub(crate) const ON_USE_DAMAGE_ITEMS: [OnUseDamageItem; 1] = [OnUseDamageItem {
    // "Inflicts 2 times as much damage to Beast and Serpentine targets." A Serpentine target is
    // no creature type a request names, so only a Beast takes it.
    item_id: 274609,
    spell_id: 1295039,
    cooldown_ms: 300_000,
    category: 1141,
    category_cooldown_ms: 10_000,
    creature_types: &["MobTypeBeast"],
    multiplier: 2.0,
}];

/// The chance on hit weapons, as `forever_items_generated::CHANCE_ON_HIT_ITEMS` lists the
/// reference's: the damage row is its own trigger.
pub(crate) const CHANCE_ON_HIT_ITEMS: &[Registration] = &[
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            buff_spell_id: 1306816,
            is_weapon_proc: true,
            trigger_spell_id: 1306816,
            rateless: true,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 279480,
            item_name: "Bigger Rock on Bigger Stick",
        }],
    ),
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            buff_spell_id: 1309967,
            is_weapon_proc: true,
            trigger_spell_id: 1309967,
            rateless: true,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 279852,
            item_name: "Rock on Stick",
        }],
    ),
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            buff_spell_id: 1320269,
            is_weapon_proc: true,
            trigger_spell_id: 1320269,
            rateless: true,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 286430,
            item_name: "Toofbreaker",
        }],
    ),
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            buff_spell_id: 1320925,
            is_weapon_proc: true,
            trigger_spell_id: 1320925,
            rateless: true,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 286650,
            item_name: "Pyromancer's Baton",
        }],
    ),
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            buff_spell_id: 1324687,
            is_weapon_proc: true,
            trigger_spell_id: 1324687,
            rateless: true,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 287005,
            item_name: "Sword of Dismemberment",
        }],
    ),
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            buff_spell_id: 1299631,
            is_weapon_proc: true,
            trigger_spell_id: 1299631,
            rateless: true,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 276419,
            item_name: "Lightcrest Hammer",
        }],
    ),
];

/// A chance on hit whose hit deals `multiplier` times its damage to a target that faces the
/// wearer, as the tooltip states it.
pub(crate) struct FacingDamageItem {
    pub(crate) item_id: i32,
    /// The spell the chance on hit casts.
    pub(crate) spell_id: i32,
    pub(crate) multiplier: f64,
}

/// The chance on hit items that deal more to a target facing the wearer.
pub(crate) const FACING_DAMAGE_ITEMS: [FacingDamageItem; 1] = [FacingDamageItem {
    // Toofbreak (1320269): "Deals 2 times as much damage if the target is facing you." The 2 is
    // the row's second effect (SpellEffect 1354775, a dummy with base points 2), which the server
    // applies.
    item_id: 286430,
    spell_id: 1320269,
    multiplier: 2.0,
}];

/// After a chance on hit item registered its spell: a hit that deals more to a target facing the
/// wearer takes its multiplier as the spell's damage multiplier when the target faces the
/// character for the whole fight. That is a character in front of the target (the request's
/// `inFrontOfTarget`, which Backstab, Shred and Ambush read) or one a target swings at, since a
/// target faces its tank. A character behind the target deals the row's own amount.
pub(crate) fn apply_facing_damage(env: &mut Environment, item: i32) {
    let Some(facing) = FACING_DAMAGE_ITEMS
        .iter()
        .find(|facing| facing.item_id == item)
    else {
        return;
    };
    let unit = env.player;
    if !(env.sim.unit(unit).pseudo_stats.in_front_of_target || env.tanking()) {
        return;
    }
    let spell = env
        .sim
        .get_spell(unit, &ActionId::spell(facing.spell_id))
        .expect("the chance on hit registered its spell");
    env.sim.spell_mut(spell).damage_multiplier *= facing.multiplier;
}

/// A chance on hit whose hit deals `multiplier` times its damage to some creature types, as the
/// tooltip states it.
pub(crate) struct CreatureTypeDamageItem {
    pub(crate) item_id: i32,
    /// The spell the chance on hit casts.
    pub(crate) spell_id: i32,
    /// The creature types, by the request's name.
    pub(crate) creature_types: &'static [&'static str],
    pub(crate) multiplier: f64,
}

/// The chance on hit items that deal more to some creature types.
pub(crate) const CREATURE_TYPE_DAMAGE_ITEMS: [CreatureTypeDamageItem; 1] =
    [CreatureTypeDamageItem {
        // Lightcrest Hammer's Smite (1299631): "Deals 2 times as much damage to Undead." The 2 is
        // the row's second effect (SpellEffect 1323966, a dummy with base points 2), which the
        // server applies.
        item_id: 276419,
        spell_id: 1299631,
        creature_types: &["MobTypeUndead"],
        multiplier: 2.0,
    }];

/// After a chance on hit item registered its spell: a hit that deals more to some creature types
/// takes its multiplier as the spell's damage multiplier when every target is of one, as a
/// fight's targets are of one type (Serpentbane Crown's use reads its Beasts the same way).
pub(crate) fn apply_creature_type_damage(env: &mut Environment, item: i32) {
    let Some(versus) = CREATURE_TYPE_DAMAGE_ITEMS
        .iter()
        .find(|versus| versus.item_id == item)
    else {
        return;
    };
    let every = env.encounter.targets.iter().all(|&target| {
        versus
            .creature_types
            .contains(&env.sim.unit(target).mob_type.as_str())
    });
    if !every {
        return;
    }
    let unit = env.player;
    let spell = env
        .sim
        .get_spell(unit, &ActionId::spell(versus.spell_id))
        .expect("the chance on hit registered its spell");
    env.sim.spell_mut(spell).damage_multiplier *= versus.multiplier;
}

/// The items whose equip effect raises the Feral Combat skill, by item and amount.
///
/// Spiderchitin Wrap (280016): "Equip: Increases Feral Combat +1." (Wowhead's Forever tooltip, 13
/// November 2026). ItemEffect 238248 is an equip (trigger type 1) of Increased Feral Combat
/// (1320726), whose SpellEffect 1355535 is a skill aura (30) of 1 on skill line 3014. SkillLine
/// 3014 is "Feral Combat", in the Weapon Skills category (6) beside Swords and Defense, and
/// SkillRaceClassInfo 2554 gives it to Druids alone (ClassMask 1024), through the Feral Combat
/// spell 1306742, a weapon proficiency (effect 25). It is read as the weapon skill a Druid's forms
/// attack with (`Sim::update_weapon_skills`); anyone else has no such line and gains nothing.
pub(crate) const FERAL_COMBAT_SKILL_ITEMS: [(i32, f64); 1] = [(280016, 1.0)];

/// The Feral Combat skill the item raises, if it is one of these.
pub(crate) fn feral_combat_skill(item: i32) -> Option<f64> {
    FERAL_COMBAT_SKILL_ITEMS
        .iter()
        .find(|(id, _)| *id == item)
        .map(|(_, amount)| *amount)
}

/// The equip damage procs, as `forever_items_generated::ITEMS` lists the reference's (Fiery Plate
/// Gauntlets, Coldflame Saber): the equip aura's row is the trigger and the row it casts the
/// damage.
pub(crate) const EQUIP_DAMAGE_PROC_ITEMS: &[Registration] = &[
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            trigger_spell_id: 1294939,
            buff_spell_id: 1294940,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 274603,
            item_name: "Dreadfrost Saber",
        }],
    ),
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            trigger_spell_id: 1295064,
            buff_spell_id: 1295065,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 274608,
            item_name: "Maw of Min'loth",
        }],
    ),
    // Farraki Kris's equip aura hears the wearer's harmful spells and casts a Periodic Leech on
    // the target (docs/mechanics.md).
    Registration::Proc(
        ProcKind::Damage,
        SpellDataProc {
            trigger_spell_id: 1294644,
            buff_spell_id: 1294645,
            ..SpellDataProc::EMPTY
        },
        &[ItemVariant {
            item_id: 274647,
            item_name: "Farraki Kris",
        }],
    ),
];

/// The item's equip damage proc, expanded to its variant, if it is one of these.
pub(crate) fn equip_damage_proc(item: i32) -> Option<SpellDataProc> {
    equip_damage_procs().find(|cfg| cfg.item_id == item)
}

/// Every equip damage proc, expanded to its variants.
pub(crate) fn equip_damage_procs() -> impl Iterator<Item = SpellDataProc> {
    EQUIP_DAMAGE_PROC_ITEMS
        .iter()
        .flat_map(|registration| match registration {
            Registration::Proc(_, cfg, variants) => expand_variants(*cfg, variants),
            _ => Vec::new(),
        })
}

/// The item's damage on-use, if it is one of these.
pub(crate) fn on_use_damage_item(item: i32) -> Option<&'static OnUseDamageItem> {
    ON_USE_DAMAGE_ITEMS.iter().find(|use_| use_.item_id == item)
}

/// The spell row an on-use item's use casts.
pub(crate) fn on_use_damage_row(item: i32) -> Option<&'static Spell> {
    on_use_damage_item(item).map(|use_| must_find(use_.spell_id))
}

/// Go `registerSpellDataOnUseCooldown` over `spellDataProcDamageSpell`, with the item effect's
/// cooldowns: the use is the item's action and a DPS cooldown.
pub(crate) fn apply_on_use_damage(env: &mut Environment, use_: &OnUseDamageItem) {
    let unit = env.player;
    let row = must_find(use_.spell_id);
    let mut config = spell_data_proc_damage_spell(env, row, false);
    config.action_id = ActionId::item(use_.item_id);
    config.cast = resolve_spell::cast(row);
    config.cast.cd = Cooldown {
        timer: Some(env.sim.new_timer(unit)),
        duration: use_.cooldown_ms * MILLISECOND,
    };
    config.cast.shared_cd = Cooldown {
        timer: Some(env.sim.category_timer(unit, use_.category)),
        duration: use_.category_cooldown_ms * MILLISECOND,
    };
    let spell = env.sim.register_spell(unit, config);
    env.sim.add_major_cooldown(
        unit,
        MajorCooldown {
            spell,
            priority: 0,
            cooldown_type: cooldown_type::DPS,
            allow_spell_queueing: false,
            timings: Vec::new(),
        },
    );
}

/// The stat on-use items: the client row's item effect holds the use (its buff, stats, duration
/// and cooldowns), and `shared_on_use`'s `SimpleStatActive` registers it as it does a reference
/// trinket's.
///
/// Heart of the Primeval (274615): "Use: Increases damage and healing done by magical spells and
/// effects by up to 19 for 30 sec. (6 Min Cooldown)" (Wowhead's Forever tooltip, 12 November
/// 2026). The client's rows (wago.tools `product=wowf`, 12 November 2026): ItemEffect 234049 is a
/// use (trigger type 0) of Heart of the Primeval (1295233) with no cooldowns of its own (-1), so
/// the spell's stand: SpellCooldowns 99612 has a recovery of 360000 ms and a category recovery of
/// 30000 ms on SpellCategories 184157's category 1141, the one the reference's stat trinkets
/// share. SpellEffect 1317148 is a damage done aura (13) of 19 on every magic school (mask 126)
/// and 1348726 a healing done aura (135) of 19; SpellMisc 842632 has DurationIndex 9, 30 sec.
pub(crate) const ON_USE_STAT_ITEMS: &[i32] = &[274615];

/// Whether the item is one of the stat on-use items.
pub(crate) fn on_use_stat_item(item: i32) -> bool {
    ON_USE_STAT_ITEMS.contains(&item)
}

/// Who an equip effect that acts when the wearer is struck can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StruckEffectReach {
    /// A character a target attacks who has a mana bar: the effect restores mana.
    TankWithMana,
    /// A character a target attacks who holds a shield: the effect acts on a block.
    TankWithShield,
}

/// An equip effect that acts only when the wearer is struck and that nothing here applies yet.
pub(crate) struct StruckEffectNotPrepared {
    pub(crate) item_id: i32,
    /// The effect, as the refusal names it.
    pub(crate) effect: &'static str,
    pub(crate) reach: StruckEffectReach,
}

/// The items whose equip effect acts only when the wearer is struck. A character the effect
/// cannot reach wears the item as a plain one, exactly; one it can reach is refused, since the
/// effect would change that fight.
///
/// Zealot Plate (274651): "Equip: When struck in combat, has a chance to energize you for 66 to
/// 110 mana. This effect cannot occur more than once every 15 sec. (15s cooldown)" (Wowhead's
/// Forever tooltip, 12 November 2026). ItemEffect 233963 is an equip (trigger type 1) of Proc Self
/// Energize 88 (1294668): SpellAuraOptions 246043 has ProcChance 100, ProcTypeMask 40 (melee
/// swings and melee abilities taken) and a proc recovery of 15000 ms, and SpellEffect 1316157
/// casts Zealotry (1294666), whose SpellEffect 1316155 is an Energize (30) of 88 with a variance
/// of 0.5 (66 to 110) on mana. A character no target attacks is never struck, and one with no
/// mana bar gains nothing.
///
/// Girdle of Extracted Answers (276742): "Equip: Deals 20 to 30 Physical damage every time you
/// block." (Wowhead's Forever tooltip, 12 November 2026). ItemEffect 236443 is an equip of Shield
/// Spike (1308777): SpellEffect 1337499 is a damage on proc aura (43) of 25 with a variance of
/// 0.4 (20 to 30), SpellAuraOptions 247236 has ProcChance 100 and ProcTypeMask 40, and
/// SpellCooldowns 100881 a recovery of 5000 ms. Only a character a target attacks who holds a
/// shield blocks.
pub(crate) const STRUCK_EFFECTS_NOT_PREPARED: [StruckEffectNotPrepared; 2] = [
    StruckEffectNotPrepared {
        item_id: 274651,
        effect: "mana when struck",
        reach: StruckEffectReach::TankWithMana,
    },
    StruckEffectNotPrepared {
        item_id: 276742,
        effect: "damage on block",
        reach: StruckEffectReach::TankWithShield,
    },
];

/// Refuses a worn item of [`STRUCK_EFFECTS_NOT_PREPARED`] on a character its effect can reach.
/// `holds_shield` is whether any worn or swapped in item is a shield.
pub(crate) fn struck_effect_not_prepared(
    env: &Environment,
    item: i32,
    holds_shield: bool,
) -> Result<(), Refusal> {
    let Some(struck) = STRUCK_EFFECTS_NOT_PREPARED
        .iter()
        .find(|struck| struck.item_id == item)
    else {
        return Ok(());
    };
    if !env.tanking() {
        return Ok(());
    }
    let reached = match struck.reach {
        StruckEffectReach::TankWithMana => env.sim.unit(env.player).mana_bar.enabled,
        StruckEffectReach::TankWithShield => holds_shield,
    };
    if !reached {
        return Ok(());
    }
    Err(Refusal::new(
        "item_effect",
        format!(
            "item {item} has an effect not prepared yet ({} on a character the target attacks)",
            struck.effect
        ),
    ))
}
