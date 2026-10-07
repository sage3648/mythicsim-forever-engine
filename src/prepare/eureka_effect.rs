//! tools/oracle-v2/main.go `eurekaEffect`: the Gnome racial's effect on the spellbook. The class
//! names its spells by masks only Go can read, so the exporter resolves them, with spell_mod.go
//! `shouldApply`'s rules, into spell positions.

use serde_json::{json, Value};

use super::env::Environment;
use super::racials::EurekaSpells;
use super::spell::{ProcMask, Resource, SpellFlag};

/// `eurekaEffect`: `None` for a player without the Eureka! aura.
pub(crate) fn eureka_effect(env: &Environment) -> Option<Value> {
    let sim = &env.sim;
    let player = env.player;
    let aura = sim.get_aura(player, "Eureka!")?;
    let masks = env.agent.eureka_spells().unwrap_or(EurekaSpells {
        cost: i64::MAX,
        damage: i64::MAX,
        tick: 0,
    });
    let class = sim.character(player).class.clone();
    let mut proc_mask = ProcMask::SPECIAL;
    if class == "ClassPriest" {
        proc_mask = proc_mask | ProcMask::SPELL_HEALING;
    }
    let spent = masks.cost | masks.damage | masks.tick;
    let direct_only = masks.damage & !masks.tick;
    let modded = |spell: super::sim::SpellId, mask: i64| {
        let spell = sim.spell(spell);
        mask != 0
            && !spell.flags.matches(SpellFlag::NO_SPELL_MODS)
            && spell.matches(mask)
            && proc_mask.matches(spell.proc_mask)
    };
    // The cost modifier names the class's resource: rage for a warrior, energy for a rogue and
    // mana otherwise.
    let pays_class_resource = |spell: super::sim::SpellId| match &sim.spell(spell).cost {
        None => false,
        Some(cost) => match cost.resource {
            Resource::Rage => class == "ClassWarrior",
            Resource::Energy => class == "ClassRogue",
            Resource::Mana => class != "ClassWarrior" && class != "ClassRogue",
            Resource::Focus => false,
        },
    };
    let (mut cost, mut damage, mut ticks, mut spending) = (vec![], vec![], vec![], vec![]);
    for (i, spell) in sim.unit(player).spellbook.iter().enumerate() {
        if pays_class_resource(*spell) && modded(*spell, masks.cost) {
            cost.push(i);
        }
        if modded(*spell, masks.damage | masks.tick) {
            damage.push(i);
        }
        if modded(*spell, direct_only) {
            ticks.push(i);
        }
        if sim.spell(*spell).matches(spent) && proc_mask.matches(sim.spell(*spell).proc_mask) {
            spending.push(i);
        }
    }
    let aura = sim.aura(aura);
    Some(json!({
        "kind": "eureka",
        "spell_id": aura.action_id.as_ref().map_or(0, |id| id.spell_id),
        "aura": aura.label,
        "cost_percent": -0.1,
        "damage_percent": 0.1,
        // Go's `1/1.1 - 1` is exact constant arithmetic, rounded once.
        "tick_cancel_percent": -1.0 / 11.0,
        "cost_spells": cost,
        "damage_spells": damage,
        "tick_cancel_spells": ticks,
        "spending_spells": spending,
    }))
}
