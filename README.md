# FinCrime: The Desk

> A deterministic, tick-based banking operations simulation engine — you run the compliance desk. Every decision has consequences.

---

## What Is This?

**FinCrime: The Desk** is a single-player simulation game in which the player manages the financial crime and operations desk of a retail bank. Built on a deterministic Rust engine, the simulation advances one discrete daily tick at a time — matching banking's own operating cadence of daily batch processing, D+1 reconciliations, and SLA deadlines measured in business days.

The core simulation models 20 subsystems spanning the full lifecycle of a retail bank: customer onboarding with rich identity verification, multi-rail payment processing (ACH, SWIFT, card authorization), fraud detection with pattern recognition, AML screening against sanctions and PEP lists, transaction monitoring with CTR/SAR filing, complaint management with CFPB-style SLA enforcement, and real-time P&L with segment economics. Every event — from a card authorization to a Suspicious Activity Report filing — is persisted to an append-only event log in SQLite, enabling deterministic replay.

The player issues commands through a Godot 4 UI client: resolve complaints, adjust product fees, set risk appetite dials, and manage incidents. The UI communicates with the Rust core through `sim-runner.exe`, a headless bridge process using stdin/stdout JSON IPC — keeping simulation logic completely isolated from rendering.

**Key features (derived from implemented subsystems and migrations 001–025):**

- 20 subsystems registered in a fixed, documented execution order
- 25 schema migrations covering every domain from macro economics to SAR filing
- Deterministic replay: same seed + same commands → identical run, byte-for-byte
- Per-subsystem seeded PRNG (PCG64Mcg) — adding subsystems never disturbs existing streams
- SQLite in WAL mode — concurrent reads from the UI while the simulation thread writes
- Snapshot checkpointing every `SNAPSHOT_INTERVAL` ticks
- Headless mode for testing and fast-forward: `cargo test` launches no GUI
- 5 UI views: Overview (6 KPIs), Customers, Complaints, Products, P&L Report

---

## Architecture Summary

```
┌──────────────────────────────────────────────┐
│  CLIENT LAYER  (Godot 4 + C#)                │
│  Main.cs → SimBridge.cs                      │
│  Views: Overview, Customers, Complaints,      │
│          Products, PnL                        │
└────────────────┬─────────────────────────────┘
                 │  stdin/stdout JSON IPC
                 │  (ProcessStartInfo pipe)
┌────────────────▼─────────────────────────────┐
│  BRIDGE LAYER  (tools/sim-runner.exe)         │
│  IPC loop: tick | get_state | command | quit  │
└────────────────┬─────────────────────────────┘
                 │  Rust function calls
┌────────────────▼─────────────────────────────┐
│  CORE LAYER  (fincrime-core, pure Rust lib)   │
│  SimEngine → 20 × SimSubsystem::update()      │
│  RngBank (PCG64Mcg, per-subsystem seed)       │
│  Event log (append-only SimEvent stream)      │
└────────────────┬─────────────────────────────┘
                 │  rusqlite (bundled)
┌────────────────▼─────────────────────────────┐
│  DATA LAYER  (SQLite, WAL mode)               │
│  25 migrations, 1 file per run                │
│  SimStore — the ONLY place SQL executes       │
└──────────────────────────────────────────────┘
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for detailed subsystem pipeline, data flow, and Mermaid diagrams.

---

## Quickstart

### Prerequisites

| Tool | Version | Notes |
|------|---------|-------|
| Rust | stable (≥ 1.75) | `rustup install stable` |
| Godot | 4.x .NET edition | Required for the UI client |
| .NET SDK | 6.0+ | Required by Godot .NET |

### Build & run headless

```bash
# Clone
git clone <repo-url>
cd fincrime_thedesk

# Build the workspace
cargo build --release

# Run a 365-tick simulation (headless, no UI)
./target/release/sim-runner --seed 42 --ticks 365 --db run.db --data-dir ./data
```

### Run with the Godot UI

```bash
# 1. Build sim-runner and copy to client/
cargo build --release -p sim-runner
copy target\release\sim-runner.exe client\sim-runner.exe

# 2. Open client/ in Godot 4 .NET edition
#    Project → Open → select client/project.godot

# 3. Press F5 (or the Play button) — the UI auto-launches sim-runner.exe
```

The UI starts paused. Click **PLAY** to begin advancing ticks at 2 ticks/second.

---

## Running the Simulation

### Headless mode (batch run)

```bash
sim-runner --seed 12345 --ticks 730 --db playthrough.db --data-dir ./data
```

After completion, the runner prints a summary:

```
=== RUN SUMMARY ===
  run_id:         run-12345-<timestamp>
  ticks run:      730
  customers:      N
  churned:        N
  total txns:     N
  complaints:     N
  sla breaches:   N

=== FINANCIAL SUMMARY (Last 4 Quarters) ===
  Q1-Y1 | Profit: $N | NIM: N.NN% | Eff: N.N%
  ...
```

### IPC mode (used by the UI)

```bash
sim-runner --ticks 0 --ipc-mode
# Now reads JSON commands from stdin, writes state JSON to stdout
```

IPC messages sent from `SimBridge.cs`:

```json
// Advance N ticks
{ "type": "tick", "count": 1 }

// Query current state (returns UiState JSON)
{ "type": "get_state" }

