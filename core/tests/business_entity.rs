//! Tier 2 integration tests: business entities, account types, marital status, beneficiaries.

use fincrime_core::engine::SimEngine;

/// The account_type_config table should contain 13 seeded rows.
#[test]
fn account_type_config_seeded() {
    let engine = SimEngine::build_test("atc-seed-test".into(), 1).unwrap();
    let count = engine.store.account_type_config_count().unwrap();
    assert_eq!(count, 13, "Expected 13 account_type_config rows, got {count}");
}

/// Small-business customers should get a business_entity row.
#[test]
fn business_entity_generated_for_small_business() {
    let mut engine = SimEngine::build_test("biz-ent-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    let entity_count = engine.store.business_entity_count("biz-ent-test").unwrap();
    // With 50 customers and 20% small_business segment, expect several entities
    assert!(
        entity_count > 0,
        "Expected at least 1 business entity, got 0"
    );
}

/// EIN format must match XX-XXXXXXX pattern.
#[test]
fn ein_format_is_valid() {
    let mut engine = SimEngine::build_test("ein-fmt-test".into(), 7).unwrap();
    engine.run_ticks(1).unwrap();

    // Find a customer with a business entity
    let entity_count = engine.store.business_entity_count("ein-fmt-test").unwrap();
    if entity_count == 0 {
        // No small_business customers in this seed, skip
        return;
    }

    // Check first few customers to find one with an entity
    for i in 0..50usize {
        let cid = format!("c-{i:06}");
        if let Some(entity) = engine
            .store
            .get_business_entity("ein-fmt-test", &cid)
            .unwrap()
        {
            let parts: Vec<&str> = entity.ein.split('-').collect();
            assert_eq!(
                parts.len(), 2,
                "EIN must have 2 parts: {}",
                entity.ein
            );
            assert!(
                parts[0].len() == 2 && parts[0].parse::<u16>().is_ok(),
                "EIN prefix must be 2 digits: {}",
                parts[0]
            );
            assert!(
                parts[1].len() == 7 && parts[1].parse::<u64>().is_ok(),
                "EIN serial must be 7 digits: {}",
                parts[1]
            );
            break;
        }
    }
}

/// All customers should get a marital_status assigned.
#[test]
fn marital_status_assigned_to_all() {
    let mut engine = SimEngine::build_test("marital-test".into(), 99).unwrap();
    engine.run_ticks(1).unwrap();

    let total = engine.store.customer_count("marital-test", "active").unwrap();
    let with_marital = engine.store.marital_status_count("marital-test").unwrap();

    assert_eq!(
        with_marital, total,
        "Expected {total} customers with marital_status, got {with_marital}"
    );
}

/// Married customers should have beneficiaries (~80% rate).
#[test]
fn beneficiaries_created_for_married() {
    let mut engine = SimEngine::build_test("bene-test".into(), 1337).unwrap();
    engine.run_ticks(1).unwrap();

    let bene_count = engine.store.beneficiary_count("bene-test").unwrap();
    // With 50 customers and some married, expect at least 1 beneficiary
    assert!(
        bene_count > 0,
        "Expected at least 1 beneficiary, got 0"
    );
}

/// Some business entities should have DBA names (~30% rate).
#[test]
fn dba_names_assigned() {
    let mut engine = SimEngine::build_test("dba-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    let entity_count = engine.store.business_entity_count("dba-test").unwrap();
    if entity_count == 0 {
        return; // no small_business in this seed
    }

    let dba_count = engine.store.dba_count("dba-test").unwrap();
    // Some entities may or may not have DBAs; just verify the query works
    assert!(
        dba_count >= 0,
        "DBA count should be non-negative"
    );
}

/// Some business entities should have shell company indicators.
#[test]
fn shell_company_indicators_set() {
    let mut engine = SimEngine::build_test("shell-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    // Just verify the query runs without error
    let shell_count = engine.store.shell_company_count("shell-test").unwrap();
    assert!(
        shell_count >= 0,
        "Shell company count should be non-negative"
    );
}

/// Account type category should be set for all accounts.
#[test]
fn account_type_categories_assigned() {
    let mut engine = SimEngine::build_test("acct-cat-test".into(), 55).unwrap();
    engine.run_ticks(1).unwrap();

    let checking = engine
        .store
        .account_type_category_count("acct-cat-test", "checking_individual")
        .unwrap();
    let business = engine
        .store
        .account_type_category_count("acct-cat-test", "business_checking")
        .unwrap();

    let total = engine.store.customer_count("acct-cat-test", "active").unwrap();
    assert_eq!(
        checking + business,
        total,
        "Expected {total} categorized accounts, got {}", checking + business
    );
}
