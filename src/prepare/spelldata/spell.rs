//! Go `spelldata/spell.go`: the accessors that read a spell row in the sim's units.
//!
//! Not ported, because they need `core` types preparation does not have yet: `TickOutcome` and
//! `TickOutcomeHitRolled` return a `core.OutcomeApplier` off a `core.Dot`. The choice they make
//! is [`TickOutcomeKind`], through [`Spell::tick_outcome_kind`] and
//! [`Spell::tick_outcome_hit_rolled_kind`].

use std::sync::OnceLock;

use super::super::dbcenums;
use super::super::sim::{Duration, NEVER_EXPIRES};
use super::super::spell::DefenseType;
use super::store::{driver_ids, must_find};
use super::{duration_from_millis, nil_effect, Effect, Power, Spell};
use crate::data::spells as rows;

/// What `Power` answers for a bar the spell does not use. Shared, like [`super::nil`].
fn nil_power() -> &'static Power {
    static NIL: OnceLock<Power> = OnceLock::new();
    NIL.get_or_init(Power::default)
}

/// What `core.ManaCostOptions` reads off a row: the two fields `Spell.ManaCost` fills.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ManaCostOptions {
    pub flat_cost: i32,
    pub base_cost_percent: f64,
}

/// Go's `tickOutcome*` choices: which of the four tick outcomes a dot's ticks roll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TickOutcomeKind {
    /// `dot.Spell.OutcomeTickMagicHitAndCrit`.
    MagicCrit,
    /// `dot.Spell.OutcomeTickPhysicalCrit`.
    PhysicalCrit,
    /// `dot.OutcomeTickMagicHit`.
    MagicHit,
    /// `dot.OutcomeTick`.
    Plain,
}

/// Which of the four ticks the two flags pick (Go `tickOutcomeKind`).
pub(crate) fn tick_outcome_kind(can_crit: bool, magic: bool) -> TickOutcomeKind {
    match (can_crit, magic) {
        (true, true) => TickOutcomeKind::MagicCrit,
        (true, false) => TickOutcomeKind::PhysicalCrit,
        (false, true) => TickOutcomeKind::MagicHit,
        (false, false) => TickOutcomeKind::Plain,
    }
}

/// Go `resolve`: the spells for these ids, leaving out an id the store does not carry rather
/// than answering it as Nil.
pub(crate) fn resolve(ids: &[i32]) -> Vec<&'static Spell> {
    ids.iter().filter_map(|&id| rows::find(id)).collect()
}

impl Spell {
    pub(crate) fn cast_time(&self) -> Duration {
        duration_from_millis(self.cast_time_ms)
    }

    /// `SpellCooldowns.RecoveryTime`. A spell gated by its category instead (Fire Blast and
    /// Cone of Cold share one) states that in [`Spell::category_cooldown`].
    pub(crate) fn cooldown(&self) -> Duration {
        duration_from_millis(self.cooldown_ms)
    }

    pub(crate) fn category_cooldown(&self) -> Duration {
        duration_from_millis(self.category_cooldown_ms)
    }

    pub(crate) fn gcd(&self) -> Duration {
        duration_from_millis(self.gcd_ms)
    }

    /// The client states a permanent aura as -1, which is `NeverExpires` here.
    pub(crate) fn duration(&self) -> Duration {
        if self.duration_ms == -1 {
            return NEVER_EXPIRES;
        }
        duration_from_millis(self.duration_ms)
    }

    /// `SpellAuraOptions.ProcCategoryRecovery`: the internal cooldown between two procs.
    pub(crate) fn icd(&self) -> Duration {
        duration_from_millis(self.icd_ms)
    }

    /// Go `SpellSchool`: `SpellMisc.SchoolMask`, in core's bits.
    pub(crate) fn spell_school(&self) -> u8 {
        self.school
    }

    /// Go `DefenseTypeCore`.
    pub(crate) fn defense_type_core(&self) -> DefenseType {
        DefenseType::from_client(self.defense_type)
    }

    /// The i-th effect the row carries, counted from 1 by position rather than by the client's
    /// EffectIndex, which has gaps. Out of range answers [`nil_effect`].
    pub(crate) fn effect_n(&self, i: i32) -> &Effect {
        if i < 1 || i as usize > self.effects.len() {
            return nil_effect();
        }
        &self.effects[(i - 1) as usize]
    }

