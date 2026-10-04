//! Prepared v2 entry point: identity and bound validation, then the spec coverage gate.
//!
//! Invalid input is a contract violation. Unsupported input is valid but outside the
//! implemented capability set; its reasons are stable strings a worker can record as
//! fallback reasons.

use crate::{
    classes,
    contracts::prepared_v2::{PreparedV2, CONTRACT, SCHEMA_VERSION},
    mechanics::{
        haste::cast_speed,
        mana::{regen_per_second_casting, regen_per_second_not_casting, RegenInputs},
    },
    rotation, SOURCE_REVISION,
};

/// The client build whose data the pinned reference prepares, from upstream/sources.json.
pub const CLIENT_BUILD: &str = env!("FOREVER_CLIENT_BUILD");

const SECOND: i64 = 1_000_000_000;

#[derive(Debug, PartialEq)]
pub enum PreparedError {
    /// The input violates the contract or its identity does not match this engine.
    Invalid(String),
    /// The input is valid but uses mechanics this engine does not implement.
    Unsupported(Vec<String>),
}

impl std::fmt::Display for PreparedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreparedError::Invalid(reason) => write!(f, "prepared input rejected: {reason}"),
            PreparedError::Unsupported(reasons) => {
                write!(f, "prepared input unsupported:")?;
                for reason in reasons {
                    write!(f, "\n  {reason}")?;
                }
                Ok(())
            }
        }
    }
}

/// Contract identity and wire limits. These never depend on implemented mechanics.
pub fn validate(prepared: &PreparedV2) -> Result<(), String> {
    if prepared.schema_version != SCHEMA_VERSION || prepared.contract != CONTRACT {
        return Err("unsupported schema version or contract".into());
    }
    if prepared.reference.engine_revision != SOURCE_REVISION {
        return Err(format!(
            "reference revision {} differs from {SOURCE_REVISION}",
            prepared.reference.engine_revision
        ));
    }
    if prepared.reference.client_build != CLIENT_BUILD {
        return Err(format!(
            "client build {} differs from {CLIENT_BUILD}",
            prepared.reference.client_build
        ));
    }
    if prepared.scenario_id.is_empty() || prepared.scenario_id.len() > 200 {
        return Err("scenario_id must contain 1 to 200 bytes".into());
    }
    if prepared.request_sha256.len() != 64
        || !prepared
            .request_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("request_sha256 must be 64 lowercase hexadecimal digits".into());
    }
    // Side::Pet(i) is the pet at position i, in Go unit index order, which fits in a u8.
    if prepared.pets.len() > usize::from(u8::MAX)
        || prepared
            .pets
            .windows(2)
            .any(|pair| pair[0].index >= pair[1].index)
    {
        return Err("pets must be fewer than 256, in increasing unit index order".into());
    }
    let sim = &prepared.sim;
    let encounter = &prepared.encounter;
    if !(1..=1_000_000).contains(&sim.iterations)
        || sim.seed <= 0
        || sim.seed > i64::MAX - i64::from(sim.iterations)
    {
        return Err("iterations must be 1 to 1000000 with a positive seed".into());
    }
    if !(SECOND..=600 * SECOND).contains(&encounter.duration_ns)
        || !(0..=120 * SECOND).contains(&encounter.duration_variation_ns)
        || encounter.duration_variation_ns >= encounter.duration_ns
    {
        return Err(
            "duration must be 1 to 600 seconds with a smaller variation of at most 120 seconds"
                .into(),
        );
    }
    let player = &prepared.player;
    if !(10_000_000..=SECOND).contains(&player.reaction_ns)
        || !(0..=SECOND).contains(&player.channel_clip_delay_ns)
        || !(0.0..=100.0).contains(&player.distance_yards)
    {
        return Err("reaction time, channel clip delay or distance is outside limits".into());
    }
    for (name, value) in [
        ("cast_speed", player.cast_speed),
        ("max_mana", player.mana.max),
        ("base_mana", player.mana.base),
        (
            "spirit_regen_per_second",
            player.mana.spirit_regen_per_second,
        ),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(format!("{name} must be finite and nonnegative"));
        }
    }
    if player.cast_speed == 0.0 {
        return Err("cast_speed must be positive".into());
    }
    // A class without a mana bar, such as a Rogue, exports no mana at all and no mana costs.
    let manaless = player.mana.max == 0.0;
    if manaless {
        let mana_cost = player.spells.iter().any(|spell| {
            spell
                .cost
                .as_ref()
                .is_some_and(|cost| cost.resource == "mana")
        });
        if mana_cost
            || player.mana.base != 0.0
            || player.mana.teardown_max != 0.0
            || player.mana.regen_per_second_casting != 0.0
            || player.mana.regen_per_second_not_casting != 0.0
        {
            return Err("a player without mana must have no mana costs or regeneration".into());
        }
    } else if !(player.mana.teardown_max > 0.0 && player.mana.teardown_max <= player.mana.max) {
        return Err("mana teardown_max must be positive and at most max".into());
    }
    // Rust recomputes Go's starting regeneration from the exported components. A mismatch
    // means the components or the formula drifted, so the fight cannot be trusted.
    let pseudo = &player.pseudo_stats;
    let inputs = RegenInputs {
        mp5: *player.stats.get("MP5").ok_or("player stats lack MP5")?,
        spirit_regen_per_second: player.mana.spirit_regen_per_second,
        spirit_regen_rate_casting: pseudo.spirit_regen_rate_casting,
        force_full_spirit_regen: pseudo.force_full_spirit_regen,
        spirit_regen_multiplier: pseudo.spirit_regen_multiplier,
        mana_regen_multiplier: 1.0,
    };
    for (name, rust, go) in if manaless {
        Vec::new()
    } else {
        vec![
            (
                "casting",
                regen_per_second_casting(inputs),
                player.mana.regen_per_second_casting,
            ),
            (
                "not casting",
                regen_per_second_not_casting(inputs),
                player.mana.regen_per_second_not_casting,
            ),
        ]
    } {
        // Go may fuse multiply-add on some architectures; allow only that rounding.
        if (rust - go).abs() > 1e-12 * go.abs().max(1.0) {
            return Err(format!(
                "mana regeneration while {name} is {rust}, Go prepared {go}"
            ));
        }
    }
    // Rust recomputes cast speed when an aura multiplies it, so the formula must agree with
    // the cast speed Go prepared.
    let haste_rating = *player
        .stats
        .get("SpellHasteRating")
        .ok_or("player stats lack SpellHasteRating")?;
    let speed = cast_speed(pseudo.cast_speed_multiplier, haste_rating);
    if (speed - player.cast_speed).abs() > 1e-12 * player.cast_speed {
        return Err(format!(
            "cast speed is {speed}, Go prepared {}",
            player.cast_speed
        ));
    }
    for spell in &player.spells {
        let cast = &spell.default_cast;
        if cast.gcd_ns < 0 || cast.cast_time_ns < 0 || cast.gcd_min_ns < 0 {
            return Err(format!(
                "spell {:?} has a negative cast timing",
                spell.action_id
            ));
        }
        if !spell.missile_speed.is_finite() || spell.missile_speed < 0.0 {
            return Err(format!(
                "spell {:?} has an invalid missile speed",
                spell.action_id
            ));
        }
    }
    Ok(())
}

