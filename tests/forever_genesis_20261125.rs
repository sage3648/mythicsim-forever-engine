//! Genesis (1223081) under Forever's rules since Blizzard's hotfixes of 25 November 2026: "Fixed a
//! bug where Genesis was not increasing the damage of Hurricane or the healing of Tranquility."
//! At 5 points it is 10% (2% a rank since hotfix push 113531). Hurricane's damage is the tick
//! spell its channel casts (1278759), so it takes the 10% as a damage percent; Tranquility's
//! heal ticks take it as Rejuvenation's do. The Go reference leaves both out.

use forever_engine::{prepare_json_with, Rules};
use serde_json::Value;
use std::{fs, path::Path};

const HURRICANE_TICK: i64 = 1278759;
/// Tranquility, ranks 1 to 4.
const TRANQUILITY: [i64; 4] = [740, 8918, 9862, 9863];

fn prepared(fixture: &str, rules: Rules) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(fixture);
    let request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    // Both fixtures spend 5 points in Genesis, the second talent of the Balance tree.
    let talents = request["raid"]["parties"][0]["players"][0]["talentsString"]
        .as_str()
        .unwrap();
    assert_eq!(&talents[1..2], "5", "{talents}");
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn spell(prepared: &Value, id: i64) -> &Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == id)
        .unwrap_or_else(|| panic!("spell {id} is registered"))
}

const BALANCE: &str = "fixtures/mage/prepared-v2/production-balance-druid.request.json";
const RESTORATION: &str = "fixtures/healers/restoration-druid.request.json";

#[test]
fn genesis_raises_hurricane_under_forevers_rules() {
    let forever = prepared(BALANCE, Rules::Forever);
    let tick = spell(&forever, HURRICANE_TICK)["damage_multiplier"]
        .as_f64()
        .unwrap();
    assert!((tick - 1.1).abs() < 1e-12, "{tick}");
    // Moonfire's periodic damage takes the same 10%.
    let moonfire = forever["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == 9835 && !spell["dot"].is_null())
        .unwrap();
    assert_eq!(moonfire["dot"]["periodic_damage_multiplier"], 1.1);

    let reference = prepared(BALANCE, Rules::GoReference);
    let tick = spell(&reference, HURRICANE_TICK)["damage_multiplier"]
        .as_f64()
        .unwrap();
    assert_eq!(tick, 1.0);
}

#[test]
fn genesis_raises_tranquility_under_forevers_rules() {
    let forever = prepared(RESTORATION, Rules::Forever);
    for id in TRANQUILITY {
        let multiplier = spell(&forever, id)["dot"]["periodic_damage_multiplier"]
            .as_f64()
            .unwrap();
        assert!((multiplier - 1.1).abs() < 1e-12, "{id}: {multiplier}");
    }
}
