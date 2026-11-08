//! The character a request prepares: its level and the spell ranks it has trained.
//!
//! Preparation reads both everywhere a spell's rank or amount depends on them, which is most
//! of the class code: the rank a ladder resolves to ([`super::spelldata::Ladder::highest`]) and
//! the caster level an amount scales to ([`character_level`]). They are set once per request
//! for the whole preparation ([`with_character`]), the way a rotation is compiled for its
//! current target (`rotation::with_current_target`). The engine prepares on one thread and
//! fights from the prepared document, so nothing reads them after preparation.
//!
//! A request without a level is a level 60 character, and one without a spellbook knows every
//! rank its level allows, so a level 60 request prepares exactly as it did before levels.

use std::cell::RefCell;
use std::collections::BTreeSet;

use super::spelldata::Spell;
use super::stats::{Stat, Stats};
use crate::contracts::request::Message;

/// Forever's level cap, the level every reference build plays at.
pub(crate) const MAX_LEVEL: i32 = 60;

/// The lowest level a levelling character sims at: below 10 a character has no talents and
/// almost no rotation to speak of.
pub(crate) const MIN_LEVEL: i32 = 10;

/// The classes whose preparation has been checked below level 60: every spell they register
/// is one the character has learned, or falls back to one it has. A class not listed refuses a
/// levelling character rather than prepare spells it does not know.
pub(crate) const LEVELLED_CLASSES: &[&str] = &[
    "ClassDruid",
    "ClassHunter",
    "ClassMage",
    "ClassPaladin",
    "ClassPriest",
    "ClassRogue",
    "ClassShaman",
    "ClassWarlock",
    "ClassWarrior",
];

/// Why a character of this class and level cannot be prepared, or None when it can.
pub(crate) fn level_refusal(class: &str, level: i32, has_sheet: bool) -> Option<String> {
    if level == MAX_LEVEL {
        return None;
    }
    if !(MIN_LEVEL..MAX_LEVEL).contains(&level) {
        return Some(format!("player level {level} is not supported"));
    }
    if !LEVELLED_CLASSES.contains(&class) {
        let name = class.strip_prefix("Class").unwrap_or(class);
        return Some(format!(
            "{name} is not simmed below level 60 yet (level {level})"
        ));
    }
    if !has_sheet {
        return Some(format!(
            "a level {level} character needs the character sheet the MythicSim addon 0.4.0 or later exports"
        ));
    }
    None
}

/// The target levels a character of `level` can fight: a level 60 character a level 60 to 63
/// target, a levelling one a target from its own level to 3 above.
pub(crate) fn target_levels(level: i32) -> std::ops::RangeInclusive<i32> {
    if level == MAX_LEVEL {
        MAX_LEVEL..=MAX_LEVEL + 3
    } else {
        level..=level + 3
    }
}

/// The character being prepared.
#[derive(Clone, Debug, Default)]
pub(crate) struct Character {
    pub level: i32,
    /// The spell ids its spellbook holds, when the export carried them.
    pub known_spells: Option<BTreeSet<i32>>,
    /// Its character sheet, when the export carried it.
    pub sheet: Option<Sheet>,
    /// What its sheet says about the numbers the engine has for level 60 only ([`calibrate`]).
    pub calibration: Calibration,
    /// Every rank of each ladder preparation asked for that the character knows none of
    /// ([`unlearned_rank`]).
    pub unlearned: BTreeSet<i32>,
    /// For each ladder preparation resolved to a rank the character knows, its other ranks and
    /// that rank ([`rewrite_rotations`]).
    pub known_as: std::collections::BTreeMap<i32, i32>,
    /// The spells a first preparation registered for the player and did not flag unlearned,
    /// with their class spell name: what a rotation can be pointed at ([`known_rank_of_name`]).
    pub registered: std::collections::BTreeMap<i32, String>,
}

/// The attributes in the sheet's order.
const ATTRIBUTES: [Stat; 5] = [
    Stat::Strength,
    Stat::Agility,
    Stat::Stamina,
    Stat::Intellect,
    Stat::Spirit,
];

