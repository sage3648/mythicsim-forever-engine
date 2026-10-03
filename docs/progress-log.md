# Progress log

Actual elapsed time per migration piece, so estimates follow demonstrated progress.
Times are UTC wall-clock for the working session that produced each change, including
reading the Go reference. They are not effort estimates for other contributors.

| Piece | Started | Finished | Elapsed | Result |
| --- | --- | --- | --- | --- |
| 2. Prepared v2 contract and release manifest | 2026-10-03 10:51 | 2026-10-03 11:17 | 0 h 26 m | Exporter, strict Rust types, coverage gate, fixture family, release manifest |
| 3. Upstream ledger and mechanics map | 2026-10-03 11:17 | 2026-10-03 11:22 | 0 h 05 m | 53 community commits reviewed; fix #622 traced to a Rust guard and regression |
| 5, 7. Fight runtime with Frostbolt v2 parity | 2026-10-03 11:22 | 2026-10-03 11:50 | 0 h 28 m | Go-ordered runtime, shared and labeled RNG, line-identical first-fight logs; 16 Frostbolt scenarios match Go |
| 8. Winter's Chill and Judgement of Wisdom | 2026-10-03 11:50 | 2026-10-03 11:54 | 0 h 04 m | Dynamic spell modifiers; found Go's enemy swing-offset draw; reference character matches Go |
| 8. Ice Lance, Fingers of Frost and Shatter | 2026-10-03 11:54 | 2026-10-03 11:56 | 0 h 02 m | In-flight cast rule and charge use; ranks 2 and 1 match Go on the first comparison |
| 8. Clearcasting, Missile Barrage and Arcane Missiles | 2026-10-03 11:56 | 2026-10-03 11:59 | 0 h 03 m | Channel ticks, partial resists and tick-length modifier; every proc talent matches Go |

## Notes

- Piece 2 included reading the Go scheduler, casting, aura, proc, channel, cooldown,
  consumable and APL code needed to design the contract. Later Frost mechanics reuse
  that reading, so their times are not directly comparable.
