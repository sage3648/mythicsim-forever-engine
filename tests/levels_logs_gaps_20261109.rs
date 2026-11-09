//! The three gaps `docs/levels-logs-2026-11-09.md` left open between the public foreverlogs.gg
//! reports and the levelling engine, settled with the auras the logs record (second pass of
//! 9 November 2026, section "The open gaps, settled"). None of them is an engine error:
//!
//! - Rend: the logged Protection Warriors tick in Defensive Stance, whose 10% damage reduction
//!   reaches the bleed, under Battle Shout or Blessing of Might. With both counted the engine's
//!   0.02 of attack power a tick reproduces the logs.
//! - Hunter shots: Hunter's Mark and Aspect of the Hawk are up for most shots. Counted at the
//!   ranks the character knows, the engine's base and ranged attack power shares reproduce them.
//! - Affliction Warlocks: the engine's item rows and Improved Corruption reproduce them. The
//!   earlier gap came from the armory's item summary (Staff of Soran'ruk's 9 Shadow damage where
//!   the row and the hits say 34) and from random suffix items, whose suffix the log's capture
//!   does not carry.
//!
//! Each fixture under `fixtures/levels/logs` is a logged character at its level, in its logged
//! gear, with the logged sheet's attributes and no talents.

use forever_engine::{prepare_json_with, Rules};
use serde_json::Value;
use std::{fs, path::Path};

fn prepared(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("fixtures/levels/logs/{name}.request.json"));
    let request = fs::read(path).unwrap();
    prepare_json_with(&request, "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{name}: {err}"))
}

fn stat(prepared: &Value, name: &str) -> f64 {
    prepared["player"]["stats"][name].as_f64().unwrap_or(0.0)
}

/// The prepared effect of this kind.
fn effect<'a>(prepared: &'a Value, kind: &str) -> &'a Value {
    prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} effect"))
}

/// An exclusive aura's value, which its category priority carries (a shout's or a mark's attack
/// power at the rank the character knows).
fn aura_value(auras: &Value, label: &str) -> f64 {
    let aura = auras
        .as_array()
        .unwrap()
        .iter()
        .find(|aura| aura["label"] == label)
        .unwrap_or_else(|| panic!("no aura {label}"));
    aura["exclusive_memberships"][0]["priority"]
        .as_f64()
        .unwrap()
}

/// The multiplier a stance aura puts on one pseudo stat.
fn stance_multiplier(prepared: &Value, aura: &str, stat: &str) -> f64 {
    effect(prepared, "pseudo_stat_auras")["auras"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["aura"] == aura && entry["stat"] == stat)
        .unwrap_or_else(|| panic!("no {stat} on {aura}"))["multiplier"]
        .as_f64()
        .unwrap()
}

/// Ranged attack power with Aspect of the Hawk up and nothing else from the stat auras.
fn ranged_attack_power_with_hawk(prepared: &Value) -> f64 {
    let auras = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| {
            effect["kind"] == "stat_auras"
                && effect["auras"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|aura| aura == "Aspect of the Hawk")
        })
        .expect("Aspect of the Hawk is a stat aura");
    let position = auras["auras"]
        .as_array()
        .unwrap()
        .iter()
        .position(|aura| aura == "Aspect of the Hawk")
        .unwrap();
    auras["combos"][1usize << position]["RangedAttackPower"]
        .as_f64()
        .unwrap()
}

fn within(engine: f64, logged: f64, share: f64, what: &str) {
    assert!(
        (engine / logged - 1.0).abs() <= share,
        "{what}: engine {engine:.2} against the logged {logged}"
    );
}

/// Rend's tick on the engine's numbers: the rank's base plus 0.02 of attack power, times the
/// stance's damage multiplier. `buffs` is the attack power of the logged Battle Shout or Blessing
/// of Might.
fn rend_tick(prepared: &Value, buffs: f64, stance: &str) -> f64 {
    let rend = effect(prepared, "rend");
    let base = rend["tick_base"].as_f64().unwrap();
    let share = rend["attack_power_per_tick"].as_f64().unwrap();
    let multiplier = match stance {
        "Battle Stance" => 1.0,
        stance => stance_multiplier(prepared, stance, "damage_dealt"),
    };
    (base + share * (stat(prepared, "AttackPower") + buffs)) * multiplier
}

