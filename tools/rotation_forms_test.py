import json
import unittest

import rotation_forms


def conditions(value):
    """Every value name a request's rotation uses."""
    if isinstance(value, dict):
        for key, inner in value.items():
            yield key
            yield from conditions(inner)
    elif isinstance(value, list):
        for inner in value:
            yield from conditions(inner)


class RotationFormsTests(unittest.TestCase):
    def test_same_requests_every_time(self):
        self.assertEqual(rotation_forms.forms(), rotation_forms.forms())
        self.assertEqual(rotation_forms.casts(), rotation_forms.casts())

    def test_forms_cover_every_build_form_and_target_count(self):
        requests = rotation_forms.forms()
        self.assertEqual(len(requests), len(rotation_forms.form_plan()))
        self.assertEqual(len(requests), len(set(requests)))
        counts = {len(request["encounter"]["targets"]) for request in requests.values()}
        self.assertEqual(counts, {1, 2, 3, 5})
        used = set(conditions([request["raid"]["parties"][0]["players"][0]["rotation"]
                               for request in requests.values()]))
        for name in ("targetUnit", "sourceUnit", "target", "auraIsInactive", "dotIsActive", "multidot",
                     "dotRemainingTime", "dotTimeToNextTick"):
            self.assertIn(name, used)

    def test_a_form_changes_only_the_rotation_and_the_targets(self):
        base = json.loads(rotation_forms.PROFILES["affliction-warlock"]["base"].read_text())
        request = rotation_forms.forms()["affliction-warlock-dot-target-unit-1-target"]
        self.assertEqual(request["encounter"], base["encounter"])
        base_items = rotation_forms.priority_list(base)
        items = rotation_forms.priority_list(request)
        self.assertEqual(items[3:], base_items)
        request["raid"]["parties"][0]["players"][0]["rotation"]["priorityList"] = []
        base["raid"]["parties"][0]["players"][0]["rotation"]["priorityList"] = []
        self.assertEqual(request, base)

    def test_casts_aim_every_priority_cast_and_leave_the_rest(self):
        requests = rotation_forms.casts()
        request = requests["production-affliction-warlock-casts-at-next-3-targets"]
        self.assertEqual(len(request["encounter"]["targets"]), 3)
        for item in rotation_forms.priority_list(request):
            action = item["action"]
            if "castSpell" in action:
                self.assertEqual(action["castSpell"]["target"], {"type": "NextTarget"})
        self.assertNotIn("production-marksmanship-hunter-casts-at-next-3-targets", requests)


if __name__ == "__main__":
    unittest.main()
