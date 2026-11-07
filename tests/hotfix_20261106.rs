//! WoW: Forever hotfixes of 6 November 2026 (blue post), under Forever's rules.
//!
//! - "The Soul Siphon talent no longer incorrectly causes Wrack to increase its own damage." The
//!   effects Soul Siphon counts are the warlock's Affliction dots and its own curse; Wrack and
//!   Drain Life are not among them, so a Wrack tick with nothing else on the target gets nothing.
//! - "Fixed an issue with Arcane Blast where its duration was not properly 18 seconds." The buff
//!   (400573) lasts 18 sec: the live row (SpellMisc duration index 85, 18000 ms in client
//!   1.60.1.70790) and its Wowhead tooltip ("18 seconds remaining"). The Go reference keeps 15.
//! - "Searing Dagger is now only activated by spells that have a damaging component." Sear
//!   (1291568) hears every harmful spell cast; Forever's rules leave out the casts that deal no
//!   damage, such as Life Tap and Faerie Fire. The client rows did not change.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const WRACK: i64 = 1316697;
const DRAIN_LIFE: i64 = 11700;
const ARCANE_BLAST: i64 = 1239700;
const ARCANE_BLAST_BUFF: i64 = 400573;
const SEARING_DAGGER: i64 = 273003;
const SEAR: i64 = 1291570;
const LIFE_TAP: i64 = 11689;
const SHADOW_BOLT: i64 = 25307;
const FAERIE_FIRE: i64 = 9907;
const WRATH: i64 = 9912;
const MAIN_HAND: usize = 14;

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2")
        .join(format!("{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

/// `request` in a fixed fight of `seconds`, 20 iterations, with `lines` as its rotation and
/// nothing before the pull.
fn with_rotation(mut request: Value, seconds: i64, lines: Vec<Value>) -> Value {
    request["encounter"]["duration"] = json!(seconds);
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(20);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    player(&mut request)["rotation"] = json!({
        "type": "TypeAPL",
        "prepullActions": [],
        "priorityList": lines.into_iter().map(|line| json!({"action": line})).collect::<Vec<_>>(),
    });
    request
}

fn cast(spell_id: i64) -> Value {
    json!({"castSpell": {"spellId": {"spellId": spell_id}}})
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

/// The first player's result of a prepared state.
fn simulate(prepared: Value) -> Value {
    let prepared = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["raidMetrics"]["parties"][0]["players"][0].clone()
}

fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

fn action(player: &Value, spell_id: i64) -> Option<&Value> {
    player["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|action| action["id"]["spellId"].as_i64() == Some(spell_id))
}

// Soul Siphon.

/// The production Affliction board request (Wrack build) with Wrack alone as its rotation.
fn wrack_alone() -> Value {
    with_rotation(
        fixture("production-affliction-warlock-wrack"),
        120,
        vec![cast(WRACK)],
    )
}

/// Neither Wrack nor Drain Life is an effect Soul Siphon counts, so a Wrack tick with nothing
/// else on the target deals what it deals with Soul Siphon worth nothing.
#[test]
fn wrack_does_not_count_itself_for_soul_siphon() {
    let forever = prepared(&wrack_alone(), Rules::Forever);
    let wrack = effect(&forever, "wrack");
    let counted: Vec<i64> = wrack["soul_siphon_live"]["dot_spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_i64().unwrap())
        .collect();
    assert!(!counted.is_empty());
    assert!(!counted.contains(&WRACK), "{counted:?}");
    assert!(!counted.contains(&DRAIN_LIFE), "{counted:?}");

    let tick = |prepared: Value| {
        let player = simulate(prepared);
        let range = &action(&player, WRACK).expect("Wrack was cast")["targets"][0]["tickRange"];
        range["total"].as_f64().unwrap() / range["count"].as_f64().unwrap()
    };
    let mut without = forever.clone();
    for effect in without["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "wrack" || effect["kind"] == "drain_life" {
            effect["soul_siphon_live"]["per_effect"] = json!(0.0);
        }
    }
    let alone = tick(forever);
    let unscaled = tick(without);
    assert!(
        (alone / unscaled - 1.0).abs() < 1e-12,
        "{alone} with Soul Siphon, {unscaled} without"
    );
}

// Arcane Blast.

/// One Arcane Blast at the pull and nothing after, in a fight of one minute: the mage fixture
/// with an Arcane string in the Forever tree, or the reference's own Arcane fixture.
fn one_arcane_blast(rules: Rules) -> Value {
    let request = match rules {
        Rules::Forever => {
            let mut request = fixture("frost-reference");
            player(&mut request)["talentsString"] = json!("050205203100311531-3-05350003");
            request
        }
        Rules::GoReference => fixture("arcane-blast"),
    };
    let first_second = json!({"cmp": {"op": "OpLt",
        "lhs": {"currentTime": {}}, "rhs": {"const": {"val": "1s"}}}});
    with_rotation(
        request,
        60,
        vec![
            json!({"castSpell": {"spellId": {"spellId": ARCANE_BLAST}}, "condition": first_second}),
        ],
    )
}

fn buff_uptime(player: &Value) -> f64 {
    player["auras"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|aura| aura["id"]["spellId"].as_i64() == Some(ARCANE_BLAST_BUFF))
        .map(|aura| aura["uptimeSecondsAvg"].as_f64().unwrap_or(0.0))
        .sum()
}

/// One Arcane Blast keeps its buff up for 18 seconds; the Go reference's row says 15.
#[test]
fn arcane_blast_lasts_eighteen_seconds() {
    let forever = prepared(&one_arcane_blast(Rules::Forever), Rules::Forever);
    let buff = forever["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Arcane Blast")
        .expect("the Arcane Blast buff");
    assert_eq!(buff["duration_ns"], 18_000_000_000_i64);
    let player = simulate(forever);
    // One cast in each of the 20 iterations.
    assert_eq!(
        action(&player, ARCANE_BLAST).unwrap()["targets"][0]["casts"],
        20
    );
    assert!(
        (buff_uptime(&player) - 18.0).abs() < 1e-9,
        "{}",
        buff_uptime(&player)
    );

    let reference = simulate(prepared(
        &one_arcane_blast(Rules::GoReference),
        Rules::GoReference,
    ));
    assert!(
        (buff_uptime(&reference) - 15.0).abs() < 1e-9,
        "{}",
        buff_uptime(&reference)
    );
}

// Searing Dagger.

/// `name` with Searing Dagger in the main hand and nothing in the off hand.
fn with_searing_dagger(name: &str) -> Value {
    let mut request = fixture(name);
    let items = player(&mut request)["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    while items.len() <= MAIN_HAND + 1 {
        items.push(json!({}));
    }
    items[MAIN_HAND] = json!({"id": SEARING_DAGGER});
    items[MAIN_HAND + 1] = json!({});
    request
}

/// The client spell ids of the casts Sear hears.
fn sear_triggers(prepared: &Value) -> Vec<i64> {
    let proc = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "spell_data_damage_proc" && effect["trigger_aura"] == "Searing Dagger"
        })
        .expect("the Searing Dagger proc");
    assert_eq!(proc["casts"], true);
    proc["trigger_spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|position| {
            let spell = &prepared["player"]["spells"][position.as_u64().unwrap() as usize];
            spell["action_id"]["spell_id"].as_i64().unwrap_or(0)
        })
        .collect()
}

/// Life Tap and Faerie Fire deal no damage, so their casts no longer sear the target; Shadow
/// Bolt and Wrath still do. The Go reference hears every one of them.
#[test]
fn searing_dagger_hears_only_damaging_casts() {
    for (name, idle, damaging) in [
        ("production-affliction-warlock-wrack", LIFE_TAP, SHADOW_BOLT),
        ("production-balance-druid", FAERIE_FIRE, WRATH),
    ] {
        let request = with_searing_dagger(name);
        let forever = sear_triggers(&prepared(&request, Rules::Forever));
        assert!(!forever.contains(&idle), "{name}: {forever:?}");
        assert!(forever.contains(&damaging), "{name}: {forever:?}");
        let reference = sear_triggers(&prepared(&request, Rules::GoReference));
        assert!(reference.contains(&idle), "{name}: {reference:?}");
        assert!(reference.contains(&damaging), "{name}: {reference:?}");
    }
}

/// A warlock that only taps life sears nothing; one that casts Shadow Bolt sears on each cast.
#[test]
fn life_tap_casts_no_longer_sear() {
    let sears = |spell_id: i64, rules: Rules| {
        let request = with_rotation(
            with_searing_dagger("production-affliction-warlock-wrack"),
            60,
            vec![cast(spell_id)],
        );
        let player = simulate(prepared(&request, rules));
        action(&player, SEAR).map_or(0, |sear| {
            sear["targets"][0]["hits"].as_i64().unwrap_or(0)
                + sear["targets"][0]["crits"].as_i64().unwrap_or(0)
        })
    };
    assert_eq!(sears(LIFE_TAP, Rules::Forever), 0);
    assert!(sears(LIFE_TAP, Rules::GoReference) > 0);
    assert!(sears(SHADOW_BOLT, Rules::Forever) > 0);
}
