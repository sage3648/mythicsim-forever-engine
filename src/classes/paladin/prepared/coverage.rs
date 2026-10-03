//! The Paladin part of the prepared v2 build gate. The shared gate is in
//! `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassPaladin",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits,
};

/// Paladin effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "consecration",
    "divine_favor",
    "hammer_of_wrath",
    "holy_shield",
    "holy_shock",
    "holy_strike",
    "illumination",
    "iron_creed",
    "judgement",
    "judgement_refresh",
    "reckoning",
    "redoubt",
    "righteous_fury",
    "sacred_arbiter",
    "sanctified_judgement",
    "seal_of_command",
    "seal_of_righteousness",
    "shield_specialization",
    "swift_judgement",
    "templars_bulwark",
    "twist_of_light",
    "vengeance",
    "vindication",
];

/// The effect kind whose implementation executes a Paladin spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    match spell.class_spell.as_deref()? {
        "seal_of_command" | "seal_of_command_proc" | "judgement_of_command" => {
            Some("seal_of_command")
        }
        "seal_of_righteousness" | "seal_of_righteousness_proc" | "judgement_of_righteousness" => {
            Some("seal_of_righteousness")
        }
        "judgement" => Some("judgement"),
        "holy_strike" => Some("holy_strike"),
        "hammer_of_wrath" => Some("hammer_of_wrath"),
        "consecration" => Some("consecration"),
        "holy_shock" => Some("holy_shock"),
        "divine_favor" => Some("divine_favor"),
        "righteous_fury" => Some("righteous_fury"),
        "swift_judgement" => Some("swift_judgement"),
        "templars_bulwark" => Some("templars_bulwark"),
        "holy_shield" | "holy_shield_proc" => Some("holy_shield"),
        _ => None,
    }
}

/// Aura labels a Paladin effect takes responsibility for, as (unit, label): its listeners and
/// every aura its behavior can activate.
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::JudgementRefresh { trigger_aura, .. }
        | Effect::SanctifiedJudgement { trigger_aura, .. }
        | Effect::SacredArbiter { trigger_aura, .. }
        | Effect::ShieldSpecialization { trigger_aura, .. }
        | Effect::Illumination { trigger_aura }
        | Effect::DivineFavor {
            aura: trigger_aura, ..
        }
        | Effect::SwiftJudgement {
            aura: trigger_aura, ..
        }
        | Effect::HolyShield {
            aura: trigger_aura, ..
        } => vec![("player", trigger_aura)],
        Effect::Vengeance {
            trigger_aura, aura, ..
        }
        | Effect::Redoubt {
            trigger_aura, aura, ..
        }
        | Effect::IronCreed {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        Effect::Vindication {
            trigger_aura,
            aura,
            target_aura,
            ..
        } => vec![
            ("player", trigger_aura),
            ("player", aura),
            ("target", target_aura),
        ],
        Effect::Reckoning {
            block_aura,
            crit_aura,
            ..
        } => vec![("player", block_aura), ("player", crit_aura)],
        Effect::RighteousFury {
            aura,
            instrument_of_law,
            ..
        } => {
            let mut claimed = vec![("player", aura.as_str())];
            if let Some(law) = instrument_of_law {
                claimed.push(("player", law.aura.as_str()));
            }
            claimed
        }
        Effect::Consecration {
            consecrated_ground: Some(ground),
            ..
        } => vec![("target", ground.aura.as_str())],
        Effect::TwistOfLight {
            trigger_aura,
            echoes,
        } => std::iter::once(("player", trigger_aura.as_str()))
            .chain(echoes.iter().map(|echo| ("player", echo.aura.as_str())))
            .collect(),
        Effect::SealOfCommand { ranks, .. } => ranks
            .iter()
            .map(|rank| ("player", rank.aura.as_str()))
            .collect(),
        Effect::SealOfRighteousness { ranks, .. } => ranks
            .iter()
            .map(|rank| ("player", rank.aura.as_str()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Class limits: Templar's Bulwark and Illumination.
fn limits(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    let mut reasons = bulwark_limits(prepared);
    // Illumination hears only heal crits, and the runtime casts no heal.
    let illumination = prepared
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::Illumination { .. }));
    let heals = reachable.iter().any(|spell| {
        spell
            .proc_mask
            .iter()
            .any(|mask| mask == "ProcMaskSpellHealing")
    });
    if illumination && heals {
        reasons.push("Illumination with a reachable heal is unsupported".into());
    }
    reasons
}

/// Templar's Bulwark is described only as the survival cooldown Go never fires without a
/// health threshold: a rotation that casts it, or a timing that fires it, is unsupported.
fn bulwark_limits(prepared: &PreparedV2) -> Vec<String> {
    let Some(bulwark) = prepared.effects.iter().find_map(|effect| match effect {
        Effect::TemplarsBulwark { spell_id } => Some(*spell_id),
        _ => None,
    }) else {
        return Vec::new();
    };
    let mut reasons = Vec::new();
    let timed =
        prepared.player.major_cooldowns.iter().any(|cooldown| {
            cooldown.action_id.spell_id == bulwark && !cooldown.timings_ns.is_empty()
        });
    if timed {
        reasons.push("Templar's Bulwark with cooldown timings is unsupported".into());
    }
    if let Ok(rotation) = crate::rotation::parse(&prepared.player.rotation) {
        let named = rotation
            .prepull
            .iter()
            .map(|prepull| &prepull.action)
            .chain(rotation.priority_list.iter().map(|item| &item.action))
            .any(|action| action.spells().iter().any(|id| id.spell_id == bulwark));
        if named {
            reasons.push("a rotation that casts Templar's Bulwark is unsupported".into());
        }
    }
    reasons
}
