//! The Balance changes of 25 November 2026 under Forever's rules (blue post "Balance Druid
//! Adjustments - November 25", live that day as a hotfix; clients 1.60.1.71037 to 71139 still
//! carry the old rows in their CDN tables, so the values are Wowhead's Forever rows of 26 November).
//!
//! - Improved Wrath is Starlight Wrath: its cast time effect (16814, -100 ms a rank) names Starfire
//!   beside Wrath.
//! - Improved Starfire has 2 ranks and only a stun chance: the cast time effect is a dummy now.
//! - Eclipse adds 7, 13 or 20% critical strike chance to Wrath at all times (408248's fifth effect).
//! - Moonfury is gone (Dreamstate takes its square) and Moonkin Form deals 10% more Arcane and
//!   Nature damage while the druid is in it.
//!
//! The Go reference keeps the old tree.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WRATH: i64 = 9912;
const STARFIRE: i64 = 9876;

fn request(talents: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-balance-druid.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(talents);
    request
}

fn prepare(talents: &str, rules: Rules) -> Value {
    prepare_json_with(request(talents).to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{talents}: {err:?}"))
}

fn spell(prepared: &Value, id: i64) -> &Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["action_id"]["spell_id"].as_i64() == Some(id))
        .unwrap_or_else(|| panic!("spell {id} is not prepared"))
}

fn school(prepared: &Value, school: &str) -> f64 {
    prepared["player"]["pseudo_stats"]["school_damage_dealt_multiplier"][school]
        .as_f64()
        .unwrap()
}

/// The board reference since 25 November: Improved Starfire 2, Dreamstate 5, Reflection 3.
const NEW_TREE: &str = "5532220115201351--0553";
/// The same with no Moonkin Form, Eclipse or Starlight Wrath.
const WITHOUT: &str = "0532220115201000--0553";

#[test]
fn starlight_wrath_also_speeds_starfire() {
    let prepared = prepare(NEW_TREE, Rules::Forever);
    let without = prepare(WITHOUT, Rules::Forever);
    let cast = |p: &Value, id| {
        spell(p, id)["default_cast"]["cast_time_ns"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(
        cast(&without, STARFIRE) - cast(&prepared, STARFIRE),
        500_000_000
    );
    assert_eq!(cast(&without, WRATH) - cast(&prepared, WRATH), 500_000_000);
}

#[test]
fn eclipse_adds_wrath_crit() {
    let prepared = prepare(NEW_TREE, Rules::Forever);
    let without = prepare(WITHOUT, Rules::Forever);
    let crit = |p: &Value, id| spell(p, id)["bonus_crit_percent"].as_f64().unwrap();
    assert_eq!(crit(&prepared, WRATH) - crit(&without, WRATH), 20.0);
    assert_eq!(crit(&prepared, STARFIRE), crit(&without, STARFIRE));
}

#[test]
fn moonkin_form_deals_ten_percent_more_arcane_and_nature_damage() {
    let prepared = prepare(NEW_TREE, Rules::Forever);
    assert_eq!(school(&prepared, "arcane"), 1.1);
    assert_eq!(school(&prepared, "nature"), 1.1);
    assert_eq!(school(&prepared, "fire"), 1.0);
    // Five points on Moonfury's square are Dreamstate's now and add nothing.
    let no_dreamstate = prepare("5532220115201301--0553", Rules::Forever);
    assert_eq!(school(&no_dreamstate, "arcane"), 1.1);
    // The Go reference still reads Moonfury there, and Moonkin Form adds no damage.
    let reference = prepare("5532220115501351--505", Rules::GoReference);
    assert!((school(&reference, "arcane") - 1.1).abs() < 1e-12);
}

#[test]
fn improved_starfire_has_two_ranks() {
    let refused = prepare_json_with(
        request("5532220115501351--055").to_string().as_bytes(),
        "rust",
        Rules::Forever,
    );
    let err = format!(
        "{:?}",
        refused.expect_err("five points in Improved Starfire")
    );
    assert!(err.contains("Improved Starfire has 2 ranks"), "{err}");
    // Its two ranks are a stun chance the sim does not model: no cast time.
    let one = prepare("5532220115101351--0553", Rules::Forever);
    let two = prepare(NEW_TREE, Rules::Forever);
    let cast = |p: &Value| {
        spell(p, STARFIRE)["default_cast"]["cast_time_ns"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(cast(&one), cast(&two));
}
