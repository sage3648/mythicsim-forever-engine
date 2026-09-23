package database

import (
	"testing"

	"github.com/wowsims/classic/sim/core/proto"
)

func TestForeverStandaloneLevelingItems(t *testing.T) {
	cases := []struct {
		name       string
		item       WowheadItem
		wantType   proto.ItemType
		wantArmor  proto.ArmorType
		wantWeapon proto.WeaponType
		wantHand   proto.HandType
		wantStat   proto.Stat
		wantValue  float64
	}{
		{
			name: "Tomb Robber's Gloves",
			item: WowheadItem{ID: 280096, Name: "Tomb Robber's Gloves", Quality: 3, Ilvl: 17,
				Class: 4, Subclass: 1, InventoryType: 10,
				Stats: WowheadStats{Armor: 21, Intellect: 6, Stamina: 3}},
			wantType: proto.ItemType_ItemTypeHands, wantArmor: proto.ArmorType_ArmorTypeCloth,
			wantStat: proto.Stat_StatIntellect, wantValue: 6,
		},
		{
			name: "Dusty Belt",
			item: WowheadItem{ID: 279897, Name: "Dusty Belt", Quality: 3, Ilvl: 17,
				Class: 4, Subclass: 2, InventoryType: 6,
				Stats: WowheadStats{Armor: 45, Agility: 5, Spirit: 4}},
			wantType: proto.ItemType_ItemTypeWaist, wantArmor: proto.ArmorType_ArmorTypeLeather,
			wantStat: proto.Stat_StatAgility, wantValue: 5,
		},
		{
			name: "Deepgrave Trousers",
			item: WowheadItem{ID: 279900, Name: "Deepgrave Trousers", Quality: 3, Ilvl: 17,
				Class: 4, Subclass: 2, InventoryType: 7,
				Stats: WowheadStats{Armor: 70, Agility: 3, Strength: 7, Spirit: 3}},
			wantType: proto.ItemType_ItemTypeLegs, wantArmor: proto.ArmorType_ArmorTypeLeather,
			wantStat: proto.Stat_StatStrength, wantValue: 7,
		},
		{
			name: "Monstrous Cleaver",
			item: WowheadItem{ID: 279864, Name: "Monstrous Cleaver", Quality: 3, Ilvl: 23,
				Class: 2, Subclass: 8, InventoryType: 17,
				Stats: WowheadStats{Stamina: 9, AttackPowerAlt: 10, DamageMin: 52, DamageMax: 78, Speed: 3.3}},
			wantType: proto.ItemType_ItemTypeWeapon, wantWeapon: proto.WeaponType_WeaponTypeSword,
			wantHand: proto.HandType_HandTypeTwoHand, wantStat: proto.Stat_StatAttackPower, wantValue: 10,
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got, ok := tc.item.ToStandaloneProto()
			if !ok {
				t.Fatal("valid equipped item was rejected")
			}
			if got.Type != tc.wantType || got.ArmorType != tc.wantArmor ||
				got.WeaponType != tc.wantWeapon || got.HandType != tc.wantHand {
				t.Fatalf("incorrect item type: %+v", got)
			}
			if got.Stats[tc.wantStat] != tc.wantValue {
				t.Fatalf("stat %s: got %v, want %v", tc.wantStat, got.Stats[tc.wantStat], tc.wantValue)
			}
			if tc.name == "Monstrous Cleaver" &&
				(got.WeaponDamageMin != 52 || got.WeaponDamageMax != 78 || got.WeaponSpeed != 3.3) {
				t.Fatalf("weapon damage was not imported: %+v", got)
			}
		})
	}
}

func TestForeverStandaloneRejectsIncompleteWeapon(t *testing.T) {
	item := WowheadItem{ID: 280000, Name: "Missing damage", Quality: 2, Ilvl: 20,
		Class: 2, Subclass: 8, InventoryType: 17}
	if _, ok := item.ToStandaloneProto(); ok {
		t.Fatal("weapon with no damage or speed must be rejected")
	}
}

func TestForeverStandaloneRejectsUnmodelledEffectOnlyItem(t *testing.T) {
	item := WowheadItem{ID: 280001, Name: "Unknown proc trinket", Quality: 3, Ilvl: 20,
		Class: 4, InventoryType: 12}
	if _, ok := item.ToStandaloneProto(); ok {
		t.Fatal("effect-only item must not be represented as a zero-stat item")
	}
}
