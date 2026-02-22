# Entrypoints — FinCrime: The Desk

## Core Entrypoints

### Library (`core/src/lib.rs`)

`fincrime-core` is a pure Rust library crate. It exports all subsystems, the engine, store, event/command types, and config. No binary — it is only used by `sim-runner` and by integration tests.

**Public surface exposed by `lib.rs`:**

| Module | Key types |
|--------|-----------|
| `engine` | `SimEngine` |
| `store` | `SimStore` |
| `event` | `SimEvent`, `EventLogEntry`, `EconomicPhase` |
| `command` | `PlayerCommand`, `QueuedCommand` |
| `rng` | `RngBank`, `SubsystemRng`, `SubsystemSlot` |
| `subsystem` | `SimSubsystem` trait |
| `config` | `SimConfig` |
| `snapshot` | `SimSnapshot`, `SNAPSHOT_INTERVAL` |
| `clock` | `SimClock`, `SimSpeed` |
| `types` | `RunId`, `Tick`, `EntityId` |

### Binary (`tools/src/main.rs` → `sim-runner`)

The only binary in the workspace. Located at `tools/src/main.rs`.

**CLI flags:**

```
sim-runner --seed <u64>     Master RNG seed (default: 42)
           --ticks <u64>    Ticks to run in batch mode (default: 365)
           --db <path>      SQLite file path (default: :memory:)
           --data-dir <dir> Config directory (default: ./data)
           --ipc-mode       Enable stdin/stdout JSON IPC (used by Godot UI)
```

**Startup sequence (`main()`):**

1. Parse CLI args
2. Open `SimStore` (SQLite, WAL mode)
3. `store.migrate()` — apply all 25 migrations
4. `store.insert_run(run_id, seed, version)` — create run record
5. `SimEngine::build(run_id, seed, &store, data_dir)` — wire all subsystems and load config
6. Branch on `--ipc-mode`:
   - IPC mode → `run_ipc_loop()` (blocking stdin loop)
   - Batch mode → `engine.run_ticks(n)` then `print_summary()`

---

## Client Entrypoint

### `client/scripts/Main.cs` — `_Ready()`

Godot calls `_Ready()` on scene load. Init sequence:

1. Fetch `SimBridge` autoload node (`/root/SimBridge`)
2. Bind all UI node references (labels, buttons, views)
3. Create a `Timer` (0.5s interval → 2 ticks/second)
4. Connect `SimBridge` signals: `StateUpdated`, `TickAdvanced`, `SimulationError`
5. Call `_simBridge.Start()` → spawns `sim-runner.exe` process
6. Call `_simBridge.RequestState()` → fetches initial state before first tick
7. Simulation starts **paused**; player presses **PLAY** to begin

### `client/scripts/SimBridge.cs` — Bridge Lifecycle

`SimBridge` is registered as an autoload singleton in `project.godot`. It:

1. Spawns `sim-runner.exe` with `--ticks 0 --ipc-mode` via `ProcessStartInfo`
2. Redirects stdin, stdout, and stderr
3. Reads stdout lines asynchronously (`ReadOutputAsync`) and dispatches via `CallDeferred`
4. Parses each stdout line as JSON; emits `StateUpdated(json)` and optionally `TickAdvanced(tick)`
5. On scene exit (`_ExitTree`): sends `{"type":"quit"}`, waits 1s, then kills if needed

---

## Configuration Loading

`SimConfig::load(data_dir)` is called from `SimEngine::build()`. It reads all JSON files from the `data/` directory hierarchy. Config is deserialized once and cloned per subsystem.

Directory structure loaded:

```
data/
├── economics/segment_economics_config.json
├── risk/risk_appetite_config.json
├── products/           (≥2 files)
├── segments/           (1 file)
├── complaints/         (2 files)
├── churn/              (1 file)
├── offers/             (1 file)
├── payment/            (1 file)
├── reconciliation/     (1 file)
└── identity/           (2 files)
```

Config is immutable during a run — changes require a process restart.

---

## Migration System

Migrations live in `migrations/` and are applied by `SimStore::migrate()` at process startup. Numbering is sequential, zero-padded, starting at `001_foundation.sql`.

**Applied migrations (001–025):**

```
001_foundation.sql           → runs, simsnapshots, event_log, player_commands tables
002_macro.sql                → macro_state table
003_customers.sql            → customers, accounts tables
004_complaints.sql           → complaints table
005_economics.sql            → pnl_snapshots, segment_pnl tables
006_pricing.sql              → products, fee_changes tables
007_offers.sql               → offers, offer_matches tables
008_churn.sql                → churn_scores, life_events, churn_cohorts tables
009_customer_close_tick.sql  → closed_at_tick column on customers
010_segment_pnl.sql          → segment_pnl_snapshots table
011_complaint_analytics.sql  → complaint_patterns, sla_snapshots, early_warning_alerts
012_risk_appetite.sql        → risk_dials, dial_changes, board_pressure_events
013_payment_rails.sql        → payment_rails, payment_batches, card_authorizations
014_reconciliation.sql       → recon_exceptions, recon_snapshots
015_customer_identity.sql    → customer_identities, customer_addresses, customer_phones
016_business_and_account_types.sql → extended account type columns
017_custodial_trust_international.sql → trust/custodial/international account tables
018_risk_scoring_joint_ownership.sql → risk scores, joint ownership tables
019_incident_outage.sql      → incidents, system_components, system_metrics
020_card_disputes.sql        → card_disputes table
021_fraud_detection.sql      → fraud_patterns, fraud_alerts
022_aml_screening.sql        → aml_screenings, aml_alerts, sanctions/PEP lists (largest: 11 KB)
023_add_customer_names.sql   → customer name columns
024_transaction_monitoring.sql → tm_alerts, ctrs tables
025_sar_filing.sql           → sars, sar_late_filings tables
```

All migrations run in a single transaction. They are append-only — no destructive migrations.

---

## Simulation Loop

`SimEngine::tick()` is the core loop step (`engine.rs:439`):

```rust
pub fn tick(&mut self) -> SimResult<Vec<SimEvent>> {
    assert!(!self.clock.paused, "tick() called on paused engine");
    let current_tick = self.clock.advance();
    let mut tick_events = vec![SimEvent::TickStarted { tick: current_tick }];

    // Inject queued player commands into the event stream
    tick_events.append(&mut self.pending_commands);

    // Execute each subsystem in registration order
    for (slot, subsystem) in &mut self.subsystems {
        let mut rng = self.rng_bank.for_subsystem_at_tick(*slot, current_tick);
        let new_events = subsystem.update(current_tick, &tick_events, &mut rng)?;

        for event in &new_events {
            self.store.append_event(&EventLogEntry { ... })?;
        }
        tick_events.extend(new_events);
    }

    tick_events.push(SimEvent::TickCompleted { tick: current_tick });

    if current_tick.is_multiple_of(SNAPSHOT_INTERVAL) {
        self.take_snapshot(current_tick)?;
    }
    Ok(tick_events)
}
```

`run_ticks(n)` calls `tick()` n times in a loop, emitting `RunInitialized` on tick 0.
