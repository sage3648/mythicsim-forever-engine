import json
from pathlib import Path
import stat
import sys
import tempfile
import textwrap
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import shadow

RESULT = {"raidMetrics": {"dps": {"avg": 500.0}}, "iterationsDone": 10}

# Stand-ins for the two binaries. Each reads its behavior from a file beside it, so one
# bundle covers every verdict.
FAKE = """#!/usr/bin/env python3
import json, sys
from pathlib import Path
mode = json.loads((Path(__file__).parent / "mode.json").read_text())
command, args = sys.argv[1], dict(zip(sys.argv[2::2], sys.argv[3::2]))
if mode.get("fail") == [Path(__file__).name, command]:
    sys.exit("broken")
if command == "prepare":
    Path(args["--outfile"]).write_text(json.dumps({"request": json.loads(Path(args["--infile"]).read_text())}))
elif command == "check":
    print(json.dumps({"supported": not mode.get("reasons"), "reasons": mode.get("reasons", [])}))
elif command == "sim" and Path(__file__).name == "forever-engine":
    Path(args["--outfile"]).write_text(json.dumps({"result": mode["rust"]}))
elif command == "sim":
    Path(args["--outfile"]).write_text(json.dumps(mode["go"]))
"""


class ShadowTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.bundle = self.root / "bundle"
        (self.bundle / "bin").mkdir(parents=True)
        for name in (shadow.ENGINE, shadow.EXPORTER):
            path = self.bundle / "bin" / name
            path.write_text(FAKE)
            path.chmod(path.stat().st_mode | stat.S_IEXEC)
        self.request = self.root / "request.json"
        self.request.write_text(json.dumps({"simOptions": {"iterations": 10}}))

    def tearDown(self):
        self.temp.cleanup()

    def verdict(self, **mode):
        mode.setdefault("go", RESULT)
        mode.setdefault("rust", RESULT)
        (self.bundle / "bin" / "mode.json").write_text(json.dumps(mode))
        return shadow.shadow(self.request, self.root / "out", self.bundle, seed=7, timeout=30)

    def test_seed_fills_only_an_unseeded_request(self):
        self.assertEqual(shadow.seeded({}, 7)["simOptions"]["randomSeed"], "7")
        self.assertEqual(shadow.seeded({"simOptions": {"randomSeed": "0"}}, 7)["simOptions"]["randomSeed"], "7")
        self.assertEqual(shadow.seeded({"simOptions": {"randomSeed": "42"}}, 7)["simOptions"]["randomSeed"], "42")

    def test_equal_results_match_and_report_timings(self):
        verdict = self.verdict()
        self.assertEqual(verdict["status"], "match")
        self.assertEqual(verdict["go_dps"], 500.0)
        self.assertEqual(verdict["iterations"], 10)
        self.assertEqual(set(verdict["timings_ms"]), {"prepare", "go", "rust"})
        self.assertIn("speedup", verdict)
        sent = json.loads((self.root / "out" / "request.json").read_text())
        self.assertEqual(sent["simOptions"]["randomSeed"], "7")

    def test_a_different_metric_is_a_mismatch(self):
        rust = {**RESULT, "raidMetrics": {"dps": {"avg": 501.0}}}
        verdict = self.verdict(rust=rust)
        self.assertEqual(verdict["status"], "mismatch")
        self.assertEqual(verdict["difference_count"], 1)
        self.assertIn("/raidMetrics/dps/avg", verdict["differences"][0])

    def test_a_different_log_line_is_a_mismatch(self):
        verdict = self.verdict(go={**RESULT, "logs": "a\nb\n"}, rust={**RESULT, "logs": "a\nc\n"})
        self.assertEqual(verdict["status"], "mismatch")
        self.assertEqual(verdict["first_log_difference"]["line"], 2)

    def test_gate_reasons_are_a_refusal(self):
        verdict = self.verdict(reasons=["needs a mechanic"])
        self.assertEqual(verdict["status"], "refused")
        self.assertEqual(verdict["reasons"], ["needs a mechanic"])
        self.assertNotIn("go", verdict["timings_ms"])

    def test_a_failing_engine_is_an_error_at_its_stage(self):
        verdict = self.verdict(fail=[shadow.ENGINE, "sim"])
        self.assertEqual(verdict["status"], "error")
        self.assertEqual(verdict["stage"], "rust")
        self.assertIn("broken", verdict["error"])

    def test_a_go_error_result_is_an_error(self):
        verdict = self.verdict(go={"error": {"message": "panic"}})
        self.assertEqual(verdict["status"], "error")
        self.assertEqual(verdict["stage"], "go")


if __name__ == "__main__":
    unittest.main()
