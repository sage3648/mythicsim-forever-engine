//! Go spell registration (sim/core/spell.go, cast.go, dot.go and the resource costs): the
//! fields a registered spell carries after its static modifiers.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;

use super::sim::{AuraConfig, AuraId, Cooldown, DotId, Duration, Sim, SpellId, UnitId, UnitType};
use super::stats::SchoolIndex;

/// Go `SpellFlag` bits, by exported name in bit order.
pub(crate) const SPELL_FLAG_NAMES: [&str; 40] = [
    "SpellFlagIgnoreResists",
    "SpellFlagIgnoreTargetModifiers",
    "SpellFlagIgnoreAttackerModifiers",
    "SpellFlagApplyArmorReduction",
    "SpellFlagCannotBeDodged",
    "SpellFlagIncludeTargetBonusDamage",
    "SpellFlagBinary",
    "SpellFlagChanneled",
    "SpellFlagDisease",
    "SpellFlagPoison",
    "SpellFlagHauntSE",
    "SpellFlagHelpful",
    "SpellFlagMeleeMetrics",
    "SpellFlagNoOnCastComplete",
    "SpellFlagNoMetrics",
    "SpellFlagNoLogs",
    "SpellFlagAPL",
    "SpellFlagMCD",
    "SpellFlagReactive",
    "SpellFlagNoOnDamageDealt",
    "SpellFlagPrepullOnly",
    "SpellFlagEncounterOnly",
    "SpellFlagPotion",
    "SpellFlagConjured",
    "SpellFlagExplosive",
    "SpellFlagCombatPotion",
    "SpellFlagNoSpellMods",
    "SpellFlagCanCastWhileMoving",
    "SpellFlagPassiveSpell",
    "SpellFlagSuppressWeaponProcs",
    "SpellFlagProc",
    "SpellFlagSupressDoTApply",
    "SpellFlagSwapped",
    "SpellFlagCastWhileIncapacitated",
    "SpellFlagPushback",
    "SpellFlagAgentReserved1",
    "SpellFlagAgentReserved2",
    "SpellFlagAgentReserved3",
    "SpellFlagAgentReserved4",
    "",
];

/// Go `SpellFlag`: bit `i` is `1 << (i + 1)`, as Go's iota starts the list at its second line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpellFlag(pub u64);

#[allow(dead_code)]
impl SpellFlag {
    pub const NONE: SpellFlag = SpellFlag(0);
    pub const IGNORE_RESISTS: SpellFlag = SpellFlag(1 << 1);
    pub const IGNORE_TARGET_MODIFIERS: SpellFlag = SpellFlag(1 << 2);
    pub const IGNORE_ATTACKER_MODIFIERS: SpellFlag = SpellFlag(1 << 3);
    pub const APPLY_ARMOR_REDUCTION: SpellFlag = SpellFlag(1 << 4);
    pub const CANNOT_BE_DODGED: SpellFlag = SpellFlag(1 << 5);
    pub const INCLUDE_TARGET_BONUS_DAMAGE: SpellFlag = SpellFlag(1 << 6);
    pub const BINARY: SpellFlag = SpellFlag(1 << 7);
    pub const CHANNELED: SpellFlag = SpellFlag(1 << 8);
    pub const DISEASE: SpellFlag = SpellFlag(1 << 9);
    pub const POISON: SpellFlag = SpellFlag(1 << 10);
    pub const HAUNT_SE: SpellFlag = SpellFlag(1 << 11);
    pub const HELPFUL: SpellFlag = SpellFlag(1 << 12);
    pub const MELEE_METRICS: SpellFlag = SpellFlag(1 << 13);
    pub const NO_ON_CAST_COMPLETE: SpellFlag = SpellFlag(1 << 14);
    pub const NO_METRICS: SpellFlag = SpellFlag(1 << 15);
    pub const NO_LOGS: SpellFlag = SpellFlag(1 << 16);
    pub const APL: SpellFlag = SpellFlag(1 << 17);
    pub const MCD: SpellFlag = SpellFlag(1 << 18);
    pub const REACTIVE: SpellFlag = SpellFlag(1 << 19);
    pub const NO_ON_DAMAGE_DEALT: SpellFlag = SpellFlag(1 << 20);
    pub const PREPULL_ONLY: SpellFlag = SpellFlag(1 << 21);
    pub const ENCOUNTER_ONLY: SpellFlag = SpellFlag(1 << 22);
    pub const POTION: SpellFlag = SpellFlag(1 << 23);
    pub const CONJURED: SpellFlag = SpellFlag(1 << 24);
    pub const EXPLOSIVE: SpellFlag = SpellFlag(1 << 25);
    pub const COMBAT_POTION: SpellFlag = SpellFlag(1 << 26);
    pub const NO_SPELL_MODS: SpellFlag = SpellFlag(1 << 27);
    pub const CAN_CAST_WHILE_MOVING: SpellFlag = SpellFlag(1 << 28);
    pub const PASSIVE_SPELL: SpellFlag = SpellFlag(1 << 29);
    pub const SUPPRESS_WEAPON_PROCS: SpellFlag = SpellFlag(1 << 30);
    pub const PROC: SpellFlag = SpellFlag(1 << 31);
    pub const SUPRESS_DOT_APPLY: SpellFlag = SpellFlag(1 << 32);
    pub const SWAPPED: SpellFlag = SpellFlag(1 << 33);
    pub const CAST_WHILE_INCAPACITATED: SpellFlag = SpellFlag(1 << 34);
    pub const PUSHBACK: SpellFlag = SpellFlag(1 << 35);
    pub const AGENT_RESERVED1: SpellFlag = SpellFlag(1 << 36);
    pub const AGENT_RESERVED2: SpellFlag = SpellFlag(1 << 37);
    pub const AGENT_RESERVED3: SpellFlag = SpellFlag(1 << 38);
    pub const AGENT_RESERVED4: SpellFlag = SpellFlag(1 << 39);
    pub const IGNORE_MODIFIERS: SpellFlag =
        SpellFlag(Self::IGNORE_ATTACKER_MODIFIERS.0 | Self::IGNORE_TARGET_MODIFIERS.0);

