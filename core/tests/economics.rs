//! Economics subsystem tests — Phase 1D.

use fincrime_core::engine::SimEngine;

/// Verify that a P&L snapshot is created exactly at tick 90 (first quarterly boundary).
#[test]
fn pnl_computed_on_quarterly_boundaries() {
    let mut engine = SimEngine::build_test("pnl-boundary-test".into(), 42).unwrap();

    engine.run_ticks(91).unwrap();

    let pnl_count = engine.store_pnl_count("pnl-boundary-test").unwrap();

    assert_eq!(pnl_count, 1, "Expected 1 P&L snapshot at tick 90");
}

/// NIM should be in the realistic range [0%, 10%].
#[test]
fn nim_within_realistic_range() {
    let mut engine = SimEngine::build_test("nim-test".into(), 123).unwrap();

    engine.run_ticks(90).unwrap();

    let pnl = engine
        .store_latest_pnl("nim-test")
        .unwrap()
        .expect("Should have a P&L snapshot");

    assert!(
        pnl.nim >= 0.0 && pnl.nim <= 10.0,
        "NIM {:.2}% outside realistic range [0%, 10%]",
        pnl.nim
    );
}

/// Efficiency ratio should match the formula: opex / gross_income × 100%.
#[test]
fn efficiency_ratio_computed_correctly() {
    let mut engine = SimEngine::build_test("efficiency-test".into(), 99).unwrap();

    engine.run_ticks(90).unwrap();

    let pnl = engine
        .store_latest_pnl("efficiency-test")
        .unwrap()
        .expect("Should have P&L");

    let expected_eff = if pnl.gross_income > 0.0 {
        (pnl.opex / pnl.gross_income) * 100.0
    } else {
        0.0
    };

    let diff = (pnl.efficiency_ratio - expected_eff).abs();
    assert!(
        diff < 0.01,
        "Efficiency ratio mismatch: computed={:.2}%, expected={:.2}%",
        pnl.efficiency_ratio,
        expected_eff
    );
}

/// Fee income in the P&L should be positive (overdraft fees accumulate over 90 ticks).
#[test]
fn fee_income_accumulates_from_transactions() {
    let mut engine = SimEngine::build_test("fee-accumulation-test".into(), 7).unwrap();

    engine.run_ticks(90).unwrap();

    let pnl = engine
        .store_latest_pnl("fee-accumulation-test")
        .unwrap()
        .expect("Should have P&L");

    assert!(
        pnl.fee_income > 0.0,
        "Expected positive fee income from overdraft fees"
    );
}

/// Running 360 ticks should produce exactly 4 quarterly snapshots with correct period labels.
#[test]
fn multiple_quarters_produce_multiple_snapshots() {
    let mut engine = SimEngine::build_test("multi-quarter-test".into(), 456).unwrap();

    engine.run_ticks(360).unwrap();

    let snapshots = engine
        .store_all_pnl_snapshots("multi-quarter-test")
        .unwrap();

    assert_eq!(snapshots.len(), 4, "Expected 4 quarterly snapshots");

    assert_eq!(snapshots[0].period, "Q1-Y1");
    assert_eq!(snapshots[1].period, "Q2-Y1");
    assert_eq!(snapshots[2].period, "Q3-Y1");
    assert_eq!(snapshots[3].period, "Q4-Y1");
}
