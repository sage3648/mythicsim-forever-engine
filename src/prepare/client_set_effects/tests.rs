//! The 5 piece bonuses of the Tier 1 class sets (ItemSets 2098 to 2115), which no request can reach:
//! an item goes to the slot its type names, so only four crafted pieces of a set can be worn. The
//! tests count extra pieces beside the equipment (`item_sets::EXTRA_PIECES`) and read the
//! prepared input a class gets with five pieces against four. The 2, 3 and 4 piece bonuses are
//! checked end to end in `tests/forever_tier1_sets_20261113.rs`.

use serde_json::{json, Value};

use crate::prepare::item_sets::EXTRA_PIECES;
use crate::Rules;

#[path = "../../../tests/support/rogue_113483.rs"]
mod rogue_113483;
#[path = "../../../tests/support/shaman_layout.rs"]
mod shaman_layout;
#[path = "../../../tests/support/warrior_trees_20261104.rs"]
mod warrior_trees_20261104;

/// The live Feral board's talents in client 1.60.1.70707's order, which include Shifting Power.
const FERAL_TALENTS: &str = "050022-55000032023012125001-055";

struct Case {
    set: &'static str,
    id: i32,
    fixture: &'static str,
}

const MAGE: Case = Case {
    set: "Manaflare Regalia",
    id: 2098,
    fixture: "fixtures/dev-notes-20261102/arcane-mage.request.json",
};
const ROGUE: Case = Case {
    set: "Grimstitch Armor",
    id: 2099,
    fixture: "fixtures/forever-gaps/combat-rogue-darkmantle-riposte.request.json",
};
const WARLOCK: Case = Case {
    set: "Demonheart Raiment",
    id: 2100,
    fixture: "fixtures/forever-gaps/demonology-warlock-mob-type-bonus.request.json",
};
const HUNTER: Case = Case {
    set: "Wildstalker Armor",
    id: 2101,
    fixture: "fixtures/forever-gaps/marksmanship-hunter-sequence-conditions.request.json",
};
const ARMS: Case = Case {
    set: "Battlegear of Glory",
    id: 2102,
    fixture: "fixtures/forever-gaps/arms-warrior-revenge.request.json",
};
const PROTECTION: Case = Case {
    set: "Battleplate of Glory",
    id: 2103,
    fixture: "fixtures/forever-gaps/protection-warrior-shield-bash.request.json",
};
const SHADOW: Case = Case {
    set: "Raiments of Conviction",
    id: 2105,
    fixture: "fixtures/forever-gaps/shadow-priest-chastise.request.json",
};
const ENHANCEMENT: Case = Case {
    set: "The Spiritcaller's Rage",
    id: 2110,
    fixture: "fixtures/forever-gaps/enhancement-shaman-beaststalker-armor.request.json",
};
const ELEMENTAL: Case = Case {
    set: "The Spiritcaller's Storm",
    id: 2111,
    fixture: "fixtures/forever-gaps/elemental-shaman-the-elements.request.json",
};
const GUARDIAN: Case = Case {
    set: "Grovekeeper Rage",
    id: 2113,
    fixture: "fixtures/forever-gaps/feral-bear-druid-thorns.request.json",
};
const BALANCE: Case = Case {
    set: "Grovekeeper Eclipse",
    id: 2114,
    fixture: "fixtures/forever-gaps/balance-druid-thorns.request.json",
};
const FERAL: Case = Case {
    set: "Grovekeeper Ferocity",
    id: 2115,
    fixture: "fixtures/mage/prepared-v2/production-feral-druid.request.json",
};

fn request(case: &Case) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(case.fixture);
    let mut request: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    rogue_113483::forever_request(&mut request);
    warrior_trees_20261104::forever_request(&mut request);
    shaman_layout::forever_request(&mut request);
    let player = &mut request["raid"]["parties"][0]["players"][0];
    match case.id {
        2113 => player["talentsString"] = json!("-5500033212310212551-055"),
        2115 => player["talentsString"] = json!(FERAL_TALENTS),
        _ => {}
    }
    request["simOptions"]["iterations"] = json!(5);
    request
}

/// The set's bonuses reached by this many pieces, counted beside the equipment, which holds none
/// of them.
fn with_pieces(case: &Case, pieces: usize) -> Result<Value, String> {
    EXTRA_PIECES.with(|extra| {
        *extra.borrow_mut() = (0..pieces)
            .map(|_| (case.id, case.set.to_string(), 0))
            .collect();
    });
    let result =
        crate::prepare_json_with(request(case).to_string().as_bytes(), "rust", Rules::Forever)
            .map_err(|err| err.to_string());
    EXTRA_PIECES.with(|extra| extra.borrow_mut().clear());
    result
}

fn spell<'a>(prepared: &'a Value, class_spell: &str) -> &'a Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["class_spell"] == class_spell)
        .unwrap_or_else(|| panic!("no spell named {class_spell}"))
}

