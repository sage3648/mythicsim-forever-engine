//! The spells the reference requests cast, read against Wowhead's Forever tooltips (the live
//! client's rows, hotfixes included) on 6 November 2026: cost, cast time, cooldown, range, channel
//! time and the level scaling of every rank the engine prepares agreed with the tooltips except the
//! rows below. Forever's rules take the live values; the Go reference keeps the frozen data.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

fn fixture(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules).unwrap()
}

/// The player's prepared spells of one class spell, in rank order.
fn spells<'a>(prepared: &'a Value, class_spell: &str) -> Vec<&'a Value> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|spell| spell["class_spell"] == class_spell)
        .collect()
}

fn spell(prepared: &Value, spell_id: i64) -> &Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| {
            spell["action_id"]["spell_id"] == spell_id && spell["action_id"]["tag"].is_null()
        })
        .unwrap_or_else(|| panic!("spell {spell_id} is not prepared"))
}

/// Chain Lightning's mana on ranks 1 to 4 as the live tooltips read it: "245 Mana", "360 Mana",
/// "425 Mana" and "495 Mana". The development notes of 2 November say "approximately 225%" of the
/// comparable Lightning Bolt rank's, which the first pass read as 248, 304, 428 and 495; the live
/// rows replace that approximation. The 2.5 sec cast is the notes' explicit number and the
/// tooltips agree, so it stays.
#[test]
fn chain_lightning_costs_are_the_live_tooltips() {
    let request = fixture("fixtures/mage/prepared-v2/production-elemental-shaman.request.json");
    let forever = prepared(&request, Rules::Forever);
    let reference = prepared(&request, Rules::GoReference);
    let costs = |prepared: &Value| -> Vec<f64> {
        spells(prepared, "chain_lightning")
            .iter()
            .map(|spell| spell["cost"]["base_cost"].as_f64().unwrap())
            .collect()
    };
    assert_eq!(costs(&forever), [245.0, 360.0, 425.0, 495.0]);
    assert_eq!(costs(&reference), [145.0, 200.0, 250.0, 315.0]);
    for (spell, live) in spells(&forever, "chain_lightning")
        .into_iter()
        .zip([245.0, 360.0, 425.0, 495.0])
    {
        assert_eq!(spell["cost"]["base_cost"], live);
        // The fixture's Elemental Alacrity takes half a second off the 2.5 sec cast.
        assert_eq!(spell["default_cast"]["cast_time_ns"], 2_000_000_000);
    }
}

/// Light's Vigil's tooltip states one range, 40 yd (a friendly spell), where the frozen rows say
/// 20. Holy Shock's tooltip states "20 - 40 yd range", hostile and friendly, which the row's 20
/// is the first of.
#[test]
fn lights_vigil_reaches_forty_yards() {
    let mut request = fixture("fixtures/healers/holy-paladin.request.json");
    let list = request["raid"]["parties"][0]["players"][0]["rotation"]["priorityList"]
        .as_array_mut()
        .unwrap();
    list.insert(
        0,
        json!({"action": {"castSpell": {
            "spellId": {"spellId": 1311595}, "target": {"type": "Self"}
        }}}),
    );
    request["simOptions"]["iterations"] = json!(20);
    let forever = prepared(&request, Rules::Forever);
    assert_eq!(spell(&forever, 1311595)["max_range"], 40.0);
}

/// Sniper Shot's tooltip reads "8 - 45 yd range": a long-range shot, where every other Hunter
/// shot is capped at the base 35. Go's `RegisterRangedSpell` caps all of them, Sniper Shot
/// included, so the reference keeps 35.
#[test]
fn sniper_shot_reaches_forty_five_yards() {
    let request = fixture("tests/classes/hunter/prepare/marksmanship-hyena.request.json");
    let forever = prepared(&request, Rules::Forever);
    let reference = prepared(&request, Rules::GoReference);
    let sniper = spells(&forever, "sniper_shot");
    assert_eq!(sniper.len(), 1);
    assert_eq!(sniper[0]["max_range"], 45.0);
    assert_eq!(spells(&reference, "sniper_shot")[0]["max_range"], 35.0);
    // The other shots keep their 35.
    for shot in spells(&forever, "aimed_shot") {
        assert_eq!(shot["max_range"], 35.0);
    }
}
