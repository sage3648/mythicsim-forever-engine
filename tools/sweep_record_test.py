import json
from pathlib import Path
import stat
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import prepared_v2
import sweep_record

REQUEST = Path(__file__).resolve().parents[1] / "fixtures" / "mage" / "prepared-v2" / "arcane-reference.request.json"

# A stand-in for the exporter and the engine. It reads what to do from a file beside it.
FAKE = """#!/usr/bin/env python3
import json, sys
from pathlib import Path
mode = json.loads((Path(__file__).parent / "mode.json").read_text())
name, command = Path(__file__).name, sys.argv[1]
args = dict(zip(sys.argv[2::2], sys.argv[3::2]))
step = "rust" if name == "engine" else ("prepare" if command == "prepare" else "go")
if mode.get("fail") == step:
    sys.exit("broken")
if step == "rust" and mode.get("refuse"):
    sys.exit("prepared input unsupported:\\n  rotation reaches spell 10202 without a known behavior")
result = {"raidMetrics": {"dps": {"avg": 500.0}}, "logs": ""}
if step == "prepare":
    Path(args["--outfile"]).write_text("{}")
elif step == "go":
    Path(args["--outfile"]).write_text(json.dumps(result))
else:
    if mode.get("differ"):
        result["raidMetrics"]["dps"]["avg"] = 501.0
    Path(args["--outfile"]).write_text(json.dumps({"result": result}))
"""


class RecordTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        for name in ("exporter", "engine"):
            path = self.root / name
            path.write_text(FAKE)
            path.chmod(path.stat().st_mode | stat.S_IEXEC)

    def tearDown(self):
        self.temp.cleanup()

    def compare(self, scenario, **mode):
        (self.root / "mode.json").write_text(json.dumps(mode))
        return prepared_v2.compare_request(self.root / "exporter", self.root / "engine", REQUEST, scenario,
                                           self.root / "out" / scenario)

    def test_each_outcome_is_its_own_row(self):
        self.assertTrue(self.compare("match")["passed"])
        mismatch = self.compare("mismatch", differ=True)
        self.assertFalse(mismatch["passed"])
        self.assertEqual(mismatch["differences"], ["/raidMetrics/dps/avg: Go 500.0, Rust 501.0"])
        self.assertTrue(self.compare("refused", refuse=True)["rust_error"].startswith("prepared input unsupported"))
        for stage in ("prepare", "go", "rust"):
            with self.subTest(stage=stage):
                row = self.compare(f"fails-{stage}", fail=stage)
                self.assertFalse(row["passed"])
                self.assertEqual(row["error"]["stage"], stage)
                self.assertNotIn("rust_error", row)

    def test_the_record_counts_errors_apart_from_refusals_and_mismatches(self):
        rows = [self.compare("match"), self.compare("mismatch", differ=True), self.compare("refused", refuse=True),
                self.compare("fails-go", fail="go")]
        (self.root / "out" / "summary.json").write_text(json.dumps({"results": rows}))
        requests = []
        for row in rows:
            path = self.root / f"{row['scenario']}.request.json"
            path.write_text(REQUEST.read_text())
            requests.append(path)
        record = sweep_record.record(self.root / "out", requests, "scope", "generator", [])
        self.assertEqual((record["matched"], record["rejected"], record["mismatched"], record["errors"]), (1, 1, 1, 1))
        by_scenario = {case["scenario"]: case for case in record["cases"]}
        self.assertEqual(by_scenario["fails-go"]["error"]["stage"], "go")
        self.assertEqual(by_scenario["refused"]["rejected"],
                         ["rotation reaches spell 10202 without a known behavior"])
        self.assertEqual(by_scenario["mismatch"]["differences"], ["/raidMetrics/dps/avg: Go 500.0, Rust 501.0"])


if __name__ == "__main__":
    unittest.main()
