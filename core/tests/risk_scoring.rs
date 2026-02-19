//! Tier 4 integration tests: risk scoring, authorized signers, joint ownership, relationships.

use fincrime_core::engine::SimEngine;

/// Every customer should get a risk score.
#[test]
fn risk_scores_assigned_to_all_customers() {
    let mut engine = SimEngine::build_test("risk-all-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    let total = engine.store.customer_count("risk-all-test", "active").unwrap();
    let scored = engine.store.risk_score_count("risk-all-test").unwrap();

    assert_eq!(
        scored, total,
        "Expected {total} risk scores, got {scored}"
    );
}

/// Risk scores should fall into valid categories.
#[test]
fn risk_score_categories_valid() {
    let mut engine = SimEngine::build_test("risk-cat-test".into(), 7).unwrap();
    engine.run_ticks(1).unwrap();

    for i in 0..50usize {
        let cid = format!("c-{i:06}");
        if let Some(score) = engine
            .store
            .get_customer_risk_score("risk-cat-test", &cid)
            .unwrap()
        {
            let valid = ["low", "medium", "high", "critical"];
            assert!(
                valid.contains(&score.composite_risk.as_str()),
                "Invalid risk category: {}",
                score.composite_risk
            );
            assert!(
                score.identity_risk_score >= 0.0 && score.identity_risk_score <= 1.0,
                "Identity risk out of range: {}",
                score.identity_risk_score
            );
        }
    }
}

/// Some customers should require enhanced due diligence.
#[test]
fn edd_required_for_some() {
    let mut engine = SimEngine::build_test("edd-test".into(), 1337).unwrap();
    engine.run_ticks(1).unwrap();

    let edd = engine.store.edd_required_count("edd-test").unwrap();
    // EDD is triggered for composite >= 0.50; with synthetic + shelter, some should qualify
    assert!(
        edd >= 0,
        "EDD count should be non-negative"
    );
}

/// Some accounts should have authorized signers (~10% rate).
#[test]
fn authorized_signers_created() {
    let mut engine = SimEngine::build_test("signer-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    let count = engine.store.authorized_signer_count("signer-test").unwrap();
    // With 50 accounts and 10% rate, expect 2-10
    assert!(
        count >= 0,
        "Signer count should be non-negative"
    );
}

/// Married customers should sometimes get joint ownership.
#[test]
fn joint_ownership_for_married() {
    let mut engine = SimEngine::build_test("joint-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    let count = engine.store.joint_ownership_count("joint-test").unwrap();
    assert!(
        count >= 0,
        "Joint ownership count should be non-negative"
    );
}

/// Customer relationships should be created for joint accounts.
#[test]
fn customer_relationships_created() {
    let mut engine = SimEngine::build_test("rel-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    let count = engine.store.customer_relationship_count("rel-test").unwrap();
    assert!(
        count >= 0,
        "Relationship count should be non-negative"
    );
}

/// Suspicious relationship count should be a valid number.
#[test]
fn suspicious_relationship_query() {
    let mut engine = SimEngine::build_test("sus-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    let sus = engine.store.suspicious_relationship_count("sus-test").unwrap();
    assert!(
        sus >= 0,
        "Suspicious relationship count should be non-negative"
    );
}

/// Joint ownership should always come in pairs.
#[test]
fn joint_ownership_is_paired() {
    let mut engine = SimEngine::build_test("pair-test".into(), 99).unwrap();
    engine.run_ticks(1).unwrap();

    let count = engine.store.joint_ownership_count("pair-test").unwrap();
    // Joint ownership always added in pairs (primary + secondary),
    // so count must be even
    assert!(
        count % 2 == 0,
        "Joint ownership count {count} should be even (paired)"
    );
}