/// A spell's cooldown in seconds with four pieces and with five.
fn cooldowns(case: &Case, class_spell: &str) -> (f64, f64) {
    let read = |pieces| {
        let prepared = with_pieces(case, pieces).unwrap_or_else(|err| panic!("{err}"));
        spell(&prepared, class_spell)["cd"]["duration_ns"]
            .as_f64()
            .unwrap()
            / 1e9
    };
    (read(4), read(5))
}

#[test]
fn the_rogues_five_piece_takes_five_energy_off_eviscerate() {
    let cost = |pieces| {
        let prepared = with_pieces(&ROGUE, pieces).unwrap();
        let spell = spell(&prepared, "eviscerate");
        (
            spell["cost"]["flat_modifier"].as_i64().unwrap(),
            spell["default_cast"]["cost"].as_f64().unwrap(),
        )
    };
    let (before, after) = (cost(4), cost(5));
    assert_eq!(after.0, before.0 - 5, "{before:?} then {after:?}");
}

#[test]
fn the_hunters_five_piece_takes_a_second_off_aimed_shot_and_multi_shot() {
    for class_spell in ["aimed_shot", "multi_shot"] {
        let (before, after) = cooldowns(&HUNTER, class_spell);
        assert_eq!(before, 6.0, "{class_spell}");
        assert_eq!(after, 5.0, "{class_spell}");
    }
}

#[test]
fn the_warriors_five_pieces_take_thirty_seconds_off_recklessness_and_shield_wall() {
    // Shield Wall's own cooldown is the row's less the talents' mods, so only the difference is
    // the bonus's.
    for (case, class_spell) in [(&ARMS, "recklessness"), (&PROTECTION, "shield_wall")] {
        let (before, after) = cooldowns(case, class_spell);
        assert!(before > 30.0, "{class_spell} {before}");
        assert_eq!(after, before - 30.0, "{class_spell}");
    }
}

#[test]
fn the_shamans_five_pieces_shorten_stormstrike_and_lava_burst() {
    assert_eq!(cooldowns(&ENHANCEMENT, "stormstrike_cast"), (8.0, 7.5));
    // The notes of 2 November 2026 remove Lava Burst under Forever's rules, where no spell is
    // registered for the bonus to shorten: the bonus is its status aura alone. Should the spell
    // return, its mask is the bonus's.
    let prepared = with_pieces(&ELEMENTAL, 5).unwrap();
    assert!(prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .all(|spell| spell["class_spell"] != "lava_burst"));
}

#[test]
fn the_ferals_five_piece_takes_a_second_off_shifting_power() {
    // Hotfix push 113531 halves the cooldown to 8 seconds under Forever's rules.
    assert_eq!(cooldowns(&FERAL, "shifting_power"), (8.0, 7.0));
}

#[test]
fn the_balance_five_piece_lengthens_insect_swarm_by_three_seconds() {
    let flat = |pieces| {
        let prepared = with_pieces(&BALANCE, pieces).unwrap();
        spell(&prepared, "insect_swarm")["dot"]["base_duration_flat_ns"]
            .as_f64()
            .unwrap()
            / 1e9
    };
    assert_eq!(flat(5) - flat(4), 3.0);
}

/// The bonuses that stay unprepared refuse the request by name, and nothing else of their set does.
#[test]
fn an_unprepared_five_piece_bonus_refuses_by_name() {
    for case in [&MAGE, &WARLOCK, &SHADOW, &GUARDIAN] {
        with_pieces(case, 4).unwrap_or_else(|err| panic!("{} with four pieces: {err}", case.set));
        let reason = with_pieces(case, 5).expect_err(case.set);
        assert!(
            reason.contains(&format!("set bonus {} 5P is not prepared yet", case.set)),
            "{reason}"
        );
    }
}

/// The five piece bonuses that are applied are reached: each set registers its status aura.
#[test]
fn the_prepared_five_piece_bonuses_register_their_aura() {
    for case in [
        &ROGUE,
        &HUNTER,
        &ARMS,
        &PROTECTION,
        &ENHANCEMENT,
        &ELEMENTAL,
        &BALANCE,
        &FERAL,
    ] {
        let prepared = with_pieces(case, 5).unwrap_or_else(|err| panic!("{}: {err}", case.set));
        let label = format!("{} 5P", case.set);
        assert!(
            prepared["player"]["auras"]
                .as_array()
                .unwrap()
                .iter()
                .any(|aura| aura["label"] == label),
            "{label}"
        );
    }
}

/// Every client set the classes contribute names all four of its bonuses: each applied, or listed
/// as not prepared.
#[test]
fn every_class_bonus_is_applied_or_named_as_not_prepared() {
    let sets = crate::classes::client_item_sets();
    assert_eq!(sets.len(), 12);
    for client in sets {
        let mut counts: Vec<i32> = client.set.bonuses.iter().map(|(n, _)| *n).collect();
        counts.extend(client.not_prepared);
        counts.sort_unstable();
        assert_eq!(counts, [2, 3, 4, 5], "{}", client.set.name);
    }
}
