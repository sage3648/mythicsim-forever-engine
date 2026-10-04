#!/usr/bin/env python3
"""Write the production race board requests for comparison with the pinned Go engine.

The application publishes race boards: for each spec, the request it simulates and one
variant per race, each with the DPS the production engine measured. This writes one
request per board and race into --output, named SLUG--RACE.request.json, plus a
boards.json index with the production DPS and engine revision. Compare the requests with
`tools/prepared_v2.py compare`; the production DPS comes from the production engine, which
may be newer than the pinned reference, so it is context, not a criterion. Uses only
Python's standard library.
"""

import argparse
import copy
import json
from pathlib import Path


def requests(boards, slugs):
    """(name, request, row) for each race variant of each selected board."""
    for board in boards["boards"]:
        if slugs and board["slug"] not in slugs:
            continue
        for variant in board["variants"]:
            race = variant["race"]["enum"]
            request = copy.deepcopy(board["request"])
            request["raid"]["parties"][0]["players"][0]["race"] = race
            row = {"slug": board["slug"], "race": race, "production_dps": variant["dps"],
                   "production_iterations": variant["iterations"]}
            yield f"{board['slug']}--{race}", request, row


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("boards", type=Path, help="the application's web/src/lib/forever-races.json")
    parser.add_argument("--slug", action="append", default=[], help="board to write; every board by default")
    parser.add_argument("--iterations", type=int, help="override the board's iteration count")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    boards = json.loads(args.boards.read_text())
    args.output.mkdir(parents=True, exist_ok=True)
    index = {"production_engine": boards.get("engineSHA"), "generated": boards.get("generatedAt"), "cases": []}
    for name, request, row in requests(boards, set(args.slug)):
        if args.iterations:
            request["simOptions"]["iterations"] = args.iterations
        (args.output / f"{name}.request.json").write_text(json.dumps(request, indent=2) + "\n")
        index["cases"].append({"scenario": name, **row})
    (args.output / "boards.json").write_text(json.dumps(index, indent=2) + "\n")
    print(f"Wrote {len(index['cases'])} requests to {args.output}")


if __name__ == "__main__":
    main()
