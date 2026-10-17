//! Mage preparation: Go sim/mage's construction and initialization, and the exporter's Mage
//! description (tools/oracle-v2/mage.go).

mod export;
pub(crate) mod items;
pub(crate) mod masks;
mod spell_data;
mod spells;
mod talents;

use crate::contracts::request::Message;
use crate::prepare::agent::{fill_talents, ClassSpellName, PrepAgent};
use crate::prepare::sim::{Sim, UnitId};
use crate::prepare::stats::Stat;
use crate::prepare::Refusal;

/// Go mage.TalentTreeSizes.
const TALENT_TREE_SIZES: [usize; 3] = [19, 17, 19];

/// tools/oracle-v2/mage.go `mageClassSpells`.
static CLASS_SPELLS: &[ClassSpellName] = &[
    ClassSpellName {
        mask: masks::ARCANE_BLAST,
        name: "arcane_blast",
    },
    ClassSpellName {
        mask: masks::ARCANE_EXPLOSION,
        name: "arcane_explosion",
    },
    ClassSpellName {
        mask: masks::ARCANE_POWER,
        name: "arcane_power",
    },
    ClassSpellName {
        mask: masks::ARCANE_MISSILES_CAST,
        name: "arcane_missiles_cast",
    },
    ClassSpellName {
        mask: masks::ARCANE_MISSILES_TICK,
        name: "arcane_missiles_tick",
    },
    ClassSpellName {
        mask: masks::BLAST_WAVE,
        name: "blast_wave",
    },
    ClassSpellName {
        mask: masks::BLIZZARD,
        name: "blizzard",
    },
    ClassSpellName {
        mask: masks::COLD_SNAP,
        name: "cold_snap",
    },
    ClassSpellName {
        mask: masks::CONE_OF_COLD,
        name: "cone_of_cold",
    },
    ClassSpellName {
        mask: masks::EVOCATION,
        name: "evocation",
    },
    ClassSpellName {
        mask: masks::FIRE_BLAST,
        name: "fire_blast",
    },
    ClassSpellName {
        mask: masks::FIREBALL,
        name: "fireball",
    },
    ClassSpellName {
        mask: masks::FLAMESTRIKE,
        name: "flamestrike",
    },
    ClassSpellName {
        mask: masks::FLAMESTRIKE_DOT,
        name: "flamestrike_dot",
    },
    ClassSpellName {
        mask: masks::FROST_ARMOR,
        name: "frost_armor",
    },
    ClassSpellName {
        mask: masks::FROSTBOLT,
        name: "frostbolt",
    },
    ClassSpellName {
        mask: masks::FROST_NOVA,
        name: "frost_nova",
    },
    ClassSpellName {
        mask: masks::ICE_BARRIER,
        name: "ice_barrier",
    },
    ClassSpellName {
        mask: masks::ICE_BLOCK,
        name: "ice_block",
    },
    ClassSpellName {
        mask: masks::ICE_LANCE,
        name: "ice_lance",
    },
    ClassSpellName {
        mask: masks::IGNITE,
        name: "ignite",
    },
    ClassSpellName {
        mask: masks::MAGE_ARMOR,
        name: "mage_armor",
    },
    ClassSpellName {
        mask: masks::MANA_GEMS,
        name: "mana_gems",
    },
    ClassSpellName {
        mask: masks::MOLTEN_ARMOR,
        name: "molten_armor",
    },
    ClassSpellName {
        mask: masks::PRESENCE_OF_MIND,
        name: "presence_of_mind",
    },
    ClassSpellName {
        mask: masks::PYROBLAST,
        name: "pyroblast",
    },
    ClassSpellName {
        mask: masks::PYROBLAST_DOT,
        name: "pyroblast_dot",
    },
    ClassSpellName {
        mask: masks::SCORCH,
        name: "scorch",
    },
    ClassSpellName {
        mask: masks::MANA_GEM,
        name: "mana_gem",
    },
    ClassSpellName {
        mask: masks::COMBUSTION,
        name: "combustion",
    },
    ClassSpellName {
        mask: masks::IMPROVED_BLIZZARD,
        name: "improved_blizzard",
    },
    ClassSpellName {
        mask: masks::FROSTFIRE_BOLT,
        name: "frostfire_bolt",
    },
    ClassSpellName {
        mask: masks::ARCANE_WRATH,
        name: "arcane_wrath",
    },
];

/// Go `Mage`: the talents and the class option the registrations read.
#[derive(Clone)]
pub(crate) struct Mage {
    talents: Message,
    /// Go `Options.DefaultMageArmor`, as the enum's name.
    default_mage_armor: String,
}

