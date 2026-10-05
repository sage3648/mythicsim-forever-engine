# Shadow sims

Shadow sims compare Rust with the pinned Go engine on real MythicSim
traffic, without users seeing them. The application samples finished
Forever Quick Sims, and a shadow worker reruns each request with both
engines. The application side is documented in the MythicSim repository at
`docs/forever-shadow-sims.md`.

This repository provides `tools/shadow.py`.

## Build a bundle

```sh
python3 tools/shadow.py build --output ~/forever-shadow/bundle-<short-sha>
```

A bundle is a folder with the release Rust engine, the Go exporter built at
the reference pin, the comparison tools and `manifest.json`, which records the
Rust revision, the pin, the client build and the exporter digest. The shadow
worker runs it with Python 3 only.

## Compare one request

```sh
python3 tools/shadow.py run --request REQUEST.json --output /tmp/shadow-run --seed 7
```

The run prints one JSON verdict and writes it, with the prepared input and
both results, into the output folder. An unseeded request gets `--seed`, so
both engines draw the same numbers.

| Status | Meaning |
| --- | --- |
| `match` | Every result field and the first-fight log equal the pinned Go engine |
| `mismatch` | `differences` and `first_log_difference` show what differs |
| `refused` | The coverage gate declined the input; `reasons` says why, and `refusals` gives each reason with its stable code |
| `error` | A step failed; `stage` and `error` say where |

`timings_ms` holds the wall time of the prepare, Go and Rust steps, process
start included. `speedup` is the Go time over the Rust time.

With `--production RESULT.json`, the production `RaidSimResult` of the same
request, the verdict also carries `production`, `rust` and `go` summaries:
DPS with its standard deviation, the fight length, active pets and the top
abilities per average fight. `versus_production` compares Rust with
production. Production used its own random numbers, so the DPS difference is
given in standard errors (`standard_errors`): a few either way is noise, a
large value is a real gap. Ability rows pair the two results by action ID.

## Comparison alerts

The application posts each shadow verdict to the admin-only Discord channel
named "comparison", through the webhook in `DISCORD_SHADOW_WEBHOOK_URL`. The
webhook URL is a secret: anyone with it can post to the channel, and this
repository is public. Keep it in `~/forever-shadow/.env`, readable only by its
owner, beside the bundles, and load it with `set -a; . ~/forever-shadow/.env;
set +a`. `.env` files are ignored by Git here. A webhook only posts; reading the
channel needs the application's admin endpoint or a Discord bot.

The bundle the worker runs is the Rust pin of the alerts. After a change lands
on `main`, build a bundle at the new commit and point the worker's
`FOREVER_SHADOW_BUNDLE` at it, as the application's
`docs/forever-shadow-sims.md` describes.

## From a mismatch to a fix

Take the request from the shadow run's output folder and accept it as a
fixture with `python3 tools/prepared_v2.py accept`. Then fix Rust until the
fixture matches, as for any other mechanics change.
