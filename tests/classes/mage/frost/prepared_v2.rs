use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, prepared_coverage, PreparedError,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{fs, path::Path};

#[derive(Deserialize)]
struct Manifest {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    prepared: String,
    expected_coverage: Coverage,
}

#[derive(Deserialize)]
struct Coverage {
    supported: bool,
    reasons: Vec<String>,
}

fn family() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/mage/frost/prepared-v2")
}

fn manifest() -> Manifest {
    serde_json::from_slice(&fs::read(family().join("manifest.json")).unwrap()).unwrap()
}

fn reference_json() -> Value {
    serde_json::from_slice(&fs::read(family().join("frost-reference.prepared.json")).unwrap())
        .unwrap()
}

type Mutation = (&'static str, fn(&mut Value));

fn parse(value: Value) -> Result<PreparedV2, serde_json::Error> {
    serde_json::from_value(value)
}

fn reasons(value: Value) -> Vec<String> {
    match check_prepared(&parse(value).unwrap()) {
        Err(PreparedError::Unsupported(reasons)) => reasons,
        other => panic!("expected unsupported, got {other:?}"),
    }
}

#[test]
fn accepted_fixtures_report_their_expected_coverage() {
    for case in manifest().cases {
        let bytes = fs::read(family().join(&case.prepared)).unwrap();
        let prepared: PreparedV2 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(prepared.scenario_id, case.id);
        let result = check_prepared(&prepared);
        let reasons = match result {
            Ok(()) => Vec::new(),
            Err(PreparedError::Unsupported(reasons)) => reasons,
            Err(err) => panic!("{} invalid: {err}", case.id),
        };
        assert_eq!(
            reasons.is_empty(),
            case.expected_coverage.supported,
            "{}",
            case.id
        );
        assert_eq!(reasons, case.expected_coverage.reasons, "{}", case.id);
    }
}

#[test]
fn real_reference_needs_only_named_mechanics() {
    // The real request is fully representable: no exporter gaps, unclaimed listeners or
    // rotation operators remain. Only unimplemented effects block it.
    let prepared = parse(reference_json()).unwrap();
    for reason in prepared_coverage(&prepared) {
        assert!(
            reason.starts_with("effect ") && reason.ends_with(" is not implemented"),
            "{reason}"
        );
    }
}

#[test]
fn unknown_fields_and_effect_kinds_fail_deserialization() {
    let mut top = reference_json();
    top["new_field"] = json!(1);
    assert!(parse(top)
        .unwrap_err()
        .to_string()
        .contains("unknown field `new_field`"));

    let mut spell = reference_json();
    spell["player"]["spells"][0]["new_modifier"] = json!(1.5);
    assert!(parse(spell)
        .unwrap_err()
        .to_string()
        .contains("unknown field `new_modifier`"));

    let mut effect = reference_json();
    effect["effects"]
        .as_array_mut()
        .unwrap()
        .push(json!({"kind": "presence_of_mind"}));
    assert!(parse(effect)
        .unwrap_err()
        .to_string()
        .contains("unknown variant `presence_of_mind`"));

    let mut parameter = reference_json();
    for effect in parameter["effects"].as_array_mut().unwrap() {
        if effect["kind"] == "winters_chill" {
            effect["extra_stack_rule"] = json!(true);
        }
    }
    assert!(parse(parameter).is_err());
}

#[test]
fn identity_and_bounds_violations_are_invalid_not_unsupported() {
    let cases: [Mutation; 5] = [
        ("revision", |v| {
            v["reference"]["engine_revision"] = json!("0".repeat(40))
        }),
        ("client", |v| {
            v["reference"]["client_build"] = json!("1.60.0.1")
        }),
        ("schema", |v| v["contract"] = json!("forever-result")),
        ("seed", |v| v["sim"]["seed"] = json!(0)),
        ("regen", |v| {
            v["player"]["mana"]["spirit_regen_per_second"] = json!(40.0)
        }),
    ];
    for (name, mutate) in cases {
        let mut value = reference_json();
        mutate(&mut value);
        let result = check_prepared(&parse(value).unwrap());
        assert!(
            matches!(result, Err(PreparedError::Invalid(_))),
            "{name}: {result:?}"
        );
    }
}

#[test]
fn exporter_gaps_block_simulation() {
    let mut value = reference_json();
    value["unrepresented"] = json!(["target auto attacks are unsupported"]);
    assert!(reasons(value)
        .contains(&"unrepresented by the exporter: target auto attacks are unsupported".into()));
}

#[test]
fn active_listeners_without_an_effect_are_reported() {
    let mut value = reference_json();
    value["effects"]
        .as_array_mut()
        .unwrap()
        .retain(|effect| effect["aura"] != "Parry Haste");
    assert!(reasons(value).contains(
        &"target aura \"Parry Haste\" listens to combat events without an effect".into()
    ));
}

#[test]
fn unsupported_rotation_operators_are_reported() {
    let mut value = reference_json();
    value["player"]["rotation"]["priorityList"][3]["action"]["condition"] =
        json!({"dotIsActive": {"spellId": {"spellId": 12579}}});
    assert!(reasons(value).contains(&"rotation item 4: value dotIsActive is unsupported".into()));

    let mut prepull = reference_json();
    prepull["player"]["rotation"]["prepullActions"] = json!([{"action": {"castSpell": {"spellId": {"spellId": 25304}}}, "doAtValue": {"const": {"val": "-1s"}}}]);
    assert!(reasons(prepull).contains(&"rotation field prepullActions is unsupported".into()));
}

#[test]
fn rotation_spells_without_behavior_are_reported() {
    let mut value = reference_json();
    value["player"]["rotation"]["priorityList"][5]["action"]["castSpell"]["spellId"] =
        json!({"spellId": 10151});
    assert!(
        reasons(value).contains(&"rotation reaches spell 10151 without a known behavior".into())
    );
}

/// Community fix ElliotWood/Forever#622 (252f57aa8), recorded in upstream/changes.json.
/// Without Fingers of Frost, pinned Go drops the Ice Lance condition and casts Ice Lance on
/// every global cooldown; the fix reads the missing aura as inactive. Rust rejects the
/// rotation until the reference adopts the fix.
#[test]
fn community_fix_622_unknown_aura_conditions_are_rejected() {
    let bytes = fs::read(family().join("frost-no-fingers.prepared.json")).unwrap();
    let prepared: PreparedV2 = serde_json::from_slice(&bytes).unwrap();
    assert!(!prepared.player.talents.contains_key("fingers_of_frost"));
    let reasons = prepared_coverage(&prepared);
    assert!(reasons.contains(
        &"rotation item 4: auraIsActive names spell 400669, which the character lacks; \
          the pinned reference drops the condition and community #622 reads it as inactive"
            .to_string()
    ));
    // The talented reference names the same aura and is not affected.
    let reference = parse(reference_json()).unwrap();
    assert!(!prepared_coverage(&reference)
        .iter()
        .any(|reason| reason.contains("#622")));
}

/// Without Missile Barrage the Arcane Missiles rule names a missing aura behind an
/// `auraIsKnown` guard. Both readings prune the rule, so the build is supported; its Go
/// golden casts no Arcane Missiles.
#[test]
fn community_fix_622_guarded_conditions_are_supported() {
    let bytes = fs::read(family().join("reference-no-missile-barrage.prepared.json")).unwrap();
    let prepared: PreparedV2 = serde_json::from_slice(&bytes).unwrap();
    assert!(!prepared.player.talents.contains_key("missile_barrage"));
    assert_eq!(prepared_coverage(&prepared), Vec::<String>::new());
}

/// The comparable view of a Go `RaidSimResult`, matching tools/prepared_v2.py `compact`.
fn compact(result: &Value) -> Value {
    let player = &result["raidMetrics"]["parties"][0]["players"][0];
    let target = &result["encounterMetrics"]["targets"][0];
    let number = |value: &Value| value.as_f64().map_or(json!(0), |_| value.clone());
    let distribution = |value: &Value| {
        json!({
            "avg": number(&value["avg"]), "stdev": number(&value["stdev"]),
            "max": number(&value["max"]), "min": number(&value["min"]),
        })
    };
    let key = |id: &Value| {
        let map: std::collections::BTreeMap<String, Value> =
            serde_json::from_value(id.clone()).unwrap();
        serde_json::to_string(&map)
            .unwrap()
            .replace(':', ": ")
            .replace(',', ", ")
    };
    let exercised = |row: &Value| {
        row["targets"].as_array().unwrap().iter().any(|target| {
            target
                .as_object()
                .unwrap()
                .iter()
                .any(|(k, v)| k != "unitIndex" && v.as_f64().is_some_and(|value| value != 0.0))
        })
    };
    let auras = |unit: &Value| {
        let mut map = serde_json::Map::new();
        for aura in unit["auras"].as_array().into_iter().flatten() {
            if aura["procsAvg"].as_f64().unwrap_or(0.0) > 0.0 {
                map.insert(
                    key(&aura["id"]),
                    json!({
                        "uptimeSecondsAvg": number(&aura["uptimeSecondsAvg"]),
                        "uptimeSecondsStdev": number(&aura["uptimeSecondsStdev"]),
                        "procsAvg": number(&aura["procsAvg"]),
                    }),
                );
            }
        }
        Value::Object(map)
    };
    let mut actions = serde_json::Map::new();
    for action in player["actions"].as_array().into_iter().flatten() {
        if exercised(action) {
            actions.insert(key(&action["id"]), action["targets"].clone());
        }
    }
    let mut resources = serde_json::Map::new();
    for resource in player["resources"].as_array().into_iter().flatten() {
        if resource["events"].as_i64().unwrap_or(0) > 0 {
            resources.insert(
                key(&resource["id"]),
                json!({
                    "events": number(&resource["events"]),
                    "gain": number(&resource["gain"]),
                    "actualGain": number(&resource["actualGain"]),
                }),
            );
        }
    }
    json!({
        "summary": {
            "iterationsDone": number(&result["iterationsDone"]),
            "avgIterationDuration": number(&result["avgIterationDuration"]),
            "firstIterationDuration": number(&result["firstIterationDuration"]),
            "dps": distribution(&player["dps"]),
            "threat": distribution(&player["threat"]),
            "secondsOomAvg": number(&player["secondsOomAvg"]),
        },
        "actions": actions,
        "auras": auras(player),
        "target_auras": auras(target),
        "resources": resources,
    })
}

/// Integers exactly; floats within 1e-9 relative, the FMA and summation-order allowance
/// documented in docs/prepared-v2.md.
fn differences(go: &Value, rust: &Value, path: &str, out: &mut Vec<String>) {
    match (go, rust) {
        (Value::Object(a), Value::Object(b)) => {
            let keys: std::collections::BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for key in keys {
                match (a.get(key), b.get(key)) {
                    (Some(x), Some(y)) => differences(x, y, &format!("{path}/{key}"), out),
                    _ => out.push(format!("{path}/{key}: present on one side only")),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (x, y)) in a.iter().zip(b).enumerate() {
                differences(x, y, &format!("{path}/{index}"), out);
            }
        }
        (Value::Number(a), Value::Number(b)) => {
            let (x, y) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            let integral = a.is_i64() || a.is_u64();
            let tolerance = if integral && (b.is_i64() || b.is_u64()) {
                0.0
            } else {
                1e-9 * x.abs().max(y.abs()).max(1.0)
            };
            if (x - y).abs() > tolerance {
                out.push(format!("{path}: Go {x}, Rust {y}"));
            }
        }
        (a, b) if a == b => {}
        (a, b) => out.push(format!("{path}: Go {a}, Rust {b}")),
    }
}

#[derive(Deserialize)]
struct Golden {
    id: String,
    prepared: String,
    go_result: Option<GoldenFile>,
    go_log: Option<GoldenFile>,
}

#[derive(Deserialize)]
struct GoldenFile {
    file: String,
}

#[derive(Deserialize)]
struct GoldenManifest {
    cases: Vec<Golden>,
}

#[test]
fn supported_cases_match_pinned_go_goldens() {
    let manifest: GoldenManifest =
        serde_json::from_slice(&fs::read(family().join("manifest.json")).unwrap()).unwrap();
    let mut checked = 0;
    for case in manifest.cases {
        let Some(golden) = case.go_result else {
            continue;
        };
        let prepared: PreparedV2 =
            serde_json::from_slice(&fs::read(family().join(&case.prepared)).unwrap()).unwrap();
        let report = forever_engine::simulate_prepared(&prepared).unwrap();
        let expected: Value =
            serde_json::from_slice(&fs::read(family().join(&golden.file)).unwrap()).unwrap();
        let mut found = Vec::new();
        differences(&expected, &compact(&report.result), "", &mut found);
        assert!(found.is_empty(), "{}:\n{}", case.id, found.join("\n"));
        if let Some(log) = case.go_log {
            let expected = fs::read_to_string(family().join(&log.file)).unwrap();
            let actual = report.result["logs"].as_str().unwrap();
            for (line, (go, rust)) in expected.lines().zip(actual.lines()).enumerate() {
                assert_eq!(go, rust, "{} log line {}", case.id, line + 1);
            }
            assert_eq!(
                expected.lines().count(),
                actual.lines().count(),
                "{} log length",
                case.id
            );
        }
        checked += 1;
    }
    assert!(checked >= 5);
}
