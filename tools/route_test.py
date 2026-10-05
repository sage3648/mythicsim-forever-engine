import json
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import route

RESULT = {"iterationsDone": 10, "raidMetrics": {"dps": {"avg": 500.0}}}
REFUSALS = [{"code": "unknown_spell", "reason": "rotation reaches spell 10202 without a known behavior"},
            {"code": "class_limit", "reason": "Deadly Aspects is incomplete"},
            {"code": "unknown_spell", "reason": "rotation reaches spell 10216 without a known behavior"}]

# Stand-ins for the two binaries. Each reads its behavior from a file beside it, so one
# bundle covers every decision.
FAKE = """#!/usr/bin/env python3
import json, sys, time
from pathlib import Path
mode = json.loads((Path(__file__).parent / "mode.json").read_text())
command, args = sys.argv[1], dict(zip(sys.argv[2::2], sys.argv[3::2]))
step = {"prepare": "prepare", "check": "check", "sim": "rust"}[command]
if mode.get("fail") == step:
    sys.exit("broken")
if mode.get("hang") == step:
    time.sleep(5)
if command == "prepare":
    request = json.loads(Path(args["--infile"]).read_text())
    Path(args["--outfile"]).write_text(json.dumps({"request": request}))
elif command == "check":
    if "check_output" in mode:
        print(mode["check_output"])
    else:
        refusals = mode.get("refusals", [])
        print(json.dumps({"supported": not refusals, "reasons": [r["reason"] for r in refusals],
                          "refusals": refusals}))
else:
    Path(args["--outfile"]).write_text(json.dumps(mode.get("report", {"identity": {"engine": "forever-engine"},
                                                                      "request_sha256": "ab", "result": mode["result"]})))
"""


class RouteTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.bundle = self.root / "bundle"
        (self.bundle / "bin").mkdir(parents=True)
        for name in (route.ENGINE, route.EXPORTER):
            path = self.bundle / "bin" / name
            path.write_text(FAKE)
            path.chmod(path.stat().st_mode | stat.S_IEXEC)
        self.request = self.root / "request.json"
        self.request.write_text(json.dumps({"simOptions": {"iterations": 10}}))
        self.output = self.root / "out"

    def tearDown(self):
        self.temp.cleanup()

    def decide(self, timeout=30, seed=7, **mode):
        mode.setdefault("result", RESULT)
        (self.bundle / "bin" / "mode.json").write_text(json.dumps(mode))
        return route.route(self.request, self.output, self.bundle, timeout, seed)

    def assert_fault(self, decision, stage):
        self.assertEqual(decision["status"], "fault")
        self.assertEqual(decision["stage"], stage)
        self.assertNotIn("refusals", decision)
        self.assertFalse((self.output / "result.json").exists())

    def test_a_supported_request_returns_the_rust_result(self):
        decision = self.decide()
        self.assertEqual(decision["status"], "rust")
        self.assertEqual(json.loads(Path(decision["result"]).read_text()), RESULT)
        self.assertEqual(decision["identity"], {"engine": "forever-engine"})
        self.assertEqual(set(decision["timings_ms"]), {"prepare", "check", "rust"})

    def test_a_refused_request_falls_back_with_its_codes(self):
        decision = self.decide(refusals=REFUSALS)
        self.assertEqual(decision["status"], "fallback")
        self.assertEqual(decision["refusals"], REFUSALS)
        self.assertEqual(decision["codes"], ["class_limit", "unknown_spell"])
        self.assertFalse((self.output / "result.json").exists())
        self.assertNotIn("rust", decision["timings_ms"])

    def test_a_crash_at_any_step_is_a_fault(self):
        for stage in ("prepare", "check", "rust"):
            with self.subTest(stage=stage):
                self.output = self.root / f"out-{stage}"
                decision = self.decide(fail=stage)
                self.assert_fault(decision, stage)
                self.assertIn("broken", decision["error"])

    def test_unreadable_or_incomplete_output_is_a_fault(self):
        self.assert_fault(self.decide(check_output="not json"), "check")
        self.output = self.root / "out-silent"
        self.assert_fault(self.decide(check_output=json.dumps({"supported": False, "refusals": []})), "check")
        self.output = self.root / "out-partial"
        self.assert_fault(self.decide(report={"result": {"raidMetrics": {}}}), "rust")

    def test_a_timeout_is_a_fault(self):
        self.assert_fault(self.decide(timeout=1, hang="rust"), "rust")

    def test_an_unreadable_request_is_a_fault(self):
        self.request.write_text("{")
        self.assert_fault(self.decide(), "request")

    def test_an_unseeded_request_gets_a_seed_and_a_seeded_one_keeps_its_own(self):
        decision = self.decide(seed=None)
        sent = json.loads((self.output / "request.json").read_text())
        self.assertEqual(decision["seed_source"], "drawn")
        self.assertTrue(0 < decision["seed"] <= route.MAX_SEED)
        self.assertEqual(sent["simOptions"]["randomSeed"], str(decision["seed"]))
        self.request.write_text(json.dumps({"simOptions": {"iterations": 10, "randomSeed": "42"}}))
        self.output = self.root / "out-seeded"
        decision = self.decide(seed=7)
        self.assertEqual((decision["seed"], decision["seed_source"]), (42, "request"))

    def test_the_command_exits_with_two_on_a_fault(self):
        (self.bundle / "bin" / "mode.json").write_text(json.dumps({"fail": "rust", "result": RESULT}))
        done = subprocess.run([sys.executable, str(Path(route.__file__)), "--request", str(self.request),
                               "--output", str(self.output), "--bundle", str(self.bundle)],
                              capture_output=True, text=True)
        self.assertEqual(done.returncode, 2)
        self.assertEqual(json.loads(done.stdout)["status"], "fault")
        self.assertEqual(json.loads((self.output / "decision.json").read_text())["stage"], "rust")


if __name__ == "__main__":
    unittest.main()
