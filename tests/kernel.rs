use forever_engine::{hit_chance, simulate, Caster, Request, Spell, Target, SOURCE_REVISION};

fn controlled() -> Request {
    Request {
        schema_version: 1,
        source_revision: SOURCE_REVISION.into(),
        scenario_id: "controlled".into(),
        iterations: 10_000,
        seed: 42,
        duration_ns: 10_000_000_000,
        reaction_ns: 100_000_000,
        caster: Caster {
            level: 60,
            max_mana: 1000.0,
            spell_power: 100.0,
            hit_percent: 100.0,
            crit_percent: 0.0,
            spell_penetration: 0.0,
            regen_casting_per_second: 0.0,
            regen_idle_per_second: 0.0,
        },
        target: Target {
            level: 63,
            frost_resistance: 0.0,
        },
        spell: Spell {
            id: 25304,
            min_damage: 100.0,
            max_damage: 100.0,
            coefficient: 1.0,
            damage_multiplier: 1.0,
            crit_multiplier: 1.5,
            mana_cost: 0.0,
            cast_ns: 2_000_000_000,
            gcd_ns: 1_500_000_000,
            travel_ns: 1_000_000_000,
        },
    }
}

#[test]
fn mean_agrees_with_analytic_binary_spell_expectation() {
    let request = controlled();
    let result = simulate(&request, false).unwrap();
    // Four missiles arrive by ten seconds. The fifth completes but lands later.
    let expected = 4.0 * 0.99 * 200.0 / 10.0;
    assert!((result.dps_mean - expected).abs() < 5.0 * result.dps_standard_error);
    assert_eq!(result.counts.casts, 5 * u64::from(request.iterations));
    assert_eq!(
        result.counts.casts,
        result.counts.hits + result.counts.crits + result.counts.misses
    );
    assert_eq!(result.counts.crits, 0);
}

#[test]
fn boundary_counts_completion_but_excludes_late_missile_damage() {
    let mut request = controlled();
    request.iterations = 1;
    request.duration_ns = 2_000_000_000;
    let pending = simulate(&request, true).unwrap();
    assert_eq!(pending.counts.casts, 1);
    assert_eq!(pending.dps_mean, 0.0);
    assert!(!pending.trace.iter().any(|event| event.event == "hit"));
    request.spell.travel_ns = 0;
    let immediate = simulate(&request, true).unwrap();
    assert!(immediate.dps_mean > 0.0);
}

#[test]
fn insufficient_mana_does_not_create_free_casts() {
    let mut request = controlled();
    request.iterations = 1;
    request.caster.max_mana = 150.0;
    request.spell.mana_cost = 100.0;
    let report = simulate(&request, true).unwrap();
    assert_eq!(report.counts.casts, 1);
    assert_eq!(report.mana_end_mean, 50.0);
    assert!(report
        .trace
        .iter()
        .any(|event| event.event == "wait_for_mana"));
}

#[test]
fn resistance_and_hit_cap_follow_binary_rules() {
    let mut request = controlled();
    request.caster.hit_percent = 0.0;
    assert!((hit_chance(&request) - 0.83).abs() < 1e-12);
    request.target.frost_resistance = 120.0;
    assert!((hit_chance(&request) - 0.83 * 0.7).abs() < 1e-12);
    request.caster.spell_penetration = 120.0;
    assert!((hit_chance(&request) - 0.83).abs() < 1e-12);
    request.caster.hit_percent = 100.0;
    assert_eq!(hit_chance(&request), 0.99);
}

#[test]
fn unsupported_fields_and_invalid_parameters_fail_closed() {
    let mut value = serde_json::to_value(controlled()).unwrap();
    value["custom_rotation"] = serde_json::json!({});
    assert!(serde_json::from_value::<Request>(value).is_err());
    let mut request = controlled();
    request.spell.id = 116;
    assert!(simulate(&request, false).is_err());
    request.spell.id = 25304;
    request.caster.max_mana = f64::NAN;
    assert!(simulate(&request, false).is_err());
    request.caster.max_mana = 1000.0;
    request.iterations = 0;
    assert!(simulate(&request, false).is_err());
}

#[test]
fn repeated_seed_reproduces_results_without_trace_affecting_rng() {
    let mut request = controlled();
    request.iterations = 100;
    let a = simulate(&request, false).unwrap();
    let b = simulate(&request, true).unwrap();
    assert_eq!(a.dps_mean, b.dps_mean);
    assert_eq!(a.dps_stdev, b.dps_stdev);
    assert_eq!(a.counts, b.counts);
    request.seed = 173;
    assert_ne!(a.dps_mean, simulate(&request, false).unwrap().dps_mean);
}
