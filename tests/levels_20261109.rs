//! Levelling characters, second pass (docs/levels.md, 9 November 2026).
//!
//! A sweep of every reference build at random levels from 10 to 59, solo and in Droptimizer
//! sets, found spells that kept their level 60 rank below 60: a rank the character has not
//! learned is never cast, and a proc or pulse read off the top rank hits for the top rank's
//! damage. The fixtures are MythicSim's levelling requests for reference builds, written by the
//! app's `TestDumpLevelledRequests` with a character sheet consistent with the gear.
//!
//! The last test runs a build of every class across levels 10 to 58 and checks what any
//! levelling sim must hold: it prepares and passes the gate, does positive damage or healing,
//! and casts or procs no ranked spell above the character's level.

use forever_engine::contracts::prepared_v2::PreparedV2;
use forever_engine::{prepare_json_with, prepared_refusals, simulate_prepared, Rules};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::{fs, path::Path};

fn request(name: &str) -> Value {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("fixtures/levels/{name}.request.json"));
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn player(request: &mut Value) -> &mut Value {
    &mut request["raid"]["parties"][0]["players"][0]
}

fn prepare(request: &Value) -> Value {
    prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("prepares: {err}"))
}

/// The fight's result, after the gate has passed the prepared request.
fn simulate(prepared: &Value) -> Value {
    let typed: PreparedV2 = serde_json::from_value(prepared.clone()).unwrap();
    let refusals = prepared_refusals(&typed);
    assert!(refusals.is_empty(), "gate refusals: {refusals:?}");
    simulate_prepared(&typed).unwrap().result
}

fn player_metrics(result: &Value) -> &Value {
    &result["raidMetrics"]["parties"][0]["players"][0]
}

/// What a unit's action with this spell id did over the run: casts and damage.
fn action(unit: &Value, spell_id: i64) -> (f64, f64) {
    let mut total = (0.0, 0.0);
    for action in unit["actions"].as_array().into_iter().flatten() {
        if action["id"]["spellId"].as_i64() != Some(spell_id) {
            continue;
        }
        for target in action["targets"].as_array().into_iter().flatten() {
            total.0 += target["casts"].as_f64().unwrap_or(0.0);
            total.1 += target["damage"].as_f64().unwrap_or(0.0);
        }
    }
    total
}

/// The player's spells by spell id, with whether the rotation can press each.
fn spells(prepared: &Value) -> HashMap<i64, bool> {
    prepared["player"]["spells"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|spell| {
            let id = spell["action_id"]["spell_id"].as_i64()?;
            let pressable = spell["flags"]
                .as_array()
                .unwrap()
                .iter()
                .any(|flag| flag == "SpellFlagAPL");
            Some((id, pressable))
        })
        .collect()
}

/// The prepared effects of one kind.
fn effects<'a>(prepared: &'a Value, kind: &str) -> Vec<&'a Value> {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == kind)
        .collect()
}

/// Flametongue Weapon's proc is the rank the shaman imbues: a level 22 shaman without a
/// spellbook knows rank 2 (8027, level 18) and hits with its proc (8028); one whose spellbook
/// holds only rank 1 (8024) hits with rank 1's (8026). Both hit with rank 6's 2498 (16344, spell
/// level 56) before, a third of a level 22 Enhancement Shaman's damage (MythicSim Quick Sims, 9
/// November 2026).
#[test]
fn a_flametongue_weapon_hits_with_the_rank_it_imbues() {
    let by_level = request("enhancement-shaman-L22-flametongue");
    let mut rank_one = by_level.clone();
    player(&mut rank_one)["knownSpells"] = json!([8017, 8024, 8042, 8071, 8075]);
    for (request, proc) in [(by_level, 8028), (rank_one, 8026)] {
        let prepared = prepare(&request);
        let registered = spells(&prepared);
        assert!(registered.contains_key(&proc), "{registered:?}");
        assert!(
            !registered.contains_key(&16344),
            "rank 6's proc is registered"
        );
        let result = simulate(&prepared);
        let player = player_metrics(&result);
        let (_, damage) = action(player, proc);
        assert!(damage > 0.0, "proc {proc} dealt no damage");
        assert_eq!(action(player, 16344), (0.0, 0.0));
    }
}