    /// The one effect with this aura and misc value. Panics when none matches, and when two do:
    /// reading the first of several silently is the bug this shape exists to prevent, and
    /// [`Spell::effect_n`] is the way past it. Go `Effect`.
    pub(crate) fn effect(&self, aura: i32, misc: i32) -> &Effect {
        let mut found = None;
        let mut matches = 0;
        for (i, e) in self.effects.iter().enumerate() {
            if e.aura == aura && e.misc == misc {
                matches += 1;
                if found.is_none() {
                    found = Some(i);
                }
            }
        }
        if matches > 1 {
            panic!(
                "spell {} has {matches} effects with aura {aura} misc {misc} - index them instead",
                self.id
            );
        }
        match found {
            Some(i) => &self.effects[i],
            None => panic!(
                "spell {} has no effect with aura {aura} misc {misc}, in {} effects",
                self.id,
                self.effects.len()
            ),
        }
    }

    /// The first effect matching all three, or [`nil_effect`]. Zero matches anything it is given
    /// as: an aura of 0 on a direct effect is what the client states there.
    pub(crate) fn find_effect(&self, effect_type: i32, aura: i32, misc: i32) -> &Effect {
        self.effects
            .iter()
            .find(|e| e.effect_type == effect_type && e.aura == aura && e.misc == misc)
            .unwrap_or_else(|| nil_effect())
    }

    /// The effect carrying the spell's direct damage, whether it states an amount or a weapon
    /// multiplier. It sits on any of the weapon effects as readily as on school damage. A health
    /// leech deals its amount as damage too; the life it hands the caster is not modelled.
    pub(crate) fn damage_effect(&self) -> &Effect {
        self.first_of_type(&[
            dbcenums::E_SCHOOL_DAMAGE,
            dbcenums::E_HEALTH_LEECH,
            dbcenums::E_WEAPON_DAMAGE,
            dbcenums::E_WEAPON_PERCENT_DAMAGE,
            dbcenums::E_NORMALIZED_WEAPON_DMG,
            dbcenums::E_WEAPON_DAMAGE_NOSCHOOL,
        ])
    }

    pub(crate) fn heal_effect(&self) -> &Effect {
        self.first_of_type(&[dbcenums::E_HEAL])
    }

    /// The effect a proc's heal lands through: `E_HEAL_PCT`, a percentage of the target's
    /// maximum health, `E_HEAL`, an amount the effect rolls, or an `A_PERIODIC_HEAL` aura that
    /// heals the amount it rolls every period.
    pub(crate) fn proc_heal_effect(&self) -> &Effect {
        let e = self.first_of_type(&[dbcenums::E_HEAL_PCT, dbcenums::E_HEAL]);
        if !e.is_nil() {
            return e;
        }
        self.first_aura(&[dbcenums::A_PERIODIC_HEAL])
    }

    /// The `A_PERIODIC_DAMAGE` aura the spell applies, or [`nil_effect`] where it applies none.
    pub(crate) fn periodic_damage_effect(&self) -> &Effect {
        self.first_aura(&[dbcenums::A_PERIODIC_DAMAGE])
    }

    /// The `A_SCHOOL_ABSORB` aura the spell applies, or [`nil_effect`] where it applies none.
    /// Its Misc is the mask of the schools it absorbs, in core's `SpellSchool` bits.
    pub(crate) fn absorb_effect(&self) -> &Effect {
        self.first_aura(&[dbcenums::A_SCHOOL_ABSORB])
    }

    pub(crate) fn energize_effect(&self) -> &Effect {
        self.first_of_type(&[dbcenums::E_ENERGIZE])
    }

    /// The effect a resource gain lands through: `E_ENERGIZE`, an amount into the bar its misc
    /// value names, or an `A_PERIODIC_ENERGIZE` aura that restores the amount every period.
    pub(crate) fn proc_energize_effect(&self) -> &Effect {
        let e = self.energize_effect();
        if !e.is_nil() {
            return e;
        }
        self.first_aura(&[dbcenums::A_PERIODIC_ENERGIZE])
    }

