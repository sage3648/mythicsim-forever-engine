from datetime import datetime, timezone
import json
from pathlib import Path
import tempfile
import unittest

import board


def issue(number, title, milestone=None, labels=(), body="", comments=(), state="OPEN", closed_at=None):
    return {"number": number, "title": title, "state": state, "closedAt": closed_at,
            "milestone": {"title": milestone} if milestone else None,
            "labels": [{"name": name} for name in labels], "body": body,
            "comments": [{"body": text} for text in comments]}


class BoardTest(unittest.TestCase):
    def render(self, issues, cases=()):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "record.json").write_text(json.dumps({"cases": list(cases)}))
            return board.render(issues, Path(directory), datetime(2026, 10, 20, tzinfo=timezone.utc))

    def test_sections_follow_milestones_then_priority(self):
        text = self.render([
            issue(25, "Start here: ordered contributor board"),
            issue(9, "Later", "2. Coverage gaps", ["priority: 1 high"]),
            issue(8, "Low", "1. Production routing", ["priority: 3 low"]),
            issue(7, "High", "1. Production routing", ["priority: 1 high"]),
            issue(27, "Tidy", None, ["priority: 2 medium"]),
            issue(6, "Done", "1. Production routing", state="CLOSED"),
        ])
        order = [line for line in text.splitlines() if line.startswith(("## 1", "## 2", "## M", "- [ ]"))]
        self.assertEqual(order, ["## 1. Production routing", "- [ ] #7 High", "- [ ] #8 Low",
                                 "## 2. Coverage gaps", "- [ ] #9 Later", "## Maintenance", "- [ ] #27 Tidy"])
        self.assertNotIn("#25", text.split("## How to pick up work")[1])

    def test_open_dependencies_and_latest_status(self):
        text = self.render([
            issue(6, "Codes", "1. Production routing"),
            issue(8, "Command", "1. Production routing", ["claimed"], body="## Depends on\n\n#6\n\n## Start here\n"),
            issue(13, "Data", "3. Rust preparation", comments=["Status: branch a", "thanks", "Status: PR open\nmore"]),
            issue(14, "Stats", "3. Rust preparation", body="## Depends on\n\n#5\n"),
        ])
        self.assertIn("- [ ] #8 Command (after #6; status: in progress)", text)
        self.assertIn("- [ ] #13 Data (status: PR open)", text)
        self.assertIn("- [ ] #14 Stats\n", text)

    def test_refusals_without_an_issue_are_candidates(self):
        issues = [issue(19, "Swings", "2. Coverage gaps",
                        body="Reason today: `unrepresented by the exporter: player aura \"<name>\" reacts to the target's swings`"),
                  issue(20, "Hardcast", "2. Coverage gaps", state="CLOSED",
                        body="Reason today: `rotation reaches item 11905, a hardcast while the target swings at the player`")]
        cases = [{"reasons": ["unrepresented by the exporter: player aura \"Uther's Strength\" reacts to the target's swings"]},
                 {"rejected": ["rotation reaches item 4, a hardcast while the target swings at the player"]},
                 {"reasons": ["the demon \"Imp 2\" is not simulated"]},
                 {"reasons": ["the demon \"Imp 3\" is not simulated"]}]
        text = self.render(issues, cases)
        candidates = text.split("## Refusals without an issue")[1]
        self.assertIn("- 2 variants: `the demon \"Imp N\" is not simulated`", candidates)
        self.assertNotIn("swings", candidates)

    def test_recently_closed_issues_are_ticked(self):
        text = self.render([
            issue(21, "Holy Nova", "2. Coverage gaps", state="CLOSED", closed_at="2026-10-15T08:00:00Z"),
            issue(5, "Old", "2. Coverage gaps", state="CLOSED", closed_at="2026-09-01T08:00:00Z"),
            issue(22, "Brand", "2. Coverage gaps"),
        ])
        recent = text.split("## Recently completed")[1]
        self.assertIn("- [x] #21 Holy Nova", recent)
        self.assertNotIn("#5 Old", text)
        self.assertNotIn("- [ ] #21", text)


if __name__ == "__main__":
    unittest.main()
