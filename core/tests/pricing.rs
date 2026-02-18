use fincrime_core::{command::PlayerCommand, engine::SimEngine};

#[test]
fn fee_change_within_limits_succeeds() {
    let mut engine = SimEngine::build_test("fee-change-test".into(), 42).unwrap();

    // Run one tick to initialize product state
    engine.run_ticks(1).unwrap();

    // Change overdraft fee from default $27.08 to $30.00 (within limit of $35)
    let cmd = PlayerCommand::SetProductFee {
        product_id: "basic_checking".into(),
        fee_type: "overdraft_fee".into(),
        new_value: 30.0,
    };

    engine.submit_command(cmd).unwrap();
    engine.run_ticks(1).unwrap();

    let state = engine
        .store_product_state("fee-change-test", "basic_checking")
        .unwrap();
    assert_eq!(state.overdraft_fee, 30.0);
}

#[test]
fn fee_change_above_hard_limit_rejected() {
    let mut engine = SimEngine::build_test("hard-limit-test".into(), 99).unwrap();

    engine.run_ticks(1).unwrap();

    // Try to set overdraft fee to $40 (hard limit is $35)
    let cmd = PlayerCommand::SetProductFee {
        product_id: "basic_checking".into(),
        fee_type: "overdraft_fee".into(),
        new_value: 40.0,
    };

    engine.submit_command(cmd).unwrap();
    engine.run_ticks(1).unwrap();

    // Verify fee did NOT change — still at default $27.08
    let state = engine
        .store_product_state("hard-limit-test", "basic_checking")
        .unwrap();
    assert_eq!(state.overdraft_fee, 27.08);
}

#[test]
fn fee_change_above_soft_limit_generates_warning() {
    let mut engine = SimEngine::build_test("soft-limit-test".into(), 7).unwrap();

    engine.run_ticks(1).unwrap();

    // Set overdraft fee to $32 (above soft limit of $29, below hard limit of $35)
    let cmd = PlayerCommand::SetProductFee {
        product_id: "basic_checking".into(),
        fee_type: "overdraft_fee".into(),
        new_value: 32.0,
    };

    engine.submit_command(cmd).unwrap();
    engine.run_ticks(1).unwrap();

    // Fee should change
    let state = engine
        .store_product_state("soft-limit-test", "basic_checking")
        .unwrap();
    assert_eq!(state.overdraft_fee, 32.0);

    // UDAAP score should increase (overdraft > $29 triggers udaap_risk_delta = 0.10)
    let udaap = engine.store_udaap_score("soft-limit-test").unwrap();
    assert!(
        udaap > 0.0,
        "UDAAP score should increase above soft limit; got {udaap}"
    );
}

#[test]
fn fee_change_history_logged() {
    let mut engine = SimEngine::build_test("history-test".into(), 123).unwrap();

    engine.run_ticks(1).unwrap();

    // First fee change
    engine
        .submit_command(PlayerCommand::SetProductFee {
            product_id: "basic_checking".into(),
            fee_type: "overdraft_fee".into(),
            new_value: 25.0,
        })
        .unwrap();
    engine.run_ticks(5).unwrap();

    // Second fee change
    engine
        .submit_command(PlayerCommand::SetProductFee {
            product_id: "basic_checking".into(),
            fee_type: "overdraft_fee".into(),
            new_value: 30.0,
        })
        .unwrap();
    engine.run_ticks(5).unwrap();

    let history = engine
        .store_fee_change_history("history-test", "basic_checking", 10)
        .unwrap();

    assert_eq!(history.len(), 2, "Should have 2 fee changes logged");
    // Most recent first (ORDER BY tick DESC)
    assert_eq!(
        history[0].new_value, 30.0,
        "Most recent change should be $30"
    );
    assert_eq!(history[1].new_value, 25.0, "Older change should be $25");
}

#[test]
fn determinism_holds_with_fee_changes() {
    const SEED: u64 = 0xFEE_C0DE;

    let mut engine_a = SimEngine::build_test(format!("det-pricing-a-{SEED}"), SEED).unwrap();
    let mut engine_b = SimEngine::build_test(format!("det-pricing-b-{SEED}"), SEED).unwrap();

    // Same sequence of commands on both engines
    for (engine, label) in [(&mut engine_a, "a"), (&mut engine_b, "b")] {
        engine.run_ticks(10).unwrap();
        engine
            .submit_command(PlayerCommand::SetProductFee {
                product_id: "basic_checking".into(),
                fee_type: "overdraft_fee".into(),
                new_value: 28.0,
            })
            .unwrap();
        engine.run_ticks(20).unwrap();
        let _ = label; // suppress warning
    }

    let run_id_a = format!("det-pricing-a-{SEED}");
    let run_id_b = format!("det-pricing-b-{SEED}");

    let state_a = engine_a
        .store_product_state(&run_id_a, "basic_checking")
        .unwrap();
    let state_b = engine_b
        .store_product_state(&run_id_b, "basic_checking")
        .unwrap();

    assert_eq!(
        state_a.overdraft_fee, state_b.overdraft_fee,
        "Both engines should have same overdraft_fee after identical commands"
    );
    assert_eq!(
        state_a.monthly_fee, state_b.monthly_fee,
        "Both engines should have same monthly_fee"
    );
}
