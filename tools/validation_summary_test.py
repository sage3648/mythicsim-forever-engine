import json
from pathlib import Path
import tempfile
import unittest

from validation_summary import census_record, group, load, one_line, outcomes, render


def write(directory, name, record):
    path = Path(directory) / name
    path.write_text(json.dumps(record))
    return path


class ValidationSummaryTests(unittest.TestCase):
    def test_one_line_takes_the_first_sentence_and_escapes_pipes(self):
        self.assertEqual(one_line("Each request. Then more."), "Each request.")
        self.assertEqual(one_line("a\n  b | c"), "a b \\| c")
        self.assertEqual(one_line(None), "")
        long = one_line("word " * 60)
        self.assertTrue(long.endswith("...") and len(long) <= 164)

    def test_outcomes_reads_counts_or_the_outcomes_map(self):
        self.assertEqual(outcomes({"matched": 3, "rejected": 1, "mismatched": 0}),
                         {"matched": 3, "rejected": 1, "mismatched": 0})
        self.assertEqual(outcomes({"outcomes": {"matched": 5, "go_error": 2}}), {"matched": 5, "go_error": 2})
        self.assertEqual(outcomes({"passed": True}), {})

    def test_groups_by_kind_and_file_name(self):
        def kind_of(name, record):
            return group(Path(name), record)

        self.assertEqual(kind_of("x-race-boards.json", {"kind": "production_corpus"}), "corpus")
        self.assertEqual(kind_of("x-preset-matrix.json", {"kind": "compatibility_sweep"}), "corpus")
        self.assertEqual(kind_of("x-sweep.json", {"kind": "compatibility_sweep"}), "sweep")
        self.assertEqual(kind_of("x.json", {"kind": "integration_regression"}), "regression")
        self.assertEqual(kind_of("x.json", {"kind": "production_census"}), "census")
        self.assertEqual(kind_of("x.json", {"kind": "fma_audit"}), "other")

    def test_render_totals_only_what_records_state(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / "validation").mkdir()
            (root / "benchmarks").mkdir()
            (root / "docs").mkdir()
            validation = root / "validation"
            write(validation, "a-race-boards.json", {"kind": "production_corpus", "scope": "Boards. More.",
                                                     "matched": 10, "rejected": 1, "mismatched": 0})
            write(validation, "b-gear-swaps.json", {"kind": "production_gear_swaps", "scope": "Swaps.",
                                                    "outcomes": {"matched": 7, "go_error": 1}})
            write(validation, "c-sweep.json", {"kind": "compatibility_sweep", "matched": 20, "rejected": 4,
                                               "mismatched": 0, "source_revision": "abcdef1234567890"})
            write(validation, "d-regression.json", {"kind": "integration_regression", "matched": 30,
                                                    "rejected": 4, "mismatched": 0})
            write(validation, "e-audit.json", {"kind": "fma_audit", "scope": "Audit."})
            write(validation, "f-production-census.json",
                  census_record({"summary": {"inputs": 29, "outcomes": {"supported": 27, "unsupported": 2}},
                                 "inputs": []}, "deadbeef", "2026-10-04", "output/prod-requests"))
            write(root / "benchmarks", "g.json", {"method": "Timing.", "source_revision": "cafe"})
            text = render(load(validation), load(root / "benchmarks"), root / "docs" / "summary.md")
        self.assertIn("- Production builds supported: 27 of 29, from "
                      "[f-production-census.json](../validation/f-production-census.json).", text)
        self.assertIn("- Production corpus: 19 variants (17 matched, 1 rejected, 0 mismatched, 1 go_error) "
                      "in 2 records.", text)
        self.assertIn("  - Race boards: 11 variants (10 matched, 1 rejected, 0 mismatched) in 1 record.", text)
        self.assertIn("- Randomized sweeps: 24 variants (20 matched, 4 rejected, 0 mismatched) in 1 record.", text)
        self.assertIn("reruns of other records' inputs and not added above: 34 variants", text)
        self.assertIn("| compatibility_sweep |  | 20 | 4 | 0 |  | abcdef123456 | "
                      "[c-sweep.json](../validation/c-sweep.json) |", text)
        self.assertIn("| fma_audit | Audit. |  |  |  |  |  | [e-audit.json](../validation/e-audit.json) |", text)
        self.assertIn("| production_gear_swaps | Swaps. | 7 |  |  | go_error 1 |", text)
        self.assertIn("[g.json](../benchmarks/g.json)", text)
        self.assertIn("- Benchmark records: 1.", text)
        self.assertIn("| production_census |", text)
        self.assertIn("|  |  |  | supported 27, unsupported 2 | deadbeef |", text)

    def test_without_a_census_says_so(self):
        with tempfile.TemporaryDirectory() as scratch:
            text = render([], [], Path(scratch) / "docs" / "summary.md")
        self.assertIn("- Production builds supported: no production census record.", text)
        self.assertIn("- Randomized sweeps: no counts in 0 records.", text)


if __name__ == "__main__":
    unittest.main()
