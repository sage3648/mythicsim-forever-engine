# Contributing

Code, small regression cases, controlled combat logs and clear documentation are
all useful contributions. Maintainers review changes before release.

## Development checks

Install stable Rust. Rust checks use frozen fixtures and do not require Go:

```sh
cargo fmt -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

For the matched benchmark, install Go 1.25.6 or later and run:

```sh
cd tools/matched-go
go test ./...
go vet ./...
```

From the repository root, check the comparison harness with Python 3:

```sh
python3 -m unittest discover -s tools -p '*_test.py'
```

CI runs these checks on pushes and pull requests. Live differential runs and heavy
benchmarks remain explicit local commands.

## Mechanics changes

1. Explain the behavior, relevant spell or item IDs and Forever client build.
2. Include evidence: an upstream commit, controlled logs or a reference case.
   Label uncertainty and assumptions.
3. Add a focused regression that fails with the old behavior. Use fixed seeds for
   random outcomes and cover relevant timing and resource boundaries.
4. Compare against Go where the same mechanic is supported. Explain intentional
   differences instead of forcing agreement with a known bug.

Do not regenerate expected outputs merely to make a change pass. Fixture updates
must explain the reference revision, generating command and reason.

## Performance changes

Keep outputs and logical work equivalent. Use release builds and retain raw samples.
Separate preparation, kernel, process and serialization costs. Do not infer
production savings from the prepared kernel. See the [kernel guide](docs/kernel.md).

## Pull requests

Keep changes small and explain the problem, resulting behavior, evidence and checks.
Coordinate larger features in an issue first. Preserve upstream licensing and
identify ported sources. Keep unsupported mechanics explicit.

Avoid em dashes and en dashes in documentation, comments and commit messages.
Remove credentials and personal information from any publicly posted logs or exports.
