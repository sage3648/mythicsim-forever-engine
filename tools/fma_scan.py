#!/usr/bin/env python3
"""List the fused multiply-adds in the pinned Go reference binary, by function and source line.

Go's arm64 compiler fuses a floating point add or subtract with a product that feeds it into one
FMADD, FMSUB, FNMSUB or FNMADD instruction, which rounds once. The rewrite works on SSA values,
so it reaches across statements and inlined calls; only an explicit float64 conversion stops it.
Rust must call mul_add exactly where Go fused, or a rare value differs in its last bit and a log
line by 0.001. This prints each fused instruction of the simulation packages and the exporter
with its Go source line, so a port can check its formula against the binary instead of guessing.

usage: python3 tools/fma_scan.py [BINARY] [--source DIR]

BINARY defaults to oracle-cache/forever-go-oracle-v2, which tools/prepared_v2.py builds; DIR
defaults to oracle-cache/source, the pinned checkout. Needs the Go toolchain for objdump.
"""
import argparse
import os
import re
import subprocess
from collections import Counter

FUSED = re.compile(r'\s(FMADDD|FMSUBD|FNMSUBD|FNMADDD)\s')
PREFIX = 'github.com/wowsims/forever/'


def scan(binary):
    """Fused instructions by (function, file:line, op), with their counts."""
    listing = subprocess.run(['go', 'tool', 'objdump', binary], capture_output=True, text=True,
                             check=True).stdout
    counts = Counter()
    function = ''
    for line in listing.splitlines():
        if line.startswith('TEXT '):
            function = line.split()[1]
            continue
        match = FUSED.search(line)
        if match and (function.startswith(PREFIX + 'sim/') or function.startswith('main.')):
            counts[(function, line.split()[0], match.group(1))] += 1
    return counts


def source_line(source, function, location):
    """The Go source text at a location, looked up in the function's package first."""
    name, number = location.rsplit(':', 1)
    candidates = []
    match = re.match(re.escape(PREFIX) + r'(sim/[\w/]+?)\.', function)
    if match:
        candidates.append(os.path.join(source, match.group(1), name))
    if function.startswith('main.'):
        candidates.append(os.path.join(os.path.dirname(__file__), 'oracle-v2', name))
    candidates += [os.path.join(path, name) for path, _, files in os.walk(source) if name in files]
    for path in candidates:
        if os.path.exists(path):
            with open(path) as handle:
                return handle.read().splitlines()[int(number) - 1].strip()
    return '?'


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('binary', nargs='?', default='oracle-cache/forever-go-oracle-v2')
    parser.add_argument('--source', default='oracle-cache/source')
    args = parser.parse_args()
    for (function, location, op), count in sorted(scan(args.binary).items()):
        short = function.replace(PREFIX, '')
        print(f'{location:36s} {op:8s} {count:2d}  {short}')
        print(f'{"":36s}   {source_line(args.source, function, location)}')


if __name__ == '__main__':
    main()
