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
| 8. Mana recovery, cooldowns and the complete reference build | 2026-10-03 11:59 | 2026-10-03 12:04 | 0 h 05 m | Cold Snap, gems, Evocation, consumables; found Go's ready-time quirk; full reference matches Go |
| 11. Randomized Frost compatibility sweep | 2026-10-03 12:05 | 2026-10-03 12:16 | 0 h 11 m | 24 variants match Go; #622 guard narrowed to conditions whose compilation differs |
| Performance: output-identical allocation pass | 2026-10-03 12:21 | 2026-10-03 12:30 | 0 h 09 m | 273 ms to 81 ms for the reference build (Go 205 ms); 41 inputs byte-identical before and after |
| 4. Reference pin centralization | 2026-10-03 12:31 | 2026-10-03 12:36 | 0 h 05 m | One written pin read by Rust, Python and Go helpers; 17 prepared inputs and 11 v1 cases reproduce unchanged |
| 10. Reports and timelines | 2026-10-03 12:36 | 2026-10-03 12:50 | 0 h 14 m | Whole Go result compared; fixed time to OOM after Go's aura teardown, zero distributions, target actions and every-fight logs; application parsers read identical timelines |
| Arcane: or and auraNumStacks | 2026-10-03 12:50 | 2026-10-03 12:55 | 0 h 05 m | Arcane preset operators with Go folding and the #622 comparison |
| Arcane: Arcane Blast and its stacks | 2026-10-03 12:55 | 2026-10-03 13:01 | 0 h 06 m | Flat damage modifier with reset rounding; stacks spent after the roll, held by Missiles; both fixtures match Go on the first comparison |
| Arcane: Arcane Power and Presence of Mind | 2026-10-03 13:01 | 2026-10-03 13:08 | 0 h 07 m | Damage, direct damage and cast time modifiers; Presence of Mind restarts its cooldown when consumed; both fixtures match Go on the first comparison |
| Arcane: Ignite as an inert listener | 2026-10-03 13:06 | 2026-10-03 13:09 | 0 h 03 m | Claimed only while no reachable spell is fire; Arcane with fire talents matches Go |
| Arcane: Touch of the Grave and the reference build | 2026-10-03 13:09 | 2026-10-03 13:15 | 0 h 06 m | Racial proc with internal cooldown and batch delay, hit-only drain, health metrics; the application's Arcane request matches Go on the first comparison |

## Notes

- Piece 2 included reading the Go scheduler, casting, aura, proc, channel, cooldown,
  consumable and APL code needed to design the contract. Later Frost mechanics reuse
  that reading, so their times are not directly comparable.
