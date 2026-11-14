//! Blizzard's WoW Forever hotfixes of 11 November 2026, the two Paladin lines that change a
//! fight, under Forever's rules only:
//!
//! - "Judgement changed to now be considered a Ranged Attack. This means it no longer counts as
//!   a Melee attack for the purposes of item and effect procs and uses the Paladin's Ranged Hit
//!   chance." Every seal's judgement but Command's is a ranged special: melee procs no longer
//!   hear it. Judgement of Command went back to a melee attack two days later.
//! - Retribution: "Unsealed Fury no longer generates echoes from Windfury Totem attacks." The
//!   talent is Unsealed Wrath (1310735) in the client; the totem's extra swing neither stores
//!   the active seal in its Echo nor spends the others.

use forever_engine::{check_prepared, prepare_json_with, simulate_prepared, Rules};
use serde_json::Value;
use std::{fs, path::Path};

/// One rank of each seal's judgement the reference also has, but Command's: Righteousness, the
/// Crusader, Wisdom, Light and Justice. Judgement of Command is a melee attack again since the
/// hotfixes of 13 November 2026 (`tests/hotfix_20261113.rs`).
const JUDGEMENTS: [i64; 5] = [20286, 20303, 20355, 20346, 20184];

/// A production twisting rotation with Unsealed Wrath and Windfury Totem (5 November 2026).
const ECHOES_FIXTURE: &str =
    "fixtures/paladin/retribution-paladin-unsealed-wrath-command-echoes.request.json";

/// The Echoes of Unsealed Wrath (1311701 to 1311705).
const ECHO_PREFIX: &str = "{SpellID: 13117";

fn fixture(path: &str) -> Vec<u8> {
    fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
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

/// Judgements are ranged specials under Forever's rules and keep Go's main hand special in the
/// reference; no effect that hears melee attacks lists one. Each side prepares a Retribution
/// request in its own talent tree.
#[test]
fn judgement_is_a_ranged_attack() {
    let forever = prepared(&fixture(ECHOES_FIXTURE), Rules::Forever);
    let reference = prepared(
        &fixture("fixtures/forever-gaps/retribution-paladin-seal-of-wisdom.request.json"),
        Rules::GoReference,
    );
    for id in JUDGEMENTS {
        assert_eq!(proc_mask(&forever, id), ["ProcMaskRangedSpecial"], "{id}");
        assert_eq!(
            proc_mask(&reference, id),
            ["ProcMaskMeleeMHSpecial"],
            "{id}"
        );
    }

    let judgements: Vec<u64> = spells(&forever)
        .iter()
        .enumerate()
        .filter(|(_, spell)| {
            spell["action_id"]["spell_id"]
                .as_i64()
                .is_some_and(|id| JUDGEMENTS.contains(&id))
        })
        .map(|(position, _)| position as u64)
        .collect();
    for effect in forever["effects"].as_array().unwrap() {
        for key in ["trigger_spells", "spend_spells"] {
            if let Some(listed) = effect.get(key).and_then(Value::as_array) {
                assert!(
                    listed
                        .iter()
                        .all(|spell| !judgements.contains(&spell.as_u64().unwrap_or(u64::MAX))),
                    "{} lists a judgement in {key}",
                    effect["kind"]
                );
            }
        }
    }
}

/// For each landed main hand swing in the debug log, whether an Echo aura changed in the same
/// instant before anything else was cast or dealt; `(swings, swings that touched an Echo)` by
/// the swing's tag: 1 the auto attack, 25584 the Windfury Totem's extra attack.
fn echo_touches(log: &str, tag: &str) -> (usize, usize) {
    let lines: Vec<&str> = log.lines().collect();
    let swing = format!("{{OtherID: 3, Tag: {tag}}} ");
    let landed = |line: &str| {
        ["Hit for", "Crit for", "Glance for", "Block for"]
            .iter()
            .any(|outcome| line.contains(outcome))
    };
    let (mut swings, mut touched) = (0, 0);
    for (index, line) in lines.iter().enumerate() {
        if !(line.contains(&swing) && landed(line)) {
            continue;
        }
        swings += 1;
        let at = line.split(']').next().unwrap();
        let touches = lines[index + 1..]
            .iter()
            .take_while(|next| {
                next.split(']').next().unwrap() == at
                    && !next.contains("Casting")
                    && !next.contains(" for ")
            })
            .any(|next| next.contains("Aura") && next.contains(ECHO_PREFIX));
        touched += usize::from(touches);
    }
    (swings, touched)
}

/// A production twisting rotation with Windfury Totem: every landed auto attack stores or
/// spends Echoes, and no Windfury Totem swing does.
#[test]
fn windfury_totem_swings_leave_the_echoes_alone() {
    let request = fixture(ECHOES_FIXTURE);
    let prepared: forever_engine::contracts::prepared_v2::PreparedV2 =
        serde_json::from_value(prepared(&request, Rules::Forever)).unwrap();
    check_prepared(&prepared).unwrap();
    let result = simulate_prepared(&prepared).unwrap().result;
    let log = result["logs"].as_str().unwrap();

    let (autos, autos_touched) = echo_touches(log, "1");
    assert!(autos > 10, "{autos} auto attacks");
    assert_eq!(autos_touched, autos, "every landed auto stores or spends");

    let (windfury, windfury_touched) = echo_touches(log, "25584");
    assert!(windfury > 0, "no Windfury Totem swing in the log");
    assert_eq!(windfury_touched, 0, "{windfury_touched} of {windfury}");
}