    pub fn matches(self, other: SpellFlag) -> bool {
        self.0 & other.0 != 0
    }

    /// The flag names the prepared contract lists, in bit order.
    pub fn names(self) -> Vec<String> {
        (0u32..64)
            .filter(|bit| self.0 & (1u64 << bit) != 0)
            .map(|bit| flag_name(bit).to_string())
            .collect()
    }
}

/// Go's stringer name for a single flag bit.
fn flag_name(bit: u32) -> String {
    match bit
        .checked_sub(1)
        .and_then(|index| SPELL_FLAG_NAMES.get(index as usize))
    {
        Some(name) if !name.is_empty() => name.to_string(),
        _ => format!("SpellFlag({})", 1u64 << bit),
    }
}

impl std::ops::BitOr for SpellFlag {
    type Output = SpellFlag;
    fn bitor(self, other: SpellFlag) -> SpellFlag {
        SpellFlag(self.0 | other.0)
    }
}

impl std::ops::BitOrAssign for SpellFlag {
    fn bitor_assign(&mut self, other: SpellFlag) {
        self.0 |= other.0;
    }
}

/// Go `ProcMask`: bit `i` is `1 << (i + 1)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProcMask(pub u32);

pub(crate) const PROC_MASK_NAMES: [&str; 10] = [
    "ProcMaskEmpty",
    "ProcMaskMeleeMHAuto",
    "ProcMaskMeleeOHAuto",
    "ProcMaskMeleeMHSpecial",
    "ProcMaskMeleeOHSpecial",
    "ProcMaskRangedAuto",
    "ProcMaskRangedSpecial",
    "ProcMaskSpellDamage",
    "ProcMaskSpellHealing",
    "ProcMaskSpellDamageProc",
];