/// Below level 40 a druid shifts into Bear Form (5487, level 10), with 180% more armor from
/// items and Bear Form (Passive)'s health (1178: 20 and 18 a level from 10, 236 at 22), not the
/// level 40 Dire Bear Form (9634) it has not learned. At 46 it is Dire Bear Form, with 9635's
/// 600 and 32 a level from 40.
#[test]
fn a_druid_below_40_shifts_into_bear_form() {
    for (name, form, health) in [
        ("feral-bear-druid-L22", 5487, 236.0),
        ("feral-bear-druid-L46", 9634, 792.0),
    ] {
        let prepared = prepare(&request(name));
        let bear = effects(&prepared, "bear_form");
        assert_eq!(bear.len(), 1, "{name}");
        assert_eq!(bear[0]["spell_id"], form, "{name}");
        assert_eq!(bear[0]["health_bonus"].as_f64(), Some(health), "{name}");
        assert_eq!(
            spells(&prepared).get(&i64::from(form)),
            Some(&true),
            "{name}"
        );
    }
}

/// A level 46 druid's Faerie Fire debuff is the rank it casts (9749, rank 3, level 42). A
/// creature spell shares the name (13752, "Faerie Fire" rank 2, level 45), and the debuff
/// resolved to it from level 45, so the rotation's check for 9749 never saw it up: the bear
/// pressed Faerie Fire 81 times a fight and never Lacerate.
#[test]
fn a_level_46_druid_keeps_its_own_faerie_fire_up() {
    let mut request = request("feral-bear-druid-L46");
    request["simOptions"]["iterations"] = 20.into();
    let result = simulate(&prepare(&request));
    let player = player_metrics(&result);
    let iterations = 20.0;
    let (faerie_fire, _) = action(player, 9749);
    assert!(
        faerie_fire / iterations < 10.0,
        "Faerie Fire pressed {} times a fight",
        faerie_fire / iterations
    );
    let (lacerate, _) = action(player, 414644);
    assert!(lacerate > 0.0, "Lacerate never pressed");
}

/// A cat's finishers and Rake are the ranks its level teaches: a level 30 cat knows Rip rank 2
/// (9492, level 28) and Rake rank 1 (1822, level 24). Read off the top rank, Rip (rank 6, level
/// 60) was unlearned at every level below 60 and Rake (rank 4) below 54, so a levelling cat
/// shredded and never bled.
#[test]
fn a_levelling_cat_rakes_and_rips() {
    let prepared = prepare(&request("feral-druid-L30"));
    let registered = spells(&prepared);
    for (rank, top) in [(9492, 9896), (1822, 9904)] {
        assert_eq!(registered.get(&rank), Some(&true), "{rank}: {registered:?}");
        assert!(!registered.contains_key(&top), "{top} is registered");
    }
    let result = simulate(&prepare(&request("feral-druid-L30")));
    for rank in [9492, 1822] {
        assert!(
            action(player_metrics(&result), rank).1 > 0.0,
            "{rank} dealt nothing"
        );
    }
}

/// Overpower and Slice and Dice are the ranks the level teaches: a level 30 Warrior's Overpower
/// rank 2 (7887, level 28), a level 30 Rogue's Slice and Dice rank 1 (5171). Each was registered
/// at its top rank (11585, level 60; 6774, level 42) and never pressed below it.
#[test]
fn overpower_and_slice_and_dice_follow_the_level() {
    for (name, rank, top) in [
        ("arms-warrior-L30", 7887, 11585),
        ("combat-rogue-L30", 5171, 6774),
    ] {
        let registered = spells(&prepare(&request(name)));
        assert_eq!(registered.get(&rank), Some(&true), "{name}: {registered:?}");
        assert!(
            !registered.contains_key(&top),
            "{name}: {top} is registered"
        );
    }
}