/// Every reason the engine cannot simulate a valid input. Empty means supported.
pub fn coverage(prepared: &PreparedV2) -> Vec<String> {
    let mut reasons: Vec<String> = prepared
        .unrepresented
        .iter()
        .map(|reason| format!("unrepresented by the exporter: {reason}"))
        .collect();
    let rotation = match rotation::parse(&prepared.player.rotation) {
        Ok(rotation) => Some(rotation),
        Err(rotation_reasons) => {
            reasons.extend(rotation_reasons);
            None
        }
    };
    reasons.extend(super::coverage::prepared_coverage(
        prepared,
        rotation.as_ref(),
    ));
    reasons
}

/// Validate and gate a prepared v2 input.
pub fn check(prepared: &PreparedV2) -> Result<(), PreparedError> {
    validate(prepared).map_err(PreparedError::Invalid)?;
    let reasons = coverage(prepared);
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(PreparedError::Unsupported(reasons))
    }
}

/// The engine identity and result of a prepared v2 simulation. `result` follows Go's
/// `RaidSimResult` JSON for the fields Rust implements.
#[derive(Debug, serde::Serialize)]
pub struct PreparedReport {
    pub engine: String,
    pub schema_version: u32,
    pub source_revision: String,
    pub scenario_id: String,
    pub request_sha256: String,
    pub elapsed_ns: u64,
    pub result: serde_json::Value,
}

/// Validate, gate and simulate a prepared v2 input.
pub fn simulate(prepared: &PreparedV2) -> Result<PreparedReport, PreparedError> {
    check(prepared)?;
    let report = classes::run_prepared(prepared).map_err(PreparedError::Invalid)?;
    let elapsed_ns = report.elapsed_ns;
    Ok(PreparedReport {
        engine: format!("forever-rust-{}", env!("CARGO_PKG_VERSION")),
        schema_version: SCHEMA_VERSION,
        source_revision: SOURCE_REVISION.into(),
        scenario_id: prepared.scenario_id.clone(),
        request_sha256: prepared.request_sha256.clone(),
        elapsed_ns,
        result: serde_json::to_value(&report)
            .map_err(|err| PreparedError::Invalid(err.to_string()))?,
    })
}