#[allow(dead_code)]
impl ProcMask {
    pub const UNKNOWN: ProcMask = ProcMask(0);
    pub const EMPTY: ProcMask = ProcMask(1 << 1);
    pub const MELEE_MH_AUTO: ProcMask = ProcMask(1 << 2);
    pub const MELEE_OH_AUTO: ProcMask = ProcMask(1 << 3);
    pub const MELEE_MH_SPECIAL: ProcMask = ProcMask(1 << 4);
    pub const MELEE_OH_SPECIAL: ProcMask = ProcMask(1 << 5);
    pub const RANGED_AUTO: ProcMask = ProcMask(1 << 6);
    pub const RANGED_SPECIAL: ProcMask = ProcMask(1 << 7);
    pub const SPELL_DAMAGE: ProcMask = ProcMask(1 << 8);
    pub const SPELL_HEALING: ProcMask = ProcMask(1 << 9);
    pub const SPELL_DAMAGE_PROC: ProcMask = ProcMask(1 << 10);
    pub const MELEE_MH: ProcMask = ProcMask(Self::MELEE_MH_AUTO.0 | Self::MELEE_MH_SPECIAL.0);
    pub const MELEE_OH: ProcMask = ProcMask(Self::MELEE_OH_AUTO.0 | Self::MELEE_OH_SPECIAL.0);
    pub const MELEE_WHITE_HIT: ProcMask = ProcMask(Self::MELEE_MH_AUTO.0 | Self::MELEE_OH_AUTO.0);
    pub const WHITE_HIT: ProcMask =
        ProcMask(Self::MELEE_MH_AUTO.0 | Self::MELEE_OH_AUTO.0 | Self::RANGED_AUTO.0);
    pub const MELEE_SPECIAL: ProcMask =
        ProcMask(Self::MELEE_MH_SPECIAL.0 | Self::MELEE_OH_SPECIAL.0);
    pub const MELEE_OR_RANGED_SPECIAL: ProcMask =
        ProcMask(Self::MELEE_SPECIAL.0 | Self::RANGED_SPECIAL.0);
    pub const MELEE: ProcMask = ProcMask(Self::MELEE_WHITE_HIT.0 | Self::MELEE_SPECIAL.0);
    pub const RANGED: ProcMask = ProcMask(Self::RANGED_AUTO.0 | Self::RANGED_SPECIAL.0);
    pub const MELEE_OR_RANGED: ProcMask = ProcMask(Self::MELEE.0 | Self::RANGED.0);
    pub const DIRECT: ProcMask = ProcMask(Self::MELEE.0 | Self::RANGED.0 | Self::SPELL_DAMAGE.0);
    pub const SPECIAL: ProcMask = ProcMask(Self::MELEE_OR_RANGED_SPECIAL.0 | Self::SPELL_DAMAGE.0);

    pub fn matches(self, other: ProcMask) -> bool {
        self.0 & other.0 != 0
    }

    pub fn names(self) -> Vec<String> {
        (0u32..32)
            .filter(|bit| self.0 & (1u32 << bit) != 0)
            .map(|bit| {
                match bit
                    .checked_sub(1)
                    .and_then(|i| PROC_MASK_NAMES.get(i as usize))
                {
                    Some(name) => name.to_string(),
                    None => format!("ProcMask({})", 1u32 << bit),
                }
            })
            .collect()
    }
}

impl std::ops::BitOr for ProcMask {
    type Output = ProcMask;
    fn bitor(self, other: ProcMask) -> ProcMask {
        ProcMask(self.0 | other.0)
    }
}

/// Go `SpellSchool` bits, the client's.
#[allow(dead_code)]
pub(crate) mod school {
    pub const NONE: u8 = 0;
    pub const PHYSICAL: u8 = 1;
    pub const HOLY: u8 = 2;
    pub const FIRE: u8 = 4;
    pub const NATURE: u8 = 8;
    pub const FROST: u8 = 16;
    pub const SHADOW: u8 = 32;
    pub const ARCANE: u8 = 64;
    pub const FROSTFIRE: u8 = FIRE | FROST;
}

/// Go `DefenseType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DefenseType {
    #[default]
    None,
    Magic,
    Melee,
    Ranged,
}

impl DefenseType {
    pub fn name(self) -> &'static str {
        match self {
            DefenseType::None => "DefenseTypeNone",
            DefenseType::Magic => "DefenseTypeMagic",
            DefenseType::Melee => "DefenseTypeMelee",
            DefenseType::Ranged => "DefenseTypeRanged",
        }
    }

    pub fn from_client(value: u8) -> DefenseType {
        match value {
            1 => DefenseType::Magic,
            2 => DefenseType::Melee,
            3 => DefenseType::Ranged,
            _ => DefenseType::None,
        }
    }
}

/// Go `Cast`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Cast {
    pub cost: f64,
    pub gcd: Duration,
    pub gcd_min: Duration,
    pub cast_time: Duration,
    pub non_empty: bool,
}

impl Cast {
    pub fn is_empty(&self) -> bool {
        *self == Cast::default()
    }
}