/// The part of the addon's character sheet ([`crate::contracts::request`] `CharacterSheet`)
/// levels read.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Sheet {
    /// Attributes without gear or buffs, in [`ATTRIBUTES`] order.
    pub base: [f64; 5],
    /// Attributes as the sheet shows them, gear and buffs included.
    pub attributes: [f64; 5],
    pub health: f64,
    pub mana: f64,
    pub melee_crit: f64,
    /// The lowest of the per-school spell crits: a school's own talents only raise it.
    pub spell_crit: f64,
}

impl Sheet {
    fn from_message(sheet: &Message) -> Option<Sheet> {
        let five = |name: &str| -> Option<[f64; 5]> { sheet.f64s(name).try_into().ok() };
        let spell_crit = sheet
            .f64s("spell_crit")
            .into_iter()
            .fold(f64::INFINITY, f64::min);
        Some(Sheet {
            base: five("base")?,
            attributes: five("attributes")?,
            health: sheet.f64("health"),
            mana: sheet.f64("mana"),
            melee_crit: sheet.f64("melee_crit"),
            spell_crit: if spell_crit.is_finite() {
                spell_crit
            } else {
                0.0
            },
        })
    }

    fn attribute(&self, stat: Stat) -> f64 {
        let index = ATTRIBUTES
            .iter()
            .position(|s| *s == stat)
            .expect("an attribute");
        self.attributes[index]
    }
}

/// Numbers the engine's tables hold for level 60 only, solved from a levelling character's sheet.
/// Each is None until [`calibrate`] has solved it, and preparation then uses the level 60 value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Calibration {
    pub crit_per_agility: Option<f64>,
    pub crit_per_intellect: Option<f64>,
    pub base_health: Option<f64>,
    pub base_mana: Option<f64>,
}

impl Character {
    /// The character a request's first player describes.
    pub(crate) fn from_request(request: &Message) -> Character {
        let Some(player) = request
            .message("raid")
            .and_then(|raid| raid.messages("parties").first().copied())
            .and_then(|party| party.messages("players").first().copied())
        else {
            return Character::default();
        };
        Character::from_player(player)
    }

    pub(crate) fn from_player(player: &Message) -> Character {
        let level = match player.i32("level") {
            0 => MAX_LEVEL,
            level => level,
        };
        let spells: BTreeSet<i32> = player
            .ints("known_spells")
            .into_iter()
            .filter_map(|id| i32::try_from(id).ok())
            .collect();
        Character {
            level,
            known_spells: (!spells.is_empty()).then_some(spells),
            sheet: player
                .message("character_sheet")
                .and_then(Sheet::from_message),
            calibration: Calibration::default(),
            unlearned: BTreeSet::new(),
            known_as: std::collections::BTreeMap::new(),
            registered: std::collections::BTreeMap::new(),
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Character> = RefCell::new(Character {
        level: MAX_LEVEL,
        ..Character::default()
    });
}

/// Runs `f` preparing `character`, and puts the previous one back after.
pub(crate) fn with_character<T>(character: Character, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Character>);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Some(previous) = self.0.take() {
                CURRENT.with(|current| *current.borrow_mut() = previous);
            }
        }
    }
    let previous = CURRENT.with(|current| current.replace(character));
    let _restore = Restore(Some(previous));
    f()
}

/// The level of the character being prepared: the caster level spell amounts scale to, the
/// attacker level of its attack tables and the level its weapon skill is capped at.
pub(crate) fn character_level() -> i32 {
    CURRENT.with(|current| current.borrow().level)
}

