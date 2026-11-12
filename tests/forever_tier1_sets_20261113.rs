//! The Tier 1 class sets of the Forever client (ItemSets 2098 to 2115), whose bonuses are the
//! ItemSetSpell spells of client 1.60.1.70425 (`src/prepare/client_set_effects.rs` and each
//! class's items module). The reference registers none of the damage and tank sets, and four
//! crafted pieces of each are in `data/items.jsonl`, so a request reaches the 2, 3 and 4 piece
//! bonuses here. The 5 piece bonuses, which no request can reach (an item goes to the slot its
//! type names), are checked in `src/prepare/client_set_effects.rs` with the pieces a test adds.
//!
//! A bonus is read as the change it makes beyond the pieces' own stats: what the nth piece adds
//! to a stat with the n-1 before it, less what that piece adds alone.

use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared,
    PrepareError, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// Where the four pieces go, in the order `Set::pieces` lists them: legs, hands, shoulder, feet.
const SLOTS: [usize; 4] = [8, 6, 2, 9];

struct Set {
    id: i64,
    name: &'static str,
    fixture: &'static str,
    pieces: [i64; 4],
    /// The creature type of the 4 piece bonus.
    versus: &'static str,
    /// Whether the 4 piece bonus is spell damage, and not melee attack power.
    spells: bool,
}

const MAGE: Set = Set {
    id: 2098,
    name: "Manaflare Regalia",
    fixture: "fixtures/dev-notes-20261102/arcane-mage.request.json",
    pieces: [273904, 273905, 273906, 273907],
    versus: "MobTypeElemental",
    spells: true,
};
const ROGUE: Set = Set {
    id: 2099,
    name: "Grimstitch Armor",
    fixture: "fixtures/forever-gaps/combat-rogue-darkmantle-riposte.request.json",
    pieces: [273928, 273929, 273930, 273931],
    versus: "MobTypeHumanoid",
    spells: false,
};
const WARLOCK: Set = Set {
    id: 2100,
    name: "Demonheart Raiment",
    fixture: "fixtures/forever-gaps/demonology-warlock-mob-type-bonus.request.json",
    pieces: [273940, 273941, 273942, 273943],
    versus: "MobTypeDemon",
    spells: true,
};
const HUNTER: Set = Set {
    id: 2101,
    name: "Wildstalker Armor",
    fixture: "fixtures/forever-gaps/marksmanship-hunter-sequence-conditions.request.json",
    pieces: [273900, 273901, 273902, 273903],
    versus: "MobTypeBeast",
    spells: false,
};
const ARMS: Set = Set {
    id: 2102,
    name: "Battlegear of Glory",
    fixture: "fixtures/forever-gaps/arms-warrior-revenge.request.json",
    pieces: [273944, 273945, 273946, 273947],
    versus: "MobTypeHumanoid",
    spells: false,
};
const PROTECTION: Set = Set {
    id: 2103,
    name: "Battleplate of Glory",
    fixture: "fixtures/forever-gaps/protection-warrior-shield-bash.request.json",
    pieces: [273948, 273949, 273950, 273951],
    versus: "",
    spells: false,
};
const SHADOW: Set = Set {
    id: 2105,
    name: "Raiments of Conviction",
    fixture: "fixtures/forever-gaps/shadow-priest-chastise.request.json",
    pieces: [273920, 273921, 273922, 273923],
    versus: "MobTypeUndead",
    spells: true,
};
const ENHANCEMENT: Set = Set {
    id: 2110,
    name: "The Spiritcaller's Rage",
    fixture: "fixtures/forever-gaps/enhancement-shaman-beaststalker-armor.request.json",
    pieces: [273932, 273933, 273934, 273935],
    versus: "MobTypeElemental",
    spells: false,
};
const ELEMENTAL: Set = Set {
    id: 2111,
    name: "The Spiritcaller's Storm",
    fixture: "fixtures/forever-gaps/elemental-shaman-the-elements.request.json",
    pieces: [276538, 276539, 276540, 276541],
    versus: "MobTypeElemental",
    spells: true,
};
const GUARDIAN: Set = Set {
    id: 2113,
    name: "Grovekeeper Rage",
    fixture: "fixtures/forever-gaps/feral-bear-druid-thorns.request.json",
    pieces: [273896, 273897, 273898, 273899],
    versus: "",
    spells: false,
};
const BALANCE: Set = Set {
    id: 2114,
    name: "Grovekeeper Eclipse",
    fixture: "fixtures/forever-gaps/balance-druid-thorns.request.json",
    pieces: [273884, 273885, 273886, 273887],
    versus: "MobTypeDemon",
    spells: true,
};
const FERAL: Set = Set {
    id: 2115,
    name: "Grovekeeper Ferocity",
    fixture: "fixtures/forever-gaps/feral-druid-defias-leather.request.json",
    pieces: [273888, 273889, 273890, 273891],
    versus: "MobTypeDemon",
    spells: false,
};

