//! The raid's target debuffs a member casts whose changes the fight applies as they come and go
//! (`RaidLive::applied_labels`): Faerie Fire, Curse of the Elements and Curse of Recklessness.
//! The raid's copy is registered but not up from the pull, so the prepared target holds none of
//! its changes. Each exclusive category it bids in is exported with every member's change while
//! its effect holds the category, the class's own copies included, and the fight applies the
//! holder's change as Go's effects' `OnGain` and `OnExpire` would.

use serde_json::{json, Map, Value};

use super::super::env::Environment;
use super::super::sim::{AuraId, CategoryId};
use super::super::stats::{Stat, SCHOOL_LEN};

/// Go school names by school index.
const SCHOOL_NAMES: [&str; SCHOOL_LEN] = [
    "none", "physical", "arcane", "fire", "frost", "holy", "nature", "shadow",
];

/// The resistance stats by school index.
const RESISTANCES: [(Stat, usize); 5] = [
    (Stat::ArcaneResistance, 2),
    (Stat::FireResistance, 3),
    (Stat::FrostResistance, 4),
    (Stat::NatureResistance, 6),
    (Stat::ShadowResistance, 7),
];

/// The target's categories that hold a raid debuff whose changes the fight applies, each with
/// every member's change, in the order of the debuffs and their effects. A category none of
/// whose members changes armor, a resistance or a school's damage taken, such as one of attack
/// power, which the target swing's export reads, is left out.
pub(crate) fn raid_debuff_categories(
    env: &Environment,
    unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    let target = env.encounter.targets[0];
    let mut names: Vec<String> = Vec::new();
    for label in env.raid_live.applied_labels() {
        let Some(aura) = env.sim.get_aura(target, label) else {
            continue;
        };
        for effect in &env.sim.aura(aura).exclusive_effects {
            let name = &env.sim.categories[env.sim.effects[effect.0].category.0].name;
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    let mut out = Vec::new();
    for name in names {
        let Some(category) = category_on_target(env, &name) else {
            continue;
        };
        let mut members = Vec::new();
        let mut changes = false;
        for effect in env.sim.categories[category.0].effects.clone() {
            let label = env.sim.aura(env.sim.effects[effect.0].aura).label.clone();
            let member = member_change(env, &name, &label, unrepresented);
            changes |= member.len() > 1;
            members.push(Value::Object(member));
        }
        if changes {
            out.push(json!({
                "kind": "raid_debuff_category", "category": name, "members": members,
            }));
        }
    }
    out
}

/// Whether the fight applies the aura's changes through a raid debuff category, so its class
/// leaves them alone.
pub(crate) fn raid_applies(env: &Environment, aura: AuraId) -> bool {
    if env.raid_live.applied_labels().next().is_none() {
        return false;
    }
    let mut scratch = Vec::new();
    let described = raid_debuff_categories(env, &mut scratch);
    let label = &env.sim.aura(aura).label;
    described.iter().any(|category| {
        category["members"]
            .as_array()
            .is_some_and(|members| members.iter().any(|member| member["aura"] == **label))
    })
}

fn category_on_target(env: &Environment, name: &str) -> Option<CategoryId> {
    let target = env.encounter.targets[0];
    env.sim
        .categories
        .iter()
        .position(|category| category.unit == target && category.name == name)
        .map(CategoryId)
}

/// One member's change while its effect alone holds the category, from a separate reset
/// simulation: the effect holding it at the reset gives it up first, as Go's `OnExpire` does,
/// then the member's `OnGain` runs. Armor, resistances and school damage taken are exported;
/// attack power is the target swing's (`enemy`); anything else is unrepresented.
fn member_change(
    env: &Environment,
    category: &str,
    label: &str,
    unrepresented: &mut Vec<String>,
) -> Map<String, Value> {
    let mut member = Map::new();
    member.insert("aura".into(), json!(label));
    let mut fresh = env.fresh();
    let target = fresh.encounter.targets[0];
    let Some(id) = category_on_target(&fresh, category) else {
        return member;
    };
    let effect = fresh.sim.categories[id.0]
        .effects
        .iter()
        .copied()
        .find(|effect| fresh.sim.aura(fresh.sim.effects[effect.0].aura).label == label)
        .expect("the member is in every reset");
    if let Some(active) = fresh.sim.categories[id.0].active_effect {
        if let Some(expire) = fresh.sim.effects[active.0].on_expire.clone() {
            expire(&mut fresh.sim, active);
        }
    }
    let before = fresh.sim.stats(target);
    let before_pseudo = fresh.sim.unit(target).pseudo_stats.clone();
    if let Some(gain) = fresh.sim.effects[effect.0].on_gain.clone() {
        gain(&mut fresh.sim, effect);
    }
    let after = fresh.sim.stats(target);
    let mut after_pseudo = fresh.sim.unit(target).pseudo_stats.clone();
    let armor = after[Stat::Armor] - before[Stat::Armor];
    if armor != 0.0 {
        member.insert("armor".into(), json!(armor));
    }
    let mut resistance = Map::new();
    for (stat, school) in RESISTANCES {
        let delta = after[stat] - before[stat];
        if delta != 0.0 {
            resistance.insert(SCHOOL_NAMES[school].into(), json!(delta));
        }
    }
    if !resistance.is_empty() {
        member.insert("resistance".into(), Value::Object(resistance));
    }
    let mut multipliers = Map::new();
    for (school, name) in SCHOOL_NAMES.iter().enumerate() {
        let (was, is) = (
            before_pseudo.school_damage_taken_multiplier[school],
            after_pseudo.school_damage_taken_multiplier[school],
        );
        if is != was {
            multipliers.insert((*name).into(), json!(is / was));
        }
    }
    if !multipliers.is_empty() {
        member.insert(
            "school_damage_taken_multiplier".into(),
            Value::Object(multipliers),
        );
    }
    for stat in Stat::ALL {
        let known = stat == Stat::Armor
            || stat == Stat::AttackPower
            || RESISTANCES
                .iter()
                .any(|(resistance, _)| *resistance == stat);
        if !known && after[stat] != before[stat] {
            unrepresented.push(format!("{label} changes target {} in a raid", stat.name()));
        }
    }
    after_pseudo.school_damage_taken_multiplier = before_pseudo.school_damage_taken_multiplier;
    if after_pseudo != before_pseudo {
        unrepresented.push(format!("{label} changes target pseudo stats in a raid"));
    }
    member
}
