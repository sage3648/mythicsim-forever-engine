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
//! - Dreadfrost Saber (274603) is a spell data damage proc (`NewSpellDataDamageProc`), as Fiery
//!   Plate Gauntlets' "Adds 4 fire damage to your weapon attack" is in the reference: its equip
//!   aura (1294939) hears the wearer's landed main hand melee hits and casts a Frost hit of 1
//!   (1294940) on the unit hit, every time.

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
];

/// The equip damage procs, as `forever_items_generated::ITEMS` lists the reference's (Fiery Plate
/// Gauntlets, Coldflame Saber): the equip aura's row is the trigger and the row it casts the
/// damage.
pub(crate) const EQUIP_DAMAGE_PROC_ITEMS: &[Registration] = &[Registration::Proc(
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
)];

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
