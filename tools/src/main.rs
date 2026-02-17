//! sim-runner: headless simulation runner for FinCrime: The Desk.
//!
//! Usage:
//!   sim-runner --seed 12345 --ticks 365 --db run.db
//!   sim-runner --seed 12345 --ticks 365 --db :memory:
//!
//! Prints a KPI summary at the end of the run.

use anyhow::Result;
use fincrime_core::{engine::SimEngine, store::SimStore};
use std::env;

fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let seed  = parse_arg(&args, "--seed",  42u64);
    let ticks = parse_arg(&args, "--ticks", 365u64);
    let db    = args.windows(2)
        .find(|w| w[0] == "--db")
        .map(|w| w[1].as_str())
        .unwrap_or(":memory:");

    println!("FinCrime: The Desk — sim-runner");
    println!("  seed:  {seed}");
    println!("  ticks: {ticks}");
    println!("  db:    {db}");
    println!();

    let store = if db == ":memory:" {
        SimStore::in_memory()?
    } else {
        SimStore::open(db)?
    };
    store.migrate()?;

    let run_id = format!("run-{seed}-{}", chrono_tick());
    store.insert_run(&run_id, seed, env!("CARGO_PKG_VERSION"))?;

    let mut engine = SimEngine::build(run_id.clone(), seed, store);
    engine.run_ticks(ticks)?;

    println!("Run complete.");
    println!("  run_id:       {run_id}");
    println!("  ticks run:    {ticks}");
    println!("  final tick:   {}", engine.clock.current_tick);
    println!("  economic phase after run:");

    // Print macro state from final MacroStateUpdated event
    // (last quarterly update visible in the run).
    let last_macro = engine
        .last_macro_state()
        .map(|s| format!(
            "    phase={:?}  rate={:.3}  fraud_mult={:.2}",
            s.economic_phase, s.base_rate, s.fraud_multiplier
        ))
        .unwrap_or_else(|| "    (no quarterly update yet)".to_string());
    println!("{last_macro}");

    Ok(())
}

fn parse_arg<T: std::str::FromStr + Copy>(args: &[String], flag: &str, default: T) -> T {
    args.windows(2)
        .find(|w| w[0] == flag)
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(default)
}

fn chrono_tick() -> u64 {
    // Simple non-deterministic counter for run_id uniqueness only.
    // Never used inside the simulation itself.
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
