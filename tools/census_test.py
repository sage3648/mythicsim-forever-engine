import json
from pathlib import Path
import tempfile
import unittest

from census import classify, render, summarize, survey, template


class FakeEngine:
    """Exports a request to a prepared file whose digest is its name; checks by lookup."""

    def __init__(self, scratch, results):
        self.scratch, self.results, self.exports = scratch, results, 0

    def export(self, request, name):
        self.exports += 1
        path = self.scratch / f"{name}.prepared.json"
        path.write_text(json.dumps({"contract": "forever-prepared", "request_sha256": request["name"]}))
        return path

    def check(self, prepared_path):
        reasons = self.results[json.loads(prepared_path.read_text())["request_sha256"]]
        return {"outcome": "unsupported" if reasons else "supported", "reasons": reasons}


class CensusTests(unittest.TestCase):
    def test_template_abstracts_numbers(self):
        self.assertEqual(template("rotation reaches spell 20554 without a known behavior"),
                         ("rotation reaches spell <n> without a known behavior", ["20554"]))
        self.assertEqual(template("survival cooldown waits for health 0.5"),
                         ("survival cooldown waits for health <n>", ["0.5"]))

    def test_classify(self):
        self.assertEqual(classify({"contract": "forever-prepared"})[0], "prepared")
        self.assertEqual(classify({"raid": {}})[0], "request")
        snapshot = {"request": {"raid": {}}, "export": {}}
        self.assertEqual(classify(snapshot), ("request", snapshot["request"]))
        self.assertEqual(classify({"cases": []})[0], None)
        self.assertEqual(classify([1, 2])[0], None)

    def test_ranks_reasons_by_inputs_blocked_alone(self):
        rows = [
            {"input": "a", "outcome": "supported", "reasons": []},
            {"input": "b", "outcome": "unsupported", "reasons": ["item 1 has no effect"]},
            {"input": "c", "outcome": "unsupported", "reasons": ["item 2 has no effect"]},
            {"input": "d", "outcome": "unsupported", "reasons": ["operator x", "item 1 has no effect"]},
            {"input": "e", "outcome": "unsupported", "reasons": ["operator x", "operator y"]},
        ]
        summary = summarize(rows)
        self.assertEqual(summary["outcomes"], {"supported": 1, "unsupported": 4})
        first, second, third = summary["reason_groups"]
        self.assertEqual((first["template"], first["blocks"], first["blocks_alone"]), ("item <n> has no effect", 3, 2))
        self.assertEqual(first["instances"], [{"numbers": "1", "inputs": 2}, {"numbers": "2", "inputs": 1}])
        # Two reasons of the same template still block an input alone.
        self.assertEqual((second["template"], second["blocks"], second["blocks_alone"]), ("operator x", 2, 0))
        self.assertEqual(third["template"], "operator y")
        self.assertIn("supported 1 of 5 (20%)", render(summary, []))

    def test_survey_skips_other_files_and_counts_each_request_once(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "a.request.json").write_text(json.dumps({"raid": {}, "name": "one"}))
            (root / "a.prepared.json").write_text(json.dumps({"contract": "forever-prepared", "request_sha256": "one"}))
            (root / "b.json").write_text(json.dumps({"request": {"raid": {}, "name": "two"}}))
            (root / "a.go-result.json").write_text(json.dumps({"raidMetrics": {}}))
            (root / "broken.json").write_text("{")
            engine = FakeEngine(root, {"one": [], "two": ["operator x"]})
            files = sorted(root.glob("*.json"))
            rows, skipped = survey(files, engine)
        self.assertEqual([row["outcome"] for row in rows], ["supported", "unsupported"])
        self.assertEqual(rows[0]["duplicates"], [rows[0]["input"].replace("prepared", "request")])
        self.assertEqual(len(skipped), 2)


if __name__ == "__main__":
    unittest.main()
