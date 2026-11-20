//! Forever's rules from public Forever combat logs on forever.warcraftlogs.com of 11 to 18
//! November 2026 (clients 1.60.1.70938 and 70959) and the live item tooltip:
//!
//! - Spell Piercing past the target's resistance is rolled a hit, as the partial resist roll
//!   turned over: a direct hit gains a tenth or two tenths of its damage around an average of
//!   piercing / (piercing + 8 x caster level + 19). 18 casters in 45 dungeon reports: 1,114
//!   non-crit hits gained a tenth, 590 two tenths and 27 three. A binary spell gains nothing
//!   (Frostbolt 0 of 412, Cone of Cold 0 of 505, Frost Nova 0 of 463, Earth Shock 0 of 256, Mind
//!   Flay 0 of 915 ticks), nor do Arcane Wrath (0 of 87), Shadow Word: Death (0 of 68) and
//!   Flamestrike (0 of 168), nor a damage over time tick (Shadow Word: Pain 0 of 1,315, Curse of
//!   Agony 0 of 658, Corruption 0 of 454), save Devouring Plague's (59 of 461) and Hellfire's
//!   (179 of 415).
//! - Puncture Armor (Bashguuder, Rivenspike) and Armor Shatter (Annihilator) never land beside
//!   Faerie Fire or Curse of Recklessness: 0 applications in 14,568 hits of 12 wearers on targets
//!   under either, against 1,523 in 72,420 on targets under neither.
//! - Waveslicer (18324) rolls 1.95 times a minute: 910 procs in 9,170 landed hits of 5 wearers.
//! - The Lion Horn of Stormwind (14557) rolls 5%: its tooltip reads "has a 5% chance ... This
//!   chance is doubled in Strongholds and Cities", and the row's 10% is the doubled chance.
//!
//! The Go reference keeps its own rules.

use forever_engine::{prepare_json_with, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

const ARCANE_BLAST: i64 = 1239700;
const ARCANE_WRATH: i64 = 1326082;
const MIND_FLAY: i64 = 18807;
const SHADOW_WORD_PAIN: i64 = 10894;
const DEVOURING_PLAGUE: i64 = 19280;
const PUNCTURE_ARMOR: i64 = 17315;

/// A live board request of 19 November 2026, which Forever's rules prepare as it is.
fn board(name: &str, iterations: u32) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "fixtures/mage/prepared-v2/board-{name}-20261119.request.json"
    ));
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["simOptions"]["iterations"] = json!(iterations);
    request["simOptions"]["debugFirstIteration"] = json!(false);
    request
}

fn prepared(request: &Value, rules: Rules) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", rules)
        .unwrap_or_else(|err| panic!("{err:?}"))
}

fn run(prepared: &Value) -> Value {
    let prepared = serde_json::from_value(prepared.clone()).unwrap();
    simulate_prepared(&prepared).unwrap().result
}

/// The prepared fight with the player's Spell Piercing set.
fn with_piercing(prepared: &Value, piercing: f64) -> Value {
    let mut prepared = prepared.clone();
    prepared["player"]["stats"]["SpellPiercing"] = json!(piercing);
    prepared
}

/// The flags of the player's spell with this id.
fn flags(prepared: &Value, spell_id: i64) -> Vec<String> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == spell_id)
        .unwrap_or_else(|| panic!("spell {spell_id} is registered"))["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|flag| flag.as_str().unwrap().to_string())
        .collect()
}

/// The damage a spell dealt over the run, every tag of it.
fn damage(result: &Value, spell_id: i64) -> f64 {
    result["raidMetrics"]["parties"][0]["players"][0]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|action| action["id"]["spellId"] == spell_id)
        .flat_map(|action| action["targets"].as_array().unwrap().iter())
        .map(|target| target["damage"].as_f64().unwrap_or(0.0))
        .sum()
}

fn armor_debuff(prepared: &Value) -> &Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "armor_debuff_proc")
        .expect("the weapon's armor debuff proc")
}

