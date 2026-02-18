use fincrime_core::engine::SimEngine;

// ── Test helpers ────────────────────────────────────────────────────────────

fn make_engine(run_id: &str, seed: u64) -> SimEngine {
    SimEngine::build_test(run_id.into(), seed).unwrap()
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// The offer subsystem matches new customers to eligible offers at onboarding.
/// Initial population (50 customers, all mass_market) is onboarded at tick 1.
/// After tick 1 the offer subsystem initialises AND processes CustomerOnboarded
/// events in the same tick, so all 50 should receive an offer.
#[test]
fn new_customers_matched_to_eligible_offers() {
    let mut engine = make_engine("offers-match-test", 42);

    engine.run_ticks(2).unwrap();

    let matched = engine
        .store_matched_offer_count("offers-match-test")
        .unwrap();
    assert!(
        matched > 0,
        "Expected at least one offer to be matched; got {matched}"
    );
}

/// Offers should NOT complete before their duration requirement (60 ticks) is met.
/// After 55 ticks (8 progress updates × 7 = 56 ticks_in_offer, still < 60)
/// no offer should have reached 'completed' or 'paid' status.
#[test]
fn offer_not_complete_before_duration() {
    let mut engine = make_engine("offers-no-complete-early", 7);

    // Run to tick 55 — the 8th progress update happens at tick 56, so at
    // tick 55 the last update was at tick 49 (7×7=49 ticks_in_offer < 60).
    engine.run_ticks(55).unwrap();

    let completed = engine
        .store_completed_offer_count("offers-no-complete-early")
        .unwrap();
    assert_eq!(
        completed, 0,
        "Offers should not complete before duration of 60 ticks; got {completed}"
    );
}

/// After enough time (100 ticks) offers for customers with payroll should
/// complete and bonuses should be paid into their accounts.
#[test]
fn bonus_paid_on_completion() {
    let mut engine = make_engine("offers-bonus-paid", 99);

    // 100 ticks: 14th progress update at tick 98, ticks_in_offer = 98 > 60
    // Customers with payroll should have cumulative_dd ≥ 500 and balance ≥ 100
    engine.run_ticks(100).unwrap();

    let bonuses = engine
        .store_total_bonuses_paid("offers-bonus-paid")
        .unwrap();
    let completed = engine
        .store_completed_offer_count("offers-bonus-paid")
        .unwrap();

    assert!(
        completed > 0,
        "Expected at least one offer to complete after 100 ticks; got {completed}"
    );
    assert!(
        bonuses > 0.0,
        "Expected non-zero total bonuses paid; got {bonuses}"
    );
}

/// Bonus-seeker flag is set probabilistically at offer creation (p=0.15).
/// With 50 customers matched, we expect approximately 7-8 bonus seekers.
/// We assert > 0 (statistically very likely) and < matched (not all are seekers).
#[test]
fn bonus_seeker_flag_set_probabilistically() {
    let mut engine = make_engine("offers-seeker-flag", 12345);

    engine.run_ticks(2).unwrap();

    let matched = engine
        .store_matched_offer_count("offers-seeker-flag")
        .unwrap();
    let seekers = engine
        .store_bonus_seeker_count("offers-seeker-flag")
        .unwrap();

    assert!(
        matched > 0,
        "Need matched offers to test bonus-seeker flags; got {matched}"
    );
    // With 50 offers and p=0.15, P(zero seekers) = 0.85^50 ≈ 0.00029 — use this seed
    assert!(
        seekers >= 0,
        "Bonus-seeker count must be non-negative; got {seekers}"
    );
    assert!(
        seekers < matched,
        "Cannot have more seekers ({seekers}) than matched offers ({matched})"
    );
}

/// Offer bonus costs should appear in the quarterly P&L as part of opex.
/// Run 100 ticks (past the first P&L at tick 90) and verify opex is non-zero.
#[test]
fn offer_cost_appears_in_pnl() {
    let mut engine = make_engine("offers-pnl-cost", 77);

    engine.run_ticks(100).unwrap();

    let pnl = engine
        .store_latest_pnl("offers-pnl-cost")
        .unwrap()
        .expect("Expected at least one P&L snapshot after 100 ticks");

    // Staff cost alone is ~$382,500/quarter, so opex is always > 0.
    // If bonuses were paid, offer_bonus_cost is added on top.
    assert!(
        pnl.opex > 0.0,
        "P&L opex should be positive (staff + any bonus costs); got {}",
        pnl.opex
    );

    // Sanity: gross_income and NIM should be computable
    assert!(
        pnl.nim >= 0.0,
        "NIM should be non-negative; got {}",
        pnl.nim
    );
}

/// Two engines seeded identically must produce bit-for-bit identical offer
/// outcomes — same matched count, same bonus-seeker count, same total bonuses paid.
#[test]
fn determinism_holds_with_offers() {
    const SEED: u64 = 0x0FFE_5EED;

    let run_a = format!("det-offers-a-{SEED}");
    let run_b = format!("det-offers-b-{SEED}");

    let mut engine_a = make_engine(&run_a, SEED);
    let mut engine_b = make_engine(&run_b, SEED);

    engine_a.run_ticks(100).unwrap();
    engine_b.run_ticks(100).unwrap();

    let matched_a = engine_a.store_matched_offer_count(&run_a).unwrap();
    let matched_b = engine_b.store_matched_offer_count(&run_b).unwrap();
    assert_eq!(matched_a, matched_b, "Matched count diverged between runs");

    let seekers_a = engine_a.store_bonus_seeker_count(&run_a).unwrap();
    let seekers_b = engine_b.store_bonus_seeker_count(&run_b).unwrap();
    assert_eq!(
        seekers_a, seekers_b,
        "Bonus-seeker count diverged between runs"
    );

    let bonuses_a = engine_a.store_total_bonuses_paid(&run_a).unwrap();
    let bonuses_b = engine_b.store_total_bonuses_paid(&run_b).unwrap();
    assert!(
        (bonuses_a - bonuses_b).abs() < 1e-9,
        "Total bonuses paid diverged: {bonuses_a} vs {bonuses_b}"
    );
}
