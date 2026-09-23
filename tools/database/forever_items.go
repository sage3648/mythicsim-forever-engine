package database

import "github.com/wowsims/classic/sim/core/proto"

// ToStandaloneProto builds a complete item from Forever's gear planner. The
// regular ToProto method only supplies metadata because Classic tooltips
// normally supply the slot, armor type, weapon damage, and base stats. Forever
// has thousands of items without Classic tooltips, including leveling gear.
func (wi WowheadItem) ToStandaloneProto() (*proto.UIItem, bool) {
	if wi.ID <= 0 || wi.Name == "" || wi.Ilvl <= 0 || wi.Quality < 0 {
		return nil, false
	}

	item := wi.ToProto()
	item.Type = foreverItemType(wi.InventoryType)
	if item.Type == proto.ItemType_ItemTypeUnknown {
		return nil, false
	}
	item.Quality = proto.ItemQuality(wi.Quality)
	item.Stats = toSlice(statsOf(wi.Stats))
	item.Unique = wi.Stats.MaxCount == 1
	item.SetId = wi.Stats.ItemSet

	switch wi.Class {
	case 4: // Armor, shields, and relics.
		item.ArmorType = foreverArmorType(wi.Subclass)
		if item.Type == proto.ItemType_ItemTypeWeapon {
			switch wi.InventoryType {
			case 14:
				item.WeaponType = proto.WeaponType_WeaponTypeShield
			case 23:
				item.WeaponType = proto.WeaponType_WeaponTypeOffHand
			default:
				return nil, false
			}
			item.HandType = proto.HandType_HandTypeOffHand
		} else if item.Type == proto.ItemType_ItemTypeRanged {
			item.RangedWeaponType = foreverRelicType(wi.Subclass)
			if item.RangedWeaponType == proto.RangedWeaponType_RangedWeaponTypeUnknown {
				return nil, false
			}
		}
	case 2: // Weapons.
		item.WeaponType = foreverWeaponType(wi.Subclass)
		item.RangedWeaponType = foreverRangedType(wi.Subclass)
		if item.Type == proto.ItemType_ItemTypeWeapon {
			if item.WeaponType == proto.WeaponType_WeaponTypeUnknown {
				return nil, false
			}
			switch wi.InventoryType {
			case 13:
				item.HandType = proto.HandType_HandTypeOneHand
			case 17:
				item.HandType = proto.HandType_HandTypeTwoHand
			case 21:
				item.HandType = proto.HandType_HandTypeMainHand
			case 22:
				item.HandType = proto.HandType_HandTypeOffHand
			default:
				return nil, false
			}
		} else if item.Type != proto.ItemType_ItemTypeRanged ||
			item.RangedWeaponType == proto.RangedWeaponType_RangedWeaponTypeUnknown {
			return nil, false
		}
		item.WeaponDamageMin = wi.Stats.DamageMin
		item.WeaponDamageMax = wi.Stats.DamageMax
		item.WeaponSpeed = wi.Stats.Speed
		if item.WeaponDamageMin <= 0 || item.WeaponDamageMax < item.WeaponDamageMin || item.WeaponSpeed <= 0 {
			return nil, false
		}
	default:
		return nil, false
	}
	if item.WeaponDamageMin == 0 {
		hasStaticStats := false
		for _, value := range item.Stats {
			if value != 0 {
				hasStaticStats = true
				break
			}
		}
		if !hasStaticStats {
			// The planner does not describe item procs. A zero-stat item
			// would appear equipped while contributing nothing to the sim.
			return nil, false
		}
	}

	return item, true
}

func foreverItemType(inventory int32) proto.ItemType {
	switch inventory {
	case 1:
		return proto.ItemType_ItemTypeHead
	case 2:
		return proto.ItemType_ItemTypeNeck
	case 3:
		return proto.ItemType_ItemTypeShoulder
	case 5, 20:
		return proto.ItemType_ItemTypeChest
	case 6:
		return proto.ItemType_ItemTypeWaist
	case 7:
		return proto.ItemType_ItemTypeLegs
	case 8:
		return proto.ItemType_ItemTypeFeet
	case 9:
		return proto.ItemType_ItemTypeWrist
	case 10:
		return proto.ItemType_ItemTypeHands
	case 11:
		return proto.ItemType_ItemTypeFinger
	case 12:
		return proto.ItemType_ItemTypeTrinket
	case 13, 14, 17, 21, 22, 23:
		return proto.ItemType_ItemTypeWeapon
	case 15, 25, 26, 28:
		return proto.ItemType_ItemTypeRanged
	case 16:
		return proto.ItemType_ItemTypeBack
	default:
		return proto.ItemType_ItemTypeUnknown
	}
}

func foreverArmorType(subclass int32) proto.ArmorType {
	switch subclass {
	case 1:
		return proto.ArmorType_ArmorTypeCloth
	case 2:
		return proto.ArmorType_ArmorTypeLeather
	case 3:
		return proto.ArmorType_ArmorTypeMail
	case 4:
		return proto.ArmorType_ArmorTypePlate
	default:
		return proto.ArmorType_ArmorTypeUnknown
	}
}

func foreverWeaponType(subclass int32) proto.WeaponType {
	switch subclass {
	case 0, 1:
		return proto.WeaponType_WeaponTypeAxe
	case 4, 5:
		return proto.WeaponType_WeaponTypeMace
	case 6:
		return proto.WeaponType_WeaponTypePolearm
	case 7, 8:
		return proto.WeaponType_WeaponTypeSword
	case 10:
		return proto.WeaponType_WeaponTypeStaff
	case 13:
		return proto.WeaponType_WeaponTypeFist
	case 15:
		return proto.WeaponType_WeaponTypeDagger
	default:
		return proto.WeaponType_WeaponTypeUnknown
	}
}

func foreverRangedType(subclass int32) proto.RangedWeaponType {
	switch subclass {
	case 2:
		return proto.RangedWeaponType_RangedWeaponTypeBow
	case 3:
		return proto.RangedWeaponType_RangedWeaponTypeGun
	case 16:
		return proto.RangedWeaponType_RangedWeaponTypeThrown
	case 18:
		return proto.RangedWeaponType_RangedWeaponTypeCrossbow
	case 19:
		return proto.RangedWeaponType_RangedWeaponTypeWand
	default:
		return proto.RangedWeaponType_RangedWeaponTypeUnknown
	}
}

func foreverRelicType(subclass int32) proto.RangedWeaponType {
	switch subclass {
	case 7:
		return proto.RangedWeaponType_RangedWeaponTypeLibram
	case 8:
		return proto.RangedWeaponType_RangedWeaponTypeIdol
	case 9:
		return proto.RangedWeaponType_RangedWeaponTypeTotem
	case 10:
		return proto.RangedWeaponType_RangedWeaponTypeSigil
	default:
		return proto.RangedWeaponType_RangedWeaponTypeUnknown
	}
}