/// The seconds a fight the target carried Puncture Armor.
fn puncture_armor_uptime(result: &Value) -> f64 {
    result["encounterMetrics"]["targets"][0]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|aura| aura["id"]["spellId"] == PUNCTURE_ARMOR)
        .map(|aura| aura["uptimeSecondsAvg"].as_f64().unwrap_or(0.0))
        .fold(0.0, f64::max)
}

// ---------------------------------------------------------------------------------------------
// Spell Piercing.
// ---------------------------------------------------------------------------------------------

/// The prepared target rolls Spell Piercing under Forever's rules; the binary spells, Arcane
/// Wrath and Shadow Word: Death are flagged to gain nothing, and Devouring Plague's ticks to
/// gain as hits do.
#[test]
fn piercing_flags_follow_the_logs() {
    let mage = prepared(&board("arcane-mage", 1), Rules::Forever);
    assert_eq!(mage["target"]["spell_piercing_rolls"], true);
    let none = "SpellFlagNoPiercingBonus".to_string();
    let ticks = "SpellFlagPiercingTicks".to_string();
    // Frostbolt rank 11, binary.
    assert!(flags(&mage, 25304).contains(&none));
    assert!(flags(&mage, ARCANE_WRATH).contains(&none));
    assert!(!flags(&mage, ARCANE_BLAST).contains(&none));
    // Arcane Explosion and Fire Blast.
    assert!(!flags(&mage, 10202).contains(&none));
    assert!(!flags(&mage, 10199).contains(&none));

    let priest = prepared(&board("shadow-priest", 1), Rules::Forever);
    assert!(flags(&priest, MIND_FLAY).contains(&none));
    // Shadow Word: Death rank 4.
    assert!(flags(&priest, 1309636).contains(&none));
    assert!(flags(&priest, DEVOURING_PLAGUE).contains(&ticks));
    assert!(!flags(&priest, SHADOW_WORD_PAIN).contains(&ticks));
    assert!(!flags(&priest, SHADOW_WORD_PAIN).contains(&none));
}

/// With 60 Spell Piercing a direct hit averages 60 / 559 more damage, 10.7%, well under the
/// reference's 60 / 315; Arcane Wrath gains nothing.
#[test]
fn a_direct_hit_rolls_piercing_and_arcane_wrath_gains_nothing() {
    let prepared = prepared(&board("arcane-mage", 1500), Rules::Forever);
    let without = run(&with_piercing(&prepared, 0.0));
    let with = run(&with_piercing(&prepared, 60.0));
    let blast = damage(&with, ARCANE_BLAST) / damage(&without, ARCANE_BLAST);
    let expected = 1.0 + 60.0 / (60.0 + 8.0 * 60.0 + 19.0);
    assert!((blast - expected).abs() < 0.025, "Arcane Blast: {blast}");
    let wrath = damage(&with, ARCANE_WRATH) / damage(&without, ARCANE_WRATH);
    assert!((wrath - 1.0).abs() < 0.025, "Arcane Wrath: {wrath}");
}

/// Mind Flay and Shadow Word: Pain's ticks gain nothing from 60 Spell Piercing; Devouring
/// Plague's ticks gain the 10.7%.
#[test]
fn ticks_gain_nothing_but_devouring_plagues() {
    let prepared = prepared(&board("shadow-priest", 1500), Rules::Forever);
    let without = run(&with_piercing(&prepared, 0.0));
    let with = run(&with_piercing(&prepared, 60.0));
    let ratio = |spell| damage(&with, spell) / damage(&without, spell);
    let flay = ratio(MIND_FLAY);
    assert!((flay - 1.0).abs() < 0.02, "Mind Flay: {flay}");
    let pain = ratio(SHADOW_WORD_PAIN);
    assert!((pain - 1.0).abs() < 0.02, "Shadow Word: Pain: {pain}");
    let plague = ratio(DEVOURING_PLAGUE);
    let expected = 1.0 + 60.0 / (60.0 + 8.0 * 60.0 + 19.0);
    assert!(
        (plague - expected).abs() < 0.03,
        "Devouring Plague: {plague}"
    );
}

/// A fight with no Spell Piercing draws no piercing roll: it reads the same whichever way
/// piercing would be counted.
#[test]
fn no_piercing_changes_nothing() {
    let prepared = prepared(&board("shadow-priest", 50), Rules::Forever);
    let mut fight = with_piercing(&prepared, 0.0);
    let rolled = run(&fight);
    fight["target"]["spell_piercing_rolls"] = json!(false);
    assert_eq!(rolled, run(&fight));
}

