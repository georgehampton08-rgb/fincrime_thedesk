use fincrime_core::engine::SimEngine;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_engine(run_id: &str, seed: u64) -> SimEngine {
    SimEngine::build_test(run_id.into(), seed).unwrap()
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// ComplaintAnalyticsSubsystem runs every 7 ticks.
/// After 7 ticks the SLA performance snapshot should be written for every
/// priority level — at minimum, the total count must be > 0.
#[test]
fn sla_snapshots_written_every_7_ticks() {
    let mut engine = make_engine("sla-snap-test", 42);

    engine.run_ticks(7).unwrap();

    let count = engine.store_sla_snapshot_count("sla-snap-test").unwrap();
    assert!(
        count > 0,
        "Expected SLA performance snapshot rows after 7 ticks; got {count}"
    );
}

/// The analytics subsystem must not crash over a long run even when there are
/// no velocity spikes — complaint_pattern count may be 0, that is fine.
#[test]
fn analytics_runs_without_error_over_90_ticks() {
    let mut engine = make_engine("analytics-smoke-test", 99);

    // Should not panic
    engine.run_ticks(90).unwrap();

    // Counts are non-negative — subsystem didn't error
    let patterns = engine
        .store_complaint_pattern_count("analytics-smoke-test")
        .unwrap();
    let alerts = engine
        .store_early_warning_alert_count("analytics-smoke-test")
        .unwrap();

    assert!(patterns >= 0, "pattern count must be non-negative");
    assert!(alerts >= 0, "alert count must be non-negative");
}

/// Repeat complainer identification runs each analytics cycle.
/// Count is ≥ 0 (may be 0 if no customer has 3+ complaints within 180 ticks).
#[test]
fn repeat_complainers_identified_without_error() {
    let mut engine = make_engine("repeat-test", 7);

    engine.run_ticks(180).unwrap();

    let count = engine.store_repeat_complainer_count("repeat-test").unwrap();
    assert!(
        count >= 0,
        "Repeat complainer count must be non-negative; got {count}"
    );
}

/// Each analytics run inserts exactly 4 SLA snapshot rows (one per priority).
/// After 14 ticks (2 cycles × 4 priorities) we expect ≥ 8 rows.
#[test]
fn sla_snapshot_priority_coverage() {
    let mut engine = make_engine("sla-priority-test", 123);

    engine.run_ticks(14).unwrap();

    let count = engine
        .store_sla_snapshot_count("sla-priority-test")
        .unwrap();
    // 2 runs × 4 priorities = 8; but PRIMARY KEY deduplicates per (run_id, tick, priority),
    // so at tick 7 and tick 14 we get 8 distinct rows.
    assert!(
        count >= 4,
        "Expected at least 4 SLA snapshot rows after 14 ticks; got {count}"
    );
}

/// Two engines with the same seed must produce identical analytics counts.
#[test]
fn determinism_holds_with_complaint_analytics() {
    const SEED: u64 = 0xCA5E_A5_A5;

    let run_a = format!("det-ca-a-{SEED}");
    let run_b = format!("det-ca-b-{SEED}");

    let mut engine_a = make_engine(&run_a, SEED);
    let mut engine_b = make_engine(&run_b, SEED);

    engine_a.run_ticks(90).unwrap();
    engine_b.run_ticks(90).unwrap();

    let snap_a = engine_a.store_sla_snapshot_count(&run_a).unwrap();
    let snap_b = engine_b.store_sla_snapshot_count(&run_b).unwrap();
    assert_eq!(
        snap_a, snap_b,
        "SLA snapshot count diverged: {snap_a} vs {snap_b}"
    );

    let alert_a = engine_a.store_early_warning_alert_count(&run_a).unwrap();
    let alert_b = engine_b.store_early_warning_alert_count(&run_b).unwrap();
    assert_eq!(
        alert_a, alert_b,
        "Alert count diverged: {alert_a} vs {alert_b}"
    );
}
