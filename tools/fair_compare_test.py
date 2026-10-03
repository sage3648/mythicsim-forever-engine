import copy
import unittest

from fair_compare import matched_differences


class FairComparisonTests(unittest.TestCase):
    def report(self):
        return {
            "source_revision": "pin", "iterations": 3000, "seed": 42,
            "scenario_id": "fixed", "dps_mean": 100.0, "dps_stdev": 10.0,
            "dps_standard_error": 0.1, "mana_end_mean": 10.0,
            "mana_delta_mean": -100.0, "counts": {"casts": 10},
            "work": {"ready_checks": 20, "damage_rolls": 10},
        }

    def test_equal_dps_does_not_hide_unequal_work(self):
        a = self.report()
        b = copy.deepcopy(a)
        b["work"]["ready_checks"] = 100
        self.assertIn("work differs", matched_differences(a, b))

    def test_nonfinite_metric_is_never_accepted(self):
        a = self.report()
        b = copy.deepcopy(a)
        b["dps_mean"] = float("nan")
        self.assertTrue(matched_differences(a, b))

    def test_wrong_case_or_seed_is_rejected(self):
        a = self.report()
        b = copy.deepcopy(a)
        b["scenario_id"] = "other"
        b["seed"] = 173
        self.assertEqual(len(matched_differences(a, b)), 2)


if __name__ == "__main__":
    unittest.main()
