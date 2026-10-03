//! Seals, from Go sim/paladin/seals.go, seal_of_command.go and seal_of_righteousness.go: one
//! seal up at a time, its aura's proc on landed white hits, and the damage a batch window
//! after the hit.

use crate::{
    contracts::prepared_v2::Effect,
    core::{
        fight::{
            melee::PhysicalOutcome, Action, AuraRef, Fight, Outcome, Side, SpellId, SpellResult,
            OUTCOME_LANDED, PRIORITY_LOW,
        },
        time::STARTING_CD_TIME,
    },
};

use super::super::agent::PaladinAgent;

/// Which seal a rank belongs to, for its proc and its Twist of Light Echo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SealKind {
    Command,
    Righteousness,
}

impl SealKind {
    /// The class spell name the exporter writes for the seal.
    pub(crate) fn name(self) -> &'static str {
        match self {
            SealKind::Command => "seal_of_command",
            SealKind::Righteousness => "seal_of_righteousness",
        }
    }
}

/// One rank of one seal: Go `sealConfig`.
#[derive(Clone, Debug)]
pub(crate) struct Seal {
    pub(crate) kind: SealKind,
    /// The castable seal spell.
    pub(crate) spell: SpellId,
    pub(crate) aura: AuraRef,
    pub(crate) judgement: SpellId,
    /// The damage spell the seal fires.
    pub(crate) proc_spell: SpellId,
    /// Seal of Righteousness: the per-hit value per hundred of swing speed.
    pub(crate) per_hit_value: f64,
    /// Seal of Command: the rank's own proc cooldown.
    pub(crate) icd_timer: usize,
}

/// Seal of Command's proc parameters, shared by every rank.
#[derive(Clone, Debug)]
pub(crate) struct CommandProc {
    pub(crate) weapon_percent: f64,
    pub(crate) coefficient: f64,
    pub(crate) chance: f64,
    pub(crate) label: String,
    pub(crate) icd: i64,
}

/// Every bound seal rank and the seal the paladin is under.
#[derive(Clone, Debug, Default)]
pub(crate) struct Seals {
    pub(crate) seals: Vec<Seal>,
    /// Go `Paladin.currentSeal`.
    pub(crate) current: Option<usize>,
    pub(crate) command: Option<CommandProc>,
    pub(crate) hand_multiplier: f64,
    pub(crate) swing_speed: f64,
    pub(crate) deal_delay: i64,
}

/// The position of a seal rank among every bound seal, in the order [`bind`] builds them:
/// Seal of Command's ranks, then Seal of Righteousness's.
pub(crate) fn seal_index(effects: &[Effect], seal_spell_id: i32) -> Option<usize> {
    let mut index = 0;
    for effect in effects {
        if let Effect::SealOfCommand { ranks, .. } = effect {
            for rank in ranks {
                if rank.seal_spell_id == seal_spell_id {
                    return Some(index);
                }
                index += 1;
            }
        }
    }
    for effect in effects {
        if let Effect::SealOfRighteousness { ranks, .. } = effect {
            for rank in ranks {
                if rank.seal_spell_id == seal_spell_id {
                    return Some(index);
                }
                index += 1;
            }
        }
    }
    None
}

/// Resolve every seal rank's spells, auras and cooldowns.
pub(crate) fn bind(fight: &mut Fight<PaladinAgent>, effects: &[Effect]) -> Result<Seals, String> {
    let find = |fight: &Fight<PaladinAgent>, id: i32, occurrence: usize| {
        fight
            .spells
            .iter()
            .enumerate()
            .filter(|(_, spell)| spell.id.spell_id == id && spell.id.tag == 0)
            .nth(occurrence)
            .map(|(index, _)| index)
            .ok_or_else(|| format!("seal spell {id} is not registered"))
    };
    let mut seals = Seals::default();
    for effect in effects {
        if let Effect::SealOfCommand {
            ranks,
            proc_spell_id,
            weapon_percent,
            coefficient,
            proc_chance,
            rng_label,
            icd_ns,
            deal_delay_ns,
        } = effect
        {
            for (position, rank) in ranks.iter().enumerate() {
                // Each rank registers its own proc spell and cooldown, in rank order.
                let icd_timer = fight.timers.len();
                fight.timers.push(STARTING_CD_TIME);
                seals.seals.push(Seal {
                    kind: SealKind::Command,
                    spell: find(fight, rank.seal_spell_id, 0)?,
                    aura: fight.player_aura(&rank.aura)?,
                    judgement: find(fight, rank.judgement_spell_id, 0)?,
                    proc_spell: find(fight, *proc_spell_id, position)?,
                    per_hit_value: 0.0,
                    icd_timer,
                });
            }
            seals.command = Some(CommandProc {
                weapon_percent: *weapon_percent,
                coefficient: *coefficient,
                chance: *proc_chance,
                label: rng_label.clone(),
                icd: *icd_ns,
            });
            seals.deal_delay = *deal_delay_ns;
        }
    }
    for effect in effects {
        if let Effect::SealOfRighteousness {
            ranks,
            hand_multiplier,
            swing_speed,
            deal_delay_ns,
        } = effect
        {
            for rank in ranks {
                seals.seals.push(Seal {
                    kind: SealKind::Righteousness,
                    spell: find(fight, rank.seal_spell_id, 0)?,
                    aura: fight.player_aura(&rank.aura)?,
                    judgement: find(fight, rank.judgement_spell_id, 0)?,
                    proc_spell: find(fight, rank.proc_spell_id, 0)?,
                    per_hit_value: rank.per_hit_value,
                    icd_timer: 0,
                });
            }
            seals.hand_multiplier = *hand_multiplier;
            seals.swing_speed = *swing_speed;
            seals.deal_delay = *deal_delay_ns;
        }
    }
    Ok(seals)
}

