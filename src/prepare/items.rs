//! Go sim/core/database.go: items, enchants and random suffixes from the imported database,
//! and the equipment a request equips.

use crate::contracts::request::{Message, Value};
use crate::data::{self, CLIENT_ITEMS, ENCHANTS, ITEMS, LIVE_ITEMS, RANDOM_SUFFIXES};

use super::sim::Rules;
use super::stats::{Stat, Stats};
use super::Refusal;

/// Go `proto.ItemSlot` order.
pub(crate) const NUM_ITEM_SLOTS: usize = 17;
pub(crate) mod slot {
    pub const HEAD: usize = 0;
    pub const NECK: usize = 1;
    pub const SHOULDER: usize = 2;
    pub const BACK: usize = 3;
    pub const CHEST: usize = 4;
    pub const WRIST: usize = 5;
    pub const HANDS: usize = 6;
    pub const WAIST: usize = 7;
    pub const LEGS: usize = 8;
    pub const FEET: usize = 9;
    pub const FINGER1: usize = 10;
    pub const FINGER2: usize = 11;
    pub const TRINKET1: usize = 12;
    pub const TRINKET2: usize = 13;
    pub const MAIN_HAND: usize = 14;
    pub const OFF_HAND: usize = 15;
    pub const RANGED: usize = 16;
}

/// Go `Enchant`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Enchant {
    pub effect_id: i32,
    pub stats: Stats,
    pub pseudo_stats: Vec<f64>,
    pub weapon_damage: f64,
    pub enchant_effects: Vec<Message>,
    pub name: String,
    pub item_type: String,
    pub enchant_type: String,
    pub extra_types: Vec<String>,
}

/// Go `RandomSuffix`.
#[derive(Clone, Debug, Default)]
pub(crate) struct RandomSuffix {
    pub id: i32,
    pub name: String,
    pub stats: Stats,
}

/// Go `Item` as `NewItem` builds it for one equipped item.
#[derive(Clone, Debug, Default)]
pub(crate) struct Item {
    pub id: i32,
    pub name: String,
    pub item_type: String,
    pub armor_type: String,
    pub weapon_type: String,
    pub hand_type: String,
    pub ranged_weapon_type: String,
    pub weapon_damage_min: f64,
    pub weapon_damage_max: f64,
    pub swing_speed: f64,
    pub quality_modifier: f64,
    pub stats: Stats,
    pub pseudo_stats: Vec<f64>,
    pub unique: bool,
    pub limit_category: i32,
    pub set_name: String,
    pub set_id: i32,
    pub class_allowlist: Vec<String>,
    pub random_suffix: RandomSuffix,
    pub enchant: Enchant,
    pub temp_enchant: i32,
    pub rand_prop_points: i32,
    pub item_effects: Vec<Message>,
    /// The item's whole database row, for its scaling options and area stats.
    pub row: Option<std::rc::Rc<Message>>,
}

impl Item {
    pub fn is_empty(&self) -> bool {
        self.id == 0
    }
}

/// Equipment copies share item data until a swap or an imbue changes a slot.
#[derive(Clone, Debug, Default)]
pub(crate) struct Equipment(std::rc::Rc<[Item; NUM_ITEM_SLOTS]>);

