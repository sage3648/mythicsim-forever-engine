//! Production refusals of 4 November 2026 that are engine gaps with client data: Natural
//! Alignment Crystal (19344) on a damage dealer, and a healer's Mana Spring Totem cast at Self.
//! Each request comes from an existing fixture with the refused item or rotation line put in.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[path = "support/shaman_layout.rs"]
mod shaman_layout;

const NATURAL_ALIGNMENT_CRYSTAL: i64 = 19344;
const NATURE_ALIGNED: i64 = 23734;
const LIGHTNING_BOLT_TOP: i64 = 15208;
const MANA_SPRING_TOTEM: i64 = 10497;
const HEALING_STREAM_TOTEM: &str = "10463";

fn fixture(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(20);
    request
}

/// An Enhancement request wearing the crystal in its second trinket slot.
fn crystal_request() -> Value {
    let mut request =
        fixture("fixtures/forever-gaps/enhancement-shaman-many-stat-auras.request.json");
    shaman_layout::forever_request(&mut request);
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"][13] =
        json!({ "id": NATURAL_ALIGNMENT_CRYSTAL });
    request
}

fn run(prepared: &Value) -> Value {
    let prepared: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    check_prepared(&prepared).unwrap();
    simulate_prepared(&prepared).unwrap().result["raidMetrics"]["parties"][0]["players"][0].clone()
}

fn position(prepared: &Value, spell_id: i64) -> usize {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .position(|spell| {
            spell["action_id"]["spell_id"] == spell_id && spell["action_id"]["tag"].is_null()
        })
        .unwrap_or_else(|| panic!("spell {spell_id}"))
}

/// Client 70790's Nature Aligned (23734): 20% more damage from every school but Physical (mask
/// 126) and 20% more mana spent on those spells, for 20 seconds on use. A damage dealer's
/// Lightning Bolt takes both, a physical strike neither.
#[test]
fn natural_alignment_crystal_raises_a_damage_dealers_spells() {
    let request = crystal_request();
    let prepared =
        prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever).unwrap();
    let effect = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "spell_mod_aura_on_use"
                && effect["item_id"] == NATURAL_ALIGNMENT_CRYSTAL
        })
        .expect("the crystal's effect");
    assert_eq!(effect["aura"], "Nature Aligned");
    let bolt = position(&prepared, LIGHTNING_BOLT_TOP);
    let changes = effect["changes"].as_array().unwrap();
    let of = |spell: usize| -> Vec<(String, f64)> {
        changes
            .iter()
            .filter(|change| change["spell"] == spell)
            .map(|change| {
                (
                    change["kind"].as_str().unwrap().to_string(),
                    change["value"].as_f64().unwrap(),
                )
            })
            .collect()
    };
    assert_eq!(
        of(bolt),
        vec![
            ("damage_done_percent".to_string(), 0.2),
            ("power_cost_percent_add".to_string(), 0.2),
        ]
    );
    for (index, spell) in prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        if spell["school"] == 1 {
            assert!(of(index).is_empty(), "physical spell {index} changed");
        }
    }

    let player = run(&prepared);
    let aura = player["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"] == NATURE_ALIGNED)
        .expect("Nature Aligned");
    assert!(aura["uptimeSecondsAvg"].as_f64().unwrap() > 19.0, "{aura}");

    // The Go reference's export still has no effect for it.
    assert!(
        prepare_json_with(request.to_string().as_bytes(), "rust", Rules::GoReference)
            .map(|prepared| {
                serde_json::from_value::<PreparedV2>(prepared)
                    .map(|prepared| check_prepared(&prepared).is_err())
                    .unwrap_or(true)
            })
            .unwrap_or(true)
    );
}

/// A Restoration Shaman rotation that casts Mana Spring Totem at Self (prod 4280d647): a totem
/// that only places its aura takes no target, so it drops as a cast at the target does.
#[test]
fn a_mana_spring_totem_cast_at_self_drops_the_totem() {
    let request = fixture("fixtures/healers/restoration-shaman.request.json");
    let text = request.to_string();
    assert!(text.contains(HEALING_STREAM_TOTEM));
    let request = text.replace(HEALING_STREAM_TOTEM, &MANA_SPRING_TOTEM.to_string());
    let prepared = prepare_json_with(request.as_bytes(), "rust", Rules::Forever).unwrap();
    let player = run(&prepared);
    let casts: f64 = player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"] == MANA_SPRING_TOTEM)
        .flat_map(|action| action["targets"].as_array().unwrap().clone())
        .map(|target| target["casts"].as_f64().unwrap_or(0.0))
        .sum();
    assert!(casts >= 20.0, "Mana Spring Totem casts {casts}");
    let up = player["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["id"]["spellId"] == MANA_SPRING_TOTEM)
        .expect("Mana Spring Totem");
    assert!(up["uptimeSecondsAvg"].as_f64().unwrap() > 60.0, "{up}");
}
