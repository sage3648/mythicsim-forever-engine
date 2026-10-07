//! Go `spelldata/store.go`: the lookups over the generated rows. The rows, the talent curves and
//! the hand triggers are [`crate::data::spells`]; this adds the driver index and the `Nil`
//! answering `Find`.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::super::dbcenums;
use super::{nil, Spell};
use crate::data::spells as rows;

/// The row for an id, or [`nil`] when the store does not carry it. Nil reads as zeroes rather
/// than crashing, so a caller can ask about a spell this build does not have. Go `Find`.
pub(crate) fn find(id: i32) -> &'static Spell {
    rows::find(id).unwrap_or_else(nil)
}

/// For a spell a caller depends on: a missing row is a generator problem, not a runtime
/// condition. Go `MustFind`.
pub(crate) fn must_find(id: i32) -> &'static Spell {
    match rows::find(id) {
        Some(spell) => spell,
        None => panic!(
            "spelldata: spell {id} is not in the store (regenerate or add it to tools/database/overrides/extra_spells.go)"
        ),
    }
}

/// Every row in id order. Go `All`.
pub(crate) fn all() -> impl Iterator<Item = &'static Spell> {
    rows::ids().map(rows::must_find)
}

/// Every row with this exact name, by linear scan over the whole store. For tests and
/// debugging: a sim should name the spell by id. Go `ByName`.
pub(crate) fn by_name(name: &str) -> Vec<&'static Spell> {
    all().filter(|spell| spell.name == name).collect()
}

/// Which spells reach a given spell: every effect that triggers it, every handler that casts it,
/// and every actionbar override that replaces a spell with it, by driver id in row order. Built
/// on first use, as Go builds it when the store is installed.
fn drivers() -> &'static HashMap<i32, Vec<i32>> {
    static DRIVERS: OnceLock<HashMap<i32, Vec<i32>>> = OnceLock::new();
    DRIVERS.get_or_init(|| {
        let mut drivers = HashMap::new();
        for spell in all() {
            for effect in &spell.effects {
                if effect.trigger_id != 0 {
                    add_driver(&mut drivers, effect.trigger_id, spell.id);
                }
                // A_OVERRIDE_ACTIONBAR_SPELLS states the replacing spell in its base points,
                // which makes the overriding spell a driver of it the same way a trigger effect
                // is.
                if effect.aura == dbcenums::A_OVERRIDE_ACTIONBAR_SPELLS && effect.base_points > 0.0
                {
                    add_driver(&mut drivers, effect.base_points as i32, spell.id);
                }
            }
            for &triggered in rows::hand_triggers(spell.id) {
                add_driver(&mut drivers, triggered, spell.id);
            }
        }
        drivers
    })
}

/// Two effects of the same spell can name the same trigger, and the caller wants the spell once.
fn add_driver(drivers: &mut HashMap<i32, Vec<i32>>, triggered: i32, driver: i32) {
    let ids = drivers.entry(triggered).or_default();
    if ids.last() == Some(&driver) {
        return;
    }
    ids.push(driver);
}

/// The ids of the spells whose effects fire this one: Go's `drivers[id]`.
pub(crate) fn driver_ids(id: i32) -> &'static [i32] {
    drivers().get(&id).map(Vec::as_slice).unwrap_or(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_misses_answer_nil() {
        assert!(find(0).is_nil());
        assert!(find(-1).is_nil());
        assert!(find(117).is_nil() || find(117).id == 117);
        assert_eq!(find(116).id, 116);
        assert!(!find(116).is_nil());
    }

    #[test]
    #[should_panic(expected = "spelldata: spell 0 is not in the store")]
    fn must_find_panics_on_a_missing_row() {
        must_find(0);
    }

    /// Go `TestGeneratedStoreShape`: the shape every accessor depends on, ids in search order and
    /// effects in position order.
    #[test]
    fn the_store_has_the_shape_accessors_depend_on() {
        let mut rows = 0;
        let mut effects = 0;
        let mut gapped = 0;
        let mut previous = i32::MIN;
        for spell in all() {
            rows += 1;
            assert!(spell.id > previous, "spell {} follows {previous}", spell.id);
            previous = spell.id;
            effects += spell.effects.len();
            let mut packed = true;
            for (position, effect) in spell.effects.iter().enumerate() {
                assert_eq!(
                    effect.spell_id, spell.id,
                    "spell {} effect {position}",
                    spell.id
                );
                if position > 0 {
                    assert!(
                        effect.index > spell.effects[position - 1].index,
                        "spell {} has effect index {} after {}",
                        spell.id,
                        effect.index,
                        spell.effects[position - 1].index
                    );
                }
                if usize::from(effect.index) != position {
                    packed = false;
                }
            }
            if !packed {
                gapped += 1;
            }
        }
        assert!(
            (5000..=12000).contains(&rows),
            "the store holds {rows} spells"
        );
        assert!(
            (6000..=16000).contains(&effects),
            "the store holds {effects} effects"
        );
        // The client's EffectIndex has gaps, which is why EffectN counts by position.
        assert!(
            gapped > 0,
            "no row states an EffectIndex that is not its position"
        );
    }

    /// Go `TestGeneratedLightningShield`: the shared dispatcher is driven by each rank.
    #[test]
    fn a_rank_drives_its_dispatcher() {
        assert!(driver_ids(26545).contains(&324));
        assert!(must_find(26545)
            .drivers()
            .iter()
            .any(|driver| driver.id == 324));
    }

    /// Go `TestGeneratedHandLink`: Retaliation's counterattack is a link the client does not
    /// state and the hand triggers supply.
    #[test]
    fn hand_triggers_come_back_out() {
        let triggered: Vec<i32> = must_find(20230).triggered().iter().map(|s| s.id).collect();
        assert!(triggered.contains(&20240), "{triggered:?}");
        let drivers: Vec<i32> = must_find(20240).drivers().iter().map(|s| s.id).collect();
        assert!(drivers.contains(&20230), "{drivers:?}");
    }

    #[test]
    fn by_name_scans_the_store() {
        assert!(by_name("Frostbolt").iter().any(|spell| spell.id == 116));
        assert!(by_name("No Such Spell").is_empty());
    }
}