impl std::ops::Deref for Equipment {
    type Target = [Item; NUM_ITEM_SLOTS];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Equipment {
    fn deref_mut(&mut self) -> &mut Self::Target {
        std::rc::Rc::make_mut(&mut self.0)
    }
}

impl<'a> IntoIterator for &'a Equipment {
    type Item = &'a Item;
    type IntoIter = std::slice::Iter<'a, Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Go `EnchantFromProto`, by effect ID.
pub(crate) fn enchant(effect_id: i32) -> Option<Enchant> {
    let row = data::proto_row(&ENCHANTS, "proto.SimEnchant", i64::from(effect_id))?;
    Some(Enchant {
        effect_id: row.i32("effect_id"),
        stats: Stats::from_proto_array(&row.f64s("stats")),
        pseudo_stats: row.f64s("pseudo_stats"),
        weapon_damage: row.f64("weapon_damage"),
        enchant_effects: row
            .messages("enchant_effects")
            .into_iter()
            .cloned()
            .collect(),
        name: row.str("name").to_string(),
        item_type: row.enum_name("type"),
        enchant_type: row.enum_name("enchant_type"),
        extra_types: row.enum_names("extra_types"),
    })
}

fn random_suffix(id: i32) -> Option<RandomSuffix> {
    let row = data::proto_row(&RANDOM_SUFFIXES, "proto.ItemRandomSuffix", i64::from(id))?;
    Some(RandomSuffix {
        id: row.i32("id"),
        name: row.str("name").to_string(),
        stats: Stats::from_proto_array(&row.f64s("stats")),
    })
}

/// A database item's scaling option at key 0, which `NewItem` reads.
pub(crate) fn scaling_option(row: &Message) -> Option<&Message> {
    row.get("scaling_options").and_then(|value| match value {
        Value::Map(entries) => entries.iter().find_map(|(key, value)| match (key, value) {
            (Value::Int(0), Value::Message(message)) => Some(message),
            _ => None,
        }),
        _ => None,
    })
}

/// Go `stats.FromProtoMap` over a `map<int32, double>` field.
pub(crate) fn stats_from_map(message: &Message, field: &str) -> Stats {
    let mut out = Stats::default();
    if let Some(Value::Map(entries)) = message.get(field) {
        for (key, value) in entries {
            if let (Value::Int(key), Value::Double(value)) = (key, value) {
                out.0[*key as usize] = *value;
            }
        }
    }
    out
}

/// Go `GetItemByID` with `ItemFromProto`: the database item without per-instance data.
pub(crate) fn database_item(id: i32) -> Option<Item> {
    let row = data::proto_row(&ITEMS, "proto.SimItem", i64::from(id))?;
    Some(item_from_row(row))
}

/// [`database_item`] under the given rules. Forever's rules read the live client's row of an
/// item whose reference row the live game has changed (`data/forever-live-items.jsonl`), and
/// also know the equippable items of the live client that the reference database lacks
/// (`data/forever-client-items.jsonl`), the low level gear a leveling character wears.
pub(crate) fn database_item_under(id: i32, rules: Rules) -> Option<Item> {
    if rules != Rules::Forever {
        return database_item(id);
    }
    if let Some(row) = data::proto_row(&LIVE_ITEMS, "proto.SimItem", i64::from(id)) {
        return Some(item_from_row(row));
    }
    database_item(id).or_else(|| {
        let row = data::proto_row(&CLIENT_ITEMS, "proto.SimItem", i64::from(id))?;
        Some(item_from_row(row))
    })
}

fn item_from_row(row: std::rc::Rc<Message>) -> Item {
    Item {
        id: row.i32("id"),
        name: row.str("name").to_string(),
        item_type: row.enum_name("type"),
        armor_type: row.enum_name("armor_type"),
        weapon_type: row.enum_name("weapon_type"),
        hand_type: row.enum_name("hand_type"),
        ranged_weapon_type: row.enum_name("ranged_weapon_type"),
        swing_speed: row.f64("weapon_speed"),
        quality_modifier: row.f64("quality_modifier"),
        pseudo_stats: row.f64s("pseudo_stats"),
        unique: row.bool("unique"),
        limit_category: row.i32("limit_category"),
        set_name: row.str("set_name").to_string(),
        set_id: row.i32("set_id"),
        class_allowlist: row.enum_names("class_allowlist"),
        item_effects: row.messages("item_effects").into_iter().cloned().collect(),
        row: Some(row),
        ..Item::default()
    }
}

/// Go `NewItem` for one `ItemSpec`.
pub(crate) fn new_item(spec: &Message, rules: Rules) -> Result<Item, Refusal> {
    let id = spec.i32("id");
    let mut item = database_item_under(id, rules)
        .ok_or_else(|| Refusal::new("unknown_item", format!("no item with id {id}")))?;
    if !spec.ints("gems").iter().all(|gem| *gem == 0) {
        return Err(Refusal::new(
            "gems",
            format!("item {id} has gems, which the database has none of"),
        ));
    }
    let row = item.row.clone().expect("a database item keeps its row");
    let scaling = scaling_option(&row).ok_or_else(|| {
        Refusal::new(
            "unknown_item",
            format!("item {id} has no base scaling option"),
        )
    })?;
    item.stats = stats_from_map(scaling, "stats");
    if rules == Rules::Forever {
        gear_attack_power_is_ranged_too(&mut item.stats);
    }
    item.weapon_damage_max = scaling.f64("weapon_damage_max");
    item.weapon_damage_min = scaling.f64("weapon_damage_min");
    item.rand_prop_points = scaling.i32("rand_prop_points");
    let suffix = spec.i32("random_suffix");
    if suffix != 0 {
        item.random_suffix = random_suffix(suffix).ok_or_else(|| {
            Refusal::new("unknown_item", format!("no random suffix with id {suffix}"))
        })?;
    }
    let enchant_id = spec.i32("enchant");
    if enchant_id != 0 {
        // Go silently ignores an enchant it does not know.
        if let Some(found) = enchant(enchant_id) {
            item.enchant = found;
        }
    }
    Ok(item)
}

/// Forever's rules: attack power on an item raises ranged attack power by as much. The equip
/// spells that read "+N Attack Power" state both auras, 99 (attack power) and 124 (ranged attack
/// power), at the same value (Attack Power 22, 9332, both at 22 in client 1.60.1.70862, the
/// equip spell of Counterattack Lodestone 18537), where most item rows carry attack power alone (Devilsaur Gauntlets 15063, Cape of the Black
/// Baron 13340). A row that already states ranged attack power keeps its own. Random suffixes
/// ("of Power") and enchants carry their own ranged attack power and are left as they are; Death's
/// Embrace (24153) is aura 99 alone.
fn gear_attack_power_is_ranged_too(stats: &mut Stats) {
    if stats[Stat::AttackPower] > 0.0 && stats[Stat::RangedAttackPower] == 0.0 {
        stats[Stat::RangedAttackPower] = stats[Stat::AttackPower];
    }
}

/// Go `ItemTypeToSlot`.
fn item_type_to_slot(item_type: &str) -> Option<usize> {
    Some(match item_type {
        "ItemTypeHead" => slot::HEAD,
        "ItemTypeNeck" => slot::NECK,
        "ItemTypeShoulder" => slot::SHOULDER,
        "ItemTypeBack" => slot::BACK,
        "ItemTypeChest" => slot::CHEST,
        "ItemTypeWrist" => slot::WRIST,
        "ItemTypeHands" => slot::HANDS,
        "ItemTypeWaist" => slot::WAIST,
        "ItemTypeLegs" => slot::LEGS,
        "ItemTypeFeet" => slot::FEET,
        "ItemTypeFinger" => slot::FINGER1,
        "ItemTypeTrinket" => slot::TRINKET1,
        "ItemTypeWeapon" => slot::MAIN_HAND,
        "ItemTypeRanged" => slot::RANGED,
        _ => return None,
    })
}

/// Go `Equipment.EquipItem`.
fn equip_item(equipment: &mut Equipment, item: Item) -> Result<(), Refusal> {
    match item.item_type.as_str() {
        "ItemTypeFinger" => {
            let target = if equipment[slot::FINGER1].is_empty() {
                slot::FINGER1
            } else {
                slot::FINGER2
            };
            equipment[target] = item;
        }
        "ItemTypeTrinket" => {
            let target = if equipment[slot::TRINKET1].is_empty() {
                slot::TRINKET1
            } else {
                slot::TRINKET2
            };
            equipment[target] = item;
        }
        "ItemTypeWeapon" => {
            if item.weapon_type == "WeaponTypeShield"
                && equipment[slot::MAIN_HAND].hand_type != "HandTypeTwoHand"
            {
                equipment[slot::OFF_HAND] = item;
            } else if item.hand_type == "HandTypeMainHand" || item.hand_type == "HandTypeUnknown" {
                equipment[slot::MAIN_HAND] = item;
            } else if item.hand_type == "HandTypeOffHand" {
                equipment[slot::OFF_HAND] = item;
            } else if item.hand_type == "HandTypeOneHand" || item.hand_type == "HandTypeTwoHand" {
                if equipment[slot::MAIN_HAND].is_empty() {
                    equipment[slot::MAIN_HAND] = item;
                } else if equipment[slot::OFF_HAND].is_empty() {
                    equipment[slot::OFF_HAND] = item;
                }
            }
        }
        other => {
            let index = item_type_to_slot(other).ok_or_else(|| {
                Refusal::new(
                    "unknown_item",
                    format!("item {} has type {other}, which has no slot", item.id),
                )
            })?;
            equipment[index] = item;
        }
    }
    Ok(())
}

/// Go `ProtoToEquipment(...).inArea(areaTypes)`.
pub(crate) fn equipment(
    spec: Option<&Message>,
    area_types: &[String],
    rules: Rules,
) -> Result<Equipment, Refusal> {
    let mut equipment: Equipment = Default::default();
    if let Some(spec) = spec {
        for (index, item) in spec.messages("items").into_iter().enumerate() {
            if index >= NUM_ITEM_SLOTS {
                return Err(Refusal::new(
                    "unknown_item",
                    "more items than slots".to_string(),
                ));
            }
            if item.i32("id") != 0 {
                equip_item(&mut equipment, new_item(item, rules)?)?;
            }
        }
    }
    if !area_types.is_empty() {
        for item in equipment.iter_mut() {
            *item = in_area(std::mem::take(item), area_types);
        }
    }
    Ok(equipment)
}

/// Go `Item.inArea`: the item's stats with those of its base scaling for the areas.
pub(crate) fn in_area(mut item: Item, area_types: &[String]) -> Item {
    if item.is_empty() || area_types.is_empty() {
        return item;
    }
    let row = item.row.clone().expect("a database item keeps its row");
    if let Some(scaling) = scaling_option(&row) {
        for area in scaling.messages("area_stats") {
            if area_types.contains(&area.enum_name("area_type")) {
                item.stats = item.stats.add(&stats_from_map(area, "stats"));
            }
        }
    }
    item
}

/// Go `PercentPseudoStats` and `FromPseudoStatsProto`.
pub(crate) fn stats_from_pseudo_stats(pseudo: &[f64]) -> Stats {
    let value = |name: &str| {
        let index = crate::contracts::request::enum_number("proto.PseudoStat", name)
            .unwrap_or_else(|| panic!("proto.PseudoStat has no {name}"))
            as usize;
        pseudo.get(index).copied().unwrap_or(0.0)
    };
    let mut out = Stats::default();
    out[Stat::PhysicalHitPercent] = value("PseudoStatMeleeHitPercent");
    out[Stat::SpellHitPercent] = value("PseudoStatSpellHitPercent");
    out[Stat::PhysicalCritPercent] = value("PseudoStatMeleeCritPercent");
    out[Stat::SpellCritPercent] = value("PseudoStatSpellCritPercent");
    out[Stat::BlockPercent] = value("PseudoStatBlockPercent");
    out[Stat::RangedHitPercent] =
        value("PseudoStatRangedHitPercent") - value("PseudoStatMeleeHitPercent");
    out[Stat::RangedCritPercent] =
        value("PseudoStatRangedCritPercent") - value("PseudoStatMeleeCritPercent");
    out[Stat::DodgePercent] = value("PseudoStatDodgePercent");
    out[Stat::ParryPercent] = value("PseudoStatParryPercent");
    out[Stat::ExpertisePercent] = value("PseudoStatExpertisePercent");
    out
}

/// Go `ItemEquipmentBaseStats`.
fn item_base_stats(item: &Item) -> Stats {
    if item.is_empty() {
        return Stats::default();
    }
    Stats::default()
        .add(&item.stats)
        .add(&stats_from_pseudo_stats(&item.pseudo_stats))
        .add(&item.random_suffix.stats)
}

/// Go `ItemEquipmentGemAndEnchantStats`, without gems, which the database has none of.
fn item_enchant_stats(item: &Item) -> Stats {
    if item.is_empty() {
        return Stats::default();
    }
    Stats::default()
        .add(&item.enchant.stats)
        .add(&stats_from_pseudo_stats(&item.enchant.pseudo_stats))
}

/// Go forever_rules.go `unifyGearHitAndCrit`.
fn unify_gear_hit_and_crit(mut equip: Stats) -> Stats {
    use super::character::constants::*;
    let hit = equip[Stat::MeleeHitRating] / PHYSICAL_HIT_RATING_PER_HIT_PERCENT
        + equip[Stat::SpellHitRating] / SPELL_HIT_RATING_PER_HIT_PERCENT;
    let crit = equip[Stat::MeleeCritRating] / PHYSICAL_CRIT_RATING_PER_CRIT_PERCENT
        + equip[Stat::SpellCritRating] / SPELL_CRIT_RATING_PER_CRIT_PERCENT;
    equip[Stat::MeleeHitRating] = hit * PHYSICAL_HIT_RATING_PER_HIT_PERCENT;
    equip[Stat::SpellHitRating] = hit * SPELL_HIT_RATING_PER_HIT_PERCENT;
    equip[Stat::MeleeCritRating] = crit * PHYSICAL_CRIT_RATING_PER_CRIT_PERCENT;
    equip[Stat::SpellCritRating] = crit * SPELL_CRIT_RATING_PER_CRIT_PERCENT;
    equip
}

/// Go `Equipment.Stats`.
pub(crate) fn equipment_stats(equipment: &Equipment) -> Stats {
    let mut out = Stats::default();
    for item in equipment {
        out = out.add(&item_base_stats(item));
        out = out.add(&item_enchant_stats(item));
    }
    unify_gear_hit_and_crit(out)
}

/// `stats.FromProtoMap(effect.GetScalingOptions()[int32(0)].GetStats())`: the stats an item or
/// enchant effect carries at the base scaling, zero where the effect names none.
pub(crate) fn effect_stats(effect: &Message) -> Stats {
    scaling_option(effect).map_or_else(Stats::default, |option| stats_from_map(option, "stats"))
}

#[cfg(test)]
mod clone_tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    struct CountingAllocator;