/// Go `NewMage`.
pub(crate) fn new_mage(
    sim: &mut Sim,
    unit: UnitId,
    player: &Message,
) -> Result<Box<dyn PrepAgent>, Refusal> {
    // Go reads options.GetMage().Options.ClassOptions, and Initialize reads it without a check.
    let class_options = player
        .message("mage")
        .and_then(|mage| mage.message("options"))
        .and_then(|options| options.message("class_options"))
        .ok_or_else(|| {
            Refusal::new(
                "class_option",
                "a mage without its class options".to_string(),
            )
        })?;
    let default_mage_armor = class_options.enum_name("default_mage_armor");
    let talents = fill_talents(
        "proto.MageTalents",
        player.str("talents_string"),
        TALENT_TREE_SIZES,
    )
    .map_err(|err| Refusal::new("talents", err))?;
    sim.enable_mana_bar(unit);
    let crit_per_agi = sim.crit_per_agi_max_level(unit);
    sim.unit_mut(unit).sdm.add_stat_dependency(
        Stat::Agility,
        Stat::PhysicalCritPercent,
        crit_per_agi,
    );
    // Forever has no Water Elemental, so no pet is created.
    Ok(Box::new(Mage {
        talents,
        default_mage_armor,
    }))
}

impl PrepAgent for Mage {
    fn add_raid_buffs(&self, raid_buffs: &mut Message) {
        raid_buffs.set_bool("arcane_brilliance", true);
    }

    fn apply_talents(&mut self, sim: &mut Sim, unit: UnitId) {
        self.apply_mage_talents(sim, unit);
    }

    /// Go `Mage.Initialize`: the passives, then the spells.
    fn initialize(&mut self, sim: &mut Sim, unit: UnitId) {
        self.register_arcane_charges(sim, unit);
        self.register_spells(sim, unit);
    }

    /// The mage package's `core.NewItemEffect(19959, ...)`: Hazza'rah's Charm of Magic.
    fn apply_item_effect(&mut self, sim: &mut Sim, unit: UnitId, item: i32) -> bool {
        match item {
            19959 => {
                items::hazzarahs_charm(sim, unit);
                true
            }
            _ => false,
        }
    }

    /// arcane_charge.go: Arcane Blast's stacks give spell damage from Intellect, a stacking stat
    /// aura.
    fn stat_auras(&self, _sim: &Sim, _unit: UnitId) -> Vec<String> {
        if self.talents.bool("arcane_blast") {
            vec!["Arcane Blast".to_string()]
        } else {
            Vec::new()
        }
    }

    fn talents(&self) -> &Message {
        &self.talents
    }

    fn class_spells(&self) -> &'static [ClassSpellName] {
        CLASS_SPELLS
    }

    fn effects(&self, sim: &Sim, unit: UnitId) -> Vec<serde_json::Value> {
        export::effects(&self.talents, sim, unit)
    }

    /// `effects`, with the Intellect an iteration's end leaves for Arcane Blast's last log line:
    /// Go's doneIteration deactivates the player's auras in registration order, so those
    /// registered before Arcane Blast's have fallen when its stacks do.
    fn export_effects(
        &self,
        env: &crate::prepare::env::Environment,
        _notes: &mut Vec<String>,
    ) -> Vec<serde_json::Value> {
        let mut effects = self.effects(&env.sim, env.player);
        if let Some(effect) = effects
            .iter_mut()
            .find(|effect| effect["kind"] == "arcane_blast")
        {
            let mut fresh = env.fresh();
            let player = fresh.player;
            for aura in fresh.sim.unit(player).auras.clone() {
                if fresh.sim.aura(aura).label == "Arcane Blast" {
                    break;
                }
                if fresh.sim.aura(aura).active {
                    fresh.sim.deactivate(aura);
                }
            }
            effect["done_intellect"] = serde_json::json!(fresh.sim.stat(player, Stat::Intellect));
        }
        effects
    }

    /// Go `Mage.EurekaSpells`: the fifteen spells the client's Eureka! rows name for cost and
    /// damage, and the missile tick on the damage list; no spell is in Tick.
    fn eureka_spells(&self) -> Option<crate::prepare::racials::EurekaSpells> {
        let cost = masks::ARCANE_BLAST
            | masks::ARCANE_EXPLOSION
            | masks::ARCANE_MISSILES_CAST
            | masks::BLAST_WAVE
            | masks::BLIZZARD
            | masks::CONE_OF_COLD
            | masks::FIRE_BLAST
            | masks::FIREBALL
            | masks::FLAMESTRIKE
            | masks::FROST_NOVA
            | masks::FROSTBOLT
            | masks::FROSTFIRE_BOLT
            | masks::ICE_LANCE
            | masks::PYROBLAST
            | masks::SCORCH;
        Some(crate::prepare::racials::EurekaSpells {
            cost,
            damage: cost | masks::ARCANE_MISSILES_TICK,
            tick: 0,
        })
    }

    /// Go `spell.Matches(mage.MageSpellManaGem)`.
    fn is_mana_gem(&self, sim: &Sim, spell: crate::prepare::sim::SpellId) -> bool {
        sim.spell(spell).matches(masks::MANA_GEM)
    }

    fn damage_effect(
        &self,
        sim: &Sim,
        spell: crate::prepare::sim::SpellId,
    ) -> Option<serde_json::Value> {
        export::damage_effect(sim, spell)
    }
}
