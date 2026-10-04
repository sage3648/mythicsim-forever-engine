#!/usr/bin/env python3
"""Summarize every validation and benchmark record into docs/validation-summary.md.

Reads validation/*.json and benchmarks/*.json and writes one Markdown table per group: each
record's kind, its scope in one line, its matched, rejected and mismatched counts as the
record states them, its source revision when it names one, and a link. Totals follow:
production builds supported, from the newest production census record; production corpus
variants (race boards, builder starters, gear swaps, buff toggles, talent census, class
options and the preset matrix); and randomized sweeps. Integration regressions rerun other
records' inputs, so they are listed but left out of the totals. Nothing is inferred: a
record without counts shows none.

    python3 tools/validation_summary.py            # write docs/validation-summary.md
    python3 tools/validation_summary.py check      # fail if the document is out of date
    python3 tools/validation_summary.py census REPORT --revision REV
        # record a tools/census.py report of the production requests as
        # validation/<date>-production-census.json

Uses only Python's standard library.
"""

import argparse
import json
import os
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "docs" / "validation-summary.md"

# Production corpus categories, by a word the record's file name carries.
CORPUS = [
    ("race-boards", "Race boards"),
    ("builder-starters", "Builder starters"),
    ("gear-swaps", "Gear swaps"),
    ("buff-toggles", "Buff toggles"),
    ("talent-census", "Talent census"),
    ("class-options", "Class options"),
    ("preset-matrix", "Preset matrix"),
]
COUNTED = ("matched", "rejected", "mismatched")


def one_line(text, limit=160):
    """The first sentence of a text, on one line, cut at a word near the limit."""
    if not text:
        return ""
    text = " ".join(str(text).split())
    sentence = re.split(r"(?<=\.)\s", text, maxsplit=1)[0]
    if len(sentence) > limit:
        sentence = sentence[:limit].rsplit(" ", 1)[0] + "..."
    return sentence.replace("|", "\\|")


def outcomes(record):
    """The outcome counts a record states: top-level counts, or its outcomes map."""
    if record.get("kind") == "production_census":
        return {key: int(value) for key, value in record.get("summary", {}).get("outcomes", {}).items()}
    if isinstance(record.get("outcomes"), dict):
        return {key: int(value) for key, value in record["outcomes"].items()}
    found = {key: int(record[key]) for key in COUNTED + ("errors",) if isinstance(record.get(key), int)}
    return found


def corpus_category(path, record):
    """The production corpus category of a record, or None."""
    name = path.name
    for word, label in CORPUS:
        if word in name:
            return label
    if str(record.get("kind", "")).startswith("production_") and record.get("kind") not in (
            "production_census", "production_benchmark"):
        return "Other production corpus"
    return None


def group(path, record):
    kind = record.get("kind", "")
    if kind == "production_census":
        return "census"
    if kind == "integration_regression":
        return "regression"
    if corpus_category(path, record):
        return "corpus"
    if kind == "compatibility_sweep":
        return "sweep"
    return "other"


def load(directory):
    records = []
    for path in sorted(Path(directory).glob("*.json")):
        record = json.loads(path.read_text())
        if isinstance(record, dict):
            records.append((path, record))
    return records


def counts_cell(found):
    if not found:
        return "", "", "", ""
    extra = ", ".join(f"{key} {value}" for key, value in sorted(found.items()) if key not in COUNTED)
    # A count the record does not state stays blank.
    return tuple(str(found[key]) if key in found else "" for key in COUNTED) + (extra,)


def link_to(path, docs):
    """A relative link from the document to a record."""
    return Path(os.path.relpath(path.resolve(), docs.parent.resolve())).as_posix()


def row(path, record, docs):
    matched, rejected, mismatched, extra = counts_cell(outcomes(record))
    scope = one_line(record.get("scope") or record.get("method"))
    revision = str(record.get("source_revision") or "")[:12]
    link = f"[{path.name}]({link_to(path, docs)})"
    return (f"| {record.get('kind', '')} | {scope} | {matched} | {rejected} | {mismatched} | {extra} "
            f"| {revision} | {link} |")


HEADER = ("| Kind | Scope | Matched | Rejected | Mismatched | Other | Source revision | Record |\n"
          "| --- | --- | ---: | ---: | ---: | --- | --- | --- |")


def totals(entries):
    """Summed outcomes of records, and how many records."""
    summed = {}
    for _, record in entries:
        for key, value in outcomes(record).items():
            summed[key] = summed.get(key, 0) + value
    return summed, len(entries)