/// Go `CastConfig`, without the closures preparation never runs.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CastConfig {
    pub default_cast: Cast,
    pub ignore_haste: bool,
    pub cd: Cooldown,
    pub shared_cd: Cooldown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resource {
    Mana,
    Energy,
    Rage,
    Focus,
}

impl Resource {
    pub fn name(self) -> &'static str {
        match self {
            Resource::Mana => "mana",
            Resource::Energy => "energy",
            Resource::Rage => "rage",
            Resource::Focus => "focus",
        }
    }
}

/// Go `SpellCost`.
#[derive(Clone, Debug)]
pub(crate) struct SpellCost {
    pub resource: Resource,
    pub base_cost: i32,
    pub flat_modifier: i32,
    pub percent_modifier: f64,
    pub additive_percent_modifier: f64,
    pub refund: f64,
    /// Whether a refund goes to metrics other than the bar's default.
    pub refund_to_own_metrics: bool,
}

/// The cost options of a spell config: Go's ManaCost, EnergyCost, RageCost and FocusCost.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CostOptions {
    pub mana_base_cost_percent: f64,
    pub mana_flat_cost: i32,
    pub mana_percent_modifier: f64,
    pub energy_cost: i32,
    pub energy_refund: f64,
    pub rage_cost: i32,
    pub rage_refund: f64,
    pub focus_cost: i32,
    pub focus_refund: f64,
}

/// Go `DotConfig` for what preparation exports.
#[derive(Clone, Default)]
pub(crate) struct DotConfig {
    /// The spell the dot belongs to, when not the one registering it.
    pub spell: Option<SpellId>,
    pub aura: AuraConfig,
    pub tick_length: Duration,
    pub number_of_ticks: i32,
    pub is_aoe: bool,
    pub self_only: bool,
    pub affected_by_cast_speed: bool,
    pub affected_by_real_haste: bool,
    pub haste_reduces_duration: bool,
    pub bonus_coefficient: f64,
    pub periodic_damage_multiplier: f64,
}

impl DotConfig {
    pub fn is_set(&self) -> bool {
        self.number_of_ticks != 0 || self.tick_length != 0
    }
}

/// Go `Dot`.
pub(crate) struct Dot {
    pub spell: SpellId,
    pub aura: AuraId,
    pub base_tick_count: i32,
    pub base_tick_length: Duration,
    pub bonus_coefficient: f64,
    pub periodic_damage_multiplier: f64,
    pub base_duration_multiplier: f64,
    pub base_duration_flat: Duration,
    pub affected_by_cast_speed: bool,
    pub affected_by_real_haste: bool,
    pub haste_reduces_duration: bool,
    pub is_channeled: bool,
}

/// Go `SpellConfig`, without the closures preparation never runs; their presence is kept
/// where Go's registration or the export reads it.
#[derive(Clone, Default)]
pub(crate) struct SpellConfig {
    pub action_id: ActionId,
    pub spell_school: u8,
    pub defense_type: DefenseType,
    pub proc_mask: ProcMask,
    pub flags: SpellFlag,
    pub missile_speed: f64,
    pub metric_splits: usize,
    pub class_spell_mask: i64,
    pub class_flags: crate::data::spells::ClassFlags,
    pub rank: i32,
    pub cost: CostOptions,
    pub cast: CastConfig,
    pub has_extra_cast_condition: bool,
    pub has_cast_requirement: bool,
    pub min_range: f64,
    pub max_range: f64,
    pub charges: i32,
    pub recharge_time: Duration,
    pub bonus_hit_percent: f64,
    pub bonus_crit_percent: f64,
    pub bonus_spell_damage: f64,
    pub bonus_expertise_percent: f64,
    pub damage_multiplier: f64,
    pub damage_multiplier_additive: f64,
    pub crit_multiplier_additive: f64,
    pub bonus_base_damage: f64,
    pub bonus_coefficient: f64,
    pub threat_multiplier: f64,
    pub flat_threat_bonus: f64,
    pub dot: DotConfig,
    pub hot: DotConfig,
    pub related_dot_spell: Option<SpellId>,
    pub related_self_buff: Option<AuraId>,
    pub related_aura_arrays: LabeledAuraArrays,
}

/// Go `LabeledAuraArrays`: per label, the aura on each target by unit index. Kept ordered so
/// iteration is deterministic.
pub(crate) type LabeledAuraArrays = std::collections::BTreeMap<String, Vec<Option<AuraId>>>;