    thread_local! {
        static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
    }

    fn count_allocation() {
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
    }

    // SAFETY: Every operation forwards the original pointer and layout to System.
    // The counter uses allocation-free thread-local storage and does not touch allocations.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            count_allocation();
            // SAFETY: The caller supplies the layout required by GlobalAlloc.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            // SAFETY: The allocation came from System with this same layout.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            count_allocation();
            // SAFETY: The caller supplies the layout required by GlobalAlloc.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            count_allocation();
            // SAFETY: The caller supplies a System allocation and valid replacement size.
            unsafe { System.realloc(ptr, layout, size) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn cloning_equipment_does_not_allocate() {
        let mut equipment: Equipment = Default::default();
        equipment[slot::MAIN_HAND].name = "Weapon".to_string();
        equipment[slot::MAIN_HAND].enchant.name = "Enchant".to_string();
        ALLOCATIONS.with(|count| count.set(Some(0)));
        let copy = std::hint::black_box(&equipment).clone();
        let allocations = ALLOCATIONS.with(|count| count.replace(None).unwrap());
        assert_eq!(copy[slot::MAIN_HAND].name, "Weapon");
        assert_eq!(
            allocations, 0,
            "equipment copies should share unchanged item data"
        );
    }
    #[test]
    fn equipment_copies_keep_slot_and_nested_mutations_independent() {
        let mut original: Equipment = Default::default();
        original[slot::MAIN_HAND].name = "Original".to_string();
        original[slot::MAIN_HAND].enchant.name = "Original enchant".to_string();
        original[slot::MAIN_HAND].pseudo_stats = vec![3.0];
        let mut changed = original.clone();
        changed[slot::MAIN_HAND].temp_enchant = 283;
        changed[slot::MAIN_HAND].enchant.name.push_str(" changed");
        changed[slot::MAIN_HAND].pseudo_stats[0] = 9.0;
        changed[slot::OFF_HAND] = Item {
            id: 123,
            ..Item::default()
        };
        assert_eq!(original[slot::MAIN_HAND].temp_enchant, 0);
        assert_eq!(original[slot::MAIN_HAND].enchant.name, "Original enchant");
        assert_eq!(original[slot::MAIN_HAND].pseudo_stats, [3.0]);
        assert_eq!(original[slot::OFF_HAND].id, 0);
        original.iter_mut().for_each(|item| item.temp_enchant = 5);
        assert_eq!(changed[slot::MAIN_HAND].temp_enchant, 283);
        assert_eq!(changed[slot::OFF_HAND].id, 123);
    }
}
