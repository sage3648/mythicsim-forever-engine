//! Attack power on gear raises ranged attack power by as much under Forever's rules. The equip
//! spells that read "+N Attack Power" state both auras, 99 and 124, at the same value (Attack
//! Power 22, 9332, the equip spell of Counterattack Lodestone 18537, client 1.60.1.70862), while
//! most item rows carry attack power alone: a Hunter in Devilsaur Gauntlets (15063, "Equip: +28
//! Attack Power.") got no ranged attack power from them. The reference's rules keep the rows.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The turret Hunter fixture with no talents, buffs, debuffs or consumables, wearing only the
/// given items, each an id with an optional random suffix.
fn request(items: &[(i64, i64)]) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/prepull/hunter-turret-aimed-shot.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(1);
    request["raid"]["buffs"] = json!({});
    request["raid"]["debuffs"] = json!({});
    request["raid"]["parties"][0]["buffs"] = json!({});
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!("");
    player["buffs"] = json!({});
    player["consumables"] = json!({});
    let items: Vec<Value> = items
        .iter()
        .map(|(id, suffix)| {
            if *suffix == 0 {
                json!({ "id": id })
            } else {
                json!({ "id": id, "randomSuffix": suffix })
            }
        })
        .collect();
    player["equipment"] = json!({ "items": items });
    serde_json::to_vec(&request).unwrap()
}

fn stats(items: &[(i64, i64)], rules: Rules) -> Value {
    let prepared =
        prepare_json_with(&request(items), "rust", rules).unwrap_or_else(|err| panic!("{err}"));
    prepared["player"]["stats"].clone()
}

/// What the items add to melee and ranged attack power under the rules.
fn added(items: &[(i64, i64)], rules: Rules) -> (f64, f64) {
    let without = stats(&[], rules);
    let with = stats(items, rules);
    let delta = |stat: &str| with[stat].as_f64().unwrap() - without[stat].as_f64().unwrap();
    (delta("AttackPower"), delta("RangedAttackPower"))
}

#[test]
fn devilsaur_gauntlets_give_a_hunter_ranged_attack_power() {
    assert_eq!(added(&[(15063, 0)], Rules::Forever), (28.0, 28.0));
    assert_eq!(added(&[(15063, 0)], Rules::GoReference), (28.0, 0.0));
}

/// The hunter reference gear the rows short of ranged attack power: Cape of the Black Baron
/// (13340, 20), Devilsaur Leggings (15062, 46), Arcanite Reaper (12784, 62) and Don Julio's Band
/// (19325, 14). Their Agility and Strength count as before, so each adds its attack power to
/// ranged over the reference and nothing to melee.
#[test]
fn every_attack_power_item_of_the_hunter_references_counts_for_ranged() {
    for (item, ap) in [(13340, 20.0), (15062, 46.0), (12784, 62.0), (19325, 14.0)] {
        let forever = added(&[(item, 0)], Rules::Forever);
        let reference = added(&[(item, 0)], Rules::GoReference);
        assert_eq!(forever.0, reference.0, "{item}");
        assert_eq!(forever.1 - reference.1, ap, "{item}");
    }
}

/// A row that states ranged attack power beside attack power keeps it: Counterattack Lodestone
/// gives 22 of each in the reference, not 44 ranged, and its live row's 30 of each under
/// Forever's rules, not 60 ranged.
#[test]
fn a_row_with_its_own_ranged_attack_power_is_not_doubled() {
    assert_eq!(added(&[(18537, 0)], Rules::Forever), (30.0, 30.0));
    assert_eq!(added(&[(18537, 0)], Rules::GoReference), (22.0, 22.0));
}

/// An "of Power" suffix carries both already, so it adds what it did: Serpentskin Gloves (8260,
/// no attack power of their own) of Power (1548, 4 and 4) add the same under both rules, 4 more
/// to each than the gloves alone.
#[test]
fn an_of_power_suffix_is_unchanged() {
    let forever = added(&[(8260, 1548)], Rules::Forever);
    assert_eq!(forever, added(&[(8260, 1548)], Rules::GoReference));
    let plain = added(&[(8260, 0)], Rules::Forever);
    assert_eq!((forever.0 - plain.0, forever.1 - plain.1), (4.0, 4.0));
}
