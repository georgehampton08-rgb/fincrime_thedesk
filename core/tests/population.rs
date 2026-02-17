//! Population and transaction generation tests.

use fincrime_core::engine::SimEngine;

#[test]
fn initial_population_generates_correct_count() {
    let mut engine = SimEngine::build_test(
        "pop-count-test".into(), 42
    ).unwrap();
    engine.run_ticks(1).unwrap();

    let count = engine.store
        .customer_count("pop-count-test", "active")
        .unwrap();
    assert_eq!(count, 50,
        "Expected 50 test customers, got {count}");
}

#[test]
fn transactions_generated_every_tick_after_tick_0() {
    let mut engine = SimEngine::build_test(
        "txn-gen-test".into(), 99
    ).unwrap();
    engine.run_ticks(5).unwrap();

    let tick1_count = engine.store
        .txn_count_for_tick("txn-gen-test", 1)
        .unwrap();
    assert!(tick1_count > 0,
        "Expected transactions at tick 1, got 0");

    let tick0_count = engine.store
        .txn_count_for_tick("txn-gen-test", 0)
        .unwrap();
    assert_eq!(tick0_count, 0,
        "Tick 0 should have no transactions (only onboarding)");
}

#[test]
fn payroll_fires_on_biweekly_boundary() {
    let mut engine = SimEngine::build_test(
        "payroll-test".into(), 7
    ).unwrap();
    engine.run_ticks(14).unwrap();

    // At tick 14, payroll credits should appear
    let payroll_count = engine.store
        .txn_count_by_category("payroll-test", 14, "payroll")
        .unwrap();
    assert!(payroll_count > 0,
        "Expected payroll credits at tick 14, got 0");
}

#[test]
fn transaction_amounts_follow_pareto_shape() {
    // Run 90 ticks to get a meaningful distribution
    let mut engine = SimEngine::build_test(
        "pareto-test".into(), 123
    ).unwrap();
    engine.run_ticks(90).unwrap();

    let amounts = engine.store
        .all_txn_amounts("pareto-test")
        .unwrap();

    assert!(amounts.len() > 100,
        "Need >100 transactions to verify distribution");

    // Pareto check: median should be much lower than mean
    let mut sorted = amounts.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    let mean: f64 = amounts.iter().sum::<f64>() / amounts.len() as f64;

    assert!(
        mean > median * 1.5,
        "Mean ({mean:.2}) should be >1.5× median ({median:.2}) for Pareto distribution"
    );
}

#[test]
fn determinism_holds_with_customers_and_transactions() {
    // The original determinism test — re-run with the full
    // customer + transaction subsystems now registered.
    const SEED: u64 = 0xFEED_BEEF_1234_ABCD;

    let mut engine_a = SimEngine::build_test(
        format!("det-full-a-{SEED}"), SEED
    ).unwrap();
    let mut engine_b = SimEngine::build_test(
        format!("det-full-b-{SEED}"), SEED
    ).unwrap();

    engine_a.run_ticks(30).unwrap();
    engine_b.run_ticks(30).unwrap();

    let count_a = engine_a.store
        .txn_count_total(&format!("det-full-a-{SEED}"))
        .unwrap();
    let count_b = engine_b.store
        .txn_count_total(&format!("det-full-b-{SEED}"))
        .unwrap();

    assert_eq!(count_a, count_b,
        "Transaction counts differ: {count_a} vs {count_b}");
}
