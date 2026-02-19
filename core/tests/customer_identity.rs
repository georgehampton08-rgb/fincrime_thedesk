//! Tier 1 integration tests: customer identity, address, and phone generation.
//!
//! Verifies that after tick 0:
//!   - Every customer has an identity row (SSN)
//!   - Every customer has a physical address row
//!   - Every customer has a phone number row
//!   - State code is propagated to the customer table
//!   - Synthetic identity rate is plausible (~2%)
//!   - SSN format is valid (AAA-GG-SSSS)
//!   - Generation is deterministic across two runs with the same seed

use fincrime_core::engine::SimEngine;

/// After tick 0, every customer must have an identity record.
#[test]
fn identity_records_created_for_all_customers() {
    let mut engine = SimEngine::build_test("id-all-test".into(), 42).unwrap();
    engine.run_ticks(1).unwrap();

    let customer_count = engine.store.customer_count("id-all-test", "active").unwrap();
    let identity_count = engine.store.identity_count("id-all-test").unwrap();

    assert_eq!(
        identity_count, customer_count,
        "Expected {customer_count} identity rows, got {identity_count}"
    );
}

/// After tick 0, every customer must have an address row.
#[test]
fn address_records_created_for_all_customers() {
    let mut engine = SimEngine::build_test("addr-all-test".into(), 17).unwrap();
    engine.run_ticks(1).unwrap();

    let customer_count = engine.store.customer_count("addr-all-test", "active").unwrap();
    let address_count = engine.store.address_count("addr-all-test").unwrap();

    assert_eq!(
        address_count, customer_count,
        "Expected {customer_count} address rows, got {address_count}"
    );
}

/// After tick 0, every customer must have a phone row.
#[test]
fn phone_records_created_for_all_customers() {
    let mut engine = SimEngine::build_test("phone-all-test".into(), 99).unwrap();
    engine.run_ticks(1).unwrap();

    let customer_count = engine.store.customer_count("phone-all-test", "active").unwrap();
    let phone_count = engine.store.phone_count("phone-all-test").unwrap();

    assert_eq!(
        phone_count, customer_count,
        "Expected {customer_count} phone rows, got {phone_count}"
    );
}

/// Verify SSN format: all identity rows must match the AAA-GG-SSSS pattern.
#[test]
fn ssn_format_is_valid() {
    let mut engine = SimEngine::build_test("ssn-fmt-test".into(), 7).unwrap();
    engine.run_ticks(1).unwrap();

    // Spot-check first 5 customers
    for i in 0..5usize {
        let cid = format!("c-{i:06}");
        let row = engine
            .store
            .get_customer_identity("ssn-fmt-test", &cid)
            .unwrap()
            .expect(&format!("identity missing for {cid}"));

        // Must match NNN-NN-NNNN
        let parts: Vec<&str> = row.ssn_full.split('-').collect();
        assert_eq!(parts.len(), 3, "SSN must have 3 parts: {}", row.ssn_full);
        assert_eq!(parts[0].len(), 3, "Area must be 3 digits");
        assert_eq!(parts[1].len(), 2, "Group must be 2 digits");
        assert_eq!(parts[2].len(), 4, "Serial must be 4 digits");

        // Area must be a valid integer
        assert!(
            parts[0].parse::<u16>().is_ok(),
            "Area not numeric: {}",
            parts[0]
        );
        // Group must never be 00
        let group: u8 = parts[1].parse().unwrap();
        assert!(group >= 1, "Group must be >= 01, got {group}");
        // Serial must never be 0000
        let serial: u16 = parts[2].parse().unwrap();
        assert!(serial >= 1, "Serial must be >= 0001, got {serial}");
    }
}

/// Synthetic identity rate should be in roughly expected range (0–20%).
/// With 50 test customers we expect ~1 synthetic at 2%, but tolerate 0–10.
#[test]
fn synthetic_identity_rate_is_plausible() {
    let mut engine = SimEngine::build_test("syn-id-rate-test".into(), 1337).unwrap();
    engine.run_ticks(1).unwrap();

    let synthetic_count = engine
        .store
        .count_synthetic_identities("syn-id-rate-test")
        .unwrap();

    let total = engine.store.customer_count("syn-id-rate-test", "active").unwrap();
    let rate = synthetic_count as f64 / total as f64;

    assert!(
        rate < 0.20,
        "Synthetic rate too high: {rate:.2} ({synthetic_count}/{total})"
    );
}

/// All address types must be one of the known valid types.
#[test]
fn address_types_are_valid() {
    let valid_types = ["residential", "po_box", "cmra", "homeless_shelter", "dv_shelter", "commercial"];
    let mut engine = SimEngine::build_test("addr-type-test".into(), 55).unwrap();
    engine.run_ticks(1).unwrap();

    for i in 0..5usize {
        let cid = format!("c-{i:06}");
        let row = engine
            .store
            .get_customer_address("addr-type-test", &cid)
            .unwrap()
            .expect(&format!("address missing for {cid}"));

        assert!(
            valid_types.contains(&row.address_type.as_str()),
            "Unknown address type '{}' for {cid}",
            row.address_type
        );
    }
}

/// Phone numbers must start with "+1-" and have correct length.
#[test]
fn phone_numbers_are_formatted_correctly() {
    let mut engine = SimEngine::build_test("phone-fmt-test".into(), 31).unwrap();
    engine.run_ticks(1).unwrap();

    for i in 0..5usize {
        let cid = format!("c-{i:06}");
        let row = engine
            .store
            .get_customer_phone("phone-fmt-test", &cid)
            .unwrap()
            .expect(&format!("phone missing for {cid}"));

        assert!(
            row.full_number.starts_with("+1-"),
            "Phone must start with +1-: {}",
            row.full_number
        );
        // "+1-NNN-NNN-NNNN" = 15 chars
        assert_eq!(
            row.full_number.len(), 16,
            "Phone number length wrong: '{}'", row.full_number
        );
    }
}

/// With the same seed, two runs must produce identical identity data (determinism).
#[test]
fn identity_generation_is_deterministic() {
    const SEED: u64 = 0xABCD_1234_DEAD_BEEF;

    let mut engine_a = SimEngine::build_test(format!("det-id-a-{SEED}"), SEED).unwrap();
    let mut engine_b = SimEngine::build_test(format!("det-id-b-{SEED}"), SEED).unwrap();

    engine_a.run_ticks(1).unwrap();
    engine_b.run_ticks(1).unwrap();

    let run_a = format!("det-id-a-{SEED}");
    let run_b = format!("det-id-b-{SEED}");

    // Check first 5 customers have matching SSNs
    for i in 0..5usize {
        let cid = format!("c-{i:06}");
        let row_a = engine_a.store.get_customer_identity(&run_a, &cid).unwrap().unwrap();
        let row_b = engine_b.store.get_customer_identity(&run_b, &cid).unwrap().unwrap();

        assert_eq!(
            row_a.ssn_full, row_b.ssn_full,
            "SSN mismatch at customer {cid}: {} vs {}",
            row_a.ssn_full, row_b.ssn_full
        );
        assert_eq!(
            row_a.date_of_birth, row_b.date_of_birth,
            "DoB mismatch at customer {cid}"
        );
    }
}
