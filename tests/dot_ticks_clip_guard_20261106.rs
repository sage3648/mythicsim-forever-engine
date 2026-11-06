//! A dot's tick count and the channel clip guard, under Forever's rules.
//!
//! `Channeled Ticks` reads the ticks a channel has dealt since it was applied, not a total
//! rebuilt from the live tick length: Missile Barrage halves the Arcane Missiles tick length and
//! its aura drops when the cast completes, after the channel has taken the shorter ticks, which
//! read each missile as its number plus the ticks the mod had taken off, and an interrupt at 3
//! fired after the first missile. This one is a runtime bug, so the Go reference rules carry it
//! too and no accepted fixture moves.
//!
//! The channel clip guard (`channel_clip_walks_rotation`, Forever's rules only) decides whether
//! to clip a channel with an `interruptIf` by reading the rotation as it would run with the
//! channel ended. The reference counts a group reference in the way as another action ready,
//! whatever the group holds, and reads the channel's own dot aura as up, so a Wrack channelled
//! from a Cooldowns group, or gated on its own dot being down, was clipped at its second tick and
//! recast every two seconds for as long as the group held.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const ARCANE_BOARD: &str = "fixtures/dev-notes-20261102/arcane-mage.request.json";
const AFFLICTION: &str =
    "fixtures/mage/prepared-v2/production-affliction-warlock-wrack.request.json";

const ARCANE_BLAST: i64 = 1239700;
const ARCANE_MISSILES: i64 = 25345;
/// The channel's cast, which the log names by its rank's own id, and a missile's tick.
const MISSILES_CAST_LOG: &str = "Casting {SpellID: 10212}";
const MISSILE_TICK_LOG: &str = "Casting {SpellID: 10274}";
const MISSILE_BARRAGE: i64 = 44404;
const WRACK: i64 = 1316697;
const SHADOW_BOLT: i64 = 25307;

