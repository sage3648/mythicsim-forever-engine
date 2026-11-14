//! Blizzard's WoW Forever hotfixes of 13 November 2026, the lines that change a fight, under
//! Forever's rules only:
//!
//! - Shaman: "Windfury Totem is now correctly activated by all melee abilities even if it doesn't
//!   involve your main hand weapon." Developers' note: "Strikes from abilities that involve your
//!   offhand weapon like Whirlwind or Mutilate do not have a chance to activate Windfury. The
//!   main-hand strike from those abilities still does." Every ability's main hand strike rolls
//!   the totem; a triggered hit (a seal's proc, Windfury Weapon's strikes) is not an ability and
//!   stays out, which no source settles yet.
//! - Paladin: "Judgement of Command now activates on-hit effects and counts as a melee attack."
//!   Developers' note: "All other judgements will continue to be considered Ranged attacks."
//! - Items: "Blessed Band of Light's Hit stat has been removed." and "Darkspear signet rings
//!   have had their Mp5 corrected and now match Alliance-equivalent rings."
//!
//! The Warrior's side of the totem is in `tests/classes/warrior/windfury_totem.rs` and the
//! shield strikes' in `tests/forever_rules_20261112.rs`.

use forever_engine::{prepare_json_with, Rules};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// A production twisting rotation with Seal of Command and Windfury Totem (5 November 2026).
const COMMAND_FIXTURE: &str =
    "fixtures/paladin/retribution-paladin-unsealed-wrath-command-echoes.request.json";

/// Every rank of Judgement of Command's damage spell.
const JUDGEMENTS_OF_COMMAND: [i64; 5] = [20425, 20962, 20961, 20967, 20968];
/// One rank of each other seal's judgement: Righteousness, the Crusader, Wisdom, Light, Justice.
const OTHER_JUDGEMENTS: [i64; 5] = [20286, 20303, 20355, 20346, 20184];
/// Seal of Command's own hit, a triggered hit.
const SEAL_OF_COMMAND_HIT: i64 = 20424;

fn fixture(path: &str) -> Vec<u8> {
    fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
}

/// The Assassination Rogue, Enhancement Shaman, Cat and Bear references' talents of 14 November
/// 2026, in Forever's trees: the production requests carry the reference's.
const ASSASSINATION_TALENTS: &str = "00530311251051031-30230302020113";
const ENHANCEMENT_TALENTS: &str = "053303102-053030310051002251-03";
const CAT_TALENTS: &str = "050022-55000032023012125001-055";
const BEAR_TALENTS: &str = "-5503003212310212551-055";

/// A request with the party's Windfury Totem up, in these talents when any are given.
fn with_totem(path: &str, talents: &str) -> Vec<u8> {
    let mut request: Value = serde_json::from_slice(&fixture(path)).unwrap();
    request["raid"]["parties"][0]["buffs"]["windfuryTotem"] = json!(true);
    if !talents.is_empty() {
        request["raid"]["parties"][0]["players"][0]["talentsString"] = json!(talents);
    }
    serde_json::to_vec(&request).unwrap()
}

fn prepared(request: &[u8], rules: Rules) -> Value {
    prepare_json_with(request, "rust", rules).unwrap_or_else(|err| panic!("{err:?}"))
}

fn spells(prepared: &Value) -> &Vec<Value> {
    prepared["player"]["spells"].as_array().unwrap()
}

fn proc_mask(prepared: &Value, spell_id: i64) -> Vec<String> {
    let spell = spells(prepared)
        .iter()
        .find(|spell| spell["action_id"]["spell_id"] == spell_id)
        .unwrap_or_else(|| panic!("no spell {spell_id}"));
    serde_json::from_value(spell["proc_mask"].clone()).unwrap()
}

/// The spellbook positions of a spell id's entries with this tag.
fn positions(prepared: &Value, spell_id: i64, tag: i64) -> Vec<u64> {
    spells(prepared)
        .iter()
        .enumerate()
        .filter(|(_, spell)| {
            spell["action_id"]["spell_id"].as_i64() == Some(spell_id)
                && spell["action_id"]["tag"].as_i64().unwrap_or(0) == tag
        })
        .map(|(position, _)| position as u64)
        .collect()
}

/// The positions the party's Windfury Totem trigger hears.
fn totem_hears(prepared: &Value) -> Vec<u64> {
    let totem = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "windfury_totem")
        .expect("a windfury_totem effect");
    assert_eq!(totem["trigger_proc_chance"], json!(0.2));
    totem["trigger_spells"]
        .as_array()
        .unwrap()
        .iter()
        .map(|position| position.as_u64().unwrap())
        .collect()
}