const ALL: [&Set; 12] = [
    &MAGE,
    &ROGUE,
    &WARLOCK,
    &HUNTER,
    &ARMS,
    &PROTECTION,
    &SHADOW,
    &ENHANCEMENT,
    &ELEMENTAL,
    &GUARDIAN,
    &BALANCE,
    &FERAL,
];

#[path = "support/rogue_113483.rs"]
mod rogue_113483;
#[path = "support/shaman_layout.rs"]
mod shaman_layout;
#[path = "support/warrior_trees_20261104.rs"]
mod warrior_trees_20261104;

fn read(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(name);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// The set's fixture request wearing the pieces at these positions of `Set::pieces`, and nothing in
/// the other three of the four slots.
fn wearing(set: &Set, worn: &[usize]) -> Value {
    let mut request = read(set.fixture);
    // The recorded talents in the order Forever's rules read (as `tests/forever_gaps.rs` does).
    rogue_113483::forever_request(&mut request);
    warrior_trees_20261104::forever_request(&mut request);
    shaman_layout::forever_request(&mut request);
    if set.id == GUARDIAN.id && set.fixture == GUARDIAN.fixture {
        request["raid"]["parties"][0]["players"][0]["talentsString"] =
            json!("-5500033212310212551-055");
    }
    request["simOptions"]["iterations"] = json!(20);
    let equipment = request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    equipment.resize(17, json!({}));
    for (position, slot) in SLOTS.iter().enumerate() {
        equipment[*slot] = if worn.contains(&position) {
            json!({ "id": set.pieces[position] })
        } else {
            json!({})
        };
    }
    request
}

fn first(set: &Set, pieces: usize) -> Value {
    wearing(set, &(0..pieces).collect::<Vec<_>>())
}

fn prepare(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err}"))
}

fn prepared(request: &Value) -> PreparedV2 {
    let prepared: PreparedV2 = serde_json::from_value(prepare(request)).unwrap();
    check_prepared(&prepared).unwrap_or_else(|err| panic!("{err:?}"));
    prepared
}

/// The average DPS over many iterations: a bonus of 36 attack power is a small share of a rogue's
/// or a warrior's damage, and the rage or energy it brings changes their rotation.
fn dps(request: &Value) -> f64 {
    let mut request = request.clone();
    request["simOptions"]["iterations"] = json!(1000);
    let request = &request;
    let result = simulate_prepared(&prepared(request)).unwrap().result;
    result["raidMetrics"]["parties"][0]["players"][0]["dps"]["avg"]
        .as_f64()
        .unwrap()
}

fn with_target_type(mut request: Value, mob_type: &str) -> Value {
    for target in request["encounter"]["targets"].as_array_mut().unwrap() {
        target["mobType"] = json!(mob_type);
    }
    request
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap()
}

/// What the nth piece adds to a value beyond the piece itself, which is its own contribution
/// alone: `read` is taken of the prepared input.
fn bonus(set: &Set, n: usize, read: impl Fn(&Value) -> f64) -> f64 {
    let with = |worn: &[usize]| read(&prepare(&wearing(set, worn)));
    let before: Vec<usize> = (0..n - 1).collect();
    let all: Vec<usize> = (0..n).collect();
    (with(&all) - with(&before)) - (with(&[n - 1]) - with(&[]))
}

/// [`bonus`] of a multiplier: the factor the nth piece multiplies the value by beyond its own.
fn factor(set: &Set, n: usize, read: impl Fn(&Value) -> f64) -> f64 {
    let with = |worn: &[usize]| read(&prepare(&wearing(set, worn)));
    let before: Vec<usize> = (0..n - 1).collect();
    let all: Vec<usize> = (0..n).collect();
    (with(&all) / with(&before)) / (with(&[n - 1]) / with(&[]))
}

fn near(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-6
}

fn effects(prepared: &PreparedV2, kind: &str, aura: &str) -> Vec<Value> {
    serde_json::to_value(prepared).unwrap()["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == kind && effect["aura"] == aura)
        .cloned()
        .collect()
}

fn spell<'a>(prepared: &'a Value, class_spell: &str) -> &'a Value {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spell| spell["class_spell"] == class_spell)
        .unwrap_or_else(|| panic!("no spell named {class_spell}"))
}

