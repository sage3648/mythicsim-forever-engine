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
import json, os, signal, sys, time
from pathlib import Path
mode = json.loads((Path(__file__).parent / "mode.json").read_text())
command, args = sys.argv[1], dict(zip(sys.argv[2::2], sys.argv[3::2]))
rust_prepare = "--request" in args
step = {"prepare": "prepare", "check": "check", "sim": "rust"}[command]
if rust_prepare and command == "prepare":
    step = "rust_prepare"
gated = command == "sim" and "--gate" in sys.argv
with (Path(__file__).parent / "calls.log").open("a") as calls:
    calls.write(command + (" --request" if rust_prepare else "") + (" --gate" if gated else "") + "\\n")
if mode.get("hang") == step:
    time.sleep(5)
if rust_prepare:
    # The engine prepares the request itself, or refuses with status 5.
    request = json.loads(Path(args["--request"]).read_text())
    mode.update(request.get("fake", {}))
    if mode.get("fail") == "rust_prepare":
        sys.exit("broken")
    if not mode.get("rust_prepares"):
        print(json.dumps({"prepared": False, "refusal": {"code": "class", "reason": "not prepared yet"}}))
        sys.exit(5)
    infile = {"request": request}
    if command == "prepare":
        command = "exporter_prepare"
else:
    infile = json.loads(Path(args["--infile"]).read_text())
    # A batch test gives one request its own behavior.
    mode.update((infile if command == "prepare" else infile["request"]).get("fake", {}))
if gated:
    # sim --gate: a refusal is reported as check reports it, with status 3, and an invalid
    # prepared input has status 4.
    if mode.get("fail") == "check":
        print("broken", file=sys.stderr)
        sys.exit(4)
    refusals = mode.get("refusals", [])
    if "check_output" in mode or refusals:
        print(mode["check_output"] if "check_output" in mode else
              json.dumps({"supported": False, "reasons": [r["reason"] for r in refusals], "refusals": refusals}))
        sys.exit(3)
if mode.get("fail") == step:
    sys.exit("broken")
if mode.get("partial_then") and command == "sim":
    # What a run stopped part way through its output would leave behind without atomic writes.
    Path(args["--outfile"]).write_text('{"identity": {"engine": "forever-engine"}, "result": {"raidMet')
    Path(args["--outfile"]).with_name(".rust-report.json.1.tmp").write_text("{")
    if mode["partial_then"] == "hang":
        time.sleep(5)
    elif mode["partial_then"] == "signal":
        os.kill(os.getpid(), signal.SIGKILL)
    elif mode["partial_then"] == "exit":
        sys.exit(0)
if command == "prepare":
    request = infile
    Path(args["--outfile"]).write_text(json.dumps({"request": request}))
elif command == "exporter_prepare":
    Path(args["--outfile"]).write_text(json.dumps(infile))
elif command == "exporter_prepare":
    Path(args["--outfile"]).write_text(json.dumps(infile))
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


class BundleTest(unittest.TestCase):
    """A bundle of fake binaries and one request."""

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