/// A levelling Rogue's poisons are the ranks it brews: at 30, Instant Poison II (8685, level 28,
/// 20 to 26 a hit) and Deadly Poison (2818, level 30, 6 a tick). Below 20 it brews none:
/// the request's Instant Poison VI made a third of a level 10 Rogue's damage.
#[test]
fn a_levelling_rogue_brews_the_poisons_its_level_teaches() {
    let prepared = prepare(&request("combat-rogue-L30"));
    let instant = effects(&prepared, "instant_poison");
    assert_eq!(instant[0]["spell_id"], 8685);
    let (min, max) = (
        instant[0]["min_damage"].as_f64().unwrap(),
        instant[0]["max_damage"].as_f64().unwrap(),
    );
    assert!(min < 23.0 && max > 23.0 && max < 30.0, "{min} to {max}");
    let deadly = effects(&prepared, "deadly_poison");
    assert_eq!(deadly[0]["spell_id"], 2818);
    assert_eq!(deadly[0]["tick_damage"], 6.0);

    let mut level_15 = request("combat-rogue-L30");
    let talents = level_15["raid"]["parties"][0]["players"][0]["talentsString"]
        .as_str()
        .unwrap()
        .to_string();
    player(&mut level_15)["level"] = 15.into();
    player(&mut level_15)["talentsString"] = trim_talents(&talents, 6).into();
    let prepared = prepare(&level_15);
    for kind in ["instant_poison", "deadly_poison"] {
        assert!(
            effects(&prepared, kind).is_empty(),
            "a level 15 Rogue has {kind}"
        );
    }
}

/// A level 42 Restoration Shaman's Mana Tide Totem is rank 2's 133 every 3 sec (17355), 221.67
/// MP5, and the check that its MP5 comes off exactly reads that rank too: it read rank 4's
/// 483.33 and refused the sim (stat_auras).
#[test]
fn a_level_42_shaman_drops_its_own_mana_tide() {
    let result = simulate(&prepare(&request("restoration-shaman-L42")));
    let hps = player_metrics(&result)["hps"]["avg"].as_f64().unwrap();
    assert!(hps > 0.0);
}

/// A levelling Hunter's Wind Serpent breathes the Lightning Breath its level teaches: rank 2
/// (25008, level 12) at 20, not rank 6's 86 to 98 (25012).
#[test]
fn a_levelling_wind_serpent_breathes_its_rank() {
    let mut request = request("hunter-L20");
    player(&mut request)["hunter"]["options"]["classOptions"]["petType"] = "WindSerpent".into();
    let prepared = prepare(&request);
    let text = prepared.to_string();
    assert!(text.contains("\"spell_id\":25008"), "no rank 2 breath");
    assert!(!text.contains("\"spell_id\":25012"), "rank 6 breath");
}

/// Every row's level and rank, from the client rows the engine reads.
fn rows() -> &'static HashMap<i64, (String, i64)> {
    static ROWS: OnceLock<HashMap<i64, (String, i64)>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/spells.jsonl");
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .map(|row| {
                (
                    row["ID"].as_i64().unwrap(),
                    (
                        row["Rank"].as_str().unwrap_or_default().to_string(),
                        row["SpellLevel"].as_i64().unwrap_or(0),
                    ),
                )
            })
            .collect()
    })
}

/// The ranked spells a unit cast or dealt damage or healing with above `level`.
fn ranks_above(unit: &Value, level: i64) -> Vec<i64> {
    let mut above = Vec::new();
    for action in unit["actions"].as_array().into_iter().flatten() {
        let Some(id) = action["id"]["spellId"].as_i64() else {
            continue;
        };
        let used = action["targets"].as_array().into_iter().flatten().any(|t| {
            ["casts", "damage", "healing", "shielding"]
                .iter()
                .any(|field| t[*field].as_f64().unwrap_or(0.0) > 0.0)
        });
        if let Some((rank, spell_level)) = rows().get(&id) {
            if used && rank.starts_with("Rank ") && *spell_level > level {
                above.push(id);
            }
        }
    }
    above
}