fn cooldown(prepared: &Value, class_spell: &str) -> f64 {
    spell(prepared, class_spell)["cd"]["duration_ns"]
        .as_f64()
        .unwrap()
        / 1e9
}

fn has_aura(prepared: &Value, label: &str) -> bool {
    prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .any(|aura| aura["label"] == label)
}

/// The pieces are what the data says: each ID is a piece of the set, and the set's rows in the
/// client carry the bonuses the engine names.
#[test]
fn the_pieces_are_the_sets_in_the_item_data() {
    let items =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/items.jsonl")).unwrap();
    for set in ALL {
        for piece in set.pieces {
            let row = items
                .lines()
                .map(|line| serde_json::from_str::<Value>(line).unwrap())
                .find(|row| row["id"] == piece)
                .unwrap_or_else(|| panic!("no item {piece}"));
            assert_eq!(row["set_id"], set.id, "{piece}");
            assert_eq!(row["set_name"], set.name, "{piece}");
        }
    }
}

/// Each set prepares, passes the gate and runs with all four pieces, and each bonus is a status
/// aura from the piece that reaches it. One piece is a plain item. The 5 piece bonus is never
/// reached here.
#[test]
fn four_pieces_prepare_run_and_name_their_bonuses() {
    for set in ALL {
        let one = prepare(&first(set, 1));
        assert!(!has_aura(&one, &format!("{} 2P", set.name)), "{}", set.name);
        for n in 2..=4 {
            let prepared = prepare(&first(set, n));
            for reached in 2..=4 {
                assert_eq!(
                    has_aura(&prepared, &format!("{} {reached}P", set.name)),
                    reached <= n,
                    "{} with {n} pieces, bonus {reached}",
                    set.name
                );
            }
            assert!(!has_aura(&prepared, &format!("{} 5P", set.name)));
        }
        let request = first(set, 4);
        simulate_prepared(&prepared(&request)).unwrap();
    }
}

/// The reference registers none of these sets, so under its rules the pieces count for nothing:
/// the Mage of the reference's own requests wears four and no bonus aura is registered.
#[test]
fn the_reference_rules_count_the_pieces_for_nothing() {
    let mut request = read("tests/classes/mage/prepare/arcane.request.json");
    let equipment = request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    equipment.resize(17, json!({}));
    for (slot, id) in SLOTS.iter().zip(MAGE.pieces) {
        equipment[*slot] = json!({ "id": id });
    }
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::GoReference)
        .unwrap_or_else(|err| panic!("{err}"));
    assert!(!has_aura(&prepared, "Manaflare Regalia 2P"));
}

/// (2) "Improves your chance to hit by 1.0%": melee and spell hit, whatever the pieces' own.
#[test]
fn the_hit_sets_add_one_percent_melee_and_spell_hit() {
    for set in [&MAGE, &ROGUE, &WARLOCK, &SHADOW, &ELEMENTAL, &BALANCE] {
        for name in ["PhysicalHitPercent", "SpellHitPercent"] {
            let added = bonus(set, 2, |prepared| stat(prepared, name));
            assert!(near(added, 1.0), "{} {name} {added}", set.name);
        }
        // And the 3 and 4 piece bonuses add none.
        for n in [3, 4] {
            let added = bonus(set, n, |prepared| stat(prepared, "SpellHitPercent"));
            assert!(near(added, 0.0), "{} {n} pieces {added}", set.name);
        }
    }
}

/// (2) "Increases your attack speed and casting speed by 1%": the melee and ranged speed
/// multiplier and the cast speed.
#[test]
fn the_haste_sets_add_one_percent_attack_and_cast_speed() {
    for set in [&HUNTER, &ARMS, &ENHANCEMENT, &FERAL] {
        let attack = factor(set, 2, |prepared| {
            prepared["melee"]["attack_speed_multiplier"]
                .as_f64()
                .unwrap()
        });
        let cast = factor(set, 2, |prepared| {
            prepared["player"]["cast_speed"].as_f64().unwrap()
        });
        assert!(near(attack, 1.01), "{} attack speed {attack}", set.name);
        // The exported cast speed is the multiplier of a cast's time.
        assert!(near(cast, 1.0 / 1.01), "{} cast speed {cast}", set.name);
    }
}