    /// The effect that ticks: an aura application whose aura carries a per-tick value, which is
    /// damage, healing, mana or a spell fired each tick.
    pub(crate) fn periodic_effect(&self) -> &Effect {
        self.effects
            .iter()
            .find(|e| {
                e.effect_type == dbcenums::E_APPLY_AURA
                    && matches!(
                        e.aura,
                        dbcenums::A_PERIODIC_DAMAGE
                            | dbcenums::A_PERIODIC_HEAL
                            | dbcenums::A_PERIODIC_ENERGIZE
                            | dbcenums::A_PERIODIC_TRIGGER_SPELL
                            | dbcenums::A_PERIODIC_LEECH
                    )
            })
            .unwrap_or_else(|| nil_effect())
    }

    /// The first effect, in effect order, of any of these types.
    fn first_of_type(&self, types: &[i32]) -> &Effect {
        self.effects
            .iter()
            .find(|e| types.contains(&e.effect_type))
            .unwrap_or_else(|| nil_effect())
    }

    /// The first `E_APPLY_AURA` effect of any of these auras. Go `FirstAura`.
    pub(crate) fn first_aura(&self, auras: &[i32]) -> &Effect {
        self.effects
            .iter()
            .find(|e| e.effect_type == dbcenums::E_APPLY_AURA && auras.contains(&e.aura))
            .unwrap_or_else(|| nil_effect())
    }

    /// The cost out of this bar, or a zero Power where the spell does not use it.
    pub(crate) fn power(&self, power_type: i32) -> &Power {
        self.powers
            .iter()
            .find(|p| p.power_type == power_type)
            .unwrap_or_else(|| nil_power())
    }

    /// The cost in the units the sim spends: rage off the client's 0-1000 bar, everything else
    /// as stated.
    pub(crate) fn power_cost(&self, power_type: i32) -> f64 {
        let cost = f64::from(self.power(power_type).cost);
        if dbcenums::power_in_tenths(power_type) {
            return cost / 10.0;
        }
        cost
    }

    /// The cost of the bar the spell spends, the first `SpellPower` row, in the units
    /// [`Spell::power_cost`] converts to. 0 for a spell with no `SpellPower` row.
    pub(crate) fn cost(&self) -> f64 {
        match self.powers.first() {
            None => 0.0,
            Some(first) => self.power_cost(first.power_type),
        }
    }

    /// The mana the cast takes: `SpellPower`'s flat `ManaCost`, or its `PowerCostPct` of base
    /// mana. The client states the percentage as a float32, read back here as the decimal it
    /// prints (13.9, not 13.8999996): core truncates the cost, so a widened float would come out
    /// one short on a whole product.
    pub(crate) fn mana_cost(&self) -> ManaCostOptions {
        let p = self.power(dbcenums::POWER_MANA);
        // Go: ParseFloat(FormatFloat(float64(p.CostPct), 'g', -1, 32), 64). Rust prints a f32 as
        // its shortest decimal that reads back as the same f32, as Go's bitSize 32 does.
        let percent = p
            .cost_pct
            .to_string()
            .parse::<f64>()
            .unwrap_or(f64::from(p.cost_pct));
        ManaCostOptions {
            flat_cost: p.cost,
            base_cost_percent: percent,
        }
    }

    /// `Spell.NameSubtext_lang`'s "Rank N" as a number; 0 for a spell the client shows no rank
    /// on, whose subtext is "Passive" or empty. Go `RankNumber` and `rankOf`.
    pub(crate) fn rank_number(&self) -> i32 {
        let Some(digits) = self.rank.strip_prefix("Rank ") else {
            return 0;
        };
        // strconv.Atoi: an optional sign and decimal digits, an error on anything else or on
        // overflow, which reads as 0. The int is then cut to int32.
        match digits.parse::<i64>() {
            Ok(rank) => rank as i32,
            Err(_) => 0,
        }
    }

