package hunter

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/stats"
)

// Aspect of the Hawk is the ranged aspect and Aspect of the Beast the melee one. Forever gave Beast
// melee attack power (110 at rank 4) and a Quick Strikes proc behind Deadly Aspects, the melee twin
// of Hawk's Quick Shots; Classic's Beast only made you untrackable. A hunter holds one aspect at a
// time. Aspect of the Viper is TBC's and does not exist on Forever.
func (hunter *Hunter) registerAspects() {
	hunter.registerAspectOfTheHawkSpell()
	hunter.registerAspectOfTheBeastSpell()
}

func (hunter *Hunter) registerAspectOfTheHawkSpell() {
	hawkRank := spellData.AspectOfTheHawk.Highest()
	actionID := core.ActionID{SpellID: hawkRank.ID}

	// Every rank of Deadly Aspects triggers the same Quick Shots (6150): 30% ranged haste for 12
	// sec. The points buy only the proc chance, 2% a rank.
	var quickShots *core.Aura
	if hunter.Talents.DeadlyAspects > 0 {
		quickShotsRank := spellData.AspectOfTheHawkTriggered.Highest()
		hasteMultiplier := 1 + quickShotsRank.Effect(dbcenums.A_MOD_RANGED_HASTE, 0).Average(core.CharacterLevel)/100

		quickShots = hunter.GetOrRegisterAura(core.Aura{
			Label:    "Quick Shots",
			ActionID: core.ActionID{SpellID: quickShotsRank.ID},
			Duration: quickShotsRank.Duration(),
			OnGain: func(aura *core.Aura, sim *core.Simulation) {
				aura.Unit.MultiplyRangedSpeed(sim, hasteMultiplier)
			},
			OnExpire: func(aura *core.Aura, sim *core.Simulation) {
				aura.Unit.MultiplyRangedSpeed(sim, 1/hasteMultiplier)
			},
		})
	}

	rap := hawkRank.Effect(dbcenums.A_MOD_RANGED_ATTACK_POWER, 0).Average(core.CharacterLevel)
	// The row states the same 2% a rank twice, once per aspect the talent covers.
	procChance := spellData.DeadlyAspects.EffectAt(1).FractionAt(hunter.Talents.DeadlyAspects)

	hunter.AspectOfTheHawkAura = hunter.GetOrRegisterAura(core.Aura{
		Label:      "Aspect of the Hawk",
		ActionID:   actionID,
		Duration:   core.NeverExpires,
		BuildPhase: core.CharacterBuildPhaseNone,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.AddStatDynamic(sim, stats.RangedAttackPower, rap)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.AddStatDynamic(sim, stats.RangedAttackPower, -rap)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if quickShots == nil || !spell.ProcMask.Matches(core.ProcMaskRangedAuto) {
				return
			}
			if sim.Proc(procChance, "Deadly Aspects") {
				quickShots.Activate(sim)
			}
		},
	})
	hunter.AspectOfTheHawkAura.NewExclusiveEffect("Aspect", true, core.ExclusiveEffect{})

	hunter.AspectOfTheHawk = hunter.RegisterSpell(core.SpellConfig{
		ActionID:       actionID,
		SpellSchool:    hawkRank.SpellSchool(),
		DefenseType:    hawkRank.DefenseTypeCore(),
		ClassSpellMask: HunterSpellAspectOfTheHawk,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          core.SpellFlagAPL,

		ManaCost: core.ManaCostOptions{
			FlatCost: int32(hawkRank.Cost()),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: hawkRank.GCD(),
			},
			IgnoreHaste: true,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return !hunter.AspectOfTheHawkAura.IsActive()
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			hunter.AspectOfTheHawkAura.Activate(sim)
		},

		RelatedSelfBuff: hunter.AspectOfTheHawkAura,
	})
}

func (hunter *Hunter) registerAspectOfTheBeastSpell() {
	beastRank := spellData.AspectOfTheBeast.Highest()
	actionID := core.ActionID{SpellID: beastRank.ID}

	// Deadly Aspects' second effect is Beast's proc chance, 2% a rank, and it triggers Quick Strikes
	// (1299448): 30% melee haste for 12 sec. Beast states no chance of its own, so the proc needs
	// the talent.
	var quickStrikes *core.Aura
	if hunter.Talents.DeadlyAspects > 0 {
		quickStrikesRank := spellData.AspectOfTheBeastTriggered.Highest()
		hasteMultiplier := 1 + quickStrikesRank.Effect(dbcenums.A_MOD_MELEE_HASTE_3, 0).Average(core.CharacterLevel)/100

		quickStrikes = hunter.GetOrRegisterAura(core.Aura{
			Label:    "Quick Strikes",
			ActionID: core.ActionID{SpellID: quickStrikesRank.ID},
			Duration: quickStrikesRank.Duration(),
			OnGain: func(aura *core.Aura, sim *core.Simulation) {
				aura.Unit.MultiplyMeleeSpeed(sim, hasteMultiplier)
			},
			OnExpire: func(aura *core.Aura, sim *core.Simulation) {
				aura.Unit.MultiplyMeleeSpeed(sim, 1/hasteMultiplier)
			},
		})
	}

	ap := beastRank.Effect(dbcenums.A_MOD_ATTACK_POWER, 0).Average(core.CharacterLevel)
	procChance := spellData.DeadlyAspects.EffectAt(2).FractionAt(hunter.Talents.DeadlyAspects)

	hunter.AspectOfTheBeastAura = hunter.GetOrRegisterAura(core.Aura{
		Label:      "Aspect of the Beast",
		ActionID:   actionID,
		Duration:   core.NeverExpires,
		BuildPhase: core.CharacterBuildPhaseNone,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.AddStatDynamic(sim, stats.AttackPower, ap)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.AddStatDynamic(sim, stats.AttackPower, -ap)
		},
		// Beast's proc flags are melee auto attacks only (0x4), so a Raptor Strike, which takes a
		// main-hand swing's place as a special attack, never procs it.
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if quickStrikes == nil || !spell.ProcMask.Matches(core.ProcMaskMeleeWhiteHit) || !result.Landed() {
				return
			}
			if sim.Proc(procChance, "Deadly Aspects") {
				quickStrikes.Activate(sim)
			}
		},
	})
	hunter.AspectOfTheBeastAura.NewExclusiveEffect("Aspect", true, core.ExclusiveEffect{})

	hunter.AspectOfTheBeast = hunter.RegisterSpell(core.SpellConfig{
		ActionID:       actionID,
		SpellSchool:    beastRank.SpellSchool(),
		DefenseType:    beastRank.DefenseTypeCore(),
		ClassSpellMask: HunterSpellAspectOfTheBeast,
		ProcMask:       core.ProcMaskEmpty,
		Flags:          core.SpellFlagAPL,

		ManaCost: core.ManaCostOptions{
			FlatCost: int32(beastRank.Cost()),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: beastRank.GCD(),
			},
			IgnoreHaste: true,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return !hunter.AspectOfTheBeastAura.IsActive()
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			hunter.AspectOfTheBeastAura.Activate(sim)
		},

		RelatedSelfBuff: hunter.AspectOfTheBeastAura,
	})
}