/// The Hunter's attack speed multiplier is the ranged one as well: the fight reads the ranged
/// haste as that multiplier times the ranged speed multiplier and the haste rating, so with the
/// 2 piece bonus Auto Shot swings faster.
#[test]
fn the_hunters_haste_reaches_auto_shot() {
    let swings = |request: &Value| -> f64 {
        let result = simulate_prepared(&prepared(request)).unwrap().result;
        let actions = result["raidMetrics"]["parties"][0]["players"][0]["actions"]
            .as_array()
            .unwrap();
        actions
            .iter()
            .filter(|action| action["id"]["otherId"] == "OtherActionShoot")
            .map(|action| action["targets"][0]["casts"].as_f64().unwrap_or(0.0))
            .sum()
    };
    // The same pieces without the bonus: the first piece and the second, against the second
    // alone with the first's slot left for the other two, so the stats differ only by the bonus
    // and the pieces' own, which carry no haste.
    let two = swings(&first(&HUNTER, 2));
    let one = swings(&first(&HUNTER, 1));
    assert!(
        two > one,
        "Auto Shot swings {two} with the bonus, {one} without"
    );
    assert!(
        (two / one - 1.01).abs() < 0.01,
        "Auto Shot swings {two} against {one}"
    );
}

/// (2) and (4) of the tank sets: "Increased Defense +7" and 1.2% less dodged or parried, which is
/// 12 expertise rating.
#[test]
fn the_tank_sets_add_defense_and_expertise() {
    for set in [&PROTECTION, &GUARDIAN] {
        let defense = bonus(set, 2, |prepared| stat(prepared, "DefenseRating"));
        assert!(near(defense, 7.0), "{} defense {defense}", set.name);
        let expertise = bonus(set, 4, |prepared| stat(prepared, "ExpertiseRating"));
        assert!(near(expertise, 12.0), "{} expertise {expertise}", set.name);
        let none = bonus(set, 3, |prepared| stat(prepared, "DefenseRating"));
        assert!(near(none, 0.0), "{} {none}", set.name);
    }
}

/// (3) Kidney Shot, Freezing Trap and Intimidating Shout are spells the engine registers, so the
/// 3 piece bonus moves their cooldown; the others name a spell it does not.
#[test]
fn the_three_piece_bonus_moves_the_cooldown_of_a_spell_the_engine_has() {
    for (set, class_spell, before, after) in [
        (&ROGUE, "kidney_shot", 20.0, 17.0),
        (&HUNTER, "freezing_trap", 30.0, 27.0),
        (&ARMS, "intimidating_shout", 180.0, 150.0),
    ] {
        let two = prepare(&first(set, 2));
        let three = prepare(&first(set, 3));
        assert_eq!(cooldown(&two, class_spell), before, "{}", set.name);
        assert_eq!(cooldown(&three, class_spell), after, "{}", set.name);
    }
}

/// The 3 piece bonuses of Counterspell, Banish, Intervene, Shackle Undead, Earthbind Totem,
/// Rebirth, Nature's Grasp and Hibernate, and of the Air and Earth totems' radius, name nothing
/// this engine registers, so they are a status aura alone: no spell of theirs is in the spellbook.
#[test]
fn the_other_three_piece_bonuses_name_no_spell_the_engine_has() {
    for (set, spells) in [
        (&MAGE, vec![2139]),
        (&WARLOCK, vec![710, 18647]),
        (&PROTECTION, vec![]),
        (&SHADOW, vec![9484, 9485, 10955]),
        (&ELEMENTAL, vec![2484]),
        (&GUARDIAN, vec![20484, 20739, 20742, 20747]),
        (&BALANCE, vec![16689, 16810, 16811, 16812]),
        (&FERAL, vec![2637, 18657, 18658]),
    ] {
        let prepared = prepare(&first(set, 3));
        let ids: Vec<i64> = prepared["player"]["spells"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|spell| spell["action_id"]["spell_id"].as_i64())
            .collect();
        for spell in spells {
            assert!(!ids.contains(&spell), "{} registers {spell}", set.name);
        }
        assert!(has_aura(&prepared, &format!("{} 3P", set.name)));
    }
}

