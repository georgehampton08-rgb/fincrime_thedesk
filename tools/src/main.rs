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
    let data_dir = args.windows(2)
        .find(|w| w[0] == "--data-dir")
        .map(|w| w[1].as_str())
        .unwrap_or("./data");

    println!("FinCrime: The Desk — sim-runner");
    println!("  seed:      {seed}");
    println!("  ticks:     {ticks}");
    println!("  db:        {db}");
    println!("  data_dir:  {data_dir}");
    println!();

    // For :memory: use SQLite shared-memory URI so multiple connections
    // (engine store + subsystem stores) all share the same in-memory database.
    let db_effective: String = if db == ":memory:" {
        format!("file:simrun_{}?mode=memory&cache=shared", chrono_tick())
    } else {
        db.to_string()
    };
    let store = SimStore::open(&db_effective)?;
    store.migrate()?;

    let run_id = format!("run-{seed}-{}", chrono_tick());
    store.insert_run(&run_id, seed, env!("CARGO_PKG_VERSION"))?;

    let mut engine = SimEngine::build(run_id.clone(), seed, &store, data_dir)?;
    engine.run_ticks(ticks)?;

    // Print summary using store test helper methods
    let customers = store.customer_count(&run_id, "active")?;
    let total_txns = store.txn_count_total(&run_id)?;
    let avg_daily = total_txns as f64 / ticks as f64;

    let complaints  = engine.store_complaint_count(&run_id)?;
    let sla_breaches = engine.store_sla_breach_count(&run_id)?;
    let backlog     = engine.store_complaint_backlog(&run_id)?;
    let churned     = engine.store_churned_count(&run_id)?;

    println!("=== RUN SUMMARY ===");
    println!("  run_id:         {run_id}");
    println!("  ticks run:      {ticks}");
    println!("  final tick:     {}", engine.clock.current_tick);
    println!("  customers:      {customers}");
    println!("  churned:        {churned}");
    println!("  total txns:     {total_txns}");
    println!("  avg daily txns: {avg_daily:.1}");
    println!("  complaints:     {complaints}");
    println!("  sla breaches:   {sla_breaches}");
    println!("  backlog:        {backlog}");

    println!();
    println!("=== FINANCIAL SUMMARY (Last 4 Quarters) ===");
    let pnl_snapshots = engine
        .store_all_pnl_snapshots(&run_id)
        .unwrap_or_default();
    if pnl_snapshots.is_empty() {
        println!("  (No quarters completed yet)");
    } else {
        let recent: Vec<_> = pnl_snapshots.into_iter().rev().take(4).collect();
        for p in recent.into_iter().rev() {
            println!("  {} | Profit: ${:.0} | NIM: {:.2}% | Eff: {:.1}%",
                p.period, p.pre_tax_profit, p.nim, p.efficiency_ratio);
        }
    }

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
