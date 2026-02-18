//! sim-runner: headless simulation runner for FinCrime: The Desk.
//!
//! Usage:
//!   sim-runner --seed 12345 --ticks 365 --db run.db
//!   sim-runner --seed 12345 --connect-port 9000

use anyhow::Result;
use fincrime_core::{engine::SimEngine, store::SimStore, types::Tick};
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum IpcCommand {
    GetState,
    Tick {
        count: u64,
    },
    Command {
        cmd: String,
        payload: serde_json::Value,
    },
    Quit,
}

#[derive(serde::Serialize)]
struct UiState {
    tick: Tick,
    paused: bool,
    active_customers: i64,
    churned_customers: i64,
    complaint_count: i64,
    sla_breaches: i64,
    backlog: i64,
    nim: f64,
    efficiency_ratio: f64,
    pre_tax_profit: f64,
    pnl_history: Vec<fincrime_core::economics_subsystem::PnLSnapshot>,
    complaints: Vec<fincrime_core::complaint_subsystem::ComplaintRecord>,
}

fn main() -> Result<()> {
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let seed = parse_arg(&args, "--seed", 42u64);
    let ticks = parse_arg(&args, "--ticks", 365u64);
    let ipc_mode = args.iter().any(|a| a == "--ipc-mode");
    let db = args
        .windows(2)
        .find(|w| w[0] == "--db")
        .map(|w| w[1].as_str())
        .unwrap_or(":memory:");
    let data_dir = args
        .windows(2)
        .find(|w| w[0] == "--data-dir")
        .map(|w| w[1].as_str())
        .unwrap_or("./data");

    if !ipc_mode {
        println!("FinCrime: The Desk — sim-runner");
        println!("  seed:      {seed}");
        println!("  ticks:     {ticks}");
        println!("  db:        {db}");
        println!("  data_dir:  {data_dir}");
        println!();
    }

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

    if ipc_mode {
        run_ipc_loop(&mut engine, &run_id)?;
    } else {
        engine.run_ticks(ticks)?;
        print_summary(&engine, &store, &run_id, ticks)?;
    }

    Ok(())
}

fn run_ipc_loop(engine: &mut SimEngine, run_id: &str) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut handle = stdin.lock();
    let mut buffer = String::new();

    loop {
        buffer.clear();
        let bytes_read = handle.read_line(&mut buffer)?;
        if bytes_read == 0 {
            break; // EOF
        }

        let cmd: IpcCommand = match serde_json::from_str(&buffer) {
            Ok(c) => c,
            Err(e) => {
                let err_json = serde_json::json!({ "error": e.to_string() });
                writeln!(stdout, "{}", err_json)?;
                stdout.flush()?;
                continue;
            }
        };

        match cmd {
            IpcCommand::Quit => break,
            IpcCommand::Tick { count } => {
                engine.run_ticks(count)?;
                let state = build_ui_state(engine, run_id)?;
                writeln!(stdout, "{}", serde_json::to_string(&state)?)?;
            }
            IpcCommand::GetState => {
                let state = build_ui_state(engine, run_id)?;
                writeln!(stdout, "{}", serde_json::to_string(&state)?)?;
            }
            IpcCommand::Command { cmd, payload } => {
                handle_command(engine, run_id, &cmd, payload)?;
                let state = build_ui_state(engine, run_id)?;
                writeln!(stdout, "{}", serde_json::to_string(&state)?)?;
            }
        }
        stdout.flush()?;
    }
    Ok(())
}

fn handle_command(
    engine: &mut SimEngine,
    run_id: &str,
    cmd: &str,
    payload: serde_json::Value,
) -> Result<()> {
    match cmd {
        "resolve_complaint" => {
            let complaint_id = payload["complaint_id"].as_str().unwrap_or_default();
            let resolution = payload["resolution"].as_str().unwrap_or("explanation_only");
            let refund = payload["refund"].as_f64().unwrap_or(0.0);

            engine.store_close_complaint_direct(
                run_id,
                complaint_id,
                engine.clock.current_tick,
                resolution,
                refund,
            )?;
        }
        _ => log::warn!("Unknown command: {}", cmd),
    }
    Ok(())
}

fn build_ui_state(engine: &SimEngine, run_id: &str) -> Result<UiState> {
    // Gather all KPIs
    let active_customers = engine.store.customer_count(run_id, "active")?;
    let churned_customers = engine.store_churned_count(run_id)?;
    let complaint_count = engine.store_complaint_count(run_id)?;
    let sla_breaches = engine.store_sla_breach_count(run_id)?;
    let backlog = engine.store_complaint_backlog(run_id)?;

    // Economics
    let pnl_snapshots = engine.store_all_pnl_snapshots(run_id)?;
    let (nim, eff, profit) = if let Some(last) = pnl_snapshots.last() {
        (last.nim, last.efficiency_ratio, last.pre_tax_profit)
    } else {
        (0.0, 0.0, 0.0)
    };

    let complaints = engine.store.open_complaints(run_id)?;

    Ok(UiState {
        tick: engine.clock.current_tick,
        paused: engine.clock.paused,
        active_customers,
        churned_customers,
        complaint_count,
        sla_breaches,
        backlog,
        nim,
        efficiency_ratio: eff,
        pre_tax_profit: profit,
        pnl_history: pnl_snapshots,
        complaints,
    })
}

fn print_summary(engine: &SimEngine, store: &SimStore, run_id: &str, ticks: u64) -> Result<()> {
    let customers = store.customer_count(run_id, "active")?;
    let total_txns = store.txn_count_total(run_id)?;
    let avg_daily = total_txns as f64 / ticks as f64;

    let complaints = engine.store_complaint_count(run_id)?;
    let sla_breaches = engine.store_sla_breach_count(run_id)?;
    let backlog = engine.store_complaint_backlog(run_id)?;
    let churned = engine.store_churned_count(run_id)?;

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
    let pnl_snapshots = engine.store_all_pnl_snapshots(run_id).unwrap_or_default();
    if pnl_snapshots.is_empty() {
        println!("  (No quarters completed yet)");
    } else {
        let recent: Vec<_> = pnl_snapshots.iter().rev().take(4).collect();
        for p in recent.iter().rev() {
            println!(
                "  {} | Profit: ${:.0} | NIM: {:.2}% | Eff: {:.1}%",
                p.period, p.pre_tax_profit, p.nim, p.efficiency_ratio
            );
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
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
