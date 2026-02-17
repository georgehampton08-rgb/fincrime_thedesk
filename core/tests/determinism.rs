//! THE MOST IMPORTANT TEST IN THE PROJECT.
//!
//! Two engines, same seed, same operations.
//! They must produce byte-identical event logs.
//! Any divergence is a blocker — do not merge until fixed.

use fincrime_core::{
    engine::SimEngine,
    store::SimStore,
};

fn build_engine(seed: u64) -> SimEngine {
    let store = SimStore::in_memory().expect("in-memory store");
    store.migrate().expect("migration");
    let run_id = format!("det-test-{seed}");
    store.insert_run(&run_id, seed, "0.1.0-test").expect("insert run");
    SimEngine::new(run_id, seed, store)
}

fn collect_event_log(engine: &SimEngine, run_id: &str) -> Vec<String> {
    // Collect all event payloads in tick+id order.
    // We read directly from the in-memory store via a helper.
    // This is acceptable in tests — production code uses the engine API.
    (0..=engine.clock.current_tick)
        .flat_map(|tick| {
            engine.store_events_for_tick(run_id, tick)
                .expect("read events")
                .into_iter()
                .map(|e| e.payload)
        })
        .collect()
}

#[test]
fn same_seed_produces_identical_event_logs() {
    const SEED: u64 = 0xDEAD_BEEF_CAFE_1234;
    const TICKS: u64 = 365; // one in-game year

    let mut engine_a = build_engine(SEED);
    let mut engine_b = build_engine(SEED);

    engine_a.run_ticks(TICKS).expect("engine_a run");
    engine_b.run_ticks(TICKS).expect("engine_b run");

    let log_a = collect_event_log(&engine_a, &format!("det-test-{SEED}"));
    let log_b = collect_event_log(&engine_b, &format!("det-test-{SEED}"));

    assert_eq!(
        log_a.len(), log_b.len(),
        "Event log lengths differ: {} vs {}",
        log_a.len(), log_b.len()
    );

    for (i, (a, b)) in log_a.iter().zip(log_b.iter()).enumerate() {
        assert_eq!(
            a, b,
            "Event log diverged at entry {i}:\n  A: {a}\n  B: {b}"
        );
    }
}

#[test]
fn different_seeds_produce_different_logs() {
    let mut engine_a = build_engine(42);
    let mut engine_b = build_engine(99);

    engine_a.run_ticks(90).expect("run a");
    engine_b.run_ticks(90).expect("run b");

    // With different seeds the macro state updates should diverge.
    // This test verifies that seed differences are actually observable.
    let log_a = collect_event_log(&engine_a, "det-test-42");
    let log_b = collect_event_log(&engine_b, "det-test-99");

    let any_different = log_a.iter().zip(log_b.iter()).any(|(a, b)| a != b);
    assert!(any_different, "Different seeds produced identical logs — seed is not being used");
}