// Issue a player command
{ "type": "command", "cmd": "resolve_complaint", "payload": { ... } }

// Shutdown cleanly
{ "type": "quit" }
```

State response includes: `tick`, `paused`, `active_customers`, `churned_customers`, `complaint_count`, `sla_breaches`, `backlog`, `nim`, `efficiency_ratio`, `pre_tax_profit`, `pnl_history`, `complaints`.

---

## Configuration

All simulation parameters are JSON files in `./data/`. The engine loads them at startup via `SimConfig::load(data_dir)`.

| Directory | Contents |
|-----------|----------|
| `data/economics/` | Segment P&L model, CLV assumptions, cost allocation |
| `data/risk/` | Risk appetite dials, board pressure thresholds |
| `data/products/` | Product definitions, fee schedules |
| `data/segments/` | Customer segment probabilities and behaviors |
| `data/complaints/` | SLA config, resolution codes, satisfaction deltas |
| `data/churn/` | Churn model parameters, life event probabilities |
| `data/offers/` | Offer catalog, bonus conditions |
| `data/payment/` | Payment rail config (ACH, SWIFT, card) |
| `data/reconciliation/` | Recon exception thresholds and aging rules |
| `data/identity/` | Identity verification config, synthetic identity rates |
| `data/typologies/` | (Reserved for future AML typology config) |

**Example** — `data/economics/segment_economics_config.json` (excerpt):

```json
{
  "cost_allocation_model": {
    "acquisition_cost_per_customer": {
      "mass_market":    85.0,
      "mid_tier":      150.0,
      "student":        45.0,
      "small_business": 320.0
    },
    "monthly_servicing_cost_per_customer": {
      "mass_market":   4.50,
      "mid_tier":      8.00,
      "student":       3.00,
      "small_business": 18.00
    }
  },
  "clv_model": {
    "discount_rate_annual": 0.12,
    "projection_horizon_years": 5
  }
}
```

To override a parameter: edit the JSON and restart the simulation. Config is read once at `SimEngine::build()`.

---

## Testing

```bash
# Run all tests in the workspace
cargo test

# Run only core library tests
cargo test -p fincrime-core

# Run tests with output (useful for determinism checks)
cargo test -p fincrime-core -- --nocapture

# Run a specific test file
cargo test -p fincrime-core --test incident
```

Tests use `SimEngine::build_test()` or `SimEngine::build_test_with_incidents()`, which spin up a temporary SQLite file (no data/ directory required). The temp database is created at `./test_<uuid>.db` and should be cleaned up after each test run.

The CI enforces: no `thread_rng`, no `SystemTime`, no `Instant::now` in `/core/src/` — any platform non-determinism fails the build.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|-------------|-----|
| `Failed to start sim-runner` in Godot | `sim-runner.exe` not in `client/` | Build and copy: `cargo build --release && copy target\release\sim-runner.exe client\` |
| `database is locked` | Two sim-runner instances pointing at the same `.db` | Kill the orphaned process; SQLite WAL allows readers but only one writer |
| No state printed in IPC mode | `--ipc-mode` flag missing | Pass `--ticks 0 --ipc-mode` to suppress summary mode |
| Test `.db` files accumulating | Tests create `test_<uuid>.db` files | `del test_*.db` in the repo root |
| Config not found | `--data-dir` not set correctly | Default is `./data`; pass `--data-dir path/to/data` |
| `thread_rng` / `SystemTime` CI failure | Non-deterministic code added to `/core` | Replace with `rng.next_f64()` or `rng.chance(p)` from `SubsystemRng` |

---

## Roadmap

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 0 | ✅ Complete | Engine scaffolding, tick model, SQLite, RNG architecture |
| Phase 1A | ✅ Complete | Macro subsystem (economic cycle) |
| Phase 1B | ✅ Complete | Customer, Account, Transaction subsystems |
| Phase 1C | ✅ Complete | Complaint subsystem, SLA enforcement |
| Phase 1D | ✅ Complete | Economics subsystem, quarterly P&L |
| Phase 2.1 | ✅ Complete | Pricing subsystem, product fee management |
| Phase 2.2 | ✅ Complete | Offer subsystem, bonus tracking |
| Phase 2.3 | ✅ Complete | Churn subsystem, life events |
| Phase 2.5 | ✅ Complete | Complaint analytics, early warning alerts |
| Phase 2.6 | ✅ Complete | Risk appetite dials, board pressure |
| Phase 3.1 | ✅ Complete | Payment hub (ACH, SWIFT, card authorization) |
| Phase 3.2 | ✅ Complete | Reconciliation, exception management |
| Phase 3.3 | ✅ Complete | Incident & Outage management |
| Phase 3.4 | ✅ Complete | Card disputes, chargebacks |
| Phase 3.5 W3 | ✅ Complete | Fraud detection, pattern recognition |
| Phase 3.5 W4 | ✅ Complete | AML screening, sanctions/PEP matching |
| Phase 3.5 W5 | ✅ Complete | Transaction monitoring, CTR filing |
| Phase 3.5 W6 | ✅ Complete | SAR filing, regulatory fines (migration 025) |
| Phase 4 | 🔲 Planned | Enterprise multi-user mode (PostgresStore swap-in) |

---

## License

[UNKNOWN: license — suggest MIT or Apache-2.0, or ask project owner]
