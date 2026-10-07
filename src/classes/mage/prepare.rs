//! Mage preparation: Go sim/mage's construction and initialization, and the exporter's Mage
//! description (tools/oracle-v2/mage.go).

use crate::contracts::request::Message;
use crate::prepare::agent::{fill_talents, ClassSpellName, PrepAgent};
use crate::prepare::sim::{Sim, UnitId};
use crate::prepare::stats::Stat;
use crate::prepare::Refusal;

/// Go mage.TalentTreeSizes.
const TALENT_TREE_SIZES: [usize; 3] = [18, 17, 19];

/// Go's Mage class mask bits, from mage.go.
pub(crate) mod masks {
    pub const ARCANE_BLAST: i64 = 1 << 1;
    pub const ARCANE_EXPLOSION: i64 = 1 << 2;
    pub const ARCANE_POWER: i64 = 1 << 3;
    pub const ARCANE_MISSILES_CAST: i64 = 1 << 4;
    pub const ARCANE_MISSILES_TICK: i64 = 1 << 5;
    pub const BLAST_WAVE: i64 = 1 << 6;
    pub const BLIZZARD: i64 = 1 << 7;
    pub const COLD_SNAP: i64 = 1 << 8;
    pub const CONE_OF_COLD: i64 = 1 << 9;
    pub const EVOCATION: i64 = 1 << 10;
    pub const FIRE_BLAST: i64 = 1 << 11;
    pub const FIREBALL: i64 = 1 << 12;
    pub const FLAMESTRIKE: i64 = 1 << 13;
    pub const FLAMESTRIKE_DOT: i64 = 1 << 14;
    pub const FROST_ARMOR: i64 = 1 << 15;
    pub const FROSTBOLT: i64 = 1 << 16;
    pub const FROST_NOVA: i64 = 1 << 17;
    pub const ICE_BARRIER: i64 = 1 << 18;
    pub const ICE_BLOCK: i64 = 1 << 19;
    pub const ICE_LANCE: i64 = 1 << 20;
    pub const IGNITE: i64 = 1 << 21;
    pub const MAGE_ARMOR: i64 = 1 << 22;
    pub const MANA_GEMS: i64 = 1 << 23;
    pub const MOLTEN_ARMOR: i64 = 1 << 24;
    pub const PRESENCE_OF_MIND: i64 = 1 << 25;
    pub const PYROBLAST: i64 = 1 << 26;
    pub const PYROBLAST_DOT: i64 = 1 << 27;
    pub const SCORCH: i64 = 1 << 28;
    pub const MANA_GEM: i64 = 1 << 29;
    pub const COMBUSTION: i64 = 1 << 30;
    pub const IMPROVED_BLIZZARD: i64 = 1 << 31;
    pub const FROSTFIRE_BOLT: i64 = 1 << 32;
}

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
];

/// Go `Mage`.
pub(crate) struct Mage {
    talents: Message,
}

/// Go `NewMage`.
pub(crate) fn new_mage(
    sim: &mut Sim,
    unit: UnitId,
    player: &Message,
) -> Result<Box<dyn PrepAgent>, Refusal> {
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
    Ok(Box::new(Mage { talents }))
}

impl PrepAgent for Mage {
    fn add_raid_buffs(&self, raid_buffs: &mut Message) {
        raid_buffs.set_bool("arcane_brilliance", true);
    }

    fn initialize(&mut self, _sim: &mut Sim, _unit: UnitId) {}

    fn talents(&self) -> &Message {
        &self.talents
    }

    fn class_spells(&self) -> &'static [ClassSpellName] {
        CLASS_SPELLS
    }
}