pub(crate) type SpellRegisteredHandler = Rc<dyn Fn(&mut Sim, SpellId)>;

/// A registered spell: Go `Spell`.
pub(crate) struct Spell {
    pub action_id: ActionId,
    pub rank: i32,
    pub unit: UnitId,
    pub spell_school: u8,
    pub school_index: SchoolIndex,
    pub defense_type: DefenseType,
    pub proc_mask: ProcMask,
    pub flags: SpellFlag,
    pub class_spell_mask: i64,
    pub class_flags: crate::data::spells::ClassFlags,
    pub missile_speed: f64,
    pub cost: Option<SpellCost>,
    pub default_cast: Cast,
    pub cd: Cooldown,
    pub shared_cd: Cooldown,
    pub ignore_haste: bool,
    pub has_extra_cast_condition: bool,
    pub has_cast_requirement: bool,
    pub min_range: f64,
    pub max_range: f64,
    pub max_charges: i32,
    /// Go's unexported `charges`: the charges the spell has now.
    pub charges: i32,
    pub recharge_time: Duration,
    pub cast_kind: CastKind,
    pub metric_splits: usize,
    pub pushback_resist: f64,
    pub bonus_hit_percent: f64,
    pub bonus_crit_percent: f64,
    pub bonus_spell_damage: f64,
    pub bonus_expertise_percent: f64,
    pub cast_time_multiplier: f64,
    pub cd_multiplier: f64,
    pub damage_multiplier: f64,
    pub damage_multiplier_additive: f64,
    pub direct_damage_multiplier_additive: f64,
    pub crit_multiplier_pct: f64,
    pub crit_multiplier_additive: f64,
    pub bonus_base_damage: f64,
    pub bonus_coefficient: f64,
    pub threat_multiplier: f64,
    pub flat_threat_bonus: f64,
    /// Per target unit index, the dot on that unit.
    pub dots: Vec<Option<DotId>>,
    pub aoe_dot: Option<DotId>,
    pub related_dot_spell: Option<SpellId>,
    pub related_self_buff: Option<AuraId>,
    pub related_aura_arrays: LabeledAuraArrays,
}

/// How Go casts the spell, chosen at registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CastKind {
    Full,
    Simple,
    AutosOrProcs,
}

impl CastKind {
    pub fn name(self) -> &'static str {
        match self {
            CastKind::Full => "full",
            CastKind::Simple => "simple",
            CastKind::AutosOrProcs => "autos_or_procs",
        }
    }
}

impl Spell {
    /// Go `spell.Matches(mask)`.
    pub fn matches(&self, mask: i64) -> bool {
        self.class_spell_mask & mask != 0
    }
}

impl Sim {
    pub(crate) fn spell(&self, id: SpellId) -> &Spell {
        &self.spells[id.0]
    }

    pub(crate) fn spell_mut(&mut self, id: SpellId) -> &mut Spell {
        &mut self.spells[id.0]
    }

