//! The Shadowfiend (401977), from Go sim/priest/shadowfiend.go and shadowfiend_pet.go: the
//! summon enables the pet for its timeline aura's duration and activates that aura. Each enable
//! inherits attack power from the priest's spell damage and shadow damage and activates the
//! pet's mana restore aura, whose landed hits give the priest a share of maximum mana. The pet
//! itself is the core's summoned [`ActivePet`], swinging a Shadow weapon with no rotation.
//!
//! [`ActivePet`]: crate::core::fight::pet::ActivePet

use std::cell::Cell;

use crate::{
    contracts::prepared_v2::{ActionId, NamedStat},
    core::fight::{Agent, AuraRef, Fight, Side, SpellResult, OUTCOME_LANDED},
};

/// The position of Shadow among the runtime's spell schools.
const SHADOW: usize = 7;

#[derive(Debug)]
pub(crate) struct Shadowfiend {
    /// The priest's timeline aura, which the summon activates.
    aura: AuraRef,
    duration: i64,
    /// The simulated pet that is the Shadowfiend.
    pet: Side,
    restore_aura: AuraRef,
    restore_fraction: f64,
    restore_metrics: usize,
    coefficient: f64,
    /// Go `statsWithoutDeps[AttackPower]`, which each summon adds the inheritance to and each
    /// dismissal subtracts it from.
    attack_power_without_deps: Cell<f64>,
    /// What the pet's dependencies add to its attack power, in order.
    dependency_terms: Vec<f64>,
    /// The pet's stats in Go's order, which its summon and dismissal lines print.
    stats: Vec<NamedStat>,
}

impl Shadowfiend {
    /// Go `ApplyStatDependencies` for the attack power: each term added in order.
    fn attack_power(&self, without_deps: f64) -> f64 {
        self.dependency_terms
            .iter()
            .fold(without_deps, |power, term| power + term)
    }

    /// Go `Stats.FlatString` of the pet with this attack power.
    fn stats_line(&self, attack_power: f64) -> String {
        let mut line = String::from("{");
        for stat in &self.stats {
            let value = if stat.stat == "AttackPower" {
                attack_power
            } else {
                stat.value
            };
            if value != 0.0 {
                line.push_str(&format!("\"{}\": {value:.3},", stat.stat));
            }
        }
        line.push('}');
        line
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    duration: i64,
    pet: &str,
    restore_aura: &str,
    restore_fraction: f64,
    restore_action_id: i32,
    coefficient: f64,
    attack_power_without_deps: f64,
    dependency_terms: &[f64],
    stats: &[NamedStat],
) -> Result<Shadowfiend, String> {
    let Some(side) = fight
        .pet_side(pet)
        .filter(|&side| fight.active_pet(side).summoned)
    else {
        return Err(format!("{pet} is not a simulated summoned pet"));
    };
    let aura = fight.player_aura(aura)?;
    let restore_aura = fight.trackers[side.index()]
        .find(restore_aura)
        .map(|index| AuraRef { side, index })
        .ok_or_else(|| format!("pet aura {restore_aura} is not registered"))?;
    if !stats.iter().any(|stat| stat.stat == "AttackPower") {
        return Err("the Shadowfiend's stats name no attack power".into());
    }
    let restore_metrics = fight.new_mana_metrics(ActionId {
        spell_id: restore_action_id,
        ..ActionId::default()
    });
    Ok(Shadowfiend {
        aura,
        duration,
        pet: side,
        restore_aura,
        restore_fraction,
        restore_metrics,
        coefficient,
        attack_power_without_deps: Cell::new(attack_power_without_deps),
        dependency_terms: dependency_terms.to_vec(),
        stats: stats.to_vec(),
    })
}

/// The summon's `ApplyEffects`: Go `EnableWithTimeout`, then the timeline aura.
pub(crate) fn summon<A: Agent>(fight: &mut Fight<A>, shadowfiend: &Shadowfiend) {
    let enabled = fight.active_pet(shadowfiend.pet).enabled;
    if !enabled {
        // Go Enable: inheritOwnerStats, then OnPetEnable before the swing starts. Disable
        // later subtracts the same inheritance.
        let inherited = (fight.player.powers.spell_damage + fight.player_school_damage()[SHADOW])
            * shadowfiend.coefficient;
        let without = shadowfiend.attack_power_without_deps.get() + inherited;
        let attack_power = shadowfiend.attack_power(without);
        let dismissed = without - inherited;
        shadowfiend.attack_power_without_deps.set(dismissed);
        let pet = fight.active_pet_mut(shadowfiend.pet);
        pet.state.powers.attack_power = attack_power;
        let inherited_line = if inherited != 0.0 {
            format!("{{\"AttackPower\": {inherited:.3},}}")
        } else {
            "{}".to_string()
        };
        pet.summon_log = vec![
            format!("Pet stats: {}", shadowfiend.stats_line(attack_power)),
            format!("Pet inherited stats: {inherited_line}"),
        ];
        pet.dismiss_log = shadowfiend.stats_line(shadowfiend.attack_power(dismissed));
        fight.activate_aura(shadowfiend.restore_aura);
    }
    fight.summon_pet(shadowfiend.pet, shadowfiend.duration);
    fight.activate_aura(shadowfiend.aura);
}

/// The mana restore aura's handler, a spell batch window after each hit: a landed hit gives
/// the priest its share of maximum mana.
pub(crate) fn restore<A: Agent>(
    fight: &mut Fight<A>,
    shadowfiend: &Shadowfiend,
    result: &SpellResult,
) {
    if result.outcome & OUTCOME_LANDED == 0 {
        return;
    }
    let amount = fight.player.powers.max_mana * shadowfiend.restore_fraction;
    fight.add_mana(amount, shadowfiend.restore_metrics);
}
