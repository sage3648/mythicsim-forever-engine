# Working in this repository

- This experimental Rust engine is separate from the MythicSim app and Go engine.
  Read README.md and ROADMAP.md before changing scope.
- Follow docs/contributor-guide.md for module ownership. Keep class spells and
  talents in their class domain, spec behavior in that class's specs domain, and
  reusable primitives in core/mechanics. Shared code must not import classes.
- Mirror class/spec integration tests under tests/classes. Preserve the public
  root API and existing JSON contracts during source-only refactors.
- Add modules for implemented behavior, not placeholder classes or specs.
- Preserve strict unsupported-input rejection and deterministic mechanics.
- Mechanics changes need evidence and focused regressions. Do not rewrite goldens
  just to agree with a changed implementation.
- Follow CONTRIBUTING.md for checks and UPSTREAM.md for reference pin updates.
- Preserve licensing and historical benchmark provenance.
- Keep changes small. Production integration and deployment are separate work.
- Do not use em dashes or en dashes in prose, comments, commits or Markdown.
- Use the configured human Git author. No AI coauthor trailers, generation footers,
  task links or session links in commits and PRs.
- Name a branch after the change itself, such as `feral-bear-parity`, never with a
  `claude/` prefix or any other tool or agent prefix.
