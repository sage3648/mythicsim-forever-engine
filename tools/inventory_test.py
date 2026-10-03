import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

import inventory


class InventoryAuditTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name) / "inventory"
        shutil.copytree(inventory.DIRECTORY, self.directory)

    def edit(self, filename, change, rehash=False):
        path = self.directory / filename
        value = inventory.load(path)
        change(value)
        path.write_text(json.dumps(value))
        if rehash:
            self.edit("manifest.json", lambda m: m["artifacts"].update({filename: inventory.digest(path)}))

    def test_frozen_inventory_passes_without_go_or_application_source(self):
        inventory.check(self.directory)

    def test_changed_frozen_values_need_explicit_review(self):
        self.edit("snapshot.json", lambda s: s["request"]["encounter"].update({"duration": 300}))
        with self.assertRaisesRegex(ValueError, "changed frozen artifact"):
            inventory.check(self.directory)

    def test_new_apl_operator_needs_inventory_even_if_snapshot_is_rehashed(self):
        def add_operator(s):
            player = s["request"]["raid"]["parties"][0]["players"][0]
            player["rotation"]["priorityList"].append({"action": {"wait": {}}})
        self.edit("snapshot.json", add_operator, rehash=True)
        with self.assertRaisesRegex(ValueError, "request field coverage"):
            inventory.check(self.directory)

    def test_new_spell_identity_needs_inventory_even_with_same_field_shape(self):
        def change_spell(s):
            player = s["request"]["raid"]["parties"][0]["players"][0]
            player["rotation"]["priorityList"][-1]["action"]["castSpell"]["spellId"]["spellId"] = 133
        self.edit("snapshot.json", change_spell, rehash=True)
        with self.assertRaisesRegex(ValueError, "action identity coverage"):
            inventory.check(self.directory)

    def test_talent_rank_or_missing_talent_breaks_coverage(self):
        self.edit("manifest.json", lambda m: m["talents"].pop())
        with self.assertRaisesRegex(ValueError, "talent coverage"):
            inventory.check(self.directory)

    def test_enchant_change_cannot_hide_behind_item_id(self):
        self.edit("manifest.json", lambda m: m["equipment"][0]["input"].update({"enchant": 0}))
        with self.assertRaisesRegex(ValueError, "enchant coverage"):
            inventory.check(self.directory)

    def test_report_tag_is_part_of_action_identity(self):
        self.edit("manifest.json", lambda m: m["action_ids"][0].update({"tag": 99}))
        with self.assertRaisesRegex(ValueError, "action identity coverage"):
            inventory.check(self.directory)

    def test_requirement_without_traceable_source_fails(self):
        self.edit("manifest.json", lambda m: m["known_gaps"][0].update({"evidence": []}))
        with self.assertRaisesRegex(ValueError, "missing source evidence"):
            inventory.check(self.directory)

    def test_inventory_cannot_declare_runtime_support(self):
        self.edit("manifest.json", lambda m: m.update({"production_eligible": True}))
        with self.assertRaisesRegex(ValueError, "inventory is not runtime support"):
            inventory.check(self.directory)

    def test_incomplete_go_reference_is_rejected_even_if_rehashed(self):
        self.edit("observation.json", lambda o: o["summary"].update({"IterationsDone": 2999}), rehash=True)
        with self.assertRaisesRegex(ValueError, "reference did not finish"):
            inventory.check(self.directory)

    def test_supplied_source_file_drift_is_detected(self):
        source_root = Path(self.temporary.name) / "app"
        manifest = inventory.load(self.directory / "manifest.json")
        for entry in manifest["source_files"].values():
            if entry["repository"] != "app":
                continue
            path = source_root / entry["path"]
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("synthetic source fixture\n")
            entry["sha256"] = inventory.digest(path)
        (self.directory / "manifest.json").write_text(json.dumps(manifest))
        revision = manifest["sources"]["app"]["revision"]
        with patch("inventory.subprocess.check_output", return_value=revision):
            inventory.check(self.directory, app_source=source_root)
            (source_root / "worker/forever/request.go").write_text("new request behavior\n")
            with self.assertRaisesRegex(ValueError, "source drift: app:worker/forever/request.go"):
                inventory.check(self.directory, app_source=source_root)

    def test_source_revision_is_checked_in_addition_to_file_hashes(self):
        with patch("inventory.subprocess.check_output", return_value="different commit"):
            with self.assertRaisesRegex(ValueError, "app revision differs"):
                inventory.check(self.directory, app_source=Path(self.temporary.name))


class ObservationComparisonTests(unittest.TestCase):
    def test_actual_frozen_output_matches_itself(self):
        observation = inventory.load(inventory.DIRECTORY / "observation.json")
        result = inventory.compare_observations(observation, observation)
        self.assertTrue(result["passed"])
        self.assertGreater(result["numeric_fields"], 100)

    def test_one_extra_cast_fails_even_for_large_count(self):
        result = inventory.compare_observations({"casts": 1000000000}, {"casts": 1000000001})
        self.assertFalse(result["passed"])
        self.assertEqual(result["differences"][0]["path"], "/casts")

    def test_damage_aura_resource_and_timeline_changes_fail(self):
        for key in ("damage", "uptimeSecondsAvg", "actualGain", "start"):
            with self.subTest(key=key):
                self.assertFalse(inventory.compare_observations({key: 100.5}, {key: 100.51})["passed"])

    def test_floating_point_roundoff_is_allowed(self):
        self.assertTrue(inventory.compare_observations({"damage": 100.5}, {"damage": 100.5 + 1e-10})["passed"])

    def test_missing_field_and_nonfinite_metric_fail(self):
        self.assertFalse(inventory.compare_observations({"damage": 1.0}, {})["passed"])
        self.assertFalse(inventory.compare_observations({"damage": 1.0}, {"damage": float("nan")})["passed"])


if __name__ == "__main__":
    unittest.main()
