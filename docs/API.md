# Internal API Reference — FinCrime: The Desk

> This document covers the internal API surface: IPC protocol, commands, events, and the store interface. It is intended for contributors extending the engine or building alternative clients.

---

## IPC Protocol (sim-runner ↔ Godot)

The bridge uses **stdin/stdout JSON IPC** over `ProcessStartInfo` pipes. One JSON object per line. No framing headers — a `\n` terminates each message.

### Transport

| Direction | Channel | Format |
|-----------|---------|--------|
| Client → sim-runner | stdin | One JSON line per message |
| sim-runner → Client | stdout | One JSON line per response |
| sim-runner → Client | stderr | Plain text log lines (not parsed) |

### Inbound messages (client → sim-runner)

Defined in `tools/src/main.rs` as `IpcCommand`:

```json
// Advance the simulation by N ticks; returns UiState
{ "type": "tick", "count": 1 }

// Query current state without advancing; returns UiState
{ "type": "get_state" }

// Issue a player command; returns UiState after applying
{ "type": "command", "cmd": "resolve_complaint", "payload": { ... } }

// Clean shutdown
{ "type": "quit" }
```

### Outbound message (sim-runner → client)

Every `tick`, `get_state`, and `command` returns a `UiState` JSON object:

```json
{
  "tick": 42,
  "paused": false,
  "active_customers": 1250,
  "churned_customers": 38,
  "complaint_count": 17,
  "sla_breaches": 3,
  "backlog": 5,
  "nim": 2.84,
  "efficiency_ratio": 61.2,
  "pre_tax_profit": 148200.0,
  "pnl_history": [
    { "period": "Q1-Y1", "gross_income": 250000.0, "pre_tax_profit": 148200.0,
      "nim": 2.84, "efficiency_ratio": 61.2, ... }
  ],
  "complaints": [
    { "complaint_id": "comp-abc", "customer_id": "cust-xyz",
      "issue": "fee_dispute", "priority": "high",
      "filed_at_tick": 38, "status": "open", ... }
  ]
}
```

### Error response

If stdin contains invalid JSON, `sim-runner` responds:

```json
{ "error": "<serde_json error message>" }
```

---

## Commands

Defined in `core/src/command.rs` as `PlayerCommand`. Commands are submitted via `SimEngine::submit_command()` and injected into the next tick's event stream.

### Clock control

| Command | Fields | Description |
|---------|--------|-------------|
| `Pause` | — | Pause the clock (disables `tick()`) |
| `Resume` | — | Resume from pause |
| `SetSpeed` | `speed: SimSpeed` | Change simulation speed |

### Gameplay commands

| Command | Fields | Description |
|---------|--------|-------------|
| `CloseComplaint` | `complaint_id: String`<br>`resolution_code: String` | Resolve an open complaint using a resolution code |
| `SetProductFee` | `product_id: String`<br>`fee_type: String`<br>`new_value: f64` | Change a product fee; governed by PricingSubsystem (UDAAP guard) |
| `SetRiskDial` | `dial_id: String`<br>`new_value: f64` | Adjust a risk appetite dial; validated by RiskAppetiteSubsystem |

**Fee types** for `SetProductFee`: `"monthly_fee"` | `"overdraft_fee"` | `"nsf_fee"` | `"atm_fee"` | `"wire_fee"`

### IPC wrapper (via `sim-runner`)

The IPC `command` message dispatches to `handle_command()` in `tools/src/main.rs`. Currently implemented:

```json
{
  "type": "command",
  "cmd": "resolve_complaint",
  "payload": {
    "complaint_id": "comp-abc123",
    "resolution": "full_refund",
    "refund": 35.0
  }
}
```

---

## Events

Defined in `core/src/event.rs` as `SimEvent`. Events are the exclusive inter-subsystem communication channel. 50+ variants, grouped by phase.

### Engine events

| Variant | Key fields | Emitted by |
|---------|-----------|-----------|
| `TickStarted` | `tick` | Engine, start of each tick |
| `TickCompleted` | `tick` | Engine, end of each tick |
| `RunInitialized` | `run_id`, `seed` | Engine, only on tick 0 |

### Core simulation events (selection)

