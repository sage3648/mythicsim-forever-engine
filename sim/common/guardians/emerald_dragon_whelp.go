package guardians

import (
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

var DragonsCall = int32(10847)

type EmeraldDragonWhelp struct {
	core.Pet

	acidSpit *core.Spell

	// When the current summon ends, so the whelp skips a spit it would not live to finish.
	disabledAt time.Duration
	timeout    *core.PendingAction
}

func NewEmeraldDragonWhelp(character *core.Character) *EmeraldDragonWhelp {
	whelpBaseStats := stats.Stats{
		stats.Health:      1500, // https://wowwiki-archive.fandom.com/wiki/Dragon%27s_Call
		stats.Intellect:   20,   // Adding the base 20 intellect to not mess with the base mana function
		stats.Mana:        500,  // TODO: Assumed value. The whelp seems to cast 3 Acid Spits (90 mana) per spawn (Rain: In the log you can see a whelp casting 4 acid spits so i'm increasing this to 500)
		stats.SpellDamage: 155,  // 220 on the old flat 374 matched the log below (~594 a spit); the client's 374-503 averages 438.5, so 155 keeps it
		// Based on this log but more data needed
		// https://sod.warcraftlogs.com/reports/xTwQVgbjF9cPnd3R#type=damage-done&ability=-13049&view=events&boss=-2&difficulty=0&wipes=2
		stats.MeleeCrit: 4.5 * core.CritRatingPerCritChance,
		stats.SpellCrit: 13 * core.CritRatingPerCritChance,
	}

	whelp := &EmeraldDragonWhelp{
		Pet: core.NewPet("Emerald Dragon Whelp", character, whelpBaseStats, emeraldWhelpingStatInheritance(), false, true),
	}
	whelp.Level = 55

	whelp.EnableManaBar()

	whelp.EnableAutoAttacks(whelp, core.AutoAttackOptions{
		// TODO: Need whelp data
		MainHand: core.Weapon{
			// These stats are a complete guess from looking at the lone log I could find with Dragon's Call below
			// https://vanilla.warcraftlogs.com/reports/tQW9mqDrx3R4AdYZ#type=damage-done&ability=-13049&boss=-2&difficulty=0&wipes=2&source=25
			BaseDamageMin: 80.0,
			BaseDamageMax: 100.0,
			SwingSpeed:    2.0,
			SpellSchool:   core.SpellSchoolPhysical,
		},
		AutoSwingMelee: true,
	})

	return whelp
}

func emeraldWhelpingStatInheritance() core.PetStatInheritance {
	return func(ownerStats stats.Stats) stats.Stats {
		// TODO: Needs more verification
		return stats.Stats{}
	}
}

func (whelp *EmeraldDragonWhelp) Initialize() {
	whelp.registerAcidSpitSpell()
}

// Summons the whelp for duration. A proc while it is out refreshes it rather than adding a
// second whelp, and the refreshed summon lasts the full duration from now.
func (whelp *EmeraldDragonWhelp) Summon(sim *core.Simulation, duration time.Duration) {
	whelp.disabledAt = sim.CurrentTime + duration
	if whelp.timeout != nil {
		whelp.timeout.Cancel(sim)
	}
	whelp.Enable(sim, whelp)
	whelp.timeout = &core.PendingAction{
		NextActionAt: whelp.disabledAt,
		OnAction: func(sim *core.Simulation) {
			whelp.timeout = nil
			whelp.Disable(sim)
		},
	}
	sim.AddPendingAction(whelp.timeout)
}

// After each swing or spit, spits half the time and otherwise waits for the next swing. The
// spit resets the swing timer, and one the whelp would not live to finish is skipped.
func (whelp *EmeraldDragonWhelp) ExecuteCustomRotation(sim *core.Simulation) {
	// Run the cast check only on swings or cast completes
	if whelp.AutoAttacks.NextAttackAt() != sim.CurrentTime+whelp.AutoAttacks.MainhandSwingSpeed() && whelp.AutoAttacks.NextAnyAttackAt()-1 > sim.CurrentTime {
		whelp.WaitUntil(sim, whelp.AutoAttacks.NextAttackAt()-1)
		return
	}

	if sim.CurrentTime+whelp.acidSpit.CastTime() < whelp.disabledAt && whelp.acidSpit.CanCast(sim, whelp.CurrentTarget) && sim.Proc(0.5, "Acid Spit Cast") {
		whelp.acidSpit.Cast(sim, whelp.CurrentTarget)
		return
	}

	whelp.WaitUntil(sim, max(sim.CurrentTime, whelp.AutoAttacks.NextAttackAt()-1))
}

func (whelp *EmeraldDragonWhelp) Reset(sim *core.Simulation) {
	whelp.timeout = nil
	whelp.Disable(sim)
}

func (whelp *EmeraldDragonWhelp) OnPetDisable(sim *core.Simulation) {
}

func (whelp *EmeraldDragonWhelp) GetPet() *core.Pet {
	return &whelp.Pet
}

func (whelp *EmeraldDragonWhelp) registerAcidSpitSpell() {
	actionID := core.ActionID{SpellID: 9591}

	whelp.acidSpit = whelp.RegisterSpell(core.SpellConfig{
		ActionID:    actionID,
		SpellSchool: core.SpellSchoolNature,
		DefenseType: core.DefenseTypeMagic,
		ProcMask:    core.ProcMaskSpellDamage,
		// All of the casts and hits in the above log had the same damage so it would seem debuffs are ignored
		Flags: core.SpellFlagIgnoreModifiers | core.SpellFlagResetAttackSwing,

		ManaCost: core.ManaCostOptions{
			FlatCost: 90,
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: time.Second * 3,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			// TODO: The one log i was looking at has 0 misses on the spell but it also has only 25 casts
			// so i can't make a good assumption. Right now we leave it with a hit check and we can remove later.
			// Client 1.60.1.69977 (spell 9591): 438.5 +-29.3%, so 374 to 503 Nature, where Era rolls
			// around 64. The whelp hits roughly six times as hard in Forever.
			spell.CalcAndDealDamage(sim, target, sim.Roll(374, 503), spell.OutcomeMagicHitAndCrit)
		},
	})
}

func constructEmeralDragonWhelps(character *core.Character) {
	if character.HasMHWeapon() && character.GetMHWeapon().ID == DragonsCall ||
		character.HasOHWeapon() && character.GetOHWeapon().ID == DragonsCall {
		// Original could have up to 3 whelps active at a time however the SoD version seems to only summon 1 whelp on a 1 minute cooldown
		character.AddPet(NewEmeraldDragonWhelp(character))
	}
}
