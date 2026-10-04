#!/usr/bin/env python3
"""Count which inputs the Rust engine would run and rank what blocks the rest.

Each input is a prepared v2 file, a RaidSimRequest, or a document holding one under
"request". Prepared inputs need only Rust. Requests are exported with the pinned Go
engine first, which needs Go, Git and protoc, as for tools/prepared_v2.py. Directories
are searched for JSON files; files that are neither kind, such as Go results and
manifests, are skipped. Inputs with the same request digest are counted once.

Unsupported reasons are grouped by template, with numbers replaced by "<n>". Groups are
ranked by the inputs they block and by the inputs they block alone, which are the
inputs that implementing that one group would make supported. Writes only to --output.
"""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from compare import ROOT, load

NUMBER = re.compile(r"\d+(?:\.\d+)?")


def template(reason):
    """A reason with its numbers abstracted, and the numbers it named."""
    return NUMBER.sub("<n>", reason), NUMBER.findall(reason)


def classify(document):
    """'prepared', 'request' or None, and the value to use."""
    if not isinstance(document, dict):
        return None, None
    if document.get("contract") == "forever-prepared":
        return "prepared", document
    if isinstance(document.get("raid"), dict):
        return "request", document
    if isinstance(document.get("request"), dict) and isinstance(document["request"].get("raid"), dict):
        return "request", document["request"]
    return None, None


def discover(paths):
    """Every JSON file under the given paths, in a stable order."""
    files = []
    for path in paths:
        if path.is_dir():
            files.extend(sorted(path.rglob("*.json")))
        else:
            files.append(path)
    return files


def label(path):
    path = path.resolve()
    return str(path.relative_to(ROOT)) if ROOT in path.parents else str(path)


class Engine:
    """The release Rust binary and, built only when a request needs it, the Go exporter."""

    def __init__(self, cache, source, scratch):
        subprocess.run(["cargo", "build", "--locked", "--release", "--quiet", "--manifest-path", ROOT / "Cargo.toml"],
                       check=True)
        self.binary = ROOT / "target" / "release" / "forever-engine"
        self.cache, self.source, self.scratch = cache, source, scratch
        self.exporter = None

    def export(self, request, name):
        if self.exporter is None:
            from prepared_v2 import build_exporter
            self.exporter = build_exporter(self.cache.resolve(), self.source)
        request_path = self.scratch / f"{name}.request.json"
        prepared_path = self.scratch / f"{name}.prepared.json"
        request_path.write_text(json.dumps(request))
        completed = subprocess.run([self.exporter, "prepare", "--infile", request_path, "--outfile", prepared_path,
                                    "--scenario", "census"], capture_output=True, text=True)
        if completed.returncode != 0:
            raise RuntimeError(completed.stderr.strip() or "exporter failed")
        return prepared_path

    def check(self, prepared_path):
        completed = subprocess.run([self.binary, "check", "--infile", prepared_path], capture_output=True, text=True)
        if completed.returncode != 0:
            return {"outcome": "invalid", "error": completed.stderr.strip()}
        report = json.loads(completed.stdout)
        return {"outcome": "supported" if report["supported"] else "unsupported", "reasons": report["reasons"]}


def survey(files, engine):
    """One row per distinct input, plus the files skipped."""
    rows, skipped, seen = [], [], {}
    for index, path in enumerate(files):
        try:
            kind, value = classify(load(path))
        except (OSError, json.JSONDecodeError):
            kind, value = None, None
        if kind is None:
            skipped.append(label(path))
            continue
        row = {"input": label(path), "kind": kind}
        try:
            prepared_path = path if kind == "prepared" else engine.export(value, f"input-{index:05d}")
        except RuntimeError as err:
            row.update({"outcome": "export_failed", "error": str(err)})
            rows.append(row)
            continue
        digest = load(prepared_path)["request_sha256"]
        if digest in seen:
            seen[digest].setdefault("duplicates", []).append(row["input"])
            continue
        row["request_sha256"] = digest
        row.update(engine.check(prepared_path))
        seen[digest] = row
        rows.append(row)
    return rows, skipped


def summarize(rows):
    """Outcome counts and reason groups, ranked by inputs blocked alone, then blocked."""
    outcomes = defaultdict(int)
    groups = {}
    for row in rows:
        outcomes[row["outcome"]] += 1
        templates = {template(reason)[0] for reason in row.get("reasons", [])}
        for reason in row.get("reasons", []):
            key, numbers = template(reason)
            group = groups.setdefault(key, {"template": key, "inputs": set(), "alone": set(), "instances": defaultdict(set)})
            group["inputs"].add(row["input"])
            if len(templates) == 1:
                group["alone"].add(row["input"])
            group["instances"][" ".join(numbers)].add(row["input"])
    ranked = sorted(groups.values(), key=lambda g: (-len(g["alone"]), -len(g["inputs"]), g["template"]))
    return {
        "inputs": len(rows),
        "outcomes": dict(sorted(outcomes.items())),
        "reason_groups": [{
            "template": group["template"],
            "blocks": len(group["inputs"]),
            "blocks_alone": len(group["alone"]),
            "instances": [{"numbers": numbers, "inputs": len(inputs)}
                          for numbers, inputs in sorted(group["instances"].items(), key=lambda item: (-len(item[1]), item[0]))],
            "examples": sorted(group["inputs"])[:3],
        } for group in ranked],
    }


def render(summary, skipped):
    total = summary["inputs"]
    supported = summary["outcomes"].get("supported", 0)
    lines = [f"{total} distinct inputs, {len(skipped)} files skipped",
             f"supported {supported} of {total} ({100 * supported / total:.0f}%)" if total else "no inputs"]
    for outcome, count in summary["outcomes"].items():
        if outcome != "supported":
            lines.append(f"{outcome} {count}")
    if summary["reason_groups"]:
        lines.append("")
        lines.append("alone  blocks  reason")
        for group in summary["reason_groups"]:
            lines.append(f"{group['blocks_alone']:>5}  {group['blocks']:>6}  {group['template']}")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("inputs", nargs="+", type=Path, help="prepared v2 files, requests or directories")
    parser.add_argument("--output", type=Path, required=True, help="census report JSON; must not exist")
    parser.add_argument("--source", default="https://github.com/sage3648/mythicsim-forever-engine-go.git",
                        help="Go reference repository, used only to export requests")
    parser.add_argument("--cache", type=Path, default=ROOT / "oracle-cache")
    args = parser.parse_args()
    if args.output.exists():
        sys.exit(f"{args.output} already exists")
    with tempfile.TemporaryDirectory() as scratch:
        engine = Engine(args.cache, args.source, Path(scratch))
        rows, skipped = survey(discover(args.inputs), engine)
    summary = summarize(rows)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({"summary": summary, "inputs": rows, "skipped": skipped}, indent=2) + "\n")
    print(render(summary, skipped))


if __name__ == "__main__":
    main()
