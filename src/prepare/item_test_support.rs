//! Test support for the item effects: a character built the way Go's tests build one, a warrior
//! with melee and ranged auto attacks, whose swing speeds the proc managers read.

use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::{Message, Request};

use super::agent::{ClassSpellName, PrepAgent};
use super::attack::{AutoAttackOptions, Weapon};
use super::env::Environment;
use super::sim::{AuraId, RageBar, Sim, SpellId, UnitId};
use super::Refusal;

struct FakeAgent {
    talents: Message,
}

impl PrepAgent for FakeAgent {
    fn talents(&self) -> &Message {
        &self.talents
    }
    fn class_spells(&self) -> &'static [ClassSpellName] {
        &[]
    }
}

fn factory(sim: &mut Sim, unit: UnitId, _player: &Message) -> Result<Box<dyn PrepAgent>, Refusal> {
    sim.unit_mut(unit).rage_bar = RageBar {
        enabled: true,
        max_rage: 100.0,
    };
    let weapon = |speed: f64, min: f64, max: f64| Weapon {
        swing_speed: speed,
        base_damage_min: min,
        base_damage_max: max,
        ..Weapon::unarmed()
    };
    sim.enable_auto_attacks(
        unit,
        AutoAttackOptions {
            main_hand: weapon(2.6, 100.0, 150.0),
            off_hand: weapon(0.0, 0.0, 0.0),
            ranged: weapon(3.0, 10.0, 20.0),
            auto_swing_melee: true,
            auto_swing_ranged: true,
            ..AutoAttackOptions::default()
        },
    );
    Ok(Box::new(FakeAgent {
        talents: Message::empty("proto.WarriorTalents"),
    }))
}

/// A warrior wearing the items, each `(id, enchant)` in the request's order.
pub(crate) fn environment(items: &[(i32, i32)]) -> Environment {
    let listing: Vec<String> = items
        .iter()
        .map(|(id, enchant)| {
            if *enchant == 0 {
                format!(r#"{{"id":{id}}}"#)
            } else {
                format!(r#"{{"id":{id},"enchant":{enchant}}}"#)
            }
        })
        .collect();
    let request = format!(
        r#"{{"simOptions":{{"iterations":1,"randomSeed":"100"}},
        "raid":{{"parties":[{{"players":[{{"name":"Warrior","class":"ClassWarrior",
          "race":"RaceHuman","buffs":{{}},"consumables":{{}},
          "equipment":{{"items":[{}]}}}}]}}]}},
        "encounter":{{"duration":180,"targets":[{{"level":63,"mobType":"MobTypeElemental"}}]}}}}"#,
        listing.join(",")
    );
    let request = Request::from_json(request.as_bytes()).expect("a valid request");
    match Environment::new(request.message(), factory) {
        Ok(env) => env,
        Err(refusal) => panic!("{refusal}"),
    }
}

/// The player's aura with the label.
pub(crate) fn aura(env: &Environment, label: &str) -> AuraId {
    env.sim
        .get_aura(env.player, label)
        .unwrap_or_else(|| panic!("no aura {label}"))
}

/// The player's spell with the action.
pub(crate) fn spell(env: &Environment, action: &ActionId) -> SpellId {
    env.sim
        .get_spell(env.player, action)
        .unwrap_or_else(|| panic!("no spell {action}"))
}