fn request(path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn set_rotation(request: &mut Value, rotation: Value) {
    request["raid"]["parties"][0]["players"][0]["rotation"] = rotation;
}

/// The first iteration's log of a fight of `seconds` with this rotation, from a prepared input
/// that `edit` may change before it runs.
fn log(mut request: Value, seconds: i64, seed: u64, edit: impl FnOnce(&mut Value)) -> String {
    request["encounter"]["duration"] = json!(seconds);
    request["encounter"]["durationVariation"] = json!(0);
    request["simOptions"]["iterations"] = json!(1);
    request["simOptions"]["debugFirstIteration"] = json!(true);
    request["simOptions"]["randomSeed"] = json!(seed.to_string());
    let mut prepared = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .unwrap_or_else(|err| panic!("{err:?}"));
    edit(&mut prepared);
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    report.result["logs"].as_str().unwrap().to_string()
}

fn time(line: &str) -> f64 {
    line[1..line.find(']').unwrap()].parse().unwrap()
}

fn reference_guard(prepared: &mut Value) {
    prepared["player"]
        .as_object_mut()
        .unwrap()
        .remove("channel_clip_walks_rotation");
}

// Channeled Ticks

/// An Arcane Mage that casts Arcane Blast until Missile Barrage is up and then Arcane Missiles,
/// ending the channel at its third tick.
fn arcane_request(threshold: i64) -> Value {
    let mut request = request(ARCANE_BOARD);
    let not_barrage =
        json!({"not": {"val": {"auraIsActive": {"auraId": {"spellId": MISSILE_BARRAGE}}}}});
    let ticks = json!({"cmp": {"op": "OpGe",
        "lhs": {"spellChanneledTicks": {"spellId": {"spellId": ARCANE_MISSILES}}},
        "rhs": {"const": {"val": threshold.to_string()}}}});
    set_rotation(
        &mut request,
        json!({"type": "TypeAPL", "prepullActions": [], "priorityList": [
            {"action": {"castSpell": {"spellId": {"spellId": ARCANE_BLAST}}, "condition": not_barrage}},
            {"action": {"channelSpell": {"spellId": {"spellId": ARCANE_MISSILES},
                "allowRecast": true, "interruptIf": ticks}}},
        ]}),
    );
    request
}

/// Each Arcane Missiles channel of the log: whether Missile Barrage paid for it and the missiles
/// it fired before the next cast.
fn missile_channels(log: &str) -> Vec<(bool, usize)> {
    let barrage_line = format!("SpellID: {MISSILE_BARRAGE}");
    let mut barrage = false;
    let mut channels: Vec<(bool, usize)> = Vec::new();
    for line in log.lines() {
        let about_barrage = line.contains(&barrage_line);
        if about_barrage && (line.contains("Aura gained") || line.contains("Aura refreshed")) {
            barrage = true;
        }
        if line.contains(MISSILES_CAST_LOG) {
            channels.push((barrage, 0));
        } else if line.contains(MISSILE_TICK_LOG) {
            if let Some(last) = channels.last_mut() {
                last.1 += 1;
            }
        }
        if about_barrage && line.contains("Aura faded") {
            barrage = false;
        }
    }
    channels
}

/// Missile Barrage's missiles tick every half second, and the interrupt at 3 waits for the third
/// of them. Before, the count read 6 at the first missile and the channel ended there.
#[test]
fn missile_barrage_missiles_count_from_the_ticks_the_channel_started_with() {
    let mut paid = 0;
    for seed in 1..=8 {
        let channels = missile_channels(&log(arcane_request(3), 60, seed, |_| {}));
        for (barrage, missiles) in channels {
            assert_eq!(missiles, 3, "seed {seed}, Missile Barrage {barrage}");
            paid += usize::from(barrage);
        }
    }
    assert!(
        paid > 10,
        "only {paid} channels were paid by Missile Barrage"
    );
}

/// The count is not a Forever rule: the reference's guard, which prepared inputs without the
/// field ask for, counts the same ticks.
#[test]
fn the_reference_guard_counts_the_same_ticks() {
    let channels = missile_channels(&log(arcane_request(3), 60, 1, reference_guard));
    assert!(channels.iter().any(|(barrage, _)| *barrage));
    assert!(channels.iter().all(|(_, missiles)| *missiles == 3));
}

// The channel clip guard

/// An Affliction Warlock whose priority list is `rows` and whose groups are `groups`, with
/// Wrack's channel interrupted at its `ticks`th tick unless the rotation would only recast it.
fn wrack_request(rows: Vec<Value>, groups: Value) -> Value {
    let mut request = request(AFFLICTION);
    let mut rotation = json!({"type": "TypeAPL", "prepullActions": [], "priorityList": rows});
    if !groups.is_null() {
        rotation["groups"] = groups;
    }
    set_rotation(&mut request, rotation);
    request
}

fn wrack_channel(ticks: i64, condition: Option<Value>) -> Value {
    let interrupt = json!({"cmp": {"op": "OpGe",
        "lhs": {"spellChanneledTicks": {"spellId": {"spellId": WRACK}}},
        "rhs": {"const": {"val": ticks.to_string()}}}});
    let mut action =
        json!({"channelSpell": {"spellId": {"spellId": WRACK}, "interruptIf": interrupt}});
    if let Some(condition) = condition {
        action["condition"] = condition;
    }
    json!({"action": action})
}

fn filler() -> Value {
    json!({"action": {"castSpell": {"spellId": {"spellId": SHADOW_BOLT}}}})
}

fn after(seconds: &str) -> Value {
    json!({"cmp": {"op": "OpGe", "lhs": {"currentTime": {}}, "rhs": {"const": {"val": seconds}}}})
}

/// How long each Wrack channel kept its dot aura on the target, in seconds, for the channels
/// that ended before the fight did.
fn wrack_channels(log: &str, fight: f64) -> Vec<f64> {
    let (mut started, mut lengths) = (None, Vec::new());
    for line in log
        .lines()
        .filter(|line| line.contains(&format!("SpellID: {WRACK}")))
    {
        if line.contains("Aura gained") {
            started = Some(time(line));
        } else if line.contains("Aura faded") {
            if let Some(start) = started.take() {
                if time(line) < fight - 0.5 {
                    lengths.push(time(line) - start);
                }
            }
        }
    }
    lengths
}

const FIGHT: i64 = 40;

fn channels(request: &Value, seed: u64, edit: impl FnOnce(&mut Value)) -> Vec<f64> {
    wrack_channels(&log(request.clone(), FIGHT, seed, edit), FIGHT as f64)
}

/// A channel the rotation would only recast runs to its end; a full Wrack channel is six ticks.
fn assert_held(channels: &[f64]) {
    assert!(channels.len() >= 4, "{channels:?}");
    assert!(
        channels.iter().all(|length| *length > 5.5),
        "a channel was clipped: {channels:?}"
    );
}

/// The reference's clip: every channel but the ones ending the fight is cut at its second tick.
fn assert_clipped_every_second_tick(channels: &[f64]) {
    assert!(channels.len() >= 10, "{channels:?}");
    assert!(
        channels.iter().all(|length| *length < 2.5),
        "a channel ran on: {channels:?}"
    );
}

/// Wrack channelled from a Cooldowns group is recast from the group, so the guard holds it. The
/// reference counted the group reference as another action ready and clipped Wrack at its second
/// tick for the rest of the fight.
#[test]
fn a_wrack_channelled_from_a_group_is_not_clipped_to_recast() {
    let request = wrack_request(
        vec![
            json!({"action": {"groupReference": {"groupName": "Cooldowns"}}}),
            filler(),
        ],
        json!([{"name": "Cooldowns", "actions": [wrack_channel(2, None)]}]),
    );
    for seed in [1, 2] {
        assert_held(&channels(&request, seed, |_| {}));
        assert_clipped_every_second_tick(&channels(&request, seed, reference_guard));
    }
}

/// A group whose first ready action is another spell is another action: between one and two
/// seconds the group's Shadow Bolt comes before its Wrack, so the channel is clipped for it at
/// its second tick, whatever the guard. The channels that start later, outside the window, are
/// held.
#[test]
fn a_group_whose_first_ready_action_is_another_spell_still_clips() {
    let window = json!({"and": {"vals": [
        after("1s"),
        {"not": {"val": after("2.2s")}},
    ]}});
    let request = wrack_request(
        vec![
            json!({"action": {"groupReference": {"groupName": "Cooldowns"}}}),
            filler(),
        ],
        json!([{"name": "Cooldowns", "actions": [
            {"action": {"castSpell": {"spellId": {"spellId": SHADOW_BOLT}}, "condition": window}},
            wrack_channel(2, None),
        ]}]),
    );
    let clipped = |length: &f64| (1.9..2.0).contains(length);
    let guarded = (1..=4)
        .flat_map(|seed| channels(&request, seed, |_| {}))
        .collect::<Vec<_>>();
    // Two ticks of 0.98 s for the channel Shadow Bolt cut short, and six for the rest.
    assert!(guarded.iter().any(clipped), "{guarded:?}");
    assert!(guarded.iter().any(|length| *length > 5.5), "{guarded:?}");
    // The reference clips every channel, held or not.
    let reference = channels(&request, 1, reference_guard);
    assert!(reference.iter().all(clipped), "{reference:?}");
}

/// A Wrack line gated on its own dot being down reads the dot as down while the guard looks, so
/// the line is ready and only recasts. The reference read the aura as up, found nothing to cast
/// and clipped the channel at its second tick.
#[test]
fn a_channel_gated_on_its_own_dot_is_not_clipped_to_recast() {
    let down = json!({"not": {"val": {"dotIsActive": {"spellId": {"spellId": WRACK}}}}});
    let request = wrack_request(vec![wrack_channel(2, Some(down)), filler()], Value::Null);
    for seed in [1, 2] {
        assert_held(&channels(&request, seed, |_| {}));
        assert_clipped_every_second_tick(&channels(&request, seed, reference_guard));
    }
}

/// The same line gated on the dot's aura rather than the dot.
#[test]
fn a_channel_gated_on_its_own_aura_is_not_clipped_to_recast() {
    let down = json!({"not": {"val": {"auraIsActive": {
        "auraId": {"spellId": WRACK}, "sourceUnit": {"type": "CurrentTarget"}}}}});
    let request = wrack_request(vec![wrack_channel(2, Some(down)), filler()], Value::Null);
    assert_held(&channels(&request, 1, |_| {}));
    assert_clipped_every_second_tick(&channels(&request, 1, reference_guard));
}

/// A sequence that is running is its current step: Shadow Bolt ahead of the channel is cast
/// first, so the channel is clipped at the third tick, when the global cooldown is over. The
/// reference passed the sequence by and held the channel.
#[test]
fn a_running_sequence_step_ahead_of_the_channel_clips_it() {
    let sequence = json!({"action": {
        "condition": after("0.5s"),
        "sequence": {"name": "bolt", "actions": [{"castSpell": {"spellId": {"spellId": SHADOW_BOLT}}}]}}});
    let request = wrack_request(
        vec![sequence, wrack_channel(3, None), filler()],
        Value::Null,
    );
    let guarded = (1..=4)
        .flat_map(|seed| channels(&request, seed, |_| {}))
        .collect::<Vec<_>>();
    let reference = (1..=4)
        .flat_map(|seed| channels(&request, seed, reference_guard))
        .collect::<Vec<_>>();
    // Three ticks of 0.98 s.
    let third_tick = |length: &f64| (2.8..3.1).contains(length);
    assert!(guarded.iter().any(third_tick), "{guarded:?}");
    assert!(!reference.iter().any(third_tick), "{reference:?}");
}

/// Only Forever's preparation writes the rule.
#[test]
fn the_clip_guard_rule_is_written_by_forever_preparation_only() {
    let request = wrack_request(vec![wrack_channel(2, None), filler()], Value::Null);
    let forever =
        prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever).unwrap();
    assert_eq!(forever["player"]["channel_clip_walks_rotation"], true);
    // The reference keeps the Go rotation, which this fixture's request fits.
    let reference = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::GoReference);
    if let Ok(reference) = reference {
        assert!(reference["player"]
            .get("channel_clip_walks_rotation")
            .is_none());
    }
}
