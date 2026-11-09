//! Shaman totems under Forever's rules (4 November 2026).
//!
//! Fire totems stand where the shaman drops them, "at the feet of the caster" (Magma Totem and
//! Searing Totem tooltips), and reach only so far from there: Magma Totem's pulse 8 yards (10581's
//! area), Fire Nova "enemies within 10 yd of your active Fire totem" (408428's area) and Searing
//! Totem's attack 20 yards (10436's range). The Go reference hits every target from any distance.
//!
//! The party's Windfury Totem and the shaman's own both give aura 10612 (an area aura with no
//! cumulative stacking and no stacking for different casters), and a main hand Windfury Weapon
//! "disables any benefit you personally receive from Windfury Totem" (every rank's tooltip). So
//! under Forever's rules the shaman's own totem adds nothing beside either, where the reference's
//! export is refused.

use forever_engine::{prepare_json_with, simulate_prepared, PrepareError, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const MAGMA_TOTEM: i64 = 10587;
const SEARING_TOTEM: i64 = 10438;
const SEARING_ATTACK: i64 = 10436;
const FIRE_NOVA: i64 = 408345;
const LIGHTNING_BOLT: i64 = 15208;
const WINDFURY_TOTEM: i64 = 10614;
const STORMSTRIKE: i64 = 17364;

fn request(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/mage/prepared-v2/{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

fn cast(spell_id: i64) -> Value {
    json!({"castSpell": {"spellId": {"spellId": spell_id}}})
}

fn not_active(spell_id: i64) -> Value {
    json!({"not": {"val": {"dotIsActive": {"spellId": {"spellId": spell_id}}}}})
}

fn time_before(seconds: &str) -> Value {
    json!({"cmp": {"lhs": {"currentTime": {}}, "op": "OpLt", "rhs": {"const": {"val": seconds}}}})
}

fn time_from(seconds: &str) -> Value {
    json!({"cmp": {"lhs": {"currentTime": {}}, "op": "OpGe", "rhs": {"const": {"val": seconds}}}})
}

/// The production Elemental request with `lines` as its rotation, standing at `distance` yards,
/// against `targets` copies of its boss in a fixed-length fight.
fn elemental(lines: Vec<Value>, distance: f64, targets: usize, seconds: i64) -> Value {
    let mut request = request("production-elemental-shaman");
    let boss = request["encounter"]["targets"][0].clone();
    request["encounter"]["targets"] = json!(vec![boss; targets]);
    request["encounter"]["duration"] = json!(seconds);
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(50);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    let player = player(&mut request);
    player["distanceFromTarget"] = json!(distance);
    player["rotation"] = json!({
        "type": "TypeAPL",
        "prepullActions": [],
        "priorityList": lines.into_iter().map(|line| json!({"action": line})).collect::<Vec<_>>(),
    });
    request
}

fn run(request: &Value, rules: Rules) -> Value {
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"));
    let prepared = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["raidMetrics"]["parties"][0]["players"][0].clone()
}

/// The casts and the damage of the player's actions with this spell id, every tag together.
fn action(player: &Value, spell_id: i64) -> (f64, f64) {
    let mut totals = (0.0, 0.0);
    for action in player["actions"].as_array().unwrap() {
        if action["id"]["spellId"].as_i64() != Some(spell_id) {
            continue;
        }
        for target in action["targets"].as_array().unwrap() {
            totals.0 += target["casts"].as_f64().unwrap_or(0.0);
            totals.1 += target["damage"].as_f64().unwrap_or(0.0);
        }
    }
    totals
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

/// The client radii, which only Forever's rules export.
#[test]
fn fire_totem_reach_is_exported_under_forever_rules() {
    let request = request("production-elemental-shaman");
    let forever =
        prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever).unwrap();
    assert_eq!(effect(&forever, "magma_totem")["radius_yards"], 8.0);
    assert_eq!(effect(&forever, "fire_nova")["radius_yards"], 10.0);
    assert_eq!(
        effect(&forever, "searing_totem")["attack_range_yards"],
        20.0
    );
    let reference =
        prepare_json_with(request.to_string().as_bytes(), "rust", Rules::GoReference).unwrap();
    assert!(effect(&reference, "magma_totem")
        .get("radius_yards")
        .is_none());
    assert!(effect(&reference, "fire_nova")
        .get("radius_yards")
        .is_none());
    assert!(effect(&reference, "searing_totem")
        .get("attack_range_yards")
        .is_none());
}

/// A Magma Totem dropped at 15 yards burns nothing; at 8 yards it burns every target. The
/// reference burns at any distance.
#[test]
fn magma_totem_reaches_eight_yards() {
    let lines = || {
        vec![
            json!({"castSpell": {"spellId": {"spellId": MAGMA_TOTEM}}, "condition": not_active(MAGMA_TOTEM)}),
            cast(LIGHTNING_BOLT),
        ]
    };
    let far = run(&elemental(lines(), 15.0, 3, 60), Rules::Forever);
    let (casts, damage) = action(&far, MAGMA_TOTEM);
    assert!(casts > 0.0, "the totem is still dropped");
    assert_eq!(damage, 0.0);

    let near = run(&elemental(lines(), 8.0, 3, 60), Rules::Forever);
    assert!(action(&near, MAGMA_TOTEM).1 > 0.0);

    let reference = run(&elemental(lines(), 15.0, 3, 60), Rules::GoReference);
    assert!(action(&reference, MAGMA_TOTEM).1 > 0.0);
}

/// Fire Nova bursts around the active fire totem: 10 yards from it.
#[test]
fn fire_nova_reaches_ten_yards_from_the_totem() {
    let lines = || {
        vec![
            json!({"castSpell": {"spellId": {"spellId": MAGMA_TOTEM}}, "condition": not_active(MAGMA_TOTEM)}),
            cast(FIRE_NOVA),
            cast(LIGHTNING_BOLT),
        ]
    };
    for (distance, reaches) in [(15.0, false), (10.0, true), (9.0, true)] {
        let player = run(&elemental(lines(), distance, 3, 60), Rules::Forever);
        let (casts, damage) = action(&player, FIRE_NOVA);
        assert!(casts > 0.0, "{distance} yards: Fire Nova is cast");
        assert_eq!(damage > 0.0, reaches, "{distance} yards");
        // Magma Totem's 8 yards: at 9 and 10 yards only the nova reaches.
        assert_eq!(
            action(&player, MAGMA_TOTEM).1 > 0.0,
            distance <= 8.0,
            "{distance} yards"
        );
    }
    let reference = run(&elemental(lines(), 15.0, 3, 60), Rules::GoReference);
    assert!(action(&reference, FIRE_NOVA).1 > 0.0);
}

/// Searing Totem attacks from 20 yards, so a caster at 15 still has it.
#[test]
fn searing_totem_attacks_from_twenty_yards() {
    let lines = || {
        vec![
            json!({"castSpell": {"spellId": {"spellId": SEARING_TOTEM}}, "condition": not_active(SEARING_TOTEM)}),
            cast(LIGHTNING_BOLT),
        ]
    };
    let at_15 = run(&elemental(lines(), 15.0, 1, 60), Rules::Forever);
    assert!(action(&at_15, SEARING_ATTACK).1 > 0.0);
    let at_25 = run(&elemental(lines(), 25.0, 1, 60), Rules::Forever);
    assert!(action(&at_25, SEARING_TOTEM).0 > 0.0);
    assert_eq!(action(&at_25, SEARING_ATTACK).1, 0.0);
    let reference = run(&elemental(lines(), 25.0, 1, 60), Rules::GoReference);
    assert!(action(&reference, SEARING_ATTACK).1 > 0.0);
}

/// The totem stays where it was dropped: moving away after the drop keeps it burning, and
/// moving in after a drop from range does not.
#[test]
fn a_totem_stays_where_it_was_dropped() {
    let lines = |to: &str| {
        vec![
            json!({"castSpell": {"spellId": {"spellId": MAGMA_TOTEM}},
                   "condition": {"and": {"vals": [not_active(MAGMA_TOTEM), time_before("1s")]}}}),
            json!({"move": {"rangeFromTarget": {"const": {"val": to}}}, "condition": time_from("2s")}),
            cast(LIGHTNING_BOLT),
        ]
    };
    // Dropped at 5 yards, then the shaman walks out to 20: the totem burns the whole fight.
    let away = run(&elemental(lines("20"), 5.0, 1, 15), Rules::Forever);
    let (casts, damage) = action(&away, MAGMA_TOTEM);
    assert_eq!(casts, 50.0, "one drop each iteration");
    assert!(damage > 0.0);
    // Dropped at 20 yards, then the shaman walks in to 5: it never reaches.
    let inside = run(&elemental(lines("5"), 20.0, 1, 15), Rules::Forever);
    assert_eq!(action(&inside, MAGMA_TOTEM), (50.0, 0.0));
}

/// The production Enhancement request (Rockbiter Weapon main hand, the party's Windfury Totem)
/// with the shaman's own Windfury Totem kept up first.
fn enhancement_with_own_windfury(party_totem: bool, imbue: &str) -> Value {
    let mut request = request("production-enhancement-shaman");
    request["simOptions"]["iterations"] = json!(200);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request["raid"]["parties"][0]["buffs"]["windfuryTotem"] = json!(party_totem);
    // The live Enhancement reference's talents on the launch tree.
    player(&mut request)["talentsString"] = json!("453003102-055030310051002251");
    let player = player(&mut request);
    player["enhancementShaman"]["options"]["classOptions"]["imbueMh"] = json!(imbue);
    let list = player["rotation"]["priorityList"].as_array_mut().unwrap();
    list.insert(
        0,
        json!({"action": {"castSpell": {"spellId": {"spellId": WINDFURY_TOTEM}},
            "condition": {"cmp": {"lhs": {"totemRemainingTime": {"totemType": "Air"}},
                "op": "OpLe", "rhs": {"const": {"val": "0s"}}}}}}),
    );
    request
}

/// The extra attacks the shaman's own totem granted (tag 10610) and the party's (tag 25584).
fn windfury_attacks(player: &Value) -> (f64, f64) {
    let mut own = 0.0;
    let mut party = 0.0;
    for action in player["actions"].as_array().unwrap() {
        let tag = action["id"]["tag"].as_i64();
        let landed: f64 = action["targets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|target| target["casts"].as_f64().unwrap_or(0.0))
            .sum();
        match tag {
            Some(10610) if action["id"]["otherId"] == "OtherActionAttack" => own += landed,
            Some(25584) if action["id"]["otherId"] == "OtherActionAttack" => party += landed,
            _ => {}
        }
    }
    (own, party)
}

fn refusal(request: &Value, rules: Rules) -> Option<String> {
    let prepared = match prepare_json_with(request.to_string().as_bytes(), "rust", rules) {
        Ok(prepared) => prepared,
        Err(PrepareError::Refused(refusal)) => return Some(refusal.reason),
        Err(other) => panic!("{other:?}"),
    };
    let prepared = serde_json::from_value(prepared).unwrap();
    forever_engine::prepared_coverage(&prepared)
        .into_iter()
        .find(|reason| reason.contains("Windfury Totem"))
}

/// Beside the party's Windfury Totem the shaman's own cast is the same aura: it is no longer
/// refused, its trigger never fires, and the party's keeps granting the extra attacks.
#[test]
fn own_windfury_totem_beside_the_partys_is_one_aura() {
    let request = enhancement_with_own_windfury(true, "RockbiterWeapon");
    assert_eq!(refusal(&request, Rules::Forever), None);
    let reason = refusal(&request, Rules::GoReference).expect("the reference refuses it");
    assert!(reason.contains("contests the party's"), "{reason}");

    let player = run(&request, Rules::Forever);
    assert!(action(&player, WINDFURY_TOTEM).0 > 0.0, "the totem is cast");
    let (own, party) = windfury_attacks(&player);
    assert_eq!(own, 0.0);
    assert!(party > 0.0);
}

/// A main hand Windfury Weapon turns off the benefit of the shaman's own totem, as it does the
/// party's: the imbue's attacks only.
#[test]
fn windfury_weapon_turns_off_the_own_totem() {
    let request = enhancement_with_own_windfury(false, "WindfuryWeapon");
    assert_eq!(refusal(&request, Rules::Forever), None);
    assert!(refusal(&request, Rules::GoReference).is_some());
    let player = run(&request, Rules::Forever);
    assert!(action(&player, WINDFURY_TOTEM).0 > 0.0);
    assert_eq!(windfury_attacks(&player), (0.0, 0.0));
    // Windfury Weapon's main hand strike still lands.
    assert!(action(&player, 439440).1 > 0.0);
    assert!(action(&player, STORMSTRIKE).0 > 0.0);
}

/// Alone, the shaman's own totem works as before.
#[test]
fn own_windfury_totem_alone_procs() {
    let request = enhancement_with_own_windfury(false, "RockbiterWeapon");
    assert_eq!(refusal(&request, Rules::Forever), None);
    let player = run(&request, Rules::Forever);
    let (own, party) = windfury_attacks(&player);
    assert!(own > 0.0);
    assert_eq!(party, 0.0);
}

/// The shaman's own totem gives the party's aura and follows its Forever rules (Forever combat
/// logs): a landed hit rolls it without damage, the auto that fires it spends no charge and
/// every outcome of an auto spends one. The reference keeps its own rules.
#[test]
fn own_windfury_totem_follows_the_partys_rules() {
    let request = enhancement_with_own_windfury(false, "RockbiterWeapon");
    let own = |rules: Rules| -> Value {
        let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", rules).unwrap();
        prepared["effects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|effect| effect["kind"] == "windfury_totem_self")
            .expect("the own totem's effect")
            .clone()
    };
    let flags = [
        "trigger_without_damage",
        "trigger_spends_no_charge",
        "any_outcome_spends",
    ];
    let forever = own(Rules::Forever);
    let reference = own(Rules::GoReference);
    for flag in flags {
        assert_eq!(forever[flag], json!(true), "{flag}");
        assert!(reference.get(flag).is_none(), "{flag}");
    }
}
