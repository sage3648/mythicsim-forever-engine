// Druid Feral (cat) export: Cat Form and its shifts, Prowl, the cat's builders and finishers,
// Shifting Power, Berserk, Blood Frenzy and Rend and Tear. Each formula mirrors the cited Go
// file at the pinned revision.
package main

import (
	"reflect"
	"time"
	"unsafe"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
	"github.com/wowsims/forever/sim/druid"
)

var (
	feralCatForm       = spelldata.Ranked(768)
	feralFuror         = spelldata.Talent(17056, 5)
	feralProwl         = spelldata.Ranked(5215, 6783, 9913)
	feralRavage        = spelldata.Ranked(6785, 6787, 9866, 9867)
	feralShred         = spelldata.Ranked(5221, 6800, 8992, 9829, 9830)
	feralClaw          = spelldata.Ranked(1082, 3029, 5201, 9849, 9850)
	feralRip           = spelldata.Ranked(1079, 9492, 9493, 9752, 9894, 9896)
	feralRake          = spelldata.Ranked(1822, 1823, 1824, 9904)
	feralBite          = spelldata.Ranked(22568, 22827, 22828, 22829, 31018)
	feralShifting      = spelldata.Ranked(1322605)
	feralBloodFrenzy   = spelldata.Talent(16958, 2)
	feralBloodFrenzyOn = spelldata.Ranked(16959)
	feralRendAndTear   = spelldata.Talent(1223246, 5)
)

// Unexported struct fields refuse Interface(); read them through their address.
func readPrivate(value reflect.Value) reflect.Value {
	return reflect.NewAt(value.Type(), unsafe.Pointer(value.UnsafeAddr())).Elem()
}

// The unit's pseudo stats before any aura applied, which each reset restores.
func initialPseudoStats(unit *core.Unit) stats.PseudoStats {
	return readPrivate(privateField(unit, "initialPseudoStats")).Interface().(stats.PseudoStats)
}

// The threat multiplier a form multiplies when the agent's reset enters it: the initial value
// with every permanent aura the reset activates, such as the Threat or Subtlety enchant, read
// from a separate reset simulation with the form's own factor taken back out.
func formStartThreat(label string, factor float64) float64 {
	simulation := core.NewSim(exportRequest, simsignals.CreateSignals())
	simulation.Reset()
	character := simulation.Raid.Parties[0].Players[0].GetCharacter()
	threat := character.PseudoStats.ThreatMultiplier
	if aura := character.GetAura(label); aura != nil && aura.IsActive() {
		threat /= factor
	}
	return threat
}

// The spellbook position of a spell, or -1.
func spellPosition(character *core.Character, spell *core.Spell) int {
	for i, s := range character.Spellbook {
		if s == spell {
			return i
		}
	}
	return -1
}

// ferocious_bite.go rolls its damage row.
func druidFeralDamageRows(rows map[int32]*spelldata.Spell) {
	if bite := feralBite.Highest(); bite != nil {
		rows[bite.ID] = bite
	}
}

// Cat Form changes stats through AddStatsDynamic and its stat dependencies (forms.go), and a
// bear's Enrage cuts its armor (enrage.go). Bear Form stays up for the whole fight of a bear,
// so its stats are the base ones.
func druidStatAuras(agent core.Agent, character *core.Character) []string {
	d := agent.(druid.DruidAgent).GetDruid()
	labels := []string{}
	if d.CatForm != nil && character.GetAura("Cat Form") != nil {
		labels = append(labels, "Cat Form")
	}
	if d.EnrageAura != nil {
		labels = append(labels, d.EnrageAura.Label)
	}
	// barkskin.go: the physical damage taken cut changes the target's swings.
	if d.Barkskin != nil && character.GetAura("Barkskin") != nil {
		labels = append(labels, "Barkskin")
	}
	// forms.go: Bear Form's stats, armor and health, which the target's swings read.
	if d.BearForm != nil && d.BearFormAura != nil {
		labels = append(labels, d.BearFormAura.Label)
	}
	return labels
}

// Rend and Tear registers one dynamic damage taken modifier on every target.
func druidDamageTakenModifiers(agent core.Agent) int {
	if agent.(druid.DruidAgent).GetDruid().Talents.RendAndTear > 0 {
		return 1
	}
	return 0
}

