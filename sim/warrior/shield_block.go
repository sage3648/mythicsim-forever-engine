package warrior

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

func (warrior *Warrior) RegisterShieldBlockCD() {
	// Forever beta client 1.60.1.69893: id, cost, cooldown, duration, school and block chance come from
	// the client table. Its 2 charges are not applied (ours blocks one attack), as before.
	row := spellData.ShieldBlock.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}
	cooldownDur := row.Cooldown
	blockChance := shared.SpellDataMin(row.Direct)

	warrior.ShieldBlockAura = warrior.RegisterAura(core.Aura{
		Label:     "Shield Block",
		ActionID:  actionID,
		Duration:  row.Duration, // 5 sec in Classic
		MaxStacks: 1,

		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.SetStacks(sim, aura.MaxStacks)
			warrior.AddStatDynamic(sim, stats.Block, blockChance*core.BlockRatingPerBlockChance)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warrior.AddStatDynamic(sim, stats.Block, -blockChance*core.BlockRatingPerBlockChance)
		},
		OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.DidBlock() {
				aura.RemoveStack(sim)
			}
		},
	})

	warrior.ShieldBlock = warrior.RegisterSpell(DefensiveStance, core.SpellConfig{
		ActionID:    actionID,
		SpellSchool: row.SpellSchool,

		RageCost: core.RageCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{},
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: cooldownDur,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warrior.PseudoStats.CanBlock
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			warrior.ShieldBlockAura.Activate(sim)
		},
	})

	warrior.AddMajorCooldown(core.MajorCooldown{
		Spell:    warrior.ShieldBlock.Spell,
		Priority: core.CooldownPriorityDefault,
		Type:     core.CooldownTypeSurvival,
		ShouldActivate: func(s *core.Simulation, c *core.Character) bool {
			// Only castable with manual APL Action
			return false
		},
	})
}