/// Which of a ladder's ranks the character knows, highest first wins.
///
/// With a spellbook the character knows the ranks it lists. A ladder none of whose ranks the
/// spellbook lists is not a trained spell (a talent, an item's spell or a triggered effect), and
/// is known up to the character's level like every rank without a spellbook: a rank whose spell
/// level is at or below the character's level. A spell level of 0 is a row without a level.
pub(crate) fn known_rank<'a>(ranks: &[&'a Spell]) -> Option<&'a Spell> {
    let known = CURRENT.with(|current| {
        let character = current.borrow();
        if let Some(book) = &character.known_spells {
            if let Some(rank) = ranks.iter().rev().find(|rank| book.contains(&rank.id)) {
                return Some(*rank);
            }
        }
        ranks
            .iter()
            .rev()
            .find(|rank| i32::from(rank.spell_level) <= character.level)
            .copied()
    })?;
    if ranks.len() > 1 {
        CURRENT.with(|current| {
            let mut character = current.borrow_mut();
            if character.level != MAX_LEVEL {
                for rank in ranks {
                    if rank.id != known.id {
                        character.known_as.insert(rank.id, known.id);
                    }
                }
            }
        });
    }
    Some(known)
}

/// A levelling character's rotations, naming the rank it knows wherever a rotation names
/// another rank of the same spell: the presets name the top rank, which a level 30 character
/// has not trained, in casts, aura and dot checks alike. Go's `RankAliases` does this only for
/// the ranks a spell lists.
///
/// `character` is the first preparation's: `known_as` maps the ranks of each ladder it resolved
/// to the rank it knows. A class that registers every rank as its own spell (a Druid's Wrath)
/// resolves no ladder of several ranks, so a rank it does not know maps to the highest known
/// rank of the same name ([`known_rank_of_name`]).
pub(crate) fn rewrite_rotations(request: &mut serde_json::Value, character: &Character) {
    use serde_json::Value;
    let known = |id: i32| -> Option<i32> {
        if let Some(known) = character.known_as.get(&id) {
            return Some(*known);
        }
        known_rank_of_name(id, character).filter(|known| *known != id)
    };
    fn walk(value: &mut Value, known: &dyn Fn(i32) -> Option<i32>) {
        match value {
            Value::Object(map) => {
                for (key, field) in map.iter_mut() {
                    if key == "spellId" {
                        if let Some(id) = field.as_i64().and_then(|id| i32::try_from(id).ok()) {
                            if let Some(rank) = known(id) {
                                *field = Value::from(rank);
                                continue;
                            }
                        }
                    }
                    walk(field, known);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|item| walk(item, known)),
            _ => {}
        }
    }
    let Some(parties) = request
        .pointer_mut("/raid/parties")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    for party in parties {
        let Some(players) = party.get_mut("players").and_then(Value::as_array_mut) else {
            continue;
        };
        for player in players {
            if let Some(rotation) = player.get_mut("rotation") {
                walk(rotation, &known);
            }
        }
    }
}

/// For a ranked spell the character does not know, the highest rank of the same name it does
/// that the first preparation registered as the same class spell (a cast for a cast, never its
/// tick). None for an unranked spell, one it knows, or one it knows no such rank of.
fn known_rank_of_name(id: i32, character: &Character) -> Option<i32> {
    let spell = crate::prepare::spelldata::find(id);
    if spell.id == 0 || !spell.rank.starts_with("Rank ") || knows_rank(character, spell) {
        return None;
    }
    let kind = character.registered.get(&id);
    same_name_ranks(spell)
        .iter()
        .map(|id| crate::prepare::spelldata::find(*id))
        .filter(|rank| knows_rank(character, rank))
        .filter(|rank| match (kind, character.registered.get(&rank.id)) {
            (_, None) => false,
            (Some(kind), Some(other)) => kind == other,
            (None, Some(_)) => true,
        })
        .max_by_key(|rank| (rank.spell_level, rank.id))
        .map(|rank| rank.id)
}

/// The group buffs a class casts on its raid and what a levelling caster casts instead when it
/// has not learned the group version: Arcane Brilliance is learned at 56 from a book, and a level
/// 21 mage buffs with its Arcane Intellect.
const SINGLE_TARGET_OF: [(&str, &str); 5] = [
    ("Arcane Brilliance", "Arcane Intellect"),
    ("Prayer of Fortitude", "Power Word: Fortitude"),
    ("Prayer of Spirit", "Divine Spirit"),
    ("Prayer of Shadow Protection", "Shadow Protection"),
    ("Gift of the Wild", "Mark of the Wild"),
];

/// The rank of a buff a levelling character casts in place of `spell`, the level 60 row core's
/// buff tables name: the highest rank of the same name it knows, or of the single-target spell
/// for a group buff it has not learned. A buff it knows no rank of keeps `spell`, noted as
/// unlearned, so its aura is never made permanent. A level 60 character keeps `spell`.
///
/// A levelling character sims solo (the app's `worker/forever/levels.go`), so every such buff is
/// one the class adds for itself (`add_raid_buffs`, `add_party_buffs`).
pub(crate) fn known_buff_rank(spell: i32) -> i32 {
    let character = current();
    if character.level == MAX_LEVEL {
        return spell;
    }
    let row = crate::prepare::spelldata::find(spell);
    if row.id == 0 {
        return spell;
    }
    let highest_known = |name: &str| -> Option<i32> {
        let mut ranks: Vec<&Spell> = crate::prepare::spelldata::all()
            .filter(|candidate| candidate.name == name)
            .filter(|candidate| knows_buff_row(&character, candidate))
            .collect();
        ranks.sort_by_key(|candidate| (candidate.spell_level, candidate.id));
        ranks.last().map(|rank| rank.id)
    };
    let single = SINGLE_TARGET_OF
        .iter()
        .find(|(group, _)| *group == row.name)
        .map(|(_, single)| *single);
    if let Some(known) = highest_known(&row.name).or_else(|| single.and_then(highest_known)) {
        return known;
    }
    CURRENT.with(|current| current.borrow_mut().unlearned.insert(spell));
    spell
}

/// Whether the character has a buff's row: a ranked row as [`knows_rank`] reads it, an unranked
/// one by its spell level alone (a spellbook lists the rank a buff casts, not every row).
fn knows_buff_row(character: &Character, row: &Spell) -> bool {
    if row.spell_level == 0 {
        return false;
    }
    if row.rank.starts_with("Rank ") {
        knows_rank(character, row)
    } else {
        i32::from(row.spell_level) <= character.level
    }
}

/// A level 60 character's base stats, `stats` from the engine's table, moved to the level of
/// the character being prepared. At 60 they stay as they are. Below it the attributes are the
/// sheet's own, base health and mana are what [`calibrate`] solved, and base attack power
/// follows the class's per-level rule: the table's level 60 base is the class's attack power per
/// level times 60, less 20 (a Warrior's 3 per level gives 160), or a constant offset for a class
/// whose base is not positive.
pub(crate) fn base_stats_at_level(stats: &mut Stats) {
    CURRENT.with(|current| {
        let character = current.borrow();
        if character.level == MAX_LEVEL {
            return;
        }
        let level = f64::from(character.level);
        if let Some(sheet) = &character.sheet {
            for (stat, value) in ATTRIBUTES.iter().zip(sheet.base) {
                stats[*stat] = value;
            }
        }
        for stat in [Stat::AttackPower, Stat::RangedAttackPower] {
            let at_60 = stats[stat];
            if at_60 > 0.0 {
                let per_level = (at_60 + 20.0) / f64::from(MAX_LEVEL);
                stats[stat] = per_level * level - 20.0;
            }
        }
        if let Some(health) = character.calibration.base_health {
            stats[Stat::Health] = health;
        }
        if let Some(mana) = character.calibration.base_mana {
            if stats[Stat::Mana] > 0.0 {
                stats[Stat::Mana] = mana;
            }
        }
    });
}

/// Crit chance per point of agility for the character being prepared: the table's level 60
/// value, or what [`calibrate`] solved below 60.
pub(crate) fn crit_per_agility(at_60: f64) -> f64 {
    CURRENT.with(|current| {
        current
            .borrow()
            .calibration
            .crit_per_agility
            .unwrap_or(at_60)
    })
}

/// Spell crit chance per point of intellect, as [`crit_per_agility`].
pub(crate) fn crit_per_intellect(at_60: f64) -> f64 {
    CURRENT.with(|current| {
        current
            .borrow()
            .calibration
            .crit_per_intellect
            .unwrap_or(at_60)
    })
}

/// The character being prepared, for [`calibrate`]'s second pass.
pub(crate) fn current() -> Character {
    CURRENT.with(|current| current.borrow().clone())
}

/// Solves the level 60 numbers from the sheet: `engine` is the player's stats as preparation
/// made them with no buffs or consumables, using `used` for the calibrated numbers.
///
/// The sheet was read with whatever buffs were up, so attribute differences are carried
/// through. With `k` crit per agility, the engine's crit `C_e` at its agility `A_e` used
/// `k_used`; the sheet's `C_s` at `A_s` is `C_e + (k - k_used) A_e + k (A_s - A_e)`, so
/// `k = (C_s - C_e + k_used A_e) / A_s`. The constant part of crit (base and gear) rides in the
/// engine's own `C_e`. Health is 10 per stamina above the first 20 and mana 15 per intellect,
/// so base health is the base used plus what the sheet has over the engine, less what its extra
/// stamina explains.
pub(crate) fn calibrate(
    sheet: &Sheet,
    engine: &Stats,
    used: &Stats,
    k_agi: f64,
    k_int: f64,
) -> Calibration {
    let solve = |sheet_crit: f64, engine_crit: f64, used_k: f64, stat: Stat| -> Option<f64> {
        let sheet_stat = sheet.attribute(stat);
        if used_k <= 0.0 || sheet_stat <= 0.0 {
            return None;
        }
        let k = (sheet_crit - engine_crit + used_k * engine[stat]) / sheet_stat;
        (k > 0.0).then_some(k)
    };
    Calibration {
        crit_per_agility: solve(
            sheet.melee_crit,
            engine[Stat::PhysicalCritPercent],
            k_agi,
            Stat::Agility,
        ),
        crit_per_intellect: solve(
            sheet.spell_crit,
            engine[Stat::SpellCritPercent],
            k_int,
            Stat::Intellect,
        ),
        // A base that is not positive means the sheet and the engine disagree about more than
        // the level (an export read mid-shapeshift, a buff the engine has no row for): the
        // level 60 base stays rather than leave the character with no health or mana.
        base_health: (sheet.health > 0.0)
            .then(|| {
                used[Stat::Health] + (sheet.health - engine[Stat::Health])
                    - 10.0 * (sheet.attribute(Stat::Stamina) - engine[Stat::Stamina])
            })
            .filter(|health| *health > 0.0),
        base_mana: (sheet.mana > 0.0 && used[Stat::Mana] > 0.0)
            .then(|| {
                used[Stat::Mana] + (sheet.mana - engine[Stat::Mana])
                    - 15.0 * (sheet.attribute(Stat::Intellect) - engine[Stat::Intellect])
            })
            .filter(|mana| *mana > 0.0),
    }
}

/// A ladder the character knows no rank of: its first rank, so the class code that registers it
/// still has a school, a label and effects to read, with every rank noted as unlearned. The
/// export then leaves the spell out of the rotation and the major cooldowns
/// (`SpellFlagUnlearned`), and an aura from it is never made permanent, so the character never
/// casts or wears what it has not trained.
pub(crate) fn unlearned_rank<'a>(ranks: &[&'a Spell]) -> Option<&'a Spell> {
    let first = ranks.first().copied()?;
    CURRENT.with(|current| {
        current
            .borrow_mut()
            .unlearned
            .extend(ranks.iter().map(|rank| rank.id));
    });
    Some(first)
}

