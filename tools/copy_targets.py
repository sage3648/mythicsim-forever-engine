#!/usr/bin/env python3
"""Write requests against several copies of the boss from requests against one.

The application's Advanced targets setting copies the boss: each request this writes is a
request of one target with that target copied N times, for N in the given counts. With
--tanks-only a request without a tank assignment is skipped, which is how the tank builds of
the accepted fixtures are picked out. A request that already has several targets is skipped.
Compare the results with `tools/prepared_v2.py compare`. The same inputs always give the same
files. Uses only Python's standard library.

    python3 tools/copy_targets.py --counts 2,3,4,5 --tanks-only --output <scratch> \\
        fixtures/mage/prepared-v2/*.request.json
"""

import argparse
import copy
import json
from pathlib import Path


def copied(request, count):
    """The request with its one target copied until the encounter has `count` of them."""
    result = copy.deepcopy(request)
    targets = result["encounter"]["targets"]
    result["encounter"]["targets"] = [copy.deepcopy(targets[0]) for _ in range(count)]
    return result


def tank_assigned(request):
    return bool(request.get("raid", {}).get("tanks"))


def write(requests, counts, output, tanks_only=False):
    """Write `NAME-N-targets.request.json` for each request path and count; return the paths."""
    output.mkdir(parents=True, exist_ok=True)
    written = []
    for path in requests:
        request = json.loads(Path(path).read_text())
        if len(request["encounter"]["targets"]) != 1 or (tanks_only and not tank_assigned(request)):
            continue
        name = Path(path).name.removesuffix(".json").removesuffix(".request")
        for count in counts:
            target = output / f"{name}-{count}-targets.request.json"
            target.write_text(json.dumps(copied(request, count), separators=(",", ":"), sort_keys=True))
            written.append(target)
    return written


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("requests", nargs="+", type=Path, help="RaidSimRequest files with one target")
    parser.add_argument("--counts", default="2,3,4,5", help="comma-separated target counts, each 2 to 5")
    parser.add_argument("--tanks-only", action="store_true", help="skip requests without a tank assignment")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    counts = [int(count) for count in args.counts.split(",")]
    if any(count < 2 or count > 5 for count in counts):
        parser.error("counts must be from 2 to 5")
    written = write(args.requests, counts, args.output, args.tanks_only)
    print(f"Wrote {len(written)} requests to {args.output}")


if __name__ == "__main__":
    main()
