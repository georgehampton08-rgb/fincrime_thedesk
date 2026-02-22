# Contributing to FinCrime: The Desk

Welcome! This guide covers everything needed to make a code contribution.

---

## Setup

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust | stable (≥ 1.75) | `rustup install stable` |
| Godot | 4.x .NET edition | [godotengine.org/download](https://godotengine.org/download) |
| .NET SDK | 6.0+ | [dotnet.microsoft.com](https://dotnet.microsoft.com/download) |

### Clone and verify

```bash
git clone <repo-url>
cd fincrime_thedesk

# Build workspace (both fincrime-core and sim-runner)
cargo build

# Run all tests — must pass before any PR
cargo test -p fincrime-core

# Confirm no platform RNG violations
grep -r "thread_rng\|SystemTime\|Instant::now" core/src/ && echo "FAIL" || echo "OK"
```

---

## Code Style

```bash
# Format (must pass in CI)
cargo fmt --all

# Lint (must pass with zero warnings in CI for fincrime-core)
cargo clippy -p fincrime-core -- -D warnings

# Check formatting without modifying
cargo fmt --check
```

There is no `.rustfmt.toml` or `clippy.toml` in the repository — default Rust style applies.

---

## Adding a Subsystem

### File naming

All subsystem files follow the pattern `<name>_subsystem.rs` in `core/src/`:

```
core/src/
├── aml_screening_subsystem.rs
├── card_dispute_subsystem.rs
├── churn_subsystem.rs
├── complaint_analytics_subsystem.rs
├── complaint_subsystem.rs
├── customer_subsystem.rs
├── economics_subsystem.rs
├── fraud_detection_subsystem.rs
├── incident_subsystem.rs
├── macro_subsystem.rs
├── offer_subsystem.rs
├── payment_hub_subsystem.rs
├── pricing_subsystem.rs
├── reconciliation_subsystem.rs
├── risk_appetite_subsystem.rs
├── transaction_monitoring_subsystem.rs
└── transaction_subsystem.rs
```

### Steps

1. **Create the file** `core/src/<name>_subsystem.rs`

2. **Implement `SimSubsystem`**:

```rust
use crate::{error::SimResult, event::SimEvent, rng::SubsystemRng, types::Tick};
use std::any::Any;

pub struct MyNewSubsystem { /* ... */ }

impl crate::subsystem::SimSubsystem for MyNewSubsystem {
    fn name(&self) -> &'static str { "my_new" }

    fn update(
        &mut self,
        tick: Tick,
        events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        // Read prior state from SimStore.
        // React to events_in.
        // Emit new SimEvent variants.
        Ok(vec![])
    }

    fn as_any(&self) -> &dyn Any { self }
}
```

1. **Add a `SubsystemSlot`** in `core/src/rng.rs` — **append only, never insert or reorder**:

```rust
pub enum SubsystemSlot {
    // ... existing slots ...
    MyNew = 20,  // always append
}
```

Add the name mapping in `SubsystemSlot::name()`.

1. **Add `pub mod`** in `core/src/lib.rs`.

2. **Register in `SimEngine::build()` and `build_test_with_config()`** at the correct execution position in `engine.rs`. Document the phase and ordering rationale in a comment following the existing style.

3. **Add `SimEvent` variants** for the subsystem's outputs in `event.rs` — append only.

4. **Add `PlayerCommand` variants** (if the player can interact) in `command.rs` — append only.

5. **Write a migration** for any new tables. See [Adding Migrations](#adding-migrations).

6. **Write tests** in a new integration test file `core/tests/<name>.rs`.

---

## Adding Migrations

### Numbering scheme

Migrations are named `NNN_description.sql` where `NNN` is a zero-padded three-digit integer. The current highest is `025_sar_filing.sql`. Add `026_your_feature.sql`.

```
migrations/
├── 025_sar_filing.sql       ← current highest
└── 026_your_feature.sql     ← new migration
```

### Rules

- **Append only** — never modify an existing migration
- **One logical change per file** — keep migrations focused
- **Use CREATE TABLE IF NOT EXISTS** — idempotent is safer
- **Add indexes** in the same file for any columns used in WHERE clauses
- All migrations run in a transaction; a failure rolls back the entire migration

### Example

```sql
-- 026_my_feature.sql
CREATE TABLE IF NOT EXISTS my_table (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      TEXT NOT NULL,
    tick        INTEGER NOT NULL,
    my_column   TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_my_table_run_tick
    ON my_table(run_id, tick);
```

Then add the corresponding `SimStore` methods in `core/src/store/`.

---

## Adding Configuration

Config files live in `data/<domain>/<name>_config.json`. They are loaded by `SimConfig::load()` in `core/src/config.rs` at engine startup.

1. Create `data/<domain>/<name>_config.json`
2. Add a corresponding struct to `config.rs` with `#[derive(Deserialize, Clone)]`
3. Add the field to `SimConfig`
4. Load it in `SimConfig::load()`
5. Pass the config to your subsystem constructor in `SimEngine::build()`

**Example** (from `data/economics/segment_economics_config.json`):

```json
{
  "cost_allocation_model": {
    "acquisition_cost_per_customer": {
      "mass_market": 85.0,
      "mid_tier": 150.0,
      "student": 45.0,
      "small_business": 320.0
    }
  }
}
```

---

## Running Tests

```bash
# All tests
cargo test

# Core library only
cargo test -p fincrime-core

# With debug output
cargo test -p fincrime-core -- --nocapture

# Specific integration test file
cargo test -p fincrime-core --test incident

# Filter by test name
cargo test -p fincrime-core customer_onboarding
```

Tests that create temp databases leave `test_<uuid>.db` files in the repo root. Clean up with:

```bash
del test_*.db    # Windows
rm test_*.db     # Linux/macOS
```

---

## PR Checklist

Before opening a pull request, verify:

- [ ] `cargo build` passes (no compilation errors)
- [ ] `cargo test -p fincrime-core` passes (all tests green)
- [ ] `cargo fmt --check` passes (no formatting issues)
- [ ] `cargo clippy -p fincrime-core -- -D warnings` passes
- [ ] No `thread_rng`, `SystemTime`, or `Instant::now` in `core/src/`
- [ ] New subsystem has a corresponding test file in `core/tests/`
- [ ] New migrations are append-only (no existing files modified)
- [ ] New `SubsystemSlot` values are appended, not inserted
- [ ] New `SimEvent` variants are appended, never removed
- [ ] PR description explains the execution-order rationale for any new subsystem
- [ ] Test `.db` files are not committed (they are in `.gitignore`)