/// (4) Attack power or spell damage against a creature type holds when every target is of it, and
/// is inert against any other: prepared and run, never refused.
#[test]
fn the_four_piece_bonus_holds_against_its_creature_type_only() {
    for set in ALL {
        if set.versus.is_empty() {
            continue;
        }
        let aura = format!("{} 4P", set.name);
        let against = with_target_type(first(set, 4), set.versus);
        let matched = effects(&prepared(&against), "mob_type_bonus", &aura);
        assert_eq!(matched.len(), 1, "{} {matched:?}", set.name);
        let (spell_damage, power) = if set.spells { (21.0, 0.0) } else { (0.0, 36.0) };
        assert_eq!(matched[0]["spell_damage"], spell_damage, "{}", set.name);
        assert_eq!(
            matched[0]["melee_attack_power"].as_f64().unwrap_or(0.0),
            power,
            "{}",
            set.name
        );
        assert_eq!(matched[0]["ranged_attack_power"], 0.0, "{}", set.name);

        for other in [
            with_target_type(first(set, 4), "MobTypeDragonkin"),
            first(set, 4),
        ] {
            assert!(
                effects(&prepared(&other), "mob_type_bonus", &aura).is_empty(),
                "{}",
                set.name
            );
            simulate_prepared(&prepared(&other)).unwrap();
        }
        // Three pieces never reach it.
        let three = with_target_type(first(set, 3), set.versus);
        assert!(effects(&prepared(&three), "mob_type_bonus", &aura).is_empty());

        let hit = dps(&against);
        // The control has no creature type, since a fixture's own gear may carry a bonus against
        // another one (the Arms Warrior's against Dragonkin).
        let miss = dps(&first(set, 4));
        if set.id == HUNTER.id {
            // A Marksmanship Hunter's attacks are ranged, which melee attack power never reaches.
            assert!(hit >= miss, "{}: {hit} against {miss}", set.name);
        } else {
            assert!(hit > miss, "{}: {hit} against {miss}", set.name);
        }
    }
}

/// With four targets the bonus holds when all of them are of the type. The prepared input carries
/// only identical copies of its first target, so a mix of types cannot be asked for.
#[test]
fn the_four_piece_bonus_holds_against_every_target_of_the_type() {
    let mut set = ROGUE;
    set.fixture = "fixtures/forever-gaps/assassination-rogue-change-target-rupture.request.json";
    let aura = format!("{} 4P", set.name);
    let request = with_target_type(first(&set, 4), "MobTypeHumanoid");
    assert_eq!(request["encounter"]["targets"].as_array().unwrap().len(), 4);
    assert_eq!(
        effects(&prepared(&request), "mob_type_bonus", &aura).len(),
        1
    );
    let other = with_target_type(first(&set, 4), "MobTypeBeast");
    assert!(effects(&prepared(&other), "mob_type_bonus", &aura).is_empty());
    simulate_prepared(&prepared(&request)).unwrap();
}

fn refusal(request: &Value, rules: Rules) -> String {
    match prepare_json_with(request.to_string().as_bytes(), "rust", rules) {
        Err(PrepareError::Refused(refusal)) => refusal.to_string(),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A healer wearing a set a client set owns gets each bonus once: the generic reading of a set's
/// bonus rows for a healer (`healer_gear`) leaves it to the client set. The rows of these sets
/// change no healer stat there (the healer path reads neither hit, defense nor expertise, and the
/// spell mods name spells a healer does not have), but it attached a gain and expiry callback to
/// the bonus aura, which is what the client set's own aura lacks.
#[test]
fn a_healer_wearing_a_client_set_gets_its_bonus_once() {
    let healer = Set {
        fixture: "fixtures/healers/holy-priest.request.json",
        ..GUARDIAN
    };
    let expertise = bonus(&healer, 4, |prepared| stat(prepared, "ExpertiseRating"));
    assert!(near(expertise, 12.0), "the fourth piece adds {expertise}");
    let defense = bonus(&healer, 2, |prepared| stat(prepared, "DefenseRating"));
    assert!(
        near(defense, 7.0),
        "the second piece adds {defense} Defense"
    );
    let prepared = prepare(&first(&healer, 4));
    let aura = prepared["player"]["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == "Grovekeeper Rage 3P")
        .unwrap();
    assert_eq!(aura["callbacks"], json!(["on_reset"]));
}

/// The reference's rules keep the Justice Battlegear 4 piece unrepresented, as it was: a refusal.
#[test]
fn the_reference_rules_still_refuse_the_paladins_creature_type_bonus() {
    let mut request =
        read("fixtures/forever-gaps/retribution-paladin-seal-of-justice.request.json");
    let equipment = request["raid"]["parties"][0]["players"][0]["equipment"]["items"]
        .as_array_mut()
        .unwrap();
    equipment.resize(17, json!({}));
    for (slot, id) in [(2, 273910), (6, 273909), (8, 273908), (9, 273911)] {
        equipment[slot] = json!({ "id": id });
    }
    let reason = refusal(&request, Rules::GoReference);
    assert!(reason.contains("Justice Battlegear 4P"), "{reason}");
}