class RouteTest(BundleTest):
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
        # The gate runs inside the Rust step, so a supported request has no step of its own for it.
        self.assertEqual(set(decision["timings_ms"]), {"rust_prepare", "prepare", "rust"})
        self.assertEqual(decision["preparation"], {"provider": "go", "code": "class", "reason": "not prepared yet"})

    def calls(self):
        return (self.bundle / "bin" / "calls.log").read_text().split("\n")[:-1]

    def test_a_single_request_gates_and_runs_in_one_engine_process(self):
        self.decide()
        self.assertEqual(self.calls(), ["sim --request --gate", "prepare", "sim --gate"])

    def test_a_request_rust_prepares_runs_in_one_engine_process(self):
        decision = self.decide(rust_prepares=True)
        self.assertEqual(decision["status"], "rust")
        self.assertEqual(decision["preparation"], {"provider": "rust"})
        self.assertEqual(json.loads(Path(decision["result"]).read_text()), RESULT)
        self.assertEqual(set(decision["timings_ms"]), {"rust"})
        self.assertEqual(self.calls(), ["sim --request --gate"])

    def test_a_request_rust_prepares_can_still_fall_back_at_the_gate(self):
        decision = self.decide(rust_prepares=True, refusals=REFUSALS)
        self.assertEqual(decision["status"], "fallback")
        self.assertEqual(decision["codes"], ["class_limit", "unknown_spell"])
        self.assertEqual(set(decision["timings_ms"]), {"check"})
        self.assertEqual(self.calls(), ["sim --request --gate"])

    def test_go_prepare_skips_rust_preparation(self):
        self.output = self.root / "out-go"
        (self.bundle / "bin" / "mode.json").write_text(json.dumps({"result": RESULT, "rust_prepares": True}))
        decision = route.route(self.request, self.output, self.bundle, 30, 7, go_prepare=True)
        self.assertEqual(decision["status"], "rust")
        self.assertEqual(decision["preparation"], {"provider": "go"})
        self.assertEqual(self.calls(), ["prepare", "sim --gate"])

    def test_a_refused_request_falls_back_with_its_codes(self):
        decision = self.decide(refusals=REFUSALS)
        self.assertEqual(decision["status"], "fallback")
        self.assertEqual(decision["refusals"], REFUSALS)
        self.assertEqual(decision["codes"], ["class_limit", "unknown_spell"])
        self.assertFalse((self.output / "result.json").exists())
        # The gate ran and nothing else did: its time is the check's.
        self.assertEqual(set(decision["timings_ms"]), {"rust_prepare", "prepare", "check"})
        self.assertEqual(self.calls(), ["sim --request --gate", "prepare", "sim --gate"])
        self.assertEqual(list(self.output.glob("*rust-report*")), [])

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

    def assert_nothing_to_read(self):
        """A faulted Rust step leaves no result and no report, partial or whole, to read."""
        leftovers = sorted(path.name for path in self.output.iterdir() if "rust-report" in path.name)
        self.assertEqual(leftovers, [])
        self.assertFalse((self.output / "result.json").exists())

    def test_a_timed_out_run_leaves_no_partial_result(self):
        decision = self.decide(timeout=1, partial_then="hang")
        self.assert_fault(decision, "rust")
        self.assertIn("timed out", decision["error"])
        self.assert_nothing_to_read()

    def test_a_run_ended_by_a_signal_is_a_fault_naming_it(self):
        decision = self.decide(partial_then="signal")
        self.assert_fault(decision, "rust")
        self.assertIn("killed by signal 9", decision["error"])
        self.assert_nothing_to_read()

    def test_a_truncated_report_is_a_fault_not_a_result(self):
        decision = self.decide(partial_then="exit")
        self.assert_fault(decision, "rust")
        self.assertIn("incomplete result", decision["error"])
        self.assert_nothing_to_read()

    def test_a_fault_before_rust_runs_leaves_no_result_either(self):
        self.assert_fault(self.decide(timeout=1, hang="prepare"), "prepare")
        self.assertFalse((self.output / "rust-report.json").exists())

    def test_a_refusal_that_claims_support_is_a_fault(self):
        supported = json.dumps({"supported": True, "refusals": []})
        self.assert_fault(self.decide(check_output=supported), "check")

    def test_files_are_written_whole_or_not_at_all(self):
        target = self.root / "whole.json"
        route.write_atomically(target, "complete\n")
        self.assertEqual(target.read_text(), "complete\n")
        # A failed write keeps the previous file and leaves no temporary file.
        with self.assertRaises(TypeError):
            route.write_atomically(target, None)
        self.assertEqual(target.read_text(), "complete\n")
        self.assertEqual(sorted(path.name for path in self.root.glob(".whole*")), [])

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