/// Whether the totem hears every entry of the spell with this tag; the spell must exist.
fn hears(prepared: &Value, heard: &[u64], spell_id: i64, tag: i64) -> bool {
    let positions = positions(prepared, spell_id, tag);
    assert!(!positions.is_empty(), "no spell {spell_id} tag {tag}");
    let listed = positions
        .iter()
        .filter(|position| heard.contains(position))
        .count();
    assert!(
        listed == 0 || listed == positions.len(),
        "{spell_id} tag {tag}: {listed} of {} heard",
        positions.len()
    );
    listed > 0
}

/// Judgement of Command is a main hand special under Forever's rules again, as in the
/// reference, and the other seals' judgements stay ranged specials.
#[test]
fn judgement_of_command_is_a_melee_attack_and_the_rest_stay_ranged() {
    let forever = prepared(&fixture(COMMAND_FIXTURE), Rules::Forever);
    for id in JUDGEMENTS_OF_COMMAND {
        assert_eq!(proc_mask(&forever, id), ["ProcMaskMeleeMHSpecial"], "{id}");
    }
    for id in OTHER_JUDGEMENTS {
        assert_eq!(proc_mask(&forever, id), ["ProcMaskRangedSpecial"], "{id}");
    }
}

/// On-hit effects hear Judgement of Command: Windfury Totem lists every rank, and so does at
/// least one other effect that listens to melee hits. No effect lists another seal's judgement.
#[test]
fn on_hit_effects_hear_judgement_of_command_alone() {
    let forever = prepared(&fixture(COMMAND_FIXTURE), Rules::Forever);
    let heard = totem_hears(&forever);
    for id in JUDGEMENTS_OF_COMMAND {
        assert!(hears(&forever, &heard, id, 0), "{id}");
    }
    let command: Vec<u64> = JUDGEMENTS_OF_COMMAND
        .iter()
        .flat_map(|id| positions(&forever, *id, 0))
        .collect();
    let others: Vec<u64> = OTHER_JUDGEMENTS
        .iter()
        .flat_map(|id| positions(&forever, *id, 0))
        .collect();
    let mut listeners = Vec::new();
    for effect in forever["effects"].as_array().unwrap() {
        for key in ["trigger_spells", "spend_spells"] {
            let Some(listed) = effect.get(key).and_then(Value::as_array) else {
                continue;
            };
            let lists = |positions: &[u64]| {
                listed
                    .iter()
                    .any(|spell| positions.contains(&spell.as_u64().unwrap_or(u64::MAX)))
            };
            assert!(
                !lists(&others),
                "{} lists another seal's judgement in {key}",
                effect["kind"]
            );
            if lists(&command) && effect["kind"] != "windfury_totem" {
                listeners.push(effect["kind"].as_str().unwrap().to_string());
            }
        }
    }
    assert!(
        !listeners.is_empty(),
        "no effect but the totem hears Judgement of Command"
    );
}

/// A request in Forever's talent trees, the main hand strikes of its abilities the totem now
/// hears and the off hand strikes and triggered hits it does not, as `(spell, tag)`.
type Heard = (
    &'static str,
    &'static str,
    &'static [(i64, i64)],
    &'static [(i64, i64)],
);

const TOTEM_CASES: [Heard; 6] = [
    // Holy Strike; Seal of Command's hit and Seal of Righteousness's are triggered hits.
    (
        COMMAND_FIXTURE,
        "",
        &[(10333, 0)],
        &[(SEAL_OF_COMMAND_HIT, 0), (25713, 0)],
    ),
    // Mutilate's main hand strike and Garrote; Mutilate's off hand strike.
    (
        "fixtures/mage/prepared-v2/production-assassination-rogue.request.json",
        ASSASSINATION_TALENTS,
        &[(1241584, 1), (11290, 0)],
        &[(1241584, 2)],
    ),
    // Stormstrike's main hand strike; its off hand strike.
    (
        "fixtures/mage/prepared-v2/production-enhancement-shaman.request.json",
        ENHANCEMENT_TALENTS,
        &[(17364, 1)],
        &[(17364, 2)],
    ),
    // Shred, Rake and Ferocious Bite, which need no weapon.
    (
        "fixtures/mage/prepared-v2/production-feral-druid.request.json",
        CAT_TALENTS,
        &[(9830, 0), (9904, 0), (31018, 0)],
        &[],
    ),
    // Lacerate's weapon hit and Swipe.
    (
        "fixtures/mage/prepared-v2/production-feral-bear-druid.request.json",
        BEAR_TALENTS,
        &[(414647, 0), (9908, 0)],
        &[],
    ),
    // Raptor Strike and Wing Clip, as before, and Strider Kick.
    (
        "fixtures/mage/prepared-v2/production-survival-hunter.request.json",
        "",
        &[(14266, 0), (14268, 0), (1317257, 0)],
        &[],
    ),
];