    /// Go `unit.RegisterSpell`.
    pub(crate) fn register_spell(&mut self, unit: UnitId, mut config: SpellConfig) -> SpellId {
        assert!(
            self.unit(unit).spellbook.len() <= 1000,
            "Over 1000 registered spells when registering {}",
            config.action_id
        );
        if config.damage_multiplier != 0.0 && config.damage_multiplier_additive == 0.0 {
            config.damage_multiplier_additive = 1.0;
        } else if config.damage_multiplier_additive != 0.0 && config.damage_multiplier == 0.0 {
            config.damage_multiplier = 1.0;
        }
        if (config.damage_multiplier != 0.0 || config.threat_multiplier != 0.0)
            && config.proc_mask == ProcMask::UNKNOWN
        {
            panic!("ProcMask for spell {} not set", config.action_id);
        }
        if (config.damage_multiplier != 0.0 || config.threat_multiplier != 0.0)
            && config.spell_school == school::NONE
        {
            panic!("SpellSchool for spell {} not set", config.action_id);
        }
        let cast = config.cast;
        assert!(
            (cast.cd.timer.is_some()) == (cast.cd.duration != 0),
            "Cast.CD timer and duration disagree for spell {}",
            config.action_id
        );
        assert!(
            (cast.shared_cd.timer.is_some()) == (cast.shared_cd.duration != 0),
            "Cast.SharedCD timer and duration disagree for spell {}",
            config.action_id
        );
        assert!(
            config.charges == 0 || config.recharge_time != 0,
            "Spell has charges but no recharge time."
        );
        let id = SpellId(self.spells.len());
        let mut spell = Spell {
            action_id: config.action_id.clone(),
            rank: config.rank,
            unit,
            spell_school: config.spell_school,
            school_index: Sim::school_index(config.spell_school),
            defense_type: config.defense_type,
            proc_mask: config.proc_mask,
            flags: config.flags,
            class_spell_mask: config.class_spell_mask,
            class_flags: config.class_flags,
            missile_speed: config.missile_speed,
            cost: None,
            default_cast: cast.default_cast,
            cd: cast.cd,
            shared_cd: cast.shared_cd,
            ignore_haste: cast.ignore_haste,
            has_extra_cast_condition: config.has_extra_cast_condition,
            has_cast_requirement: config.has_cast_requirement,
            min_range: 0.0,
            max_range: 0.0,
            max_charges: config.charges,
            charges: config.charges,
            recharge_time: config.recharge_time,
            cast_kind: CastKind::Full,
            metric_splits: config.metric_splits.max(1),
            pushback_resist: 0.0,
            bonus_hit_percent: config.bonus_hit_percent,
            bonus_crit_percent: config.bonus_crit_percent,
            bonus_spell_damage: config.bonus_spell_damage,
            bonus_expertise_percent: config.bonus_expertise_percent,
            cast_time_multiplier: 1.0,
            cd_multiplier: 1.0,
            damage_multiplier: config.damage_multiplier,
            damage_multiplier_additive: config.damage_multiplier_additive,
            direct_damage_multiplier_additive: 0.0,
            crit_multiplier_pct: 1.0,
            crit_multiplier_additive: config.crit_multiplier_additive,
            bonus_base_damage: config.bonus_base_damage,
            bonus_coefficient: config.bonus_coefficient,
            threat_multiplier: config.threat_multiplier,
            flat_threat_bonus: config.flat_threat_bonus,
            dots: Vec::new(),
            aoe_dot: None,
            related_dot_spell: config.related_dot_spell,
            related_self_buff: config.related_self_buff,
            related_aura_arrays: config.related_aura_arrays.clone(),
        };
        let options = config.cost;
        spell.cost = if options.mana_base_cost_percent != 0.0 || options.mana_flat_cost != 0 {
            let base_mana = self.unit(unit).mana_bar.base_mana;
            Some(SpellCost {
                resource: Resource::Mana,
                base_cost: if options.mana_flat_cost > 0 {
                    options.mana_flat_cost
                } else {
                    // Go: int32(BaseCostPercent * BaseMana) / 100
                    ((options.mana_base_cost_percent * base_mana) as i32) / 100
                },
                flat_modifier: 0,
                percent_modifier: if options.mana_percent_modifier == 0.0 {
                    1.0
                } else {
                    options.mana_percent_modifier
                },
                additive_percent_modifier: 1.0,
                refund: 0.0,
                refund_to_own_metrics: false,
            })
        } else if options.energy_cost != 0 {
            Some(simple_cost(
                Resource::Energy,
                options.energy_cost,
                options.energy_refund,
            ))
        } else if options.rage_cost != 0 {
            Some(simple_cost(
                Resource::Rage,
                options.rage_cost,
                options.rage_refund,
            ))
        } else if options.focus_cost != 0 {
            Some(simple_cost(
                Resource::Focus,
                options.focus_cost,
                options.focus_refund,
            ))
        } else {
            None
        };
        if spell.cd.timer.is_none() && spell.recharge_time > 0 {
            spell.cd.timer = Some(self.new_timer(unit));
        }
        self.spells.push(spell);
        self.create_dots(id, &config.dot, false);
        self.create_dots(id, &config.hot, true);
        let spell = self.spell_mut(id);
        if let Some(cost) = &spell.cost {
            spell.default_cast.cost = f64::from(cost.base_cost);
        }
        assert!(
            !(spell.default_cast.is_empty() && spell.cost.is_some()),
            "Empty DefaultCast with a cost for spell {}",
            spell.action_id
        );
        spell.cast_kind = if spell.default_cast.is_empty() {
            if !config.has_extra_cast_condition
                && cast.cd.timer.is_none()
                && cast.shared_cd.timer.is_none()
                && !config.has_cast_requirement
            {
                CastKind::AutosOrProcs
            } else {
                CastKind::Simple
            }
        } else {
            CastKind::Full
        };
        if config.min_range != 0.0 || config.max_range != 0.0 {
            spell.min_range = config.min_range;
            spell.max_range = config.max_range;
            spell.has_extra_cast_condition = true;
        }
        self.unit_mut(unit).spellbook.push(id);
        for handler in self.spell_registered_handlers(unit) {
            handler(self, id);
        }
        id
    }

