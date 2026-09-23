package main

import (
	"testing"

	"github.com/wowsims/classic/sim/core/proto"
)

func TestSimmableStarterGear(t *testing.T) {
	item := &proto.UIItem{
		Id:      263005,
		Name:    "Thendal Watcher's Vest",
		Ilvl:    8,
		Quality: proto.ItemQuality_ItemQualityUncommon,
		Type:    proto.ItemType_ItemTypeChest,
	}
	if !simmableItemFilter(item.Id, item) {
		t.Fatal("valid starter gear must remain in the sim database")
	}
}