/// The logged Protection Warriors' Rend ticks, in Defensive Stance unless named. The ticks are
/// rounded at random in game (within one Rend, 103 of 189 applications of three or more ticks
/// read two adjacent values), so the mean non-crit tick is the unrounded value. Level 18 has no
/// point in Arms and level 23 three; levels 15 and 22 read 4 to 5% under the engine, inside what
/// a sheet read at the boss pull and buffs the log does not show leave open.
#[test]
fn rend_ticks_match_the_logs_in_defensive_stance() {
    // fixture, attack power, Battle Shout's at the level, Rend's base, extra buff attack power,
    // stance, logged mean non-crit tick, ticks, allowed share
    let cases = [
        // Level 15, rank 2 under its own Battle Shout rank 2: 70 ticks.
        (
            "protection-warrior-L15",
            129.0,
            21.0,
            7.0,
            0.0,
            "Defensive Stance",
            9.49,
            0.055,
        ),
        // Level 18, rank 2 under Battle Shout rank 2, sheet strength 68: 115 ticks.
        (
            "protection-warrior-L18",
            173.0,
            22.0,
            7.0,
            0.0,
            "Defensive Stance",
            9.79,
            0.01,
        ),
        // Level 22, rank 3 under a Paladin's Blessing of Might rank 2 (25): 158 ticks.
        (
            "protection-warrior-L22",
            224.0,
            0.0,
            9.0,
            25.0,
            "Defensive Stance",
            12.15,
            0.04,
        ),
        // The same Warrior in Battle Stance: 7 ticks.
        (
            "protection-warrior-L22",
            224.0,
            0.0,
            9.0,
            25.0,
            "Battle Stance",
            13.29,
            0.055,
        ),
        // Level 23, rank 3 with no shout (93 ticks) and under Battle Shout rank 3 (95 ticks).
        (
            "protection-warrior-L23",
            269.0,
            0.0,
            9.0,
            0.0,
            "Defensive Stance",
            13.05,
            0.01,
        ),
        (
            "protection-warrior-L23",
            269.0,
            33.0,
            9.0,
            0.0,
            "Defensive Stance",
            13.51,
            0.01,
        ),
    ];
    for (name, attack_power, shout, base, extra, stance, logged, share) in cases {
        let prepared = prepared(name);
        assert_eq!(stat(&prepared, "AttackPower"), attack_power, "{name}");
        let rend = effect(&prepared, "rend");
        assert_eq!(rend["tick_base"].as_f64().unwrap(), base, "{name}");
        assert_eq!(
            rend["attack_power_per_tick"].as_f64().unwrap(),
            0.02,
            "{name}"
        );
        let own = aura_value(&prepared["player"]["auras"], "Battle Shout (Player)");
        if shout > 0.0 {
            assert_eq!(own, shout, "{name}: Battle Shout at the level");
        }
        let tick = rend_tick(&prepared, shout + extra, stance);
        within(tick, logged, share, &format!("{name} {stance}"));
    }
}

/// Defensive Stance's 10% less damage reaches Rend's ticks: the one logged Warrior who changed
/// stance with Rend up ticked 13.29 in Battle Stance (7 ticks) and 12.15 in Defensive Stance
/// (158 ticks, the same Blessing of Might and no shout), 1.094 times, and its white hits on the
/// same creatures 1.097 times (33 in Battle Stance). Without the stance the logs would ask for
/// 0.0154 of attack power a tick, the share the first pass fitted.
#[test]
fn defensive_stance_reaches_rend() {
    let prepared = prepared("protection-warrior-L22");
    let multiplier = stance_multiplier(&prepared, "Defensive Stance", "damage_dealt");
    assert_eq!(multiplier, 0.9);
    let battle = rend_tick(&prepared, 25.0, "Battle Stance");
    let defensive = rend_tick(&prepared, 25.0, "Defensive Stance");
    assert!(((battle / defensive) - 13.29 / 12.15).abs() < 0.02);
}

/// Hunter shots on the engine's numbers: Arcane Shot's base plus 0.15 of ranged attack power,
/// Serpent Sting's tick plus 0.05, Hunter's Mark and Aspect of the Hawk at the ranks the level
/// knows. Level 17's report logs the aspect; level 22's logs no aspect or stance for anyone, and
/// the Hunter's shots read as Aspect of the Hawk's 35 up throughout.
#[test]
fn hunter_shots_match_the_logs_with_mark_and_aspect() {
    // fixture, sheet ranged attack power, Hawk's ranged attack power, Mark's, Arcane Shot's base,
    // Serpent Sting's tick
    for (name, sheet, hawk, mark, arcane_base, sting_base) in [
        ("marksmanship-hunter-L17", 130.0, 20.0, 26.0, 35.0, 7.0),
        ("marksmanship-hunter-L22", 178.0, 35.0, 59.0, 50.0, 14.0),
    ] {
        let prepared = prepared(name);
        assert_eq!(stat(&prepared, "RangedAttackPower"), sheet, "{name}");
        assert_eq!(
            ranged_attack_power_with_hawk(&prepared),
            sheet + hawk,
            "{name}"
        );
        let marked = aura_value(&prepared["target"]["auras"], "Hunter's Mark (External)");
        assert_eq!(marked, mark, "{name}: Hunter's Mark at the level");
        let arcane = effect(&prepared, "arcane_shot");
        assert_eq!(
            arcane["base_damage"].as_f64().unwrap(),
            arcane_base,
            "{name}"
        );
        assert_eq!(arcane["rap_coefficient"].as_f64().unwrap(), 0.15, "{name}");
        let sting = effect(&prepared, "serpent_sting");
        assert_eq!(sting["tick_base"].as_f64().unwrap(), sting_base, "{name}");
        assert_eq!(
            sting["attack_power_share"].as_f64().unwrap(),
            0.05,
            "{name}"
        );
    }
    let shot = |base: f64, rap: f64| base + 0.15 * rap;
    let tick = |base: f64, rap: f64| base + 0.05 * rap;
    // Level 17 (one report): logged mean non-crit hits by the auras up.
    for (engine, logged, what) in [
        (shot(35.0, 130.0), 54.66, "Arcane Shot, nothing up (32)"),
        (shot(35.0, 150.0), 57.25, "Arcane Shot, Hawk (4)"),
        (shot(35.0, 176.0), 61.40, "Arcane Shot, Hawk and Mark (10)"),
        (tick(7.0, 130.0), 13.50, "Serpent Sting, nothing up (44)"),
        (tick(7.0, 156.0), 14.60, "Serpent Sting, Mark (10)"),
        (tick(7.0, 176.0), 16.00, "Serpent Sting, Hawk and Mark (6)"),
    ] {
        within(engine, logged, 0.02, &format!("level 17 {what}"));
    }
    // Level 22 (another report), Hawk taken as up.
    for (engine, logged, what) in [
        (shot(50.0, 213.0), 83.08, "Arcane Shot (38)"),
        (shot(50.0, 272.0), 91.89, "Arcane Shot, Mark (27)"),
        (tick(14.0, 213.0), 24.71, "Serpent Sting (96)"),
        (tick(14.0, 272.0), 27.81, "Serpent Sting, Mark (69)"),
    ] {
        within(engine, logged, 0.02, &format!("level 22 {what}"));
    }
}

