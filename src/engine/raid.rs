//! Raid simulations: several players, each in their own fight, on one clock.
//!
//! A raid input is a `RaidSimRequest` with up to eight parties of five. Each member becomes a
//! single-player request (the raid's buffs and debuffs, the member's party buffs and the member),
//! which Rust prepares and gates as it would a Quick Sim. The runner then steps every member's
//! fight in time order, ties by raid order, with one fight length per iteration: the first
//! member's roll. Each member keeps its own random streams, seeded from the raid's seed plus its
//! position, so the first member of a raid fights exactly as it would alone.
//!
//! The result is Go's `RaidSimResult`: every member's player metrics in its party, and the raid's
//! and each party's DPS and HPS over the iterations' totals. A `mythicsim` block names each
//! member's raid index, party and slot.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{
    contracts::prepared_v2::PreparedV2,
    core::fight::{
        metrics::Distribution,
        raid::{RaidMember, SharedDebuff},
        Run,
    },
    prepare::{PrepareError, Rules},
};

use super::prepared::{refusals, validate, PreparedReport};

/// The raid input's schema.
pub const RAID_SCHEMA: u32 = 1;
/// Go's raid: eight parties of five.
pub const MAX_PARTIES: usize = 8;
pub const PARTY_SIZE: usize = 5;
/// How far apart the members' seeds are: further than any iteration count, so no two members
/// ever share an iteration's seed.
const SEED_STRIDE: i64 = 1 << 32;

/// A raid input: the request and who casts what on whom.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RaidInput {
    pub schema: u32,
    pub request: Value,
    #[serde(default)]
    pub assignments: Assignments,
}

/// External cooldowns members give each other, by raid index.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Assignments {
    #[serde(default)]
    pub innervate: Vec<Assignment>,
    #[serde(default)]
    pub power_infusion: Vec<Assignment>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub from: usize,
    pub to: usize,
}

/// A member of the raid and its own request.
#[derive(Debug, Clone)]
pub struct Member {
    /// Go's raid index: five per party, then the slot.
    pub raid_index: usize,
    pub party: usize,
    pub slot: usize,
    pub name: String,
    pub class: String,
    pub request: Value,
}

/// Why a raid did not run.
#[derive(Debug)]
pub enum RaidError {
    /// The raid input is not valid.
    Invalid(String),
    /// Rust does not support at least one member; each refusal names its member.
    Refused(Vec<MemberRefusal>),
}

impl std::fmt::Display for RaidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaidError::Invalid(reason) => write!(f, "raid rejected: {reason}"),
            RaidError::Refused(refusals) => {
                write!(f, "raid refused: ")?;
                for (index, refusal) in refusals.iter().enumerate() {
                    if index > 0 {
                        write!(f, "; ")?;
                    }
                    write!(
                        f,
                        "member {} ({}): {}",
                        refusal.member, refusal.code, refusal.reason
                    )?;
                }
                Ok(())
            }
        }
    }
}

/// A member Rust does not support, and why.
#[derive(Debug, Clone, Serialize)]
pub struct MemberRefusal {
    /// The member's raid index.
    pub member: usize,
    pub name: String,
    pub stage: &'static str,
    pub code: String,
    pub reason: String,
}

fn invalid(reason: impl Into<String>) -> RaidError {
    RaidError::Invalid(reason.into())
}

