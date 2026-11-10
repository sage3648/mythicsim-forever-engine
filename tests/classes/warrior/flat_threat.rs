//! The flat threat Heroic Strike, Cleave and Slam add beyond their damage under Forever's rules:
//! whole numbers from Forever combat logs on client 1.60.1.70862 (8 November 2026). Heroic Strike
//! ranks 1 to 4 add 17, 28, 40 and 60 (Rank 4 measured at 60.08 over 104 clean readings) and the
//! later ranks 2.5 a spell level; Cleave rank 1 adds 30 and rank 2 46, the later ranks 1.5 a
//! spell level; Slam adds 1.5 a spell level on every rank (Rank 1 at spell level 20 adds 30).

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const HEROIC_STRIKE: [i64; 9] = [78, 284, 285, 1608, 11564, 11565, 11566, 11567, 25286];
const CLEAVE: [i64; 5] = [845, 7369, 11608, 11609, 20569];
const SLAM: [i64; 5] = [1240193, 1464, 8820, 11604, 11605];

/// The production Fury request at a level with its character sheet, talents read as Forever's
/// rules read them. The sheet's numbers only set the character's stats, not its spell ranks.
fn warrior_at(level: i64, rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-warrior.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    if rules == Rules::Forever {
        crate::warrior_trees_20261104::forever_request(&mut request);
    }
    if level < 60 {
        let player = &mut request["raid"]["parties"][0]["players"][0];
        player["level"] = json!(level);
        player["talentsString"] = json!("");
        player["characterSheet"] = json!({
            "attributes": [59, 40, 55, 20, 25],
            "base": [46, 35, 43, 17, 22],
            "health": 800,
            "level": level,
            "mana": 0,
            "meleeCrit": 5.0,
            "spellCrit": [0, 0, 0, 0, 0, 0],
        });
    }
    request
}

/// The flat threat of the one rank of a ladder the prepared player knows, by spell id.
fn flat_threat(level: i64, rules: Rules, ladder: &[i64]) -> (i64, f64) {
    let request = warrior_at(level, rules);
    let prepared = prepare_json_with(request.to_string().as_bytes(), "warrior", rules)
        .unwrap_or_else(|err| panic!("{err:?}"));
    let spell = prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            ladder.contains(&spell["action_id"]["spell_id"].as_i64().unwrap_or(0))
                && spell["action_id"]["tag"].as_i64().unwrap_or(0) == 0
        })
        .unwrap_or_else(|| panic!("no rank of {ladder:?} at level {level}"));
    (
        spell["action_id"]["spell_id"].as_i64().unwrap(),
        spell["flat_threat_bonus"].as_f64().unwrap(),
    )
}

/// Slam: 30 for the rank 1 row (spell level 20), 45 for 1464 (spell level 30) and 81 for the
/// level 60 rank. The reference adds 140 to every rank.
#[test]
fn slam_adds_one_and_a_half_threat_a_spell_level() {
    assert_eq!(flat_threat(24, Rules::Forever, &SLAM), (1240193, 30.0));
    assert_eq!(flat_threat(32, Rules::Forever, &SLAM), (1464, 45.0));
    assert_eq!(flat_threat(60, Rules::Forever, &SLAM), (11605, 81.0));
    assert_eq!(flat_threat(32, Rules::GoReference, &SLAM).1, 140.0);
}

/// Heroic Strike: Rank 4's 60 at level 24, Rank 5's 80 (2.5 at spell level 32) and Rank 8's 140
/// at level 60. The reference adds 2.54 a spell level, 81.28 at Rank 5.
#[test]
fn heroic_strike_adds_whole_number_threat() {
    assert_eq!(
        flat_threat(24, Rules::Forever, &HEROIC_STRIKE),
        (1608, 60.0)
    );
    assert_eq!(
        flat_threat(32, Rules::Forever, &HEROIC_STRIKE),
        (11564, 80.0)
    );
    assert_eq!(
        flat_threat(60, Rules::Forever, &HEROIC_STRIKE),
        (11567, 140.0)
    );
    let (id, reference) = flat_threat(32, Rules::GoReference, &HEROIC_STRIKE);
    assert_eq!(id, 11564);
    assert!((reference - 2.54 * 32.0).abs() < 1e-9, "{reference}");
}

/// Cleave: rank 2's 46 at level 32 and rank 5's 90 (1.5 at spell level 60). The reference adds
/// 1.53 a spell level.
#[test]
fn cleave_adds_whole_number_threat() {
    assert_eq!(flat_threat(32, Rules::Forever, &CLEAVE), (7369, 46.0));
    assert_eq!(flat_threat(60, Rules::Forever, &CLEAVE), (20569, 90.0));
    let (_, reference) = flat_threat(32, Rules::GoReference, &CLEAVE);
    assert!((reference - 1.53 * 30.0).abs() < 1e-9, "{reference}");
}