class BatchTest(BundleTest):
    """A batch runs in Rust only when the gate supports every request."""

    def write_batch(self, *behaviors):
        paths = []
        for index, fake in enumerate(behaviors):
            path = self.root / f"candidate-{index}.json"
            path.write_text(json.dumps({"simOptions": {"iterations": 10}, "fake": fake}))
            paths.append(path)
        return paths

    def decide_batch(self, *behaviors, seed=7):
        (self.bundle / "bin" / "mode.json").write_text(json.dumps({"result": RESULT}))
        return route.route_batch(self.write_batch(*behaviors), self.output, self.bundle, 30, seed)

    def results(self):
        return sorted(self.output.glob("*/result.json"))

    def test_a_fully_supported_batch_runs_every_request_in_rust(self):
        decision = self.decide_batch({}, {}, {})
        self.assertEqual(decision["status"], "rust")
        self.assertEqual([entry["status"] for entry in decision["requests"]], ["rust"] * 3)
        self.assertEqual(len(self.results()), 3)
        # A batch gates every request with `check` before any runs.
        calls = (self.bundle / "bin" / "calls.log").read_text().split("\n")[:-1]
        self.assertEqual(calls, ["prepare --request", "prepare", "check"] * 3 + ["sim"] * 3)
        self.assertEqual({entry["preparation"]["provider"] for entry in decision["requests"]}, {"go"})

    def test_a_batch_rust_prepares_needs_no_exporter(self):
        decision = self.decide_batch({"rust_prepares": True}, {"rust_prepares": True})
        self.assertEqual(decision["status"], "rust")
        calls = (self.bundle / "bin" / "calls.log").read_text().split("\n")[:-1]
        self.assertEqual(calls, ["prepare --request", "check"] * 2 + ["sim"] * 2)
        self.assertEqual([entry["preparation"] for entry in decision["requests"]], [{"provider": "rust"}] * 2)

    def test_one_refused_candidate_sends_the_whole_batch_to_go(self):
        decision = self.decide_batch({}, {"refusals": REFUSALS}, {})
        self.assertEqual(decision["status"], "fallback")
        self.assertEqual(decision["codes"], ["class_limit", "unknown_spell"])
        self.assertEqual([entry.get("status") for entry in decision["requests"]], [None, "fallback", None])
        self.assertEqual(decision["requests"][1]["refusals"], REFUSALS)
        self.assertEqual(self.results(), [])
        for entry in decision["requests"]:
            self.assertNotIn("rust", entry["timings_ms"])

    def test_a_fault_in_any_request_leaves_no_result(self):
        for stage in ("prepare", "check", "rust"):
            with self.subTest(stage=stage):
                self.output = self.root / f"batch-{stage}"
                decision = self.decide_batch({}, {"fail": stage}, {})
                self.assertEqual(decision["status"], "fault")
                self.assertEqual(decision["requests"][1]["stage"], stage)
                self.assertEqual(self.results(), [])
                self.assertNotIn("rust", {entry.get("status") for entry in decision["requests"]})

    def test_unseeded_candidates_share_one_seed(self):
        decision = self.decide_batch({}, {}, seed=None)
        self.assertEqual(decision["seed_source"], "drawn")
        seeds = {json.loads((folder / "request.json").read_text())["simOptions"]["randomSeed"]
                 for folder in sorted(self.output.iterdir())}
        self.assertEqual(seeds, {str(decision["seed"])})

    def test_the_command_decides_a_batch(self):
        (self.bundle / "bin" / "mode.json").write_text(json.dumps({"result": RESULT}))
        paths = self.write_batch({}, {"refusals": REFUSALS})
        done = subprocess.run([sys.executable, str(Path(route.__file__)), "--batch", *map(str, paths),
                               "--output", str(self.output), "--bundle", str(self.bundle)],
                              capture_output=True, text=True)
        self.assertEqual(done.returncode, 0)
        self.assertEqual(json.loads(done.stdout)["status"], "fallback")


if __name__ == "__main__":
    unittest.main()
