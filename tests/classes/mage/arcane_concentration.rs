//! Arcane Concentration under Forever's rules: an Arcane Missiles channel rolls once as it starts,
//! and a Flamestrike ground tick rolls once, on the first enemy it lands on. Forever combat logs:
//! Clearcasting came from 8.9% of 347 channel starts (45.3% of 5-missile channels gained it,
//! against 46.9% expected with the start roll and 41.0% without), and from 13 of 82 Flamestrike
//! ticks.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

const CLEARCASTING: &str = "SpellID: 12536}";

/// The Arcane board's request with one action in its rotation, against `targets` copies of
/// its target for ten minutes.
fn request(spell_id: u32, targets: usize, seed: u64) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/dev-notes-20261102/arcane-mage.request.json");
    let mut request: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    request["raid"]["parties"][0]["players"][0]["rotation"] = json!({
        "type": "TypeAPL",
        "priorityList": [{"action": {"castSpell": {"spellId": {"spellId": spell_id}}}}],
    });
    let target = request["encounter"]["targets"][0].clone();
    request["encounter"]["targets"] = Value::Array(vec![target; targets]);
    request["encounter"]["duration"] = 600.into();
    request["encounter"]["durationVariation"] = 0.into();
    request["simOptions"]["randomSeed"] = seed.to_string().into();
    request["simOptions"]["debugFirstIteration"] = true.into();
    request["simOptions"]["iterations"] = 1.into();
    request
}

/// The talent's proc chance and the first fight's log lines, without the damage breakdowns.
fn run(request: &Value) -> (f64, Vec<String>) {
    let prepared = prepare_json_with(request.to_string().as_bytes(), "rust", Rules::Forever)
        .expect("the request prepares");
    let chance = prepared["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|effect| effect["kind"] == "arcane_concentration")
        .expect("the build has Arcane Concentration")["proc_chance"]
        .as_f64()
        .unwrap();
    let typed: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&typed).unwrap();
    let lines = report.result["logs"]
        .as_str()
        .unwrap()
        .lines()
        .filter(|line| !line.contains("[DEBUG]"))
        .map(str::to_string)
        .collect();
    (chance, lines)
}

fn time(line: &str) -> &str {
    &line[..line.find(']').unwrap()]
}

fn clearcasting_proc(line: &str) -> bool {
    line.contains(CLEARCASTING) && (line.contains("Aura gained") || line.contains("Aura refreshed"))
}

/// Each Arcane Missiles channel (rank 7, 10212) rolls as it starts, between its cast and its
/// channel aura on the target, before any missile flies. Nine seeded ten-minute fights of nothing
/// but Arcane Missiles, about a thousand channels: Clearcasting comes from the starts at about
/// the talent's chance (a little under, as a missile's proc just before holds the start roll
/// under the 1 sec cooldown), and the channel that procs it does not spend it.
#[test]
fn arcane_missiles_channel_start_rolls_for_clearcasting() {
    const CAST: &str = "Completed cast {SpellID: 10212}";
    const CHANNEL: &str = "[Target 1] Aura gained: {SpellID: 10212}";
    let (mut channels, mut procs, mut chance) = (0, 0, 0.0);
    for seed in 1..=9u64 {
        let (proc_chance, lines) = run(&request(25345, 1, seed));
        chance = proc_chance;
        for (i, line) in lines.iter().enumerate() {
            if !line.contains(CAST) {
                continue;
            }
            channels += 1;
            let Some(gain) = lines[i + 1..]
                .iter()
                .take_while(|next| !next.contains(CHANNEL))
                .position(|next| clearcasting_proc(next))
            else {
                continue;
            };
            procs += 1;
            // Spent before the roll, so the proc stays up through the cast's own moment, short
            // of the fight's end, where every aura fades.
            let at = time(line);
            if at == time(lines.last().unwrap()) {
                continue;
            }
            let spent = lines[i + 2 + gain..]
                .iter()
                .take_while(|next| time(next) == at)
                .any(|next| next.contains(CLEARCASTING) && next.contains("Aura faded"));
            assert!(
                !spent,
                "seed {seed}: the channel at {at}] spent its own proc"
            );
        }
    }
    assert!(channels > 900, "only {channels} channels");
    let rate = f64::from(procs) / f64::from(channels);
    assert!(
        rate > chance / 2.0 && rate < chance * 1.5,
        "Clearcasting from {procs} of {channels} channel starts ({rate:.3}), want about {chance}"
    );
}

/// A Flamestrike (rank 6, 10216) ground tick on three enemies rolls once, after the first enemy
/// it lands on. Five seeded ten-minute fights: Clearcasting comes from about the talent's chance
/// of the ticks, well under the 27% that a roll on each enemy would give.
#[test]
fn flamestrike_ground_tick_rolls_once_for_clearcasting() {
    const TICK: &str = "{SpellID: 10216} tick";
    let (mut ticks, mut procs, mut chance) = (0, 0, 0.0);
    for seed in 1..=5u64 {
        let (proc_chance, lines) = run(&request(10216, 3, seed));
        chance = proc_chance;
        // The tick's landed lines so far, counted from its first enemy's line.
        let mut landed = 0;
        for pair in lines.windows(2) {
            if !pair[0].contains(TICK) {
                continue;
            }
            if pair[0].contains(&format!("[Target 1] {TICK}")) {
                ticks += 1;
                landed = 0;
            }
            if !pair[0].contains("Miss") {
                landed += 1;
            }
            if clearcasting_proc(&pair[1]) {
                // The roll follows the first landed enemy's line, never a later one.
                assert_eq!(landed, 1, "seed {seed}: {} after {}", pair[1], pair[0]);
                procs += 1;
            }
        }
    }
    assert!(ticks > 500, "only {ticks} ticks");
    let rate = procs as f64 / ticks as f64;
    assert!(
        rate > chance / 2.0 && rate < chance * 1.5,
        "Clearcasting from {procs} of {ticks} ticks on three enemies ({rate:.3}), want about {chance}"
    );
}
