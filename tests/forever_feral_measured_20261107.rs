//! The feral druid mechanics measured in game on 7 November 2026: a Tauren druid's combat log at
//! levels 22 to 25 on client 1.60.1.70862. Rake's hit and tick, Rip's tick and Swipe take shares
//! of attack power that no client row states; an avoided Ferocious Bite gets 80% of its base
//! energy back. Forever's rules read the measured shares; the Go reference keeps its literals.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The live reference builds' talents in Forever's order (the fixtures' strings are the old tree's,
/// which Forever's rules refuse).
const FERAL_CAT: &str = "050022-55000032023012125001-055";
const FERAL_BEAR: &str = "-5500033212310212551-055";
const BITE: i64 = 31018;

fn production(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/production-{name}.request.json"
    ));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn druid(name: &str, talents: &str) -> Value {
    let mut request = production(name);
    request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(talents);
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

/// Rake takes 6.5% of attack power on the hit and on every tick (0.0648 and 0.0652 fitted), Rip
/// 0.0114 a tick for each combo point up to four, Swipe 3% a hit, where the reference has 5%, 1%
/// and 5%. Ferocious Bite's 3% a combo point was not measured and keeps the notes' 25%.
#[test]
fn the_measured_shares_are_forever_rules_only() {
    let cat = prepared(&druid("feral-druid", FERAL_CAT), Rules::Forever);
    assert_eq!(number(&effect(&cat, "rake")["attack_power_share"]), 0.065);
    let rip = effect(&cat, "rip");
    assert_eq!(number(&rip["attack_power_share_per_combo_point"]), 0.0114);
    assert_eq!(number(&rip["attack_power_share_max_points"]), 4.0);
    let bite = effect(&cat, "ferocious_bite");
    assert_eq!(number(&bite["attack_power_per_combo_point"]), 0.0375);

    let reference = prepared(&production("feral-druid"), Rules::GoReference);
    assert_eq!(
        number(&effect(&reference, "rake")["attack_power_share"]),
        0.05
    );
    let rip = effect(&reference, "rip");
    assert_eq!(number(&rip["attack_power_share_per_combo_point"]), 0.01);
    let bite = effect(&reference, "ferocious_bite");
    assert_eq!(number(&bite["attack_power_per_combo_point"]), 0.03);
}

/// Swipe's 3% a hit, the share rank 2 read on 457 hits as 0.0301, on every rank: the engine
/// casts the highest rank the character knows and takes the one share for it.
#[test]
fn swipe_takes_three_percent_of_attack_power() {
    let bear = prepared(&druid("feral-bear-druid", FERAL_BEAR), Rules::Forever);
    assert_eq!(
        number(&effect(&bear, "swipe")["attack_power_coefficient"]),
        0.03
    );
    let reference = prepared(&production("feral-bear-druid"), Rules::GoReference);
    assert_eq!(
        number(&effect(&reference, "swipe")["attack_power_coefficient"]),
        0.05
    );
}

fn cast(spell_id: i64, condition: Option<Value>) -> Value {
    let mut action = json!({"castSpell": {"spellId": {"spellId": spell_id}}});
    if let Some(condition) = condition {
        action["condition"] = condition;
    }
    json!({"action": action})
}

/// A cat that builds with Shred and bites at one combo point, against a target that dodges half
/// of what it throws: the first fight's log holds both landed and dodged bites.
fn dodged_bites(rules: Rules) -> Vec<String> {
    let mut request = match rules {
        Rules::Forever => druid("feral-druid", FERAL_CAT),
        Rules::GoReference => production("feral-druid"),
    };
    let player = &mut request["raid"]["parties"][0]["players"][0];
    let cat_form = json!({"not": {"val": {"auraIsActive": {"auraId": {"spellId": 768}}}}});
    let one_point = json!({"cmp": {"lhs": {"currentComboPoints": {}}, "op": "OpGe",
        "rhs": {"const": {"val": "1"}}}});
    player["rotation"] = json!({"type": "TypeAPL", "prepullActions": [], "priorityList": [
        cast(768, Some(cat_form)),
        cast(BITE, Some(one_point)),
        cast(9830, None),
    ]});
    request["simOptions"]["iterations"] = json!(1);
    let mut prepared = prepared(&request, rules);
    prepared["melee"]["defender_dodge"] = json!(0.5);
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    let log = report.result["logs"].as_str().unwrap().to_string();
    let lines: Vec<&str> = log.lines().collect();
    // Each dodged bite with the lines that follow it up to the next cast.
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains("{SpellID: 31018} Dodge"))
        .map(|(at, _)| {
            lines[at - 2..]
                .iter()
                .take_while(|line| !line.contains("Casting"))
                .copied()
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

/// Ferocious Bite's row flags Discount Power On Miss: of its 35 base energy an avoided bite
/// gives back 80% (28), keeps the extra energy and keeps the combo points. The log's five
/// avoided bites cost 6.1 to 6.7 energy, the row's 20% of 35 less the energy gained meanwhile.
/// The reference does the same, as the Go goldens record.
#[test]
fn an_avoided_bite_gets_back_80_percent_of_its_base_energy() {
    for rules in [Rules::Forever, Rules::GoReference] {
        let dodged = dodged_bites(rules);
        // A bite Clearcasting made free ("Spent 0.000") has nothing to give back.
        let paid: Vec<_> = dodged
            .iter()
            .filter(|bite| !bite.contains("Spent 0.000"))
            .collect();
        assert!(
            paid.len() > 5,
            "{rules:?}: {} paid dodged bites",
            paid.len()
        );
        for bite in paid {
            assert!(bite.contains("Spent 35.000 energy"), "{rules:?}: {bite}");
            assert!(bite.contains("Gained 28.000 energy"), "{rules:?}: {bite}");
            assert!(!bite.contains("combo points"), "{rules:?}: {bite}");
        }
    }
}