/// A talent string with at most `points` spent: the smallest tree first, its deepest talent
/// first, as the app's `trimTalents` levels a reference build.
fn trim_talents(talents: &str, points: u32) -> String {
    let mut trees: Vec<Vec<u32>> = talents
        .split('-')
        .map(|tree| tree.chars().map(|c| c.to_digit(10).unwrap()).collect())
        .collect();
    let spent = |trees: &Vec<Vec<u32>>| trees.iter().flatten().sum::<u32>();
    while spent(&trees) > points {
        let smallest = (0..trees.len())
            .filter(|&i| trees[i].iter().sum::<u32>() > 0)
            .min_by_key(|&i| trees[i].iter().sum::<u32>())
            .unwrap();
        let deepest = trees[smallest].iter().rposition(|&d| d > 0).unwrap();
        trees[smallest][deepest] -= 1;
    }
    trees
        .iter()
        .map(|tree| {
            let text: String = tree
                .iter()
                .map(|d| char::from_digit(*d, 10).unwrap())
                .collect();
            text.trim_end_matches('0').to_string()
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Every class across levels 10 to 58, tanks and healers among them: each prepares and passes the
/// gate, does positive damage (healing for a healer, threat for a tank) and casts or procs no
/// ranked spell above its level, its pets' included. The sheet stays the fixture's, which the
/// calibration solves at each level.
#[test]
fn every_class_sims_across_the_levels() {
    let builds = [
        ("arms-warrior-L30", "dps"),
        ("protection-warrior-L30", "tank"),
        ("retribution-paladin-L30", "dps"),
        ("protection-paladin-L27", "tank"),
        ("holy-paladin-L30", "healer"),
        ("hunter-L20", "dps"),
        ("combat-rogue-L30", "dps"),
        ("shadow-priest-L30", "dps"),
        ("holy-priest-L30", "healer"),
        ("enhancement-shaman-L22-flametongue", "dps"),
        ("elemental-shaman-L30", "dps"),
        ("restoration-shaman-L42", "healer"),
        ("frost-mage-L20", "dps"),
        ("affliction-warlock-L30", "dps"),
        ("feral-druid-L30", "dps"),
        ("feral-bear-druid-L46", "tank"),
        ("balance-druid-L30", "dps"),
        ("restoration-druid-L30", "healer"),
    ];
    let mut failures = Vec::new();
    for (name, role) in builds {
        let base = request(name);
        let talents = base["raid"]["parties"][0]["players"][0]["talentsString"]
            .as_str()
            .unwrap()
            .to_string();
        for level in [10, 24, 38, 52, 58] {
            let mut request = base.clone();
            request["simOptions"]["iterations"] = 10.into();
            player(&mut request)["level"] = level.into();
            // A spellbook is the fixture's level's; without one the level decides.
            player(&mut request)
                .as_object_mut()
                .unwrap()
                .remove("knownSpells");
            // A dungeon boss two levels up, as the app sims a levelling character.
            for target in request["encounter"]["targets"].as_array_mut().unwrap() {
                target["level"] = (level + 2).into();
            }
            player(&mut request)["talentsString"] = trim_talents(&talents, level as u32 - 9).into();
            let prepared =
                match prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever) {
                    Ok(prepared) => prepared,
                    Err(err) => {
                        failures.push(format!("{name} at {level}: {err}"));
                        continue;
                    }
                };
            let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
            let refusals = prepared_refusals(&typed);
            if !refusals.is_empty() {
                failures.push(format!("{name} at {level}: gate {refusals:?}"));
                continue;
            }
            let result = simulate_prepared(&typed).unwrap().result;
            let player = player_metrics(&result);
            let metric = match role {
                "healer" => "hps",
                "tank" => "threat",
                _ => "dps",
            };
            let value = player[metric]["avg"].as_f64().unwrap_or(0.0);
            if !(value.is_finite() && value > 0.0) {
                failures.push(format!("{name} at {level}: {metric} {value}"));
            }
            if role == "tank" && player["dtps"]["avg"].as_f64().unwrap_or(0.0) <= 0.0 {
                failures.push(format!("{name} at {level}: takes no damage"));
            }
            let mut above = ranks_above(player, level);
            for pet in player["pets"].as_array().into_iter().flatten() {
                above.extend(ranks_above(pet, level));
            }
            if !above.is_empty() {
                failures.push(format!(
                    "{name} at {level}: ranks above the level {above:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn trimming_talents_keeps_the_main_tree() {
    assert_eq!(trim_talents("0502052-5-053502030101301251", 11), "--0533");
    assert_eq!(trim_talents("--053", 20), "--053");
}
