"""Write the buff toggle validation record from a comparison of the generated variants.

python3 validation/2026-10-04-production-buff-toggles/record.py VARIANTS_DIR COMPARE_DIR OUT.json

VARIANTS_DIR holds generate.py's output and index.json; COMPARE_DIR is the --output of
`tools/prepared_v2.py compare` (its summary.json) over every variant. A variant whose Rust run
refused the input is rejected, with its reasons; one Go refused is a Go error; any other
failure is a mismatch. Uses only Python's standard library.
"""
from collections import Counter, defaultdict
import json
import sys
from pathlib import Path

variants, compare, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
index = {entry["scenario"]: entry for entry in json.load(open(variants / "index.json"))}
rows = {row["scenario"]: row for row in json.load(open(compare / "summary.json"))["results"]}
sources = json.load(open("upstream/sources.json"))["sources"]
reference = next(source for source in sources if source["id"] == "reference")


def reasons(row):
    error = row.get("rust_error", "")
    if "prepared input unsupported" not in error:
        return None
    lines = error.split("prepared input unsupported:", 1)[1].strip().splitlines()
    return sorted(line.strip() for line in lines if line.strip())


def describe(change):
    if change["kind"] == "base":
        return "unchanged"
    return f"{change['group']}.{change['field']}: {change['from']} -> {change['to']}"


cases = []
outcomes = Counter()
base_reasons = {}
for scenario, entry in sorted(index.items()):
    row = rows[scenario]
    found = reasons(row)
    if row["passed"]:
        outcome = "matched"
    elif "go_error" in row:
        outcome = "go_error"
    elif found is not None:
        outcome = "rejected"
    else:
        outcome = "mismatched"
    outcomes[outcome] += 1
    case = {"scenario": scenario, "base": entry["base"], "change": entry["change"], "outcome": outcome}
    if outcome == "matched":
        case["dps"] = row["rust_dps"]
    elif outcome == "rejected":
        case["reasons"] = found
    elif outcome == "mismatched":
        case["first_differences"] = row.get("differences", [])[:5]
        case["first_log_difference"] = row.get("first_log_difference")
        case["rust_error"] = row.get("rust_error", "")[-500:] or None
    if entry["change"]["kind"] == "base" and found:
        base_reasons[entry["base"]] = found
    cases.append(case)

added = defaultdict(lambda: {"variants": 0, "changes": set()})
for case in cases:
    if case["outcome"] != "rejected" or case["change"]["kind"] == "base":
        continue
    for reason in set(case["reasons"]) - set(base_reasons.get(case["base"], [])):
        added[reason]["variants"] += 1
        added[reason]["changes"].add(describe(case["change"]).split(":")[0])
by_frequency = sorted(
    ({"reason": reason, "variants": value["variants"], "fields": sorted(value["changes"])}
     for reason, value in added.items()),
    key=lambda item: (-item["variants"], item["reason"]))
all_reasons = Counter(reason for case in cases if case["outcome"] == "rejected" for reason in case["reasons"])

record = {
    "kind": "production_buff_toggles",
    "date": "2026-10-04",
    "reference": {"engine_revision": reference["pinned_revision"], "client_build": reference["client_build"]},
    "scope": (
        "Each of the 29 production requests (validation/2026-10-04-production-buff-toggles/requests) with one "
        "change to a raid buff, party buff, individual buff or debuff the production application's builder "
        "names (web/src/lib/forever-assumption-items.ts BUFF_NAMES and DEBUFF_NAMES): a boolean turned on "
        "when off and removed when on; a tristate (Battle Shout, Mana Spring Totem) set to each level it does "
        "not have, Regular, Improved or Missing. Each request also runs unchanged."),
    "generator": "python3 validation/2026-10-04-production-buff-toggles/generate.py <scratch> 50",
    "rerun": "python3 tools/prepared_v2.py compare --output <compare> <scratch>/*.request.json",
    "iterations": 50,
    "criteria": {"comparison": "full RaidSimResult except elapsedNs, lists keyed by ID", "integer_counts": "exact",
                 "floats": "relative 1e-9", "first_fight_debug_log": "line identical"},
    "outcomes": dict(sorted(outcomes.items())),
    "bases_rejected": base_reasons,
    "rejection_reasons_added_by_toggles": by_frequency,
    "rejection_reasons_all": [{"reason": reason, "variants": count} for reason, count in all_reasons.most_common()],
    "notes": [],
    "cases": cases,
}
if out.exists():
    record["notes"] = json.load(open(out)).get("notes", [])
out.write_text(json.dumps(record, indent=1) + "\n")
print(dict(outcomes))
for item in by_frequency:
    print(f"{item['variants']:4} {item['reason']}  [{', '.join(item['fields'])}]")
for case in cases:
    if case["outcome"] in ("mismatched", "go_error"):
        print(case["outcome"], case["scenario"])