// ---------------------------------------------------------------------------------------------
// Puncture Armor.
// ---------------------------------------------------------------------------------------------

/// The Fury board wears Bashguuder and fights under the raid's Faerie Fire and Curse of
/// Recklessness: Puncture Armor never lands.
#[test]
fn puncture_armor_never_lands_beside_faerie_fire() {
    let request = board("warrior", 200);
    let prepared = prepared(&request, Rules::Forever);
    let blocked: Vec<&str> = armor_debuff(&prepared)["blocked_by"]
        .as_array()
        .expect("the debuff names what blocks it")
        .iter()
        .map(|label| label.as_str().unwrap())
        .collect();
    assert!(blocked.contains(&"Faerie Fire (External)"), "{blocked:?}");
    assert!(
        blocked.contains(&"Curse of Recklessness (External)"),
        "{blocked:?}"
    );
    assert_eq!(puncture_armor_uptime(&run(&prepared)), 0.0);
}

/// Without either debuff on the target, Puncture Armor lands as before, beside Sunder Armor.
#[test]
fn puncture_armor_lands_beside_sunder_armor() {
    let mut request = board("warrior", 200);
    request["raid"]["debuffs"]["faerieFire"] = json!(false);
    request["raid"]["debuffs"]["curseOfRecklessness"] = json!(false);
    assert_eq!(request["raid"]["debuffs"]["sunderArmor"], true);
    let prepared = prepared(&request, Rules::Forever);
    let uptime = puncture_armor_uptime(&run(&prepared));
    assert!(uptime > 30.0, "{uptime}");
}

// ---------------------------------------------------------------------------------------------
// Waveslicer and The Lion Horn of Stormwind.
// ---------------------------------------------------------------------------------------------

/// The Fury board's main hand weapon's position in its equipment list, with the off hand after
/// it; preparation places an item by its type.
const MAIN_HAND: usize = 12;

/// Waveslicer rolls 1.95 times a minute of its 3.1 s speed, in place of the 1.08 assumed for a
/// chance on hit the client states no rate for.
#[test]
fn waveslicer_rolls_its_measured_rate() {
    let mut request = board("warrior", 1);
    let equipment = request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    equipment[MAIN_HAND] = json!({ "id": 18324 });
    equipment.remove(MAIN_HAND + 1);
    let prepared = prepared(&request, Rules::Forever);
    let speed = prepared["melee"]["main_hand"]["swing_speed"]
        .as_f64()
        .unwrap();
    assert_eq!(speed, 3.1);
    let proc = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["trigger_aura"]
                .as_str()
                .is_some_and(|aura| aura.contains("Waveslicer"))
        })
        .expect("Waveslicer's chance on hit");
    let chances = proc["chances"].as_array().unwrap();
    assert!(!chances.is_empty());
    for chance in chances {
        let chance = chance["chance"].as_f64().unwrap();
        assert!((chance - 1.95 * 3.1 / 60.0).abs() < 1e-9, "{chance}");
    }
}

/// The proc of The Lion Horn of Stormwind on a tank who wears it.
fn lion_horn_chance(mut request: Value, rules: Rules) -> f64 {
    request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": 14557 }));
    let prepared = prepared(&request, rules);
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["trigger_aura"] == "The Lion Horn of Stormwind")
        .expect("the Lion Horn's proc")["proc_chance"]
        .as_f64()
        .unwrap()
}

/// The Lion Horn of Stormwind rolls 5% on a hit the Protection board takes, half the row's 10%.
#[test]
fn the_lion_horn_rolls_half_its_row() {
    let chance = lion_horn_chance(board("protection-warrior", 1), Rules::Forever);
    assert_eq!(chance, 0.05);
}

/// The reference keeps the row's 10%.
#[test]
fn the_reference_keeps_the_lion_horns_row() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/production-protection-warrior-3-targets.request.json");
    let request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(lion_horn_chance(request, Rules::GoReference), 0.1);
}