/// Go `Paladin.activeSeal`: the current seal while its aura is up.
pub(crate) fn active_seal(fight: &Fight<PaladinAgent>) -> Option<usize> {
    let current = fight.agent.seals.current?;
    fight
        .aura(fight.agent.seals.seals[current].aura)
        .active
        .then_some(current)
}

/// Go `Paladin.applySeal`: the replaced seal leaves its Echo, then the seal category drops
/// whatever seal was up as the new one activates; recasting the same seal refreshes it.
pub(crate) fn apply(fight: &mut Fight<PaladinAgent>, seal: usize) {
    if let Some(previous) = fight.agent.seals.current {
        let previous_aura = fight.agent.seals.seals[previous].aura;
        if previous != seal && fight.aura(previous_aura).active {
            let kind = fight.agent.seals.seals[previous].kind;
            super::super::talents::twist_of_light::leave_echo(fight, kind, previous);
        }
    }
    fight.agent.seals.current = Some(seal);
    let aura = fight.agent.seals.seals[seal].aura;
    // Go ExclusiveEffect.Activate: an equal bid from a fresh seal takes the category.
    if !fight.aura(aura).active {
        for other in 0..fight.agent.seals.seals.len() {
            let other_aura = fight.agent.seals.seals[other].aura;
            if other_aura != aura && fight.aura(other_aura).active {
                fight.deactivate_aura(other_aura);
            }
        }
    }
    fight.activate_aura(aura);
}

/// The seal aura's proc trigger: landed white hits that are not procs, handled a batch
/// window later.
pub(crate) fn on_spell_hit_dealt(
    fight: &mut Fight<PaladinAgent>,
    aura: AuraRef,
    spell: SpellId,
    result: &SpellResult,
) {
    let state = &fight.spells[spell];
    if state.flags.proc || !state.white_hit || result.outcome & OUTCOME_LANDED == 0 {
        return;
    }
    fight.schedule_delayed_proc(aura, spell, *result);
}

/// The delayed handler of a seal's trigger: Seal of Command rolls its proc, Seal of
/// Righteousness always fires.
pub(crate) fn on_delayed_proc(fight: &mut Fight<PaladinAgent>, seal: usize, target: Side) {
    match fight.agent.seals.seals[seal].kind {
        SealKind::Command => try_command(fight, seal, target),
        SealKind::Righteousness => {
            let spell = fight.agent.seals.seals[seal].proc_spell;
            fight.cast(spell, target);
        }
    }
}

/// Seal of Command's `tryProc`: the rank's cooldown, then the proc manager's roll.
pub(crate) fn try_command(fight: &mut Fight<PaladinAgent>, seal: usize, target: Side) {
    let command = fight
        .agent
        .seals
        .command
        .clone()
        .expect("Seal of Command is bound");
    let timer = fight.agent.seals.seals[seal].icd_timer;
    if fight.timers[timer] > fight.now || !fight.proc(command.chance, &command.label) {
        return;
    }
    fight.timers[timer] = fight.now + command.icd;
    let spell = fight.agent.seals.seals[seal].proc_spell;
    fight.cast(spell, target);
}

/// Go `dealAfterBatch`: the computed hit lands a batch window later, at low priority.
fn deal_after_batch(fight: &mut Fight<PaladinAgent>, spell: SpellId, result: SpellResult) {
    let at = fight.now + fight.agent.seals.deal_delay;
    fight.schedule(at, PRIORITY_LOW, Action::DelayedDamage { spell, result });
}

/// Seal of Command's proc: a share of weapon damage and spell power, plus the target's extra
/// Holy damage taken at the full coefficient, on the melee table.
pub(crate) fn command_proc(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side) {
    let command = fight
        .agent
        .seals
        .command
        .clone()
        .expect("Seal of Command is bound");
    let attack_power = fight.melee_attack_power();
    let weapon = fight.mh_weapon_damage(attack_power);
    let mut base =
        (weapon + command.coefficient * fight.spell_power(spell)) * command.weapon_percent;
    let holy = crate::core::fight::school_index(2);
    let target_bonus = fight.config.target_school_bonus_spell_damage[holy];
    base += command.coefficient * target_bonus;
    let result = fight.calc_damage_with(
        spell,
        target,
        base,
        Outcome::Table(PhysicalOutcome::MeleeSpecialHitAndCrit { count: true }),
    );
    deal_after_batch(fight, spell, result);
}

/// Seal of Righteousness's proc: the per-hit value scaled by the weapon's speed and hand.
pub(crate) fn righteousness_proc(
    fight: &mut Fight<PaladinAgent>,
    spell: SpellId,
    target: Side,
    seal: usize,
) {
    let seals = &fight.agent.seals;
    let value = seals.seals[seal].per_hit_value;
    let base = value / 100.0 * seals.hand_multiplier * seals.swing_speed;
    let result = fight.calc_damage_with(
        spell,
        target,
        base,
        Outcome::Table(PhysicalOutcome::MeleeSpecialCritOnly { count: true }),
    );
    deal_after_batch(fight, spell, result);
}
