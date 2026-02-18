use fincrime_core::engine::SimEngine;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn make_engine(run_id: &str, seed: u64) -> SimEngine {
    SimEngine::build_test(run_id.into(), seed).unwrap()
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// Segment P&L is computed once per quarter (every 90 ticks).
/// After 90 ticks, we expect at least 1 segment P&L record.
#[test]
fn segment_pnl_computed_quarterly() {
    let mut engine = make_engine("seg-pnl-test", 42);

    engine.run_ticks(90).unwrap();

    let count = engine.store_segment_pnl_count("seg-pnl-test").unwrap();
    assert!(
        count > 0,
        "Expected segment P&L records after 90 ticks; got {count}"
    );
}

/// Revenue components (nii + fee_income + interchange_income) must sum to gross_income.
/// Cost components must sum to total_cost.
#[test]
fn segment_pnl_components_sum_correctly() {
    let mut engine = make_engine("seg-components-test", 99);

    engine.run_ticks(90).unwrap();

    let pnls = engine
        .store_all_segment_pnls("seg-components-test", 90)
        .unwrap();

    assert!(!pnls.is_empty(), "Expected segment P&L rows at tick 90");

    for pnl in &pnls {
        let revenue_sum = pnl.nii + pnl.fee_income + pnl.interchange_income;
        assert!(
            (revenue_sum - pnl.gross_income).abs() < 0.01,
            "Revenue components ({:.2}) don't match gross_income ({:.2})",
            revenue_sum,
            pnl.gross_income
        );

        let cost_sum = pnl.acquisition_cost
            + pnl.servicing_cost
            + pnl.complaint_cost
            + pnl.retention_cost
            + pnl.churn_replacement_cost
            + pnl.allocated_opex;
        assert!(
            (cost_sum - pnl.total_cost).abs() < 0.01,
            "Cost components ({:.2}) don't match total_cost ({:.2})",
            cost_sum,
            pnl.total_cost
        );
    }
}

/// customer_margin = segment_profit / gross_income (when gross_income > 0).
#[test]
fn customer_margin_calculation_correct() {
    let mut engine = make_engine("margin-test", 7);

    engine.run_ticks(90).unwrap();

    let pnls = engine.store_all_segment_pnls("margin-test", 90).unwrap();

    for pnl in &pnls {
        if pnl.gross_income > 0.0 {
            let expected_margin = pnl.segment_profit / pnl.gross_income;
            assert!(
                (expected_margin - pnl.customer_margin).abs() < 0.001,
                "Margin mismatch: expected {:.4} got {:.4}",
                expected_margin,
                pnl.customer_margin
            );
        }
    }
}

/// below_target_margin flag is a boolean — verify it is set correctly.
/// (We can't assert a specific value without knowing the exact P&L,
/// but we verify the flag is consistent with the margin calculation.)
#[test]
fn below_target_margin_flag_fires() {
    let mut engine = make_engine("target-margin-test", 123);

    engine.run_ticks(180).unwrap();

    let pnls = engine
        .store_all_segment_pnls("target-margin-test", 180)
        .unwrap();

    // All flag values must be booleans — Rust enforces this, but we also verify
    // the field is present (i.e., the query succeeded and the data round-trips).
    for pnl in &pnls {
        // below_target_margin must agree with the arithmetic when margin is far below target
        // (target for mass_market = 0.18, warning = -0.10, so threshold = 0.08)
        if pnl.customer_margin < 0.08 {
            assert!(
                pnl.below_target_margin,
                "Segment {} has margin {:.3} < 0.08 but flag is not set",
                pnl.segment, pnl.customer_margin
            );
        }
    }
}

/// Two engines with the same seed must produce identical segment P&L record counts.
#[test]
fn determinism_holds_with_segment_pnl() {
    const SEED: u64 = 0x5E6F_AB12;

    let run_a = format!("det-seg-a-{SEED}");
    let run_b = format!("det-seg-b-{SEED}");

    let mut engine_a = make_engine(&run_a, SEED);
    let mut engine_b = make_engine(&run_b, SEED);

    engine_a.run_ticks(90).unwrap();
    engine_b.run_ticks(90).unwrap();

    let count_a = engine_a.store_segment_pnl_count(&run_a).unwrap();
    let count_b = engine_b.store_segment_pnl_count(&run_b).unwrap();
    assert_eq!(
        count_a, count_b,
        "Segment P&L count diverged: {count_a} vs {count_b}"
    );
}
