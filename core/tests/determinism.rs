//! THE MOST IMPORTANT TEST IN THE PROJECT.
//!
//! Two engines, same seed, same operations.
//! They must produce byte-identical event logs.
//! Any divergence is a blocker — do not merge until fixed.

use fincrime_core::engine::SimEngine;

fn build_engine(seed: u64) -> SimEngine {
    let run_id = format!("det-test-{seed}");
    SimEngine::build_test(run_id, seed).expect("build test engine")
}

fn collect_event_log(engine: &SimEngine, run_id: &str) -> Vec<String> {
    // Collect all event payloads in tick+id order.
    // We read directly from the store via a helper.
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

#[test]
fn macro_updates_on_quarterly_boundaries_only() {
    let run_id = "macro-boundary-test".to_string();
    let mut engine = SimEngine::build_test(run_id.clone(), 1).expect("build engine");
    engine.run_ticks(91).unwrap(); // just past first quarter

    // Collect all events
    let macro_events: Vec<_> = (0u64..=91)
        .flat_map(|tick| {
            engine.store_events_for_tick(&run_id, tick).unwrap()
        })
        .filter(|e| e.event_type == "macro_state_updated")
        .collect();

    // Should have exactly 1 macro event (at tick 90)
    assert_eq!(
        macro_events.len(), 1,
        "Expected 1 macro update in 91 ticks, got {}",
        macro_events.len()
    );
    assert_eq!(macro_events[0].tick, 90,
        "Macro update should fire at tick 90");
}

#[test]
fn engine_pauses_and_resumes_correctly() {
    let mut engine = SimEngine::build_test("pause-test".to_string(), 7).expect("build engine");

    // Should start paused
    assert!(engine.clock.paused);

    engine.run_ticks(10).unwrap();
    assert_eq!(engine.clock.current_tick, 10);

    // Should be paused again after run_ticks completes
    assert!(engine.clock.paused);
}
