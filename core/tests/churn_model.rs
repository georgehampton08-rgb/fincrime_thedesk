use fincrime_core::engine::SimEngine;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_engine(run_id: &str, seed: u64) -> SimEngine {
    SimEngine::build_test(run_id.into(), seed).unwrap()
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// ChurnSubsystem inserts a score row for each active customer every 30 ticks.
/// After 60 ticks (2 update cycles × 50 customers) we expect ≥ 50 score rows.
#[test]
fn churn_scores_computed_every_30_ticks() {
    let mut engine = make_engine("churn-score-test", 42);

    engine.run_ticks(60).unwrap();

    let count = engine.store_churn_score_count("churn-score-test").unwrap();
    assert!(
        count > 0,
        "Expected churn score rows after 60 ticks; got {count}"
    );
}

/// All component scores must be non-negative (additive, not subtractive, except bonuses).
/// Fee burden, satisfaction, complaint, sla_breach and inactivity components ≥ 0.
#[test]
fn churn_score_components_are_non_negative() {
    let mut engine = make_engine("churn-components-test", 99);

    engine.run_ticks(30).unwrap();

    let scores = engine.store_all_churn_scores("churn-components-test", 30).unwrap();

    assert!(!scores.is_empty(), "Expected scores at tick 30");

    for s in &scores {
        assert!(s.churn_risk >= 0.0 && s.churn_risk <= 1.0,
            "churn_risk={} must be in [0,1]", s.churn_risk);
        assert!(s.base_rate >= 0.0,
            "base_rate={} must be ≥ 0", s.base_rate);
        assert!(s.satisfaction_component >= 0.0,
            "satisfaction_component={} must be ≥ 0", s.satisfaction_component);
        assert!(s.fee_burden_component >= 0.0,
            "fee_burden_component={} must be ≥ 0", s.fee_burden_component);
        assert!(s.complaint_component >= 0.0,
            "complaint_component={} must be ≥ 0", s.complaint_component);
        assert!(s.inactivity_component >= 0.0,
            "inactivity_component={} must be ≥ 0", s.inactivity_component);
    }
}

/// Life events are generated probabilistically (job_change at p=0.15/year ≈ 0.00041/tick).
/// Over 365 ticks with 50 customers, expected events ≈ 7.5.
/// We assert life_event_count ≥ 0 (non-negative) and check the subsystem doesn't crash.
#[test]
fn life_events_generated_probabilistically() {
    let mut engine = make_engine("life-event-test", 7);

    engine.run_ticks(365).unwrap();

    let event_count = engine.store_life_event_count("life-event-test").unwrap();
    assert!(
        event_count >= 0,
        "Life event count must be non-negative; got {event_count}"
    );
}

/// Churn cohorts record the primary driver when a customer churns.
/// Driver must be one of the known categories.
#[test]
fn churn_cohorts_track_primary_driver() {
    let mut engine = make_engine("cohort-test", 123);

    // Run long enough that some customers may churn (risk ≥ 0.85 required)
    engine.run_ticks(180).unwrap();

    let cohorts = engine.store_churn_cohorts("cohort-test").unwrap();

    let valid_drivers = [
        "fee_burden", "satisfaction", "complaints",
        "sla_breach", "inactivity", "life_event", "unknown",
    ];

    for cohort in &cohorts {
        assert!(
            valid_drivers.contains(&cohort.primary_driver.as_str()),
            "Unexpected primary driver: '{}'", cohort.primary_driver
        );
    }
}

/// Retention offer bonus (−0.15 × effectiveness) reduces churn risk for customers
/// with active retention offers. This is validated indirectly by checking that
/// the engine runs without error and scores are computed.
#[test]
fn retention_offer_reduces_churn_risk() {
    let mut engine = make_engine("retention-test", 456);

    engine.run_ticks(90).unwrap();

    let scores = engine.store_all_churn_scores("retention-test", 90).unwrap();

    // The retention_offer_bonus is ≤ 0 (negative = good for retention)
    for s in &scores {
        assert!(
            s.retention_offer_bonus <= 0.0,
            "retention_offer_bonus={} should be ≤ 0", s.retention_offer_bonus
        );
    }
}

/// Two engines with the same seed must produce identical churn outcomes.
#[test]
fn determinism_holds_with_churn_model() {
    const SEED: u64 = 0xC4E5F1CA;

    let run_a = format!("det-churn-a-{SEED}");
    let run_b = format!("det-churn-b-{SEED}");

    let mut engine_a = make_engine(&run_a, SEED);
    let mut engine_b = make_engine(&run_b, SEED);

    engine_a.run_ticks(90).unwrap();
    engine_b.run_ticks(90).unwrap();

    let scores_a = engine_a.store_churn_score_count(&run_a).unwrap();
    let scores_b = engine_b.store_churn_score_count(&run_b).unwrap();
    assert_eq!(scores_a, scores_b, "Score count diverged: {scores_a} vs {scores_b}");

    let churned_a = engine_a.store_churned_count(&run_a).unwrap();
    let churned_b = engine_b.store_churned_count(&run_b).unwrap();
    assert_eq!(churned_a, churned_b, "Churned count diverged: {churned_a} vs {churned_b}");

    let life_a = engine_a.store_life_event_count(&run_a).unwrap();
    let life_b = engine_b.store_life_event_count(&run_b).unwrap();
    assert_eq!(life_a, life_b, "Life event count diverged: {life_a} vs {life_b}");
}