/// Affliction Warlocks on the engine's item rows. Corruption takes Improved Corruption's 10%
/// (5 points, the Affliction tree's first row); Shadow Bolt, Immolate and Drain Life take no
/// talent the two characters can have shown.
#[test]
fn affliction_warlocks_match_the_logs_on_the_item_rows() {
    // Level 24: Staff of Soran'ruk's row gives 34 Shadow and 34 Fire damage (the armory says 9).
    let prepared24 = prepared("affliction-warlock-L24");
    let spell_damage = stat(&prepared24, "SpellDamage");
    assert_eq!(spell_damage, 19.0);
    let shadow = spell_damage + stat(&prepared24, "ShadowDamage");
    let fire = spell_damage + stat(&prepared24, "FireDamage");
    assert_eq!((shadow, fire), (66.0, 53.0));
    for (engine, logged, what) in [
        // Shadow Bolt rank 4 at 24 (61), 17 hits.
        (61.0 + 0.857 * shadow, 119.24, "Shadow Bolt 4"),
        // Corruption rank 3, 322 ticks, with Improved Corruption.
        ((22.0 + 0.2 * shadow) * 1.1, 38.54, "Corruption 3"),
        // Drain Life rank 2, 87 ticks.
        (14.0 + 0.1 * shadow, 20.40, "Drain Life 2"),
        // Immolate rank 3's tick, 66 ticks.
        (12.0 + 0.13 * fire, 18.82, "Immolate 3 tick"),
    ] {
        within(engine, logged, 0.02, &format!("level 24 {what}"));
    }
    // Level 25: 36 spell damage from the items.
    let prepared25 = prepared("affliction-warlock-L25");
    let spell_damage = stat(&prepared25, "SpellDamage");
    assert_eq!(spell_damage, 36.0);
    for (engine, logged, what) in [
        // Shadow Bolt rank 4 at 25 (62), 99 hits.
        (62.0 + 0.857 * spell_damage, 93.18, "Shadow Bolt 4"),
        // Corruption rank 3, 432 ticks, with Improved Corruption.
        ((22.0 + 0.2 * spell_damage) * 1.1, 32.01, "Corruption 3"),
    ] {
        within(engine, logged, 0.02, &format!("level 25 {what}"));
    }
}

/// A level 18 Warlock whose hits read 22 to 29% over its items is short of spell damage, not of
/// a talent: Shadow Bolt rank 3 (61.06 over 77 hits), Immolate rank 2's tick (8.66 over 47) and
/// Corruption rank 2 (19.18 over 539, with Improved Corruption) each ask for 21 to 23 spell
/// damage, where the logged items give 9. Four of them (Shimmering Boots and Cloak, Hook Dagger,
/// Willow Branch) are random suffix items, and the log's capture carries no suffix.
#[test]
fn a_warlock_over_its_items_is_missing_its_suffixes() {
    let prepared = prepared("affliction-warlock-L18");
    assert_eq!(stat(&prepared, "SpellDamage"), 9.0);
    let asked = [
        (61.06 - 43.0) / 0.8,
        (8.66 - 6.0) / 0.13,
        (19.18 / 1.1 - 13.0) / 0.2,
    ];
    for spell_damage in asked {
        assert!(
            (20.0..=24.0).contains(&spell_damage),
            "asks for {spell_damage:.1}"
        );
    }
}