/// Whether the character has not trained `spell_id`: a rank of a ladder it knows none of, or a
/// ranked spell above its level or missing from its spellbook, which a class that registers
/// every rank of a spell (Arcane Missiles, Wrath) registers all the same.
pub(crate) fn is_unlearned(spell_id: i32) -> bool {
    if spell_id == 0 {
        return false;
    }
    CURRENT.with(|current| {
        let character = current.borrow();
        if character.unlearned.contains(&spell_id) {
            return true;
        }
        if character.level == MAX_LEVEL {
            return false;
        }
        let row = crate::prepare::spelldata::find(spell_id);
        row.id != 0 && row.rank.starts_with("Rank ") && !knows_rank(&character, row)
    })
}

/// Whether the character has trained an unranked spell such as a conjure: in its spellbook, or
/// without one at or below its level. Every spell is known at level 60.
pub(crate) fn knows_spell(spell_id: i32) -> bool {
    CURRENT.with(|current| {
        let character = current.borrow();
        if character.level == MAX_LEVEL {
            return true;
        }
        if let Some(book) = &character.known_spells {
            return book.contains(&spell_id);
        }
        i32::from(crate::prepare::spelldata::find(spell_id).spell_level) <= character.level
    })
}

/// Whether the character knows this rank: in its spellbook, or without one at or below its
/// level. A spellbook that lists no rank of the spell says nothing about it (a talent or a
/// triggered row), and the level decides.
fn knows_rank(character: &Character, rank: &Spell) -> bool {
    if let Some(book) = &character.known_spells {
        if book.contains(&rank.id) {
            return true;
        }
        if same_name_ranks(rank).iter().any(|id| book.contains(id)) {
            return false;
        }
    }
    i32::from(rank.spell_level) <= character.level
}

