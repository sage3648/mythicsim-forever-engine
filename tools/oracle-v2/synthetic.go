// Synthetic scenarios: a registration the pinned Go engine does not make, for a mechanic no
// real build reaches.
//
// At the pin no registered spell is both a channel and a hardcast: every class channel has a
// global cooldown and no cast time, Diamond Flask (item 20130) has neither, and the client rows
// that state both (Mind Control, Ritual of Summoning, Ritual of Doom, Eyes of the Beast, Far
// Sight and Longsight) are registered by nothing. Go's "Pushback trigger" still has a branch
// for such a channel (sim/core/character.go: IsChanneled), so Rust ports it and needs a Go
// golden to check it against. This file adds one cast time to a real channel, through Go's own
// spell mod, for one item and only for requests that opt in by naming a player with the prefix
// below. Every other request registers nothing, so no prepared input or Go golden depends on
// this file.
package main

import (
	"strings"
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/mage"
)

// A request opts in by naming a player with this prefix.
const syntheticChannelCastPlayer = "synthetic-channel-cast"

// The item the registration hangs on: the Mithril Insignia, a trinket with stats and no effect
// in the client data, which no accepted request equips.
const syntheticChannelCastItem = 8663

// The cast time added to every Arcane Missiles rank.
const syntheticChannelCastTime = 3 * time.Second

// registerSyntheticItems registers the effects of the synthetic scenarios the request opts in
// to, before Go builds its characters.
func registerSyntheticItems(request *proto.RaidSimRequest) {
	for _, party := range request.GetRaid().GetParties() {
		for _, player := range party.GetPlayers() {
			if strings.HasPrefix(player.GetName(), syntheticChannelCastPlayer) && !core.HasItemEffect(syntheticChannelCastItem) {
				core.NewItemEffect(syntheticChannelCastItem, func(agent core.Agent) {
					agent.GetCharacter().AddStaticMod(core.SpellModConfig{
						ClassMask: mage.MageSpellArcaneMissilesCast,
						Kind:      core.SpellMod_CastTime_Flat,
						TimeValue: syntheticChannelCastTime,
					})
				})
			}
		}
	}
}
