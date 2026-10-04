import json
from pathlib import Path
import tempfile
import unittest

import preset_matrix

PRESETS = """
import AlphaApl from './apls/alpha.apl.json';
import BetaApl from './apls/beta.apl.json';
import LaunchGear from './gear_sets/launch.gear.json';
export const BLANK_APL = PresetUtils.makePresetAPLRotation('Blank', BlankAPL);
export const ALPHA = PresetUtils.makePresetAPLRotation('Alpha', AlphaApl);
export const BETA = PresetUtils.makePresetAPLRotation('Beta Two', BetaApl);
export const T1 = PresetUtils.makePresetTalents('Fire 0/35/16', SavedTalents.create({ talentsString: '-0355' }));
export const GEAR_BLANK = PresetUtils.makePresetGear('Blank', { items: [] });
export const GEAR_LAUNCH = PresetUtils.makePresetGear('Launch', LaunchGear);
"""


class PresetMatrixTest(unittest.TestCase):
    def test_presets_skip_inline_blanks_and_keep_source_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            spec = Path(tmp)
            (spec / "presets.ts").write_text(PRESETS)
            rotations, talents, gear = preset_matrix.presets(spec)
        self.assertEqual(rotations, [("Alpha", "apls/alpha.apl.json"), ("Beta Two", "apls/beta.apl.json")])
        self.assertEqual(talents, [("Fire 0/35/16", "-0355")])
        self.assertEqual(gear, [("Launch", "gear_sets/launch.gear.json")])

    def test_rotation_stems_pick_their_production_base(self):
        self.assertEqual(preset_matrix.base_name("hunter/dps", "apls/sv_melee.apl.json"), "survival-hunter")
        self.assertEqual(preset_matrix.base_name("hunter/dps", "apls/sv.apl.json"), "marksmanship-hunter")
        self.assertEqual(preset_matrix.base_name("mage/dps", "apls/fire_lowrank.apl.json"), "fire-mage")

    def test_presets_sharing_a_name_take_their_file_stem(self):
        gear = [("Launch", "gear_sets/launch.gear.json"), ("P1 BiS", "gear_sets/p1.gear.json"),
                ("Launch", "gear_sets/smite_launch.gear.json")]
        self.assertEqual(preset_matrix.labels(gear), ["launch", "p1-bis", "smite-launch"])
        self.assertEqual(preset_matrix.labels([("Fire", "-03"), ("Fire", "-05")]), ["fire-1", "fire-2"])

    def test_items_the_database_lacks_leave_their_slot_empty(self):
        gear, missing = preset_matrix.known_gear({"items": [{"id": 1}, {}, {"id": 2, "enchant": 9}]}, {1})
        self.assertEqual(gear, {"items": [{"id": 1}, {}, {}]})
        self.assertEqual(missing, [2])

    def test_build_replaces_rotation_talents_and_gear_only(self):
        base = {"raid": {"parties": [{"players": [{"race": "RaceTroll", "rotation": {}, "talentsString": "1",
                                                   "equipment": {"items": []}}]}]}}
        request = preset_matrix.build(base, {"type": "TypeAPL"}, "-0355", {"items": [{"id": 1}]})
        player = request["raid"]["parties"][0]["players"][0]
        self.assertEqual(player["race"], "RaceTroll")
        self.assertEqual(player["rotation"], {"type": "TypeAPL"})
        self.assertEqual(player["talentsString"], "-0355")
        self.assertEqual(player["equipment"], {"items": [{"id": 1}]})
        self.assertEqual(json.dumps(base["raid"]["parties"][0]["players"][0]["equipment"]), '{"items": []}')


if __name__ == "__main__":
    unittest.main()