def describe(summed):
    total = sum(summed.values())
    order = [key for key in COUNTED if key in summed] + sorted(key for key in summed if key not in COUNTED)
    parts = ", ".join(f"{summed[key]} {key}" for key in order)
    return f"{total} variants ({parts})" if total else "no counts"


def records(count):
    return f"{count} record" if count == 1 else f"{count} records"


def render(validation, benchmarks, docs=OUTPUT):
    groups = {name: [] for name in ("census", "corpus", "sweep", "regression", "other")}
    for path, record in validation:
        groups[group(path, record)].append((path, record))
    lines = [
        "# Validation summary",
        "",
        "Generated by `python3 tools/validation_summary.py` from the records under "
        "validation/ and benchmarks/. Every count is what "
        "a record states; regenerate after adding or refreshing one. Totals add the records' "
        "counts, and records can cover neighboring ground, as the Hunter and the all-class "
        "preset matrices do with different requests.",
        "",
        "## Totals",
        "",
    ]
    census = sorted(groups["census"], key=lambda entry: entry[0].name)
    if census:
        path, record = census[-1]
        summary = record.get("summary", {})
        supported = summary.get("outcomes", {}).get("supported", 0)
        total = summary.get("inputs", 0)
        lines.append(f"- Production builds supported: {supported} of {total}, from "
                     f"[{path.name}]({link_to(path, docs)}).")
    else:
        lines.append("- Production builds supported: no production census record.")
    corpus_total, corpus_records = totals(groups["corpus"])
    lines.append(f"- Production corpus: {describe(corpus_total)} in {records(corpus_records)}.")
    by_category = {}
    for path, record in groups["corpus"]:
        by_category.setdefault(corpus_category(path, record), []).append((path, record))
    for _, label in CORPUS + [(None, "Other production corpus")]:
        if label in by_category:
            summed, count = totals(by_category[label])
            lines.append(f"  - {label}: {describe(summed)} in {records(count)}.")
    sweep_total, sweep_records = totals(groups["sweep"])
    lines.append(f"- Randomized sweeps: {describe(sweep_total)} in {records(sweep_records)}.")
    regression_total, regression_records = totals(groups["regression"])
    lines.append(f"- Integration regressions, reruns of other records' inputs and not added above: "
                 f"{describe(regression_total)} in {records(regression_records)}.")
    lines.append(f"- Benchmark records: {len(benchmarks)}.")
    sections = [
        ("Production census", groups["census"]),
        ("Production corpus", groups["corpus"]),
        ("Randomized sweeps", groups["sweep"]),
        ("Integration regressions", groups["regression"]),
        ("Other validation", groups["other"]),
        ("Benchmarks", benchmarks),
    ]
    for title, entries in sections:
        if not entries:
            continue
        lines += ["", f"## {title}", "", HEADER]
        lines += [row(path, record, docs) for path, record in entries]
    return "\n".join(lines) + "\n"


def census_record(report, revision, date, inputs):
    """A validation record of a tools/census.py report."""
    return {
        "kind": "production_census",
        "date": date,
        "source_revision": revision,
        "scope": f"Which production requests ({inputs}) the Rust engine runs, each exported with the "
                 "pinned Go engine and checked by the Rust gate, with the reasons that block the rest.",
        "generator": f"python3 tools/census.py --output <scratch> {inputs}",
        "summary": report["summary"],
        "inputs": report["inputs"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", nargs="?", choices=["write", "check", "census"], default="write")
    parser.add_argument("report", nargs="?", type=Path, help="census: the tools/census.py report")
    parser.add_argument("--revision", help="census: the source revision the report ran at")
    parser.add_argument("--date", help="census: the record's date, by default today in UTC")
    parser.add_argument("--inputs", default="output/prod-requests", help="census: what the census read")
    parser.add_argument("--validation", type=Path, default=ROOT / "validation")
    parser.add_argument("--benchmarks", type=Path, default=ROOT / "benchmarks")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()
    if args.command == "census":
        if args.report is None or not args.revision:
            sys.exit("census needs a report and --revision")
        import datetime
        date = args.date or datetime.datetime.now(datetime.timezone.utc).date().isoformat()
        record = census_record(json.loads(args.report.read_text()), args.revision, date, args.inputs)
        path = args.validation / f"{date}-production-census.json"
        if path.exists():
            sys.exit(f"{path} already exists")
        path.write_text(json.dumps(record, indent=1) + "\n")
        print(f"wrote {path}")
        return
    text = render(load(args.validation), load(args.benchmarks), args.output.resolve())
    if args.command == "check":
        if not args.output.exists() or args.output.read_text() != text:
            sys.exit(f"{args.output} is out of date: run python3 tools/validation_summary.py")
        print(f"{args.output} is current")
        return
    args.output.write_text(text)
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
