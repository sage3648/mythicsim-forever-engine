import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import architecture_check

GOLDEN_LOG = "[0.00] [Target 1] Aura gained: {SpellID: 1}\n[1.00] [mage (#1)] {SpellID: 2} Hit for 134.436 damage\n"


class CompareCaseTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.family = Path(self.temp.name) / "family"
        self.results = Path(self.temp.name) / "results"
        self.family.mkdir()
        self.results.mkdir()
        (self.family / "case.go-result.json").write_text(json.dumps({"raidMetrics": {"dps": {"avg": 500.0}}}))
        (self.family / "case.go-log.txt").write_text(GOLDEN_LOG)
        self.case = {"id": "case", "go_result": {"file": "case.go-result.json"},
                     "go_log": {"file": "case.go-log.txt"}}

    def tearDown(self):
        self.temp.cleanup()

    def run_with(self, logs, dps=500.0, error=None):
        result = {"raidMetrics": {"dps": {"avg": dps}}, "logs": logs}
        (self.results / "case.json").write_text(json.dumps(result))
        if error is not None:
            (self.results / "case.json.err").write_text(error)
        return architecture_check.compare_case(self.case, self.results, self.family)

    def test_a_run_with_the_stat_lines_the_golden_leaves_out_matches(self):
        logs = "[0.00] [Target 1] Dynamic stat change: {\"Armor\": -505.000,}\n" + GOLDEN_LOG
        self.assertEqual(self.run_with(logs), {"id": "case", "matched": True})

    def test_a_last_digit_names_the_golden_and_the_platform(self):
        row = self.run_with(GOLDEN_LOG.replace("134.436", "134.437"))
        self.assertFalse(row["matched"])
        self.assertNotIn("differences", row)
        self.assertEqual(row["first_log_difference"]["line"], 2)
        self.assertIn("134.436", row["first_log_difference"]["golden"])
        self.assertIn("134.437", row["first_log_difference"]["platform"])

    def test_a_result_difference_is_listed(self):
        row = self.run_with(GOLDEN_LOG, dps=501.0)
        self.assertEqual(row["differences"], ["/raidMetrics/dps/avg: Go 500.0, Rust 501.0"])

    def test_a_failed_run_is_an_error(self):
        row = self.run_with(GOLDEN_LOG, error="panic: boom\nfailed\n")
        self.assertIn("panic: boom", row["error"])
        self.assertNotIn("matched", row)


if __name__ == "__main__":
    unittest.main()