| Variant | Key fields | Emitted by |
|---------|-----------|-----------|
| `CustomerOnboarded` | `tick`, `customer_id`, `segment`, `account_id` | CustomerSubsystem |
| `CustomerChurned` | `tick`, `customer_id`, `segment`, `churn_risk` | ChurnSubsystem |
| `FeeCharged` | `tick`, `customer_id`, `fee_type`, `amount` | TransactionSubsystem |
| `ComplaintFiled` | `tick`, `complaint_id`, `customer_id`, `issue`, `priority` | ComplaintSubsystem |
| `ComplaintResolved` | `tick`, `complaint_id`, `resolution_code`, `satisfaction_delta` | ComplaintSubsystem |
| `SLABreached` | `tick`, `complaint_id`, `days_overdue` | ComplaintSubsystem |
| `QuarterlyPnLComputed` | `tick`, `period`, `gross_income`, `nim`, `efficiency_ratio` | EconomicsSubsystem |
| `ProductFeeChanged` | `tick`, `product_id`, `fee_type`, `old_value`, `new_value` | PricingSubsystem |
| `PaymentBatchCreated` | `tick`, `batch_id`, `rail_id`, `item_count`, `total_amount` | PaymentHubSubsystem |
| `ReconExceptionCreated` | `tick`, `exception_id`, `rail_id`, `delta_amount` | ReconciliationSubsystem |
| `FraudAlertGenerated` | `tick`, `alert_id`, `entity_id`, `fraud_score`, `severity` | FraudDetectionSubsystem |
| `AMLScreeningHit` | `tick`, `screening_id`, `customer_id`, `match_type`, `match_score` | AMLScreeningSubsystem |
| `CTRFiled` | `tick`, `ctr_id`, `customer_id`, `amount` | TransactionMonitoringSubsystem |
| `SARFiled` | `tick`, `sar_id`, `customer_id`, `activity_type`, `suspicious_amount` | (migration 025) |
| `IncidentCreated` | `tick`, `incident_id`, `component`, `severity`, `description` | IncidentSubsystem |
| `BoardPressureFired` | `tick`, `pressure_type`, `message`, `severity` | RiskAppetiteSubsystem |

### Economic phase enum

```rust
pub enum EconomicPhase {
    Expansion,   // fraud_multiplier: 1.0
    Peak,        // fraud_multiplier: 1.1
    Contraction, // fraud_multiplier: 1.35
    Trough,      // fraud_multiplier: 1.6
}
```

### Event log persistence

Every event is persisted as an `EventLogEntry`:

```rust
pub struct EventLogEntry {
    pub id: Option<i64>,
    pub run_id: RunId,
    pub tick: Tick,
    pub subsystem: String,
    pub event_type: String,
    pub payload: String, // JSON-encoded SimEvent
}
```

---

## Store Interface

`SimStore` (`core/src/store/`) is the exclusive SQL layer. All subsystems hold a `SimStore` instance opened via `store.reopen()` (separate connection per subsystem, WAL mode enables concurrent reads).

### Key query methods

| Method | Returns | Description |
|--------|---------|-------------|
| `customer_count(run_id, status)` | `i64` | Count by status ("active"/"churned") |
| `complaint_count(run_id)` | `i64` | Total complaints filed |
| `sla_breach_count(run_id)` | `i64` | Total SLA breaches |
| `open_complaints(run_id)` | `Vec<ComplaintRecord>` | All open complaints |
| `latest_pnl_snapshots(run_id, n)` | `Vec<PnLSnapshot>` | Most recent N quarterly snapshots |
| `all_pnl_snapshots(run_id)` | `Vec<PnLSnapshot>` | Full P&L history |
| `churn_scores_at_tick(run_id, tick)` | `Vec<ChurnScore>` | Churn scores for a tick |
| `events_for_tick(run_id, tick)` | `Vec<EventLogEntry>` | All events at a tick (for replay) |
| `append_event(entry)` | `SimResult<()>` | Append event to log (engine only) |
| `save_snapshot(run_id, tick, json)` | `SimResult<()>` | Persist snapshot JSON |
| `migrate()` | `SimResult<()>` | Apply all 25 pending migrations |
| `insert_run(run_id, seed, version)` | `SimResult<()>` | Create run record |

### Ownership model

- `SimEngine` holds the primary `SimStore`
- Each subsystem receives `store.reopen()` — a new connection to the same SQLite file
- WAL mode: one concurrent writer (engine/subsystem doing inserts) + unlimited concurrent readers
- All SQL is parameterized — no dynamic SQL string concatenation
