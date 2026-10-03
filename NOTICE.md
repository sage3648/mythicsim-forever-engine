# Provenance and acknowledgments

This repository starts with the bounded Rust prototype and matched benchmark
developed in [MythicSim](https://github.com/sage3648/mythicsim), commits
`fdcea74ae6` and `bd0f4af6`. Only engine code, fixtures, benchmark tools and reports
were extracted. Application configuration and private simulation data are excluded.

RNG behavior, combat semantics and the Go reference helper derive from the
MIT-licensed wowsims lineage, including
[ElliotWood/Forever](https://github.com/ElliotWood/Forever) and
[sage3648/mythicsim-forever-engine-go](https://github.com/sage3648/mythicsim-forever-engine-go).
Upstream copyright and permission notices are preserved in [LICENSE](LICENSE).

Fixtures use synthetic characters at revision
`6823b49eb8aff741f197ef36d83766ef6a218285`. Historical reports and snapshots retain
the original experiment's date, compiler settings and source digest. Extracting
them does not constitute a new timing measurement.
