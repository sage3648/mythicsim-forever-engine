//! Live item rows corrected on 27 November 2026, under Forever's rules.
//!
//! Pendant of Celerity (22340): Wowhead's Forever tooltip (read 27 November 2026) gives +10
//! Agility, +10 Frost Resistance and "Equip: Improves your chance to hit by 1.0%." Its live row
//! of 17 November held 20 hit rating, 2% hit, so a caster sims it a whole percent of hit above
//! Nacreous Shell Necklace (22403), whose tooltip states the same 1.0% beside 8 Intellect and up
//! to 9 spell damage and healing. A Demonology Warlock's Best Gear ranked the Pendant first
//! (#warlock, 27 November 2026).

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const PENDANT_OF_CELERITY: i64 = 22_340;
const NACREOUS_SHELL_NECKLACE: i64 = 22_403;
/// Stormstrike Hammer, a one-hander with no hit.
const STORMSTRIKE_HAMMER: i64 = 19_104;

/// The holy paladin fixture with no talents wearing only Stormstrike Hammer and the given neck
/// (0 for none).
fn neck_request(neck: i64) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/holy-paladin-healing.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!("");
    let mut items = vec![json!({ "id": STORMSTRIKE_HAMMER })];
    if neck != 0 {
        items.push(json!({ "id": neck }));
    }
    player["equipment"] = json!({ "items": items });
    serde_json::to_vec(&request).unwrap()
}

/// What the neck adds to a stat under Forever's rules.
fn added(neck: i64, stat: &str) -> f64 {
    let value = |neck: i64| {
        let prepared = prepare_json_with(&neck_request(neck), "rust", Rules::Forever)
            .unwrap_or_else(|err| panic!("{err:?}"));
        prepared["player"]["stats"][stat].as_f64().unwrap()
    };
    value(neck) - value(0)
}

#[test]
fn pendant_of_celerity_gives_one_percent_hit() {
    for stat in ["MeleeHitRating", "SpellHitRating"] {
        assert_eq!(added(PENDANT_OF_CELERITY, stat), 10.0, "{stat}");
    }
    assert_eq!(added(PENDANT_OF_CELERITY, "FrostResistance"), 10.0);
}

#[test]
fn nacreous_shell_necklace_matches_the_pendants_hit() {
    for stat in ["MeleeHitRating", "SpellHitRating"] {
        assert_eq!(
            added(NACREOUS_SHELL_NECKLACE, stat),
            added(PENDANT_OF_CELERITY, stat),
            "{stat}"
        );
    }
    for (stat, value) in [
        ("Intellect", 8.0),
        ("SpellDamage", 9.0),
        ("HealingPower", 9.0),
    ] {
        assert!(added(NACREOUS_SHELL_NECKLACE, stat) >= value, "{stat}");
    }
}
