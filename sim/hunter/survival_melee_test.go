package hunter

import (
	"testing"

	"github.com/wowsims/forever/sim/core"
)

// MythicSim's melee Survival: wowtbc.gg's 5/10/35 build with its unspent point in Focused Fire, in
// melee range on sv_melee.apl.json. TestSurvival sims Survival talents from range.
var SurvivalMeleeTalents = "501-005005-500230230250222151"

func TestSurvivalMelee(t *testing.T) {
	config := hunterSuite("sv_melee", SurvivalMeleeTalents)
	config.StartingDistance = core.MaxMeleeRange
	core.RunTestSuite(t, t.Name(), core.FullCharacterTestSuiteGenerator([]core.CharacterSuiteConfig{config}))
}
