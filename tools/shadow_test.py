import json
from pathlib import Path
import stat
import sys
import tempfile
import textwrap
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import shadow

def result(dps, fireball=1000.0):
    """A small RaidSimResult: 10 fights of 10 seconds."""
    return {"iterationsDone": 10, "avgIterationDuration": 10.0, "raidMetrics": {"dps": {"avg": dps}, "parties": [{"players": [{
        "dps": {"avg": dps, "stdev": 10.0},
        "actions": [
            {"id": {"spellId": 133}, "targets": [{"damage": fireball, "casts": 20, "hits": 15, "crits": 5}]},
            {"id": {"otherId": "OtherActionAttack"}, "targets": [{"damage": 500.0, "hits": 10}]},
            {"id": {"spellId": 1}, "targets": [{}]},
        ],
        "pets": [{"name": "Imp", "dps": {"avg": 3.0}}, {"name": "Felhunter", "dps": {"avg": 0}}],
    }]}]}}


RESULT = result(500.0)

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

    def verdict(self, production=None, **mode):
        mode.setdefault("go", RESULT)
        mode.setdefault("rust", RESULT)
        (self.bundle / "bin" / "mode.json").write_text(json.dumps(mode))
        production_path = None
        if production is not None:
            production_path = self.root / "production.json"
            production_path.write_text(json.dumps(production))
        return shadow.shadow(self.request, self.root / "out", self.bundle, seed=7, timeout=30,
                             production_path=production_path)

    def test_a_summary_is_per_average_fight(self):
        brief = shadow.summary(RESULT)
        self.assertEqual((brief["dps"], brief["stdev"], brief["iterations"]), (500.0, 10.0, 10))
        fireball, melee = brief["abilities"]
        self.assertEqual(fireball["id"], {"spellId": 133})
        self.assertEqual((fireball["dps"], fireball["casts"], fireball["crit_pct"]), (10.0, 2.0, 25.0))
        self.assertEqual(melee["dps"], 5.0)
        self.assertEqual(brief["pets"], [{"name": "Imp", "dps": 3.0}])

    def test_rust_is_compared_with_production_in_standard_errors(self):
        verdict = self.verdict(production=result(498.0, fireball=900.0))
        self.assertEqual(verdict["status"], "match")
        self.assertEqual(verdict["production"]["dps"], 498.0)
        versus = verdict["versus_production"]
        self.assertAlmostEqual(versus["dps_difference"], 2.0)
        # Standard error 10 / sqrt(10) on each side.
        self.assertAlmostEqual(versus["standard_errors"], 2.0 / (10 / 10 ** 0.5 * 2 ** 0.5))
        fireball = versus["abilities"][0]
        self.assertEqual((fireball["production_dps"], fireball["rust_dps"]), (9.0, 10.0))

    def test_a_refusal_still_summarizes_production(self):
        verdict = self.verdict(production=RESULT, reasons=["needs a mechanic"])
        self.assertEqual(verdict["status"], "refused")
        self.assertEqual(verdict["production"]["dps"], 500.0)
        self.assertNotIn("versus_production", verdict)

    def test_an_unreadable_production_result_is_noted_not_fatal(self):
        verdict = self.verdict(production={"raidMetrics": {}})
        self.assertEqual(verdict["status"], "match")
        self.assertIn("production_error", verdict)

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
        verdict = self.verdict(rust=result(500.0, fireball=1001.0))
        self.assertEqual(verdict["status"], "mismatch")
        self.assertEqual(verdict["difference_count"], 1)
        self.assertIn("damage", verdict["differences"][0])

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
