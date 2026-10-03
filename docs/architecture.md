# Architecture

## Current implementation

```mermaid
flowchart LR
    A[Fixed Go test characters] --> B[Pinned Go preparation helper]
    B --> C[Prepared JSON snapshots]
    C --> D[Rust Frostbolt kernel]
    D --> E[JSON metrics and optional trace]
    A --> F[Pinned full Go simulation]
    F --> G[Differential comparison]
    E --> G
```

Rust owns the event loop and aggregation, and never calls Go during a fight.
Checked-in snapshots and goldens allow Rust tests without Go. `tools/matched-go`
is a benchmark control for the same narrow model, not the full Go engine.

## Proposed hybrid

```mermaid
flowchart TD
    A[Character, gear and rotation] --> B{Validated Rust coverage?}
    B -->|Yes| C[Go prepares character and spell parameters]
    C --> D[Rust executes complete supported fight]
    B -->|No| E[Existing Go engine executes fight]
    D --> F[Common result adapter]
    E --> F
    F --> G[MythicSim results]
```

Route on all supported mechanics, not just class name. Compare results in development
before enabling real requests. Use a versioned preparation contract and batch across
a process boundary, rather than calling Go for individual events.

The current prototype lacks this router, complete preparation adapter, full fight
model and shared production result contract.

## Intended full cutover

```mermaid
flowchart TD
    A[Character, gear and rotation] --> B[Rust character preparation]
    C[Versioned Forever item, spell and talent data] --> B
    B --> D[Rust rotation and combat simulation]
    D --> E[Rust result aggregation]
    E --> F[MythicSim results]
```

The API, frontend and job system can retain their contracts through adapters.
Go can remain a development reference after leaving production. Community fixes
still need reviewed ports and regressions. See [UPSTREAM.md](../UPSTREAM.md) and
the [roadmap](../ROADMAP.md). The [migration plan](hybrid-migration-plan.md) defines
the staged path to removing Go from production while keeping the development reference.
