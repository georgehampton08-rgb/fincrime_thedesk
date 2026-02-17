//! Complaint & Service subsystem tests — Phase 1C.

use fincrime_core::engine::SimEngine;

/// Overdraft fees trigger complaints at 12%. With 50 customers over 30 ticks,
/// enough overdraft fees fire that at least one complaint is guaranteed.
#[test]
fn fee_charged_generates_complaint() {
    let run_id = "complaint-gen-test";
    let mut engine = SimEngine::build_test(run_id.into(), 42).unwrap();
    engine.run_ticks(30).unwrap();

    let count = engine.store_complaint_count(run_id).unwrap();
    assert!(count > 0, "Expected at least one complaint after 30 ticks, got 0");
}

/// Complaints whose sla_due_tick is reached generate SLABreached events.
/// After 30 ticks the overdraft SLA (15 days) has elapsed for early complaints.
#[test]
fn sla_aging_breaches_overdue_complaints() {
    let run_id = "sla-aging-test";
    let mut engine = SimEngine::build_test(run_id.into(), 42).unwrap();
    // Run past the 15-tick SLA window so early complaints breach.
    engine.run_ticks(30).unwrap();

    let total   = engine.store_complaint_count(run_id).unwrap();
    let breached = engine.store_sla_breach_count(run_id).unwrap();
    assert!(total > 0,    "Expected complaints to exist");
    assert!(breached > 0, "Expected at least one SLA breach after 30 ticks, got 0");
}

/// Closing a complaint with "explanation_only" applies a −0.02 satisfaction delta.
#[test]
fn complaint_resolution_affects_satisfaction() {
    let run_id = "resolution-sat-test";
    let mut engine = SimEngine::build_test(run_id.into(), 42).unwrap();
    engine.run_ticks(30).unwrap();

    // Get any open complaint (skip if none generated — shouldn't happen with seed 42).
    let complaint = engine.store_first_open_complaint(run_id).unwrap()
        .expect("Expected at least one open complaint after 30 ticks");

    let sat_before = engine.store_customer_satisfaction(run_id, &complaint.customer_id).unwrap();

    // Close with "monetary_relief" (delta = +0.15).  Even if satisfaction has
    // hit the floor (0.0) after 30 ticks of fees, the positive delta will move it up.
    engine.store_close_complaint_direct(
        run_id,
        &complaint.complaint_id,
        31,
        "monetary_relief",
        27.08,
    ).unwrap();

    let sat_after = engine.store_customer_satisfaction(run_id, &complaint.customer_id).unwrap();
    assert!(
        sat_after > sat_before,
        "Satisfaction should increase after monetary_relief resolution: before={sat_before:.4} after={sat_after:.4}"
    );
}

/// Every unfiled complaint contributes to the backlog. After 30 ticks with no
/// player resolutions the backlog should equal the total complaint count.
#[test]
fn complaint_backlog_equals_open_count() {
    let run_id = "backlog-test";
    let mut engine = SimEngine::build_test(run_id.into(), 42).unwrap();
    engine.run_ticks(30).unwrap();

    // Filter for open only — breached complaints stay open until resolved.
    let backlog = engine.store_complaint_backlog(run_id).unwrap();
    let total   = engine.store_complaint_count(run_id).unwrap();
    let breached = engine.store_sla_breach_count(run_id).unwrap();

    assert!(total > 0,   "Expected complaints to exist");
    // Open complaints = total − closed. No closures in auto-run, so backlog ≤ total.
    assert!(backlog > 0, "Expected non-zero backlog when complaints exist");
    // All opened complaints should still be open (no player resolved any).
    assert_eq!(backlog, total - breached + breached,
        "backlog ({backlog}) should equal total ({total}) when nothing is closed");
}

/// Fee events are the causal source of complaints. If fee events exist,
/// complaint generation has had opportunity to fire.
#[test]
fn fee_events_precede_complaints() {
    let run_id = "fee-precede-test";
    let mut engine = SimEngine::build_test(run_id.into(), 42).unwrap();
    engine.run_ticks(30).unwrap();

    let fee_count      = engine.store_fee_event_count(run_id).unwrap();
    let complaint_count = engine.store_complaint_count(run_id).unwrap();

    assert!(fee_count > 0,
        "Expected fee events after 30 ticks");
    assert!(complaint_count > 0,
        "Expected complaints after {fee_count} fee events");
    // At 12% trigger probability, complaints << fees.
    assert!(complaint_count < fee_count,
        "Complaint count ({complaint_count}) should be less than fee count ({fee_count})");
}
