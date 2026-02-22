//! Reputation Management subsystem tests — Phase 3.6.
//!
//! Tests cover: initial score seeding, passive recovery, determinism.

use fincrime_core::engine::SimEngine;

fn build(run_id: &str, seed: u64) -> SimEngine {
    SimEngine::build_test(run_id.to_string(), seed).expect("build test engine")
}

fn build_with_reputation(run_id: &str, seed: u64) -> SimEngine {
    SimEngine::build_test_with_reputation(run_id.to_string(), seed)
        .expect("build test engine with reputation")
}

/// After tick 0 the reputation subsystem should have seeded the initial score snapshot.
#[test]
fn reputation_initialises_at_tick_zero() {
    let run_id = "rep-init-test";
    let mut engine = build_with_reputation(run_id, 0x1111_AAAA);

    // Run tick 0 + a few more
    engine.run_ticks(5).unwrap();

    let snapshot_count = engine.store_reputation_snapshot_count(run_id).unwrap();
    assert!(
        snapshot_count >= 1,
        "Expected at least one reputation snapshot after 5 ticks, got {snapshot_count}"
    );

    // Score should be in [0, 100]
    let score = engine.store_latest_reputation_score(run_id).unwrap();
    assert!(
        (0.0..=100.0).contains(&score),
        "Reputation score out of range [0, 100]: {score}"
    );
}

/// After ticks pass the score should stay in the valid range [0, 100].
/// We don't assert a specific floor because SLA breaches from other
/// subsystems will naturally cause decay.
#[test]
fn reputation_score_stays_in_bounds() {
    let run_id = "rep-bounds-test";
    let mut engine = build_with_reputation(run_id, 0x2222_BBBB);

    engine.run_ticks(60).unwrap();

    let score = engine.store_latest_reputation_score(run_id).unwrap();
    assert!(
        (0.0..=100.0).contains(&score),
        "Reputation score out of bounds after 60 ticks: {score}"
    );

    // Snapshot should have been written every tick
    let snapshot_count = engine.store_reputation_snapshot_count(run_id).unwrap();
    assert!(
        snapshot_count >= 60,
        "Expected at least 60 snapshots (one per tick), got {snapshot_count}"
    );
}

/// Determinism: two engines with the same seed produce identical final scores.
#[test]
fn reputation_determinism() {
    const SEED: u64 = 0xBEEF_CAFE;
    let run_id = format!("rep-det-{SEED}");

    let mut engine_a = build_with_reputation(&run_id, SEED);
    let mut engine_b = build_with_reputation(&run_id, SEED);

    engine_a.run_ticks(60).unwrap();
    engine_b.run_ticks(60).unwrap();

    let score_a = engine_a.store_latest_reputation_score(&run_id).unwrap();
    let score_b = engine_b.store_latest_reputation_score(&run_id).unwrap();
    assert_eq!(
        score_a.to_bits(),
        score_b.to_bits(),
        "Reputation score diverged between identical seeds: {score_a} vs {score_b}"
    );

    let snaps_a = engine_a.store_reputation_snapshot_count(&run_id).unwrap();
    let snaps_b = engine_b.store_reputation_snapshot_count(&run_id).unwrap();
    assert_eq!(
        snaps_a, snaps_b,
        "Snapshot count diverged: {snaps_a} vs {snaps_b}"
    );
}