#[test]
fn windfury_totem_hears_every_ability_main_hand_strike() {
    for (name, talents, heard_spells, unheard_spells) in TOTEM_CASES {
        let forever = prepared(&with_totem(name, talents), Rules::Forever);
        let heard = totem_hears(&forever);
        for (spell, tag) in heard_spells {
            assert!(
                hears(&forever, &heard, *spell, *tag),
                "{name}: {spell} tag {tag}"
            );
        }
        for (spell, tag) in unheard_spells {
            assert!(
                !hears(&forever, &heard, *spell, *tag),
                "{name}: {spell} tag {tag}"
            );
        }
        // No off hand strike and no triggered hit that is not an auto attack or an extra swing.
        for position in &heard {
            let spell = &spells(&forever)[*position as usize];
            let masks = spell["proc_mask"].to_string();
            assert!(masks.contains("ProcMaskMeleeMH"), "{name}: {masks}");
            assert!(!masks.contains("ProcMaskMeleeOH"), "{name}: {masks}");
            let auto = masks.contains("ProcMaskMeleeMHAuto");
            let passive = spell["flags"].to_string().contains("SpellFlagPassiveSpell");
            assert!(auto || !passive, "{name}: {}", spell["action_id"]);
        }
    }
}

/// Stormstrike Hammer, a one-hander that leaves the fingers free.
const STORMSTRIKE_HAMMER: i64 = 19104;

/// The holy paladin fixture with no talents wearing only Stormstrike Hammer and the given ring
/// (0 for none).
fn ring_request(ring: i64) -> Vec<u8> {
    let mut request: Value = serde_json::from_slice(&fixture(
        "fixtures/mage/prepared-v2/holy-paladin-healing.request.json",
    ))
    .unwrap();
    request["simOptions"]["iterations"] = json!(20);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    player["talentsString"] = json!("");
    let mut items = vec![json!({ "id": STORMSTRIKE_HAMMER })];
    if ring != 0 {
        items.push(json!({ "id": ring }));
    }
    player["equipment"] = json!({ "items": items });
    serde_json::to_vec(&request).unwrap()
}

/// What the ring adds to a stat under the given rules.
fn added(ring: i64, rules: Rules, stat: &str) -> f64 {
    let value = |ring: i64| {
        prepared(&ring_request(ring), rules)["player"]["stats"][stat]
            .as_f64()
            .unwrap()
    };
    value(ring) - value(0)
}

/// Blessed Band of Light (272407) keeps its Stamina, Spirit, healing and spell damage and gives
/// no hit under Forever's rules; the reference keeps the pinned row's 1%.
#[test]
fn blessed_band_of_light_gives_no_hit() {
    const BAND: i64 = 272_407;
    for stat in ["MeleeHitRating", "SpellHitRating"] {
        assert_eq!(added(BAND, Rules::Forever, stat), 0.0, "{stat}");
    }
    assert_eq!(added(BAND, Rules::GoReference, "MeleeHitRating"), 10.0);
    for (stat, value) in [("HealingPower", 31.0), ("SpellDamage", 10.0)] {
        assert_eq!(added(BAND, Rules::Forever, stat), value, "{stat}");
    }
}

/// The Darkspear Signets (272068 to 272071) restore what the Alliance's Militant's Bands
/// (271913 to 271916) do, 3, 3, 2 and 2 mana per 5 sec, where the pinned rows say 9, 7, 6 and 4.
#[test]
fn darkspear_signets_match_the_militants_bands() {
    const RINGS: [(i64, i64, f64, f64); 4] = [
        (272_068, 271_913, 3.0, 9.0),
        (272_069, 271_914, 3.0, 7.0),
        (272_070, 271_915, 2.0, 6.0),
        (272_071, 271_916, 2.0, 4.0),
    ];
    for (signet, band, live, pinned) in RINGS {
        assert_eq!(added(signet, Rules::Forever, "MP5"), live, "{signet}");
        assert_eq!(added(band, Rules::Forever, "MP5"), live, "{band}");
        assert_eq!(added(signet, Rules::GoReference, "MP5"), pinned, "{signet}");
        assert_eq!(
            added(signet, Rules::Forever, "Stamina"),
            added(band, Rules::Forever, "Stamina"),
            "{signet}"
        );
    }
}