func druidFeralEffects(d *druid.Druid, character *core.Character) []map[string]any {
	effects := []map[string]any{}
	talents := d.Talents
	target := character.Env.Encounter.ActiveTargetUnits[0]
	initial := initialPseudoStats(&character.Unit)

	if d.CatForm != nil && d.CatFormAura != nil {
		// forms.go RegisterCatFormAura and registerCatFormSpell, with Furor's carry over.
		furor := 0.0
		if talents.Furor > 0 {
			furor = feralFuror.EffectAt(2).ValueAt(talents.Furor)
		}
		breaking := []int{}
		for i, spell := range character.Spellbook {
			if spell.Flags.Matches(core.SpellFlagPotion | core.SpellFlagConjured | core.SpellFlagExplosive) {
				breaking = append(breaking, i)
			}
		}
		mainHand := d.WeaponFromMainHand()
		cat := d.GetCatWeapon()
		// movement.go NewPassiveMovementSpeedEffect: the form's speed applies when its effect holds
		// the category, which no other passive speed effect shares here.
		for _, ee := range d.CatFormAura.ExclusiveEffects {
			if privateField(ee.Category, "effects").Len() != 1 {
				*classNotes = append(*classNotes, "Cat Form's movement speed shares its category")
			}
		}
		effects = append(effects, map[string]any{
			"kind": "cat_form", "spell_id": feralCatForm.Highest().ID, "aura": d.CatFormAura.Label,
			"initial_threat_multiplier":       formStartThreat(d.CatFormAura.Label, druid.CatFormThreatMultiplier),
			"threat_multiplier":               druid.CatFormThreatMultiplier,
			"initial_spirit_regen_multiplier": initial.SpiritRegenMultiplier, "spirit_regen_multiplier": druid.AnimalSpiritRegenSuppression,
			"initial_movement_speed_multiplier": initial.MovementSpeedMultiplier, "movement_speed_bonus": 0.25,
			"furor_max": furor, "cost_spells": spellsMatching(character, druid.DruidSpellFaerieFire),
			"gcd_spells": spellsMatching(character, druid.DruidSpellFaerieFire), "gcd_delta_ns": nanos(-500 * time.Millisecond),
			"form_breaking_spells": breaking, "main_hand": exportWeapon(&mainHand), "cat_weapon": exportWeapon(&cat),
		})

		if d.ProwlAura != nil { // prowl.go
			rank := feralProwl.Highest()
			effects = append(effects, map[string]any{
				"kind": "prowl", "spell_id": rank.ID, "aura": d.ProwlAura.Label,
				"movement_speed_multiplier": 1 + rank.Effect(dbcenums.A_MOD_DECREASE_SPEED, 0).BaseValue()/100,
			})
		}

		// ravage.go, shred.go and claw.go: the rank's flat damage plus main hand weapon damage, a combo
		// point when it lands and a refund when it does not. Ravage needs Prowl, and both it and Shred
		// need the druid behind the target.
		builders := []map[string]any{}
		for _, entry := range []struct {
			kind  string
			spell *druid.DruidSpell
			rank  *spelldata.Spell
		}{{"ravage", d.Ravage, feralRavage.Highest()}, {"shred", d.Shred, feralShred.Highest()}, {"claw", d.Claw, feralClaw.Highest()}} {
			if entry.spell == nil {
				continue
			}
			builders = append(builders, map[string]any{
				"kind": entry.kind, "spell": spellPosition(character, entry.spell.Spell),
				"flat_damage": entry.rank.DamageEffect().Average(core.CharacterLevel),
			})
		}
		effects = append(effects, map[string]any{"kind": "cat_builders", "builders": builders, "cannot_shred": d.CannotShredTarget})

	}
	if d.Rip != nil { // rip.go: the tick base and points per combo point; the attack power share is a Go literal
		rank := feralRip.Highest()
		// spell_result.go TargetDamageMultiplier: a bleed tick also takes the target's periodic
		// physical multiplier.
		if target.PseudoStats.PeriodicPhysicalDamageTakenMultiplier != 1 {
			*classNotes = append(*classNotes, "the target takes a periodic physical damage multiplier")
		}
		tick := rank.PeriodicEffect()
		effects = append(effects, map[string]any{
			"kind": "rip", "spell": spellPosition(character, d.Rip.Spell),
			"tick_base": tick.Average(core.CharacterLevel), "tick_per_combo_point": float64(tick.PointsPerResource),
			"attack_power_share_per_combo_point": 0.01, "attack_power_share_max_points": 4.0,
			"tick_can_crit":         rank.PeriodicCanCrit(),
			"tick_magic":            rank.DefenseTypeCore() == core.DefenseTypeMagic,
			"expected_combo_points": 5.0, "short_name": d.Rip.ShortName,
		})
	}
	if d.Rake != nil { // rake.go: a flat hit and a flat bleed tick, neither scaling with attack power
		rank := feralRake.Highest()
		effects = append(effects, map[string]any{
			"kind": "rake", "spell": spellPosition(character, d.Rake.Spell),
			"flat_damage":   rank.DamageEffect().Average(core.CharacterLevel),
			"tick_base":     rank.PeriodicEffect().Average(core.CharacterLevel),
			"tick_can_crit": rank.PeriodicCanCrit(),
			"tick_magic":    rank.DefenseTypeCore() == core.DefenseTypeMagic,
			"short_name":    d.Rake.ShortName,
		})
	}
	if d.FerociousBite != nil { // ferocious_bite.go: client values, and 3% attack power a point, a Go literal
		rank := feralBite.Highest()
		effects = append(effects, map[string]any{
			"kind": "ferocious_bite", "spell": spellPosition(character, d.FerociousBite.Spell),
			"damage_per_energy":            spelldata.Ranked(22568, 22827, 22828, 22829, 31018).EffectAt(2).FractionAt(rank.RankNumber()),
			"damage_per_combo_point":       float64(rank.DamageEffect().PointsPerResource),
			"attack_power_per_combo_point": 0.03,
		})
	}
	if d.ShiftingPower != nil { // shifting_power.go
		effects = append(effects, map[string]any{
			"kind": "shifting_power", "spell": spellPosition(character, d.ShiftingPower.Spell),
			"energy": feralShifting.Highest().EnergizeEffect().Average(core.CharacterLevel) + d.WolfsheadShiftingPowerEnergy,
		})
	}
	if d.FaerieFire != nil && d.FaerieFireAuras != nil { // faerie_fire.go and buffs FaerieFireAura
		aura := d.FaerieFireAuras.Get(target)
		effects = append(effects, map[string]any{
			"kind": "faerie_fire", "spell": spellPosition(character, d.FaerieFire.Spell), "aura": aura.Label,
			"armor_reduction": buffs.FaerieFireValue(0), "refresh": exclusiveRefresh(aura),
		})
	}
	if d.BerserkAura != nil { // talents_feral_combat.go applyBerserk: +100% crit, a Go literal
		effects = append(effects, map[string]any{
			"kind": "berserk", "spell_id": d.BerserkAura.ActionID.SpellID, "aura": d.BerserkAura.Label, "crit_percent": 100.0,
			"crit_spells": spellsMatching(character, druid.DruidSpellClaw|druid.DruidSpellShred|druid.DruidSpellRake|druid.DruidSpellRavage),
		})
	}
	if talents.BloodFrenzy > 0 { // talents_feral_combat.go applyBloodFrenzy
		trigger := core.ProcTrigger{
			Name: "Blood Frenzy (Cat)", Callback: core.CallbackOnSpellHitDealt, ClassSpellMask: druid.DruidSpellBuilder,
			Outcome: core.OutcomeCrit, ProcChance: feralBloodFrenzy.EffectAt(1).FractionAt(talents.BloodFrenzy),
		}
		// The bear half: Rage on any melee crit in Bear Form, the client's energize in its own units.
		bear := core.ProcTrigger{ProcMask: core.ProcMaskMelee}
		effects = append(effects, map[string]any{
			"kind": "blood_frenzy", "trigger_aura": trigger.Name, "bear_trigger_aura": "Blood Frenzy (Bear)",
			"proc_chance": trigger.ProcChance, "trigger_spells": procTriggerSpells(character, trigger),
			"outcome": outcomeNames(trigger.Outcome), "trigger_immediately": trigger.TriggerImmediately,
			"metrics_action_id":   actionID(core.ActionID{SpellID: feralBloodFrenzyOn.Highest().ID}),
			"bear_trigger_spells": procTriggerSpells(character, bear),
			"bear_rage":           feralBloodFrenzyOn.Highest().EffectN(1).BaseValue() / 10,
		})
	}
	if talents.RendAndTear > 0 { // talents_feral_combat.go applyRendAndTear
		bleeds := []int{}
		for _, spell := range []*druid.DruidSpell{d.Rip, d.Rake, d.Lacerate} {
			if spell != nil {
				bleeds = append(bleeds, spellPosition(character, spell.Spell))
			}
		}
		special := []int{}
		for i, spell := range character.Spellbook {
			if spell.ProcMask.Matches(core.ProcMaskMeleeSpecial) {
				special = append(special, i)
			}
		}
		effects = append(effects, map[string]any{
			"kind": "rend_and_tear", "multiplier": feralRendAndTear.EffectAt(1).MultiplierAt(talents.RendAndTear),
			"spells": special, "bleed_spells": bleeds,
		})
	}
	// feralcat.go EnableRageBar: the cat's rage bar, whose listener acts only while it is the
	// current power bar, which only Bear Form makes it. A potion still gives it Rage, and the
	// Rage gained still makes threat at each fight's end.
	if character.GetAura("RageBar") != nil && d.BearForm == nil {
		effects = append(effects, rageBarEffect(character, 1))
	}
	return effects
}