/// Every ranked row with this rank's name.
fn same_name_ranks(rank: &Spell) -> &'static [i32] {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static RANKS: OnceLock<HashMap<String, Vec<i32>>> = OnceLock::new();
    let ranks = RANKS.get_or_init(|| {
        let mut ranks: HashMap<String, Vec<i32>> = HashMap::new();
        for row in crate::prepare::spelldata::all() {
            if row.rank.starts_with("Rank ") {
                ranks.entry(row.name.clone()).or_default().push(row.id);
            }
        }
        ranks
    });
    ranks.get(&rank.name).map_or(&[], Vec::as_slice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::Ladder;

    // Frostbolt ranks 1 to 3: spell levels 4, 8 and 14.
    fn frostbolt() -> Ladder {
        Ladder::ranked(&[116, 205, 837])
    }

    #[test]
    fn a_request_without_a_level_is_level_60() {
        assert_eq!(character_level(), MAX_LEVEL);
        assert_eq!(frostbolt().highest().id, 837);
    }

    #[test]
    fn a_rank_is_known_from_its_spell_level() {
        let at = |level| {
            let character = Character {
                level,
                ..Character::default()
            };
            with_character(character, || frostbolt().highest().id)
        };
        assert_eq!(at(4), 116);
        assert_eq!(at(13), 205);
        assert_eq!(at(14), 837);
        let below = Character {
            level: 3,
            ..Character::default()
        };
        assert_eq!(
            with_character(below, || {
                let rank = frostbolt().highest().id;
                (rank, is_unlearned(116), is_unlearned(837))
            }),
            (116, true, true),
            "below the first rank the first stands in, unlearned"
        );
        assert!(!is_unlearned(116), "the next character starts clean");
        assert_eq!(character_level(), MAX_LEVEL, "the level goes back after");
    }

    #[test]
    fn the_spellbook_names_the_trained_rank() {
        let character = Character {
            level: 20,
            known_spells: Some([116, 205].into_iter().collect()),
            ..Character::default()
        };
        assert_eq!(with_character(character, || frostbolt().highest().id), 205);
    }

    #[test]
    fn a_ladder_the_spellbook_does_not_list_follows_the_level() {
        let character = Character {
            level: 20,
            known_spells: Some([133].into_iter().collect()),
            ..Character::default()
        };
        assert_eq!(with_character(character, || frostbolt().highest().id), 837);
    }
}
