# Hardening Plan — FinCrime: The Desk

> This document identifies hardening priorities for the simulation engine and its runtime surface. FinCrime: The Desk is a single-player desktop simulation, not a networked service — but hardening still matters for correctness, crash safety, and future extensibility.

---

## Threat Model

**Actors**

| Actor | Trust Level | Interaction surface |
|-------|-------------|---------------------|
| Local player | Trusted | Godot UI (keyboard/mouse) |
| Config files | Semi-trusted | JSON files in `./data/` |
| sim-runner IPC | Process-local | stdin/stdout, controlled by SimBridge.cs |
| SQLite database | Trusted | Local filesystem |

**Key risks in a desktop simulation:**

- Malformed config JSON crashing the process on startup
- Invalid IPC JSON causing parse failures or panics
- Player-supplied values (fees, risk dials) violating game invariants
- Test database files accumulating and not being cleaned up
- Future network exposure if a multiplayer mode is added

---

## Input Validation

### Config validation (`core/src/config.rs`)

**Current state:** Config is deserialized with `serde_json`. Missing fields or wrong types produce a startup panic. Numeric bounds (e.g., rate = -5.0) are not validated.

**P0 action:** Add range validation after deserialization. Example:

```rust
fn validate(&self) -> anyhow::Result<()> {
    anyhow::ensure!(
        self.economics.discount_rate_annual > 0.0 && self.economics.discount_rate_annual < 1.0,
        "discount_rate_annual must be in (0, 1)"
    );
    // ... per-field checks
    Ok(())
}
```

Call `config.validate()?` inside `SimConfig::load()` before returning.

### IPC command validation (`tools/src/main.rs`)

**Current state:** Invalid JSON returns `{"error": "..."}` and continues the loop. Valid JSON with unknown `cmd` values are silently ignored with `log::warn!`.

**P1 action:** Return structured errors for unknown commands rather than silently dropping:

```json
{ "error": "unknown_command", "cmd": "foobar" }
```

### Player command validation

**Current state:** `PricingSubsystem` emits `FeeChangeRejected` for UDAAP violations. `RiskAppetiteSubsystem` emits `RiskDialRejected` for out-of-bounds dial values.

**Status:** Already guarded — no P0 action needed. Ensure unit tests cover rejection paths.

---

## Error Handling

### Rust `Result<>` patterns

The codebase uses `SimResult<T>` (= `Result<T, SimError>`) throughout `fincrime-core`. Error propagation via `?` is consistent.

**P1 action:** Audit all `.unwrap()` and `.expect()` calls in `core/src/`. Replace with `?` or meaningful context:

```bash
grep -n "\.unwrap()\|\.expect(" core/src/**/*.rs
```

Any `.unwrap()` that can be triggered by external input (config, IPC) is a potential crash.

### Panic safety

Known panics (by design):

- `assert!(!self.clock.paused, ...)` in `tick()` — legitimate invariant
- `assert!(n > 0, ...)` in `SubsystemRng::next_u64_below()` — legitimate invariant

**P0:** All panics in the IPC hot path (stdin loop, `build_ui_state()`) should be converted to `Result`-returning code with proper error propagation.

### Crash recovery

**Current state:** If sim-runner crashes, `SimBridge.cs` catches the process exit and sets `_isRunning = false`. The UI shows "SimRunner process exited." but does not attempt a restart.

**P2 action:** Implement restart logic in `SimBridge.cs`:

```csharp
// In ReadOutputAsync(), after _isRunning = false:
if (!_userQuit) {
    CallDeferred(nameof(RestartSimulation));
}
```

The SQLite database supports crash recovery via WAL — uncommitted write-ahead log frames are rolled back on next open.

---

## Logging & Observability

### Current logging

`sim-runner` calls `env_logger::init()`. Log output goes to stderr. The subsystem loop uses `log::debug!` for snapshot saves.

**Enable debug logging:**

```bash
RUST_LOG=debug sim-runner --seed 42 --ticks 365
RUST_LOG=fincrime_core=trace sim-runner --seed 42 --ticks 365
```

### P1 recommendations

1. **Structured event log is already the primary audit trail** — every state change is an `EventLogEntry` in SQLite with run_id, tick, subsystem, event_type, and JSON payload. This is production-grade observability for a simulation.

2. **Add per-tick summary logging** at `log::info!` level: customers onboarded, transactions, complaints filed, so headless runs have visible progress without `--nocapture`.

3. **Add startup logging** in `SimEngine::build()`: log the number of subsystems registered and the config file paths loaded.

---

## Performance

### Current approach

No benchmarks directory exists. Performance is validated implicitly by test coverage and headless run times.

The engine is single-threaded by design (tick model is sequential). Each subsystem gets its own SQLite connection (`store.reopen()`) enabling concurrent reads, but writes are serialized by SQLite's WAL lock.

### P2 profiling recommendations

```bash
# Profile a 365-tick run
cargo build --release
RUST_LOG=warn ./target/release/sim-runner --seed 42 --ticks 3650 --db /tmp/bench.db

# Use cargo-flamegraph for Rust profiling
cargo install flamegraph
cargo flamegraph --bin sim-runner -- --seed 42 --ticks 365
```

**Expected hotspots:** SQL INSERT in `append_event()` (called for every event), `serde_json::to_string()` for event serialization, customer subsystem for large simulations.

---

## Deterministic Replay

The RNG architecture guarantees determinism:

1. Same `seed` → same `RngBank` → same `SubsystemRng` per subsystem per tick
2. RNG seeds are: `master_seed XOR (subsystem_index × K) XOR (tick × K2)`
3. `RunInitialized { run_id, seed }` is the first event logged — seed is always retrievable from the event log

**Replay test pattern:**

```rust
let run1 = run_n_ticks(seed, 30);
let run2 = run_n_ticks(seed, 30);
assert_eq!(run1.events_for_tick(run1.run_id, 15),
           run2.events_for_tick(run2.run_id, 15));
```

---

## Prioritized Checklist

### P0 — Do before any external release

- [ ] Config numeric range validation in `SimConfig::validate()`
- [ ] Replace `.unwrap()` in IPC hot path (`run_ipc_loop`, `build_ui_state`) with `Result`
- [ ] Test that malformed `data/*.json` files produce meaningful error messages, not panics

### P1 — Do in next development sprint

- [ ] Structured error responses for unknown IPC commands
- [ ] Audit all `.unwrap()` / `.expect()` in `core/src/`
- [ ] Add per-tick progress logging at `log::info!` level
- [ ] Add startup logging (subsystem count, config paths)

### P2 — Nice to have

- [ ] Sim-runner restart logic in `SimBridge.cs` on unexpected exit
- [ ] Flamegraph profiling of a 1-year headless run
- [ ] Formal benchmark in `core/benches/` for tick throughput
- [ ] Config schema validation (JSON Schema or custom validator)