/// Split a raid input into its members' requests.
pub fn split(input: &RaidInput) -> Result<Vec<Member>, RaidError> {
    if input.schema != RAID_SCHEMA {
        return Err(invalid(format!(
            "schema {} is not {RAID_SCHEMA}",
            input.schema
        )));
    }
    let request = input
        .request
        .as_object()
        .ok_or_else(|| invalid("the request is not an object"))?;
    let raid = request
        .get("raid")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("a request without a raid"))?;
    if raid
        .get("targetDummies")
        .and_then(Value::as_i64)
        .unwrap_or(0)
        != 0
    {
        return Err(invalid("target dummies are unsupported"));
    }
    let parties = raid
        .get("parties")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("a raid without parties"))?;
    let active = match raid
        .get("numActiveParties")
        .and_then(Value::as_i64)
        .unwrap_or(0)
    {
        0 => parties.len(),
        n if n > 0 => (n as usize).min(parties.len()),
        n => return Err(invalid(format!("{n} active parties"))),
    };
    if active > MAX_PARTIES {
        return Err(invalid(format!("{active} parties, at most {MAX_PARTIES}")));
    }
    let seed = random_seed(request)?;
    let tanks = raid
        .get("tanks")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // Go `spreadRaidWidePartyBuffs`: Leader of the Pack and Moonkin Aura reach the whole raid.
    let raid_wide: Vec<&str> = ["leaderOfThePack", "moonkinAura"]
        .into_iter()
        .filter(|field| {
            parties[..active].iter().any(|party| {
                party
                    .pointer(&format!("/buffs/{field}"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
        })
        .collect();

    let mut members = Vec::new();
    for (party_index, party) in parties[..active].iter().enumerate() {
        let players = party
            .get("players")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if players.len() > PARTY_SIZE {
            return Err(invalid(format!(
                "party {} has {} players, at most {PARTY_SIZE}",
                party_index + 1,
                players.len()
            )));
        }
        for (slot, player) in players.iter().enumerate() {
            // An empty slot: Go's UI sends a player with no class.
            let Some(class) = player.get("class").and_then(Value::as_str) else {
                continue;
            };
            if class.is_empty() || class == "ClassUnknown" {
                continue;
            }
            let raid_index = PARTY_SIZE * party_index + slot;
            let mut party_buffs = party.get("buffs").cloned().unwrap_or_else(|| json!({}));
            for field in &raid_wide {
                party_buffs[*field] = Value::Bool(true);
            }
            let mut own_party = Map::new();
            own_party.insert("players".into(), Value::Array(vec![player.clone()]));
            own_party.insert("buffs".into(), party_buffs);
            if let Some(squawks) = party.get("battleSquawks") {
                own_party.insert("battleSquawks".into(), squawks.clone());
            }
            let mut own_raid = raid.clone();
            own_raid.insert(
                "parties".into(),
                Value::Array(vec![Value::Object(own_party)]),
            );
            own_raid.remove("numActiveParties");
            own_raid.insert("tanks".into(), Value::Array(own_tanks(&tanks, raid_index)?));
            let mut own = request.clone();
            own.insert("raid".into(), Value::Object(own_raid));
            let position = members.len() as i64;
            let own_seed = seed
                .checked_add(position * SEED_STRIDE)
                .ok_or_else(|| invalid("the random seed is too large for the raid"))?;
            own.entry("simOptions")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| invalid("simOptions is not an object"))?
                .insert("randomSeed".into(), Value::String(own_seed.to_string()));
            members.push(Member {
                raid_index,
                party: party_index,
                slot,
                name: player
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                class: class.to_string(),
                request: Value::Object(own),
            });
        }
    }
    if members.is_empty() {
        return Err(invalid("a raid without players"));
    }
    assign(&mut members, &input.assignments)?;
    Ok(members)
}

/// The raid's seed, which a raid needs so its members' seeds can follow from it.
fn random_seed(request: &Map<String, Value>) -> Result<i64, RaidError> {
    let seed = request
        .get("simOptions")
        .and_then(|options| options.get("randomSeed"))
        .ok_or_else(|| invalid("a raid needs simOptions.randomSeed"))?;
    match seed {
        Value::String(text) => text
            .parse()
            .map_err(|_| invalid(format!("random seed {text}"))),
        Value::Number(number) => number.as_i64().ok_or_else(|| invalid("random seed")),
        _ => Err(invalid("random seed")),
    }
}

/// The raid's tank list as one member's request reads it: the member is player 0 of its own
/// raid, and any other member a raid index with no player behind it, which leaves its targets
/// untanked in this member's fight. The list keeps its order, so the encounter's tank indexes
/// still point at the same entries.
fn own_tanks(tanks: &[Value], raid_index: usize) -> Result<Vec<Value>, RaidError> {
    tanks
        .iter()
        .map(|tank| {
            let kind = tank.get("type").and_then(Value::as_str).unwrap_or("Player");
            if kind != "Player" {
                return Err(invalid(format!(
                    "tank type {kind} is unsupported in a raid"
                )));
            }
            let index = tank.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let own = if index == raid_index {
                0
            } else {
                1 + MAX_PARTIES * PARTY_SIZE
            };
            Ok(json!({ "type": "Player", "index": own }))
        })
        .collect()
}

/// Until members cast their external cooldowns themselves, an assignment is one more of the
/// target's assumed Innervates or Power Infusions, from a member who can cast it.
fn assign(members: &mut [Member], assignments: &Assignments) -> Result<(), RaidError> {
    let lists = [
        (
            &assignments.innervate,
            "ClassDruid",
            "innervates",
            "Innervate",
        ),
        (
            &assignments.power_infusion,
            "ClassPriest",
            "powerInfusions",
            "Power Infusion",
        ),
    ];
    for (list, caster_class, field, spell) in lists {
        for assignment in list {
            let caster = members
                .iter()
                .find(|member| member.raid_index == assignment.from)
                .ok_or_else(|| {
                    invalid(format!(
                        "{spell} from raid index {}, no member",
                        assignment.from
                    ))
                })?;
            if caster.class != caster_class {
                return Err(invalid(format!(
                    "{spell} from {} ({}), who cannot cast it",
                    caster.name, caster.class
                )));
            }
            let target = members
                .iter_mut()
                .find(|member| member.raid_index == assignment.to)
                .ok_or_else(|| {
                    invalid(format!(
                        "{spell} on raid index {}, no member",
                        assignment.to
                    ))
                })?;
            let player = target
                .request
                .pointer_mut("/raid/parties/0/players/0")
                .and_then(Value::as_object_mut)
                .expect("a member's request holds its player");
            let buffs = player
                .entry("buffs")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| invalid("player buffs are not an object"))?;
            let count = buffs.get(field).and_then(Value::as_i64).unwrap_or(0);
            buffs.insert(field.into(), json!(count + 1));
        }
    }
    Ok(())
}

/// The raid debuffs whose state every member's fight shares. A debuff a member casts is on the
/// target in every fight only while that member keeps it up, with its stacks and expiry; the
/// raid's copy then no longer counts from the pull. These are the debuffs whose effect the fight
/// applies when the aura goes up; the others stay as the request's toggles say (docs/raid.md).
pub(crate) const SHARED_DEBUFFS: &[SharedDebuff] = &[
    SharedDebuff {
        name: "judgement_of_wisdom",
        external: "Judgement of Wisdom (External)",
        own_prefix: "Judgement of Wisdom Rank ",
    },
    SharedDebuff {
        name: "judgement_of_light",
        external: "Judgement of Light (External)",
        own_prefix: "Judgement of Light Rank ",
    },
];

/// A prepared member, ready to simulate.
pub struct PreparedMember {
    pub member: Member,
    pub prepared: PreparedV2,
}

/// Prepare and gate every member. Refuses the raid when Rust does not support any of them,
/// naming each such member; never runs a fight.
pub fn prepare(members: Vec<Member>, rules: Rules) -> Result<Vec<PreparedMember>, RaidError> {
    let mut prepared_members = Vec::with_capacity(members.len());
    let mut refused = Vec::new();
    for member in members {
        let refuse = |stage, code: &str, reason: String| MemberRefusal {
            member: member.raid_index,
            name: member.name.clone(),
            stage,
            code: code.to_string(),
            reason,
        };
        let bytes = serde_json::to_vec(&member.request).map_err(|err| invalid(err.to_string()))?;
        let value = match crate::prepare::prepare_json_with(&bytes, "raid", rules) {
            Ok(value) => value,
            Err(PrepareError::Refused(refusal)) => {
                refused.push(refuse("prepare", refusal.code, refusal.reason));
                continue;
            }
            Err(PrepareError::Invalid(reason)) => {
                return Err(invalid(format!(
                    "member {} ({}): {reason}",
                    member.raid_index, member.name
                )))
            }
            Err(PrepareError::Fault(reason)) => {
                refused.push(refuse("prepare", "preparation_fault", reason));
                continue;
            }
        };
        let prepared: PreparedV2 = serde_json::from_value(value).map_err(|err| {
            invalid(format!(
                "member {}: prepared output: {err}",
                member.raid_index
            ))
        })?;
        validate(&prepared)
            .map_err(|err| invalid(format!("member {}: {err}", member.raid_index)))?;
        let gate = refusals(&prepared);
        if !gate.is_empty() {
            refused.extend(
                gate.into_iter()
                    .map(|refusal| refuse("check", refusal.code, refusal.reason)),
            );
            continue;
        }
        prepared_members.push(PreparedMember { member, prepared });
    }
    if refused.is_empty() {
        Ok(prepared_members)
    } else {
        Err(RaidError::Refused(refused))
    }
}

/// A member's result and where it stands in the raid.
pub struct MemberReport {
    pub member: Member,
    pub report: PreparedReport,
}

/// A raid's result.
pub struct RaidReport {
    pub members: Vec<MemberReport>,
    pub raid_dps: Distribution,
    pub raid_hps: Distribution,
    /// Each party's DPS and HPS, by party index; a party without members has none.
    pub parties: Vec<Option<(Distribution, Distribution)>>,
    /// The raid index of each member that casts each of [`SHARED_DEBUFFS`].
    pub casters: Vec<Vec<usize>>,
    pub elapsed_ns: u64,
}

/// Simulate prepared members together.
pub fn run(members: Vec<PreparedMember>, rotation_stats: bool) -> Result<RaidReport, RaidError> {
    let started = crate::core::stopwatch::Stopwatch::start();
    let iterations = members[0].prepared.sim.iterations;
    let seed = members[0].prepared.sim.seed;
    if members
        .iter()
        .any(|member| member.prepared.sim.iterations != iterations)
    {
        return Err(invalid("members with different iteration counts"));
    }
    let mut fights: Vec<Box<dyn RaidMember>> = members
        .iter()
        .map(|member| crate::classes::raid_member(&member.prepared).map_err(invalid))
        .collect::<Result<_, _>>()?;
    for fight in &mut fights {
        fight.prepare_run();
    }
    // A debuff is live when a member casts it: every fight then shares its state.
    let mut casters = vec![Vec::new(); SHARED_DEBUFFS.len()];
    for (index, fight) in fights.iter_mut().enumerate() {
        for (key, (own, _)) in fight.share_auras(SHARED_DEBUFFS).into_iter().enumerate() {
            if own {
                casters[key].push(members[index].member.raid_index);
            }
        }
    }
    for (key, debuff) in SHARED_DEBUFFS.iter().enumerate() {
        if !casters[key].is_empty() {
            for fight in &mut fights {
                fight.drive_external(debuff);
            }
        }
    }
    let mut states = Vec::new();
    let party_count = members
        .iter()
        .map(|member| member.member.party)
        .max()
        .unwrap_or(0)
        + 1;
    let mut parties: Vec<Option<(Distribution, Distribution)>> = (0..party_count)
        .map(|party| {
            members
                .iter()
                .any(|member| member.member.party == party)
                .then(|| (Distribution::default(), Distribution::default()))
        })
        .collect();
    let mut raid_dps = Distribution::default();
    let mut raid_hps = Distribution::default();
    let mut runs: Vec<Run> = (0..fights.len()).map(|_| Run::default()).collect();
    let mut active = vec![true; fights.len()];
    let mut due = BinaryHeap::new();
    for iteration in 0..iterations {
        fights[0].set_duration_override(None);
        fights[0].begin_iteration(iteration);
        let duration = fights[0].duration();
        for fight in &mut fights[1..] {
            fight.set_duration_override(Some(duration));
            fight.begin_iteration(iteration);
        }
        // What the resets did to shared auras is each fight's own.
        for fight in &mut fights {
            states.clear();
            fight.drain_shared(&mut states);
        }
        due.clear();
        for (index, fight) in fights.iter_mut().enumerate() {
            active[index] = true;
            due.push(Reverse((fight.next_time(), index)));
        }
        // Always the earliest event of any fight, ties in raid order. An entry is stale when its
        // fight's next event moved since it was pushed.
        while let Some(Reverse((time, index))) = due.pop() {
            if !active[index] {
                continue;
            }
            let next = fights[index].next_time();
            if next != time {
                due.push(Reverse((next, index)));
                continue;
            }
            if fights[index].step() {
                let next = fights[index].next_time();
                due.push(Reverse((next, index)));
            } else {
                active[index] = false;
            }
            // A shared aura that changed reaches every other fight still running, at once.
            states.clear();
            fights[index].drain_shared(&mut states);
            if !states.is_empty() {
                let now = fights[index].now();
                for other in 0..fights.len() {
                    if other != index && active[other] {
                        fights[other].apply_shared(now, &states);
                        due.push(Reverse((fights[other].next_time(), other)));
                    }
                }
            }
        }
        let mut damage = 0.0;
        let mut healing = 0.0;
        let mut party_totals = vec![(0.0, 0.0); party_count];
        for (index, fight) in fights.iter_mut().enumerate() {
            let (member_damage, member_healing) = fight.end_iteration(iteration, &mut runs[index]);
            states.clear();
            fight.drain_shared(&mut states);
            damage += member_damage;
            healing += member_healing;
            let party = &mut party_totals[members[index].member.party];
            party.0 += member_damage;
            party.1 += member_healing;
        }
        let iteration_seed = seed + i64::from(iteration);
        raid_dps.total = damage;
        raid_dps.done_iteration(duration, iteration_seed, false);
        raid_hps.total = healing;
        raid_hps.done_iteration(duration, iteration_seed, false);
        for (party, totals) in parties.iter_mut().zip(party_totals) {
            if let Some((dps, hps)) = party {
                dps.total = totals.0;
                dps.done_iteration(duration, iteration_seed, false);
                hps.total = totals.1;
                hps.done_iteration(duration, iteration_seed, false);
            }
        }
    }
    let elapsed_ns = started.elapsed_ns();
    let mut reports = Vec::with_capacity(fights.len());
    for ((mut fight, run), member) in fights.into_iter().zip(runs).zip(members) {
        let fight_report = fight.finish(run, elapsed_ns);
        let mut report = PreparedReport::of_fight(&member.prepared, fight_report)
            .map_err(|err| invalid(err.to_string()))?;
        if rotation_stats {
            report.add_rotation_stats();
        }
        reports.push(MemberReport {
            member: member.member,
            report,
        });
    }
    Ok(RaidReport {
        members: reports,
        raid_dps,
        raid_hps,
        parties,
        casters,
        elapsed_ns,
    })
}

impl RaidReport {
    /// Go's `RaidSimResult` for the raid, with the `mythicsim` block beside it.
    pub fn to_json(&self, request_sha256: &str) -> Result<Value, String> {
        let first = &self.members[0].report;
        let mut result = first.result.clone();
        let distribution =
            |d: &Distribution| serde_json::to_value(d.report()).map_err(|e| e.to_string());
        let mut parties = Vec::new();
        for (party_index, party) in self.parties.iter().enumerate() {
            let Some((dps, hps)) = party else {
                parties.push(json!({ "players": [] }));
                continue;
            };
            let players: Vec<Value> = self
                .members
                .iter()
                .filter(|member| member.member.party == party_index)
                .map(|member| {
                    member
                        .report
                        .result
                        .pointer("/raidMetrics/parties/0/players/0")
                        .cloned()
                        .unwrap_or(Value::Null)
                })
                .collect();
            parties.push(
                json!({ "dps": distribution(dps)?, "hps": distribution(hps)?, "players": players }),
            );
        }
        result["raidMetrics"] = json!({
            "dps": distribution(&self.raid_dps)?,
            "hps": distribution(&self.raid_hps)?,
            "parties": parties,
        });
        let members: Vec<Value> = self
            .members
            .iter()
            .map(|member| {
                let player = member
                    .report
                    .result
                    .pointer("/raidMetrics/parties/0/players/0");
                let read = |path: &str| {
                    player
                        .and_then(|player| player.pointer(path))
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0)
                };
                json!({
                    "raidIndex": member.member.raid_index,
                    "party": member.member.party,
                    "slot": member.member.slot,
                    "name": member.member.name,
                    "class": member.member.class,
                    "dps": read("/dps/avg"),
                    "dpsStdev": read("/dps/stdev"),
                    "hps": read("/hps/avg"),
                    "tanking": read("/dtps/avg") > 0.0,
                    "avgIterationDuration": member.report.result["avgIterationDuration"],
                    "scenarioId": member.report.scenario_id,
                })
            })
            .collect();
        Ok(json!({
            "engine": first.engine,
            "schema_version": first.schema_version,
            "source_revision": first.source_revision,
            "identity": first.identity,
            "scenario_id": "raid",
            "request_sha256": request_sha256,
            "elapsed_ns": self.elapsed_ns,
            "result": result,
            "mythicsim": {
                "raidSchema": RAID_SCHEMA,
                "members": members,
                "sharedDebuffs": SHARED_DEBUFFS
                    .iter()
                    .zip(&self.casters)
                    .map(|(debuff, casters)| json!({
                        "debuff": debuff.name,
                        "casters": casters,
                        "live": !casters.is_empty(),
                    }))
                    .collect::<Vec<_>>(),
            },
        }))
    }
}

/// Split, prepare, gate and simulate a raid input.
pub fn simulate(input: &[u8], rules: Rules, rotation_stats: bool) -> Result<Value, RaidError> {
    let raid: RaidInput =
        serde_json::from_slice(input).map_err(|err| invalid(format!("raid input: {err}")))?;
    let members = split(&raid)?;
    let prepared = prepare(members, rules)?;
    let report = run(prepared, rotation_stats)?;
    let digest = crate::contracts::request::sha256::hex(input);
    report.to_json(&digest).map_err(RaidError::Invalid)
}
