#!/usr/bin/env python3
"""Audit the upstream ledger: sources, reviewed community changes and the mechanics map.

check                     Offline consistency audit. Needs Python only.
check --community PATH    Also require the ledger to list exactly the commits in the
                          reviewed range of a local clone of the community repository.
"""

import argparse
from pathlib import Path
import re
import subprocess
import sys

from compare import PIN, ROOT, load

LEDGER = ROOT / "upstream"


def resolve(prefix, commits):
    matches = [commit for commit in commits if commit.startswith(prefix)]
    return matches[0] if len(matches) == 1 else None


def check(directory=LEDGER, community=None):
    sources = {source["id"]: source for source in load(directory / "sources.json")["sources"]}
    ledger = load(directory / "changes.json")
    mapping = load(directory / "mechanics-map.json")
    errors = []

    def require(condition, message):
        if not condition:
            errors.append(message)

    reference, upstream = sources["reference"], sources["community"]
    require(reference["pinned_revision"] == PIN, "reference pin differs from the comparison pin")
    reviewed = upstream["last_reviewed"]
    require(reviewed["from"] == upstream["adopted_base"] == reference["community_base"],
            "review must start at the adopted community base")
    require((ledger["range"]["from"], ledger["range"]["to"]) == (reviewed["from"], reviewed["to"]),
            "ledger range differs from sources.json")
    changes = ledger["changes"]
    commits = [change["commit"] for change in changes]
    require(ledger["range"]["commits"] == len(changes), "ledger commit count differs from its entries")
    require(len(commits) == len(set(commits)), "duplicate ledger entries")
    mechanics = {entry["id"]: entry for entry in mapping["mechanics"]}
    require(len(mechanics) == len(mapping["mechanics"]), "duplicate mechanic identifiers")
    for change in changes:
        label = change["commit"][:9]
        require(re.fullmatch(r"[0-9a-f]{40}", change["commit"]) is not None, f"{label}: commit must be a full SHA")
        require(change["disposition"] in ledger["dispositions"], f"{label}: unknown disposition")
        require(bool(change.get("reason")), f"{label}: a disposition needs a reason")
        for mechanic in change.get("mechanics", []):
            require(mechanic in mechanics, f"{label}: unknown mechanic {mechanic}")
            if mechanic in mechanics:
                linked = [resolve(p, commits) for p in mechanics[mechanic]["upstream_changes"]]
                require(change["commit"] in linked, f"{label}: {mechanic} does not link back")
        if change["disposition"] == "applicable":
            adoption = change.get("adoption", {})
            require(adoption.get("reference") in ("pending", "adopted"), f"{label}: applicable change needs reference adoption state")
            require(bool(adoption.get("rust")), f"{label}: applicable change needs its Rust state")
            require(bool(change.get("regression")), f"{label}: applicable change needs a regression")
            for path in change.get("regression", []):
                require((ROOT / path.split("#")[0]).is_file(), f"{label}: missing regression {path}")
    for entry in mapping["mechanics"]:
        require(entry["status"] in mapping["statuses"], f"{entry['id']}: unknown status")
        for path in entry["rust"] + entry["tests"]:
            require((ROOT / path).exists(), f"{entry['id']}: missing {path}")
        for prefix in entry["upstream_changes"]:
            commit = resolve(prefix, commits)
            require(commit is not None, f"{entry['id']}: {prefix} is not a unique ledger commit")
            if commit:
                change = next(c for c in changes if c["commit"] == commit)
                require(entry["id"] in change.get("mechanics", []), f"{entry['id']}: {prefix} does not link back")
    if community is not None:
        listed = subprocess.check_output(
            ["git", "rev-list", "--reverse", f"{reviewed['from']}..{reviewed['to']}"],
            cwd=community, text=True).split()
        require(listed == commits, "ledger does not list exactly the reviewed community range in order")
    if errors:
        raise ValueError("\n".join(errors))
    return ledger, mapping


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["check"], nargs="?", default="check")
    parser.add_argument("--community", type=Path, help="local clone of the community repository")
    args = parser.parse_args()
    try:
        ledger, mapping = check(community=args.community)
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f"upstream ledger failed: {error}", file=sys.stderr)
        return 1
    counts = {}
    for change in ledger["changes"]:
        counts[change["disposition"]] = counts.get(change["disposition"], 0) + 1
    summary = ", ".join(f"{count} {name}" for name, count in sorted(counts.items()))
    print(f"Upstream ledger checked: {len(ledger['changes'])} reviewed commits ({summary}); "
          f"{len(mapping['mechanics'])} mapped mechanics.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