    /// Go `unit.GetSpell`: the first registered spell with this action.
    pub(crate) fn get_spell(&self, unit: UnitId, action: &ActionId) -> Option<SpellId> {
        self.unit(unit)
            .spellbook
            .iter()
            .copied()
            .find(|id| &self.spell(*id).action_id == action)
    }

    /// Go `unit.GetOrRegisterSpell`.
    pub(crate) fn get_or_register_spell(&mut self, unit: UnitId, config: SpellConfig) -> SpellId {
        match self.get_spell(unit, &config.action_id) {
            Some(id) => id,
            None => self.register_spell(unit, config),
        }
    }

    fn create_dots(&mut self, spell: SpellId, config: &DotConfig, is_hot: bool) {
        if !config.is_set() {
            return;
        }
        let periodic_damage_multiplier = if config.periodic_damage_multiplier == 0.0 {
            1.0
        } else {
            config.periodic_damage_multiplier
        };
        let dot_spell = config.spell.unwrap_or(spell);
        let caster = self.spell(dot_spell).unit;
        let is_channeled = self.spell(dot_spell).flags.matches(SpellFlag::CHANNELED);
        let mut aura_config = config.aura.clone();
        if aura_config.action_id.is_none() {
            aura_config.action_id = Some(self.spell(dot_spell).action_id.clone());
        }
        let duration = config.tick_length * Duration::from(config.number_of_ticks);
        let make = |sim: &mut Sim, aura: AuraId| {
            // newDot: the aura lasts the ticks and gains tick handlers.
            let a = sim.aura_mut(aura);
            a.duration = duration;
            a.on_gain = Some(chain_noop(a.on_gain.take()));
            a.on_expire = Some(chain_noop(a.on_expire.take()));
            sim.dots.push(Dot {
                spell: dot_spell,
                aura,
                base_tick_count: config.number_of_ticks,
                base_tick_length: config.tick_length,
                bonus_coefficient: config.bonus_coefficient,
                periodic_damage_multiplier,
                base_duration_multiplier: 1.0,
                base_duration_flat: 0,
                affected_by_cast_speed: config.affected_by_cast_speed,
                affected_by_real_haste: config.affected_by_real_haste,
                haste_reduces_duration: config.haste_reduces_duration,
                is_channeled,
            });
            DotId(sim.dots.len() - 1)
        };
        if config.is_aoe || config.self_only {
            let aura = self.get_or_register_aura(caster, aura_config);
            let dot = make(self, aura);
            self.spell_mut(spell).aoe_dot = Some(dot);
        } else {
            aura_config.label = format!("{}-{}", aura_config.label, self.unit(caster).unit_index);
            let count = self.units.len();
            if self.spell(spell).dots.is_empty() {
                self.spell_mut(spell).dots = vec![None; count];
            }
            let caster_is_enemy = self.unit(caster).unit_type == UnitType::Enemy;
            for target in self.all_units() {
                let opponent = caster_is_enemy != (self.unit(target).unit_type == UnitType::Enemy);
                if is_hot != opponent {
                    let aura = self.get_or_register_aura(target, aura_config.clone());
                    let dot = make(self, aura);
                    let index = self.unit(target).unit_index as usize;
                    self.spell_mut(spell).dots[index] = Some(dot);
                }
            }
        }
    }
}

fn simple_cost(resource: Resource, cost: i32, refund: f64) -> SpellCost {
    SpellCost {
        resource,
        base_cost: cost,
        flat_modifier: 0,
        percent_modifier: 1.0,
        additive_percent_modifier: 1.0,
        refund,
        refund_to_own_metrics: false,
    }
}

/// A dot's tick handlers exist only in a fight; preparation records that the callbacks are set.
fn chain_noop(old: Option<super::sim::AuraCallback>) -> super::sim::AuraCallback {
    old.unwrap_or_else(|| Rc::new(|_: &mut Sim, _| {}))
}