    /// The spells the tooltip names, in the order it names them. An id the store does not carry
    /// is left out rather than answered as Nil.
    pub(crate) fn refs(&self) -> Vec<&'static Spell> {
        resolve(&self.ref_ids)
    }

    /// The spell an `A_OVERRIDE_ACTIONBAR_SPELLS` effect of `overrider` puts on the action bar
    /// in place of this one. It panics when `overrider` does not replace this spell.
    pub(crate) fn overridden_by(&self, overrider: &Spell) -> &'static Spell {
        let effect = overrider.effect(dbcenums::A_OVERRIDE_ACTIONBAR_SPELLS, self.id);
        must_find(effect.base_points as i32)
    }

    /// The form an `A_MOD_SHAPESHIFT` effect puts the caster in, or none (0).
    pub(crate) fn shapeshift_form(&self) -> dbcenums::ShapeshiftForm {
        self.effects
            .iter()
            .find(|e| e.aura == dbcenums::A_MOD_SHAPESHIFT)
            .map_or(0, |e| e.misc as dbcenums::ShapeshiftForm)
    }

    /// The spells whose effects fire this one, from the trigger index.
    pub(crate) fn drivers(&self) -> Vec<&'static Spell> {
        resolve(driver_ids(self.id))
    }

    /// Every spell this one's effects fire, deduped, in effect order, and then the ones a
    /// server-side handler casts off it.
    pub(crate) fn triggered(&self) -> Vec<&'static Spell> {
        let mut ids: Vec<i32> = Vec::new();
        for e in &self.effects {
            if e.trigger_id != 0 && !ids.contains(&e.trigger_id) {
                ids.push(e.trigger_id);
            }
        }
        for &id in rows::hand_triggers(self.id) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        resolve(&ids)
    }

    /// The outcome a periodic tick rolls: a tick that can crit where the client marks Periodic
    /// Can Crit, a plain tick otherwise, on the hit table the row's defense type names. Go
    /// `TickOutcome`, as the choice of outcome.
    pub(crate) fn tick_outcome_kind(&self) -> TickOutcomeKind {
        tick_outcome_kind(
            self.periodic_can_crit(),
            self.defense_type_core() == DefenseType::Magic,
        )
    }

    /// A tick of a damage over time whose hit was rolled when it was applied: a crit only where
    /// the row states Periodic Can Crit, on the crit of the dot spell's defense type. Go
    /// `TickOutcomeHitRolled`, as the choice of outcome: the magic-hit-only choice reads as the
    /// plain tick here, as it falls to Go's default.
    pub(crate) fn tick_outcome_hit_rolled_kind(&self, dot_spell_is_magic: bool) -> TickOutcomeKind {
        match tick_outcome_kind(self.periodic_can_crit(), dot_spell_is_magic) {
            TickOutcomeKind::MagicHit => TickOutcomeKind::Plain,
            kind => kind,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::sim::{MILLISECOND, SECOND};
    use crate::prepare::spelldata::nil;

    fn spell_with_powers(powers: Vec<Power>) -> Spell {
        Spell {
            powers,
            ..Spell::default()
        }
    }

    fn power(power_type: i32, cost: i32) -> Power {
        Power {
            power_type,
            cost,
            ..Power::default()
        }
    }

    /// Go `TestEffectOutOfRange`.
    #[test]
    fn effects_out_of_range_answer_the_nil_effect() {
        let frostbolt = must_find(116);
        assert_eq!(frostbolt.effects.len(), 2);
        assert!(frostbolt.effect_n(3).is_nil());
        assert!(frostbolt.effect_n(0).is_nil());
        assert!(frostbolt.effect_n(-1).is_nil());
        assert!(!frostbolt.effect_n(2).is_nil());
        assert_eq!(nil_effect().average(60), 0.0);
        assert_eq!(nil().cast_time(), 0);
    }

    /// Go `TestTriggerAndDrivers`, on rows of the pinned store: Arcane Missiles rank 2 ticks
    /// 7269, and Lightning Shield's aura triggers the shared dispatcher 26545.
    #[test]
    fn triggers_and_drivers() {
        assert_eq!(must_find(5144).effect_n(1).trigger().id, 7269);
        assert_eq!(must_find(324).effect_n(1).trigger().id, 26545);
        assert!(must_find(324)
            .triggered()
            .iter()
            .any(|spell| spell.id == 26545));
        assert!(must_find(7269)
            .drivers()
            .iter()
            .any(|spell| spell.id == 5144));
    }

    /// Go `TestTimes`.
    #[test]
    fn times_are_milliseconds_as_durations() {
        assert_eq!(must_find(324).icd(), 3500 * MILLISECOND);
        assert_eq!(must_find(116).cast_time(), 1500 * MILLISECOND);
        assert_eq!(must_find(116).gcd(), 1500 * MILLISECOND);
        // The client's permanent aura.
        let permanent = Spell {
            duration_ms: -1,
            ..Spell::default()
        };
        assert_eq!(permanent.duration(), NEVER_EXPIRES);
        let five_seconds = Spell {
            duration_ms: 5000,
            ..Spell::default()
        };
        assert_eq!(five_seconds.duration(), 5 * SECOND);
    }

    /// Go `TestPowerCost`.
    #[test]
    fn power_costs_are_in_the_sims_units() {
        assert_eq!(must_find(116).power_cost(dbcenums::POWER_MANA), 25.0);
        // Rage is on the client's 0-1000 bar: 300 is 30 rage.
        let rager = spell_with_powers(vec![power(dbcenums::POWER_RAGE, 300)]);
        assert_eq!(rager.power_cost(dbcenums::POWER_RAGE), 30.0);
        assert_eq!(must_find(116).power_cost(dbcenums::POWER_ENERGY), 0.0);
    }

    /// Go `TestCost`.
    #[test]
    fn cost_reads_the_first_power_row() {
        let cases = [
            ("no power row", Spell::default(), 0.0),
            (
                "first of several powers, mana",
                spell_with_powers(vec![power(0, 25), power(1, 300)]),
                25.0,
            ),
            (
                "first of several powers, rage",
                spell_with_powers(vec![power(1, 300), power(0, 25)]),
                30.0,
            ),
        ];
        for (name, spell, want) in cases {
            assert_eq!(spell.cost(), want, "{name}");
        }
    }

    /// Go `TestManaCostReadsTheStatedPercent`: Multi-Shot's 13.9% of base mana is a float32 in
    /// the client (13.8999996). Widened as it stands, a 1000 base mana hunter would pay 138; the
    /// client's own number is 139.
    #[test]
    fn mana_cost_reads_the_stated_percent() {
        let multi_shot = spell_with_powers(vec![Power {
            power_type: dbcenums::POWER_MANA,
            cost_pct: 13.9,
            ..Power::default()
        }]);
        assert_eq!(
            multi_shot.mana_cost(),
            ManaCostOptions {
                flat_cost: 0,
                base_cost_percent: 13.9
            }
        );
        assert_ne!(f64::from(13.9f32), 13.9);
        let flat = spell_with_powers(vec![power(dbcenums::POWER_MANA, 115)]);
        assert_eq!(
            flat.mana_cost(),
            ManaCostOptions {
                flat_cost: 115,
                base_cost_percent: 0.0
            }
        );
    }

    /// Go `TestRankNumber`, and Atoi's edges.
    #[test]
    fn rank_number_reads_the_subtext() {
        let cases = [
            ("Rank 1", 1),
            ("Rank 12", 12),
            ("", 0),
            ("Passive", 0),
            ("Rank ", 0),
            ("Rank x", 0),
            ("Rank +3", 3),
            ("Rank -3", -3),
            ("Rank 3 ", 0),
            ("Rank 99999999999999999999", 0),
        ];
        for (rank, want) in cases {
            let spell = Spell {
                rank: rank.to_string(),
                ..Spell::default()
            };
            assert_eq!(spell.rank_number(), want, "{rank:?}");
        }
        assert_eq!(must_find(116).rank_number(), 1);
    }

    /// Go `TestEffectPanics`.
    #[test]
    #[should_panic(expected = "has 2 effects with aura 42 misc 3")]
    fn an_ambiguous_effect_panics() {
        let ambiguous = Spell {
            id: 7,
            effects: vec![
                Effect {
                    aura: 42,
                    misc: 3,
                    ..Effect::default()
                },
                Effect {
                    aura: 42,
                    misc: 3,
                    ..Effect::default()
                },
            ],
            ..Spell::default()
        };
        ambiguous.effect(42, 3);
    }

    #[test]
    #[should_panic(expected = "has no effect with aura 99 misc 0")]
    fn a_missing_effect_panics() {
        Spell {
            id: 7,
            ..Spell::default()
        }
        .effect(99, 0);
    }

    /// Go `TestTickOutcomeKind`.
    #[test]
    fn tick_outcome_kind_picks_one_of_four() {
        assert_eq!(tick_outcome_kind(true, true), TickOutcomeKind::MagicCrit);
        assert_eq!(
            tick_outcome_kind(true, false),
            TickOutcomeKind::PhysicalCrit
        );
        assert_eq!(tick_outcome_kind(false, true), TickOutcomeKind::MagicHit);
        assert_eq!(tick_outcome_kind(false, false), TickOutcomeKind::Plain);
    }

    /// Go `TestGeneratedStanceAndAuraRestriction`'s forms.
    #[test]
    fn shapeshift_forms() {
        for (id, want) in [
            (2457, dbcenums::FORM_BATTLE_STANCE),
            (71, dbcenums::FORM_DEFENSIVE_STANCE),
            (2458, dbcenums::FORM_BERSERKER_STANCE),
            (768, dbcenums::FORM_CAT_FORM),
            (9634, dbcenums::FORM_DIRE_BEAR_FORM),
            (24858, dbcenums::FORM_MOONKIN_FORM),
        ] {
            assert_eq!(must_find(id).shapeshift_form(), want, "spell {id}");
        }
        assert_eq!(must_find(116).shapeshift_form(), 0);
    }

    /// Go `TestGeneratedStanceAndAuraRestriction`'s masks.
    #[test]
    fn stance_masks() {
        let wrath = must_find(5176);
        assert_eq!(
            (wrath.stance_mask, wrath.stance_exclude),
            (0x4000_0000, 0x2)
        );
        let healing_touch = must_find(5185);
        assert_eq!(
            (healing_touch.stance_mask, healing_touch.stance_exclude),
            (0x2, 0x4000_0000)
        );
        let shifting_power = must_find(1322605);
        assert_eq!(
            (shifting_power.caster_aura, shifting_power.stance_mask),
            (768, 0x1)
        );
    }

    /// Go `TestGeneratedInterruptFlags`.
    #[test]
    fn interrupt_flags() {
        for (id, want) in [(116, 15u32), (11605, 15), (1269268, 5)] {
            let spell = must_find(id);
            assert_eq!(spell.interrupt_flags, want, "{} ({id})", spell.name);
            assert_eq!(spell.pushed_back(), want == 15, "{} ({id})", spell.name);
        }
    }

    /// Go `TestGeneratedLightningShield` and `TestGeneratedOverrideRow`.
    #[test]
    fn proc_columns() {
        let shield = must_find(324);
        assert_eq!(shield.proc_flags, [0x222A8, 0]);
        assert_eq!(shield.proc_charges, 3);
        assert_eq!(shield.icd_ms, 3500);
        assert_eq!(
            shield.proc_chance_source,
            crate::prepare::spelldata::proc_chance_source::ALWAYS
        );
        assert_eq!(shield.effect_n(1).trigger_id, 26545);
        assert_eq!(shield.ref_ids, [26364]);
        assert_eq!(shield.refs()[0].id, 26364);

        let armor_shatter = must_find(16928);
        assert_eq!(armor_shatter.rppm, 1.0);
        assert_eq!(
            armor_shatter.proc_chance_source,
            crate::prepare::spelldata::proc_chance_source::PPM
        );
    }

    /// Go `TestGeneratedFrostbolt`: the row the package comment's units are read off.
    #[test]
    fn frostbolt_rank_one() {
        let s = must_find(116);
        assert_eq!((s.name.as_str(), s.rank.as_str()), ("Frostbolt", "Rank 1"));
        assert_eq!(s.school, 16);
        assert_eq!((s.spell_level, s.max_level), (4, 8));
        assert_eq!(s.power_cost(0), 25.0);
        let damage = s.effect_n(2);
        assert_eq!((damage.base_points, damage.ppl), (19.0, 0.5));
        assert!(
            (0.4069..=0.4071).contains(&damage.sp_coef),
            "{}",
            damage.sp_coef
        );
        // 19 plus half a point for each of the four levels between the rank's own 4 and its cap
        // of 8.
        assert_eq!(damage.average(60), 21.0);
        assert_eq!(s.damage_effect().index, damage.index);
        assert!(s.heal_effect().is_nil());
        assert_eq!(s.defense_type_core(), DefenseType::Magic);
    }
}
