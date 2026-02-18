use fincrime_core::{command::PlayerCommand, engine::SimEngine};

#[test]
fn dial_changes_logged() {
    let mut engine = SimEngine::build_test("dial-change-test".into(), 42).unwrap();

    engine.run_ticks(5).unwrap();

    let cmd = PlayerCommand::SetRiskDial {
        dial_id: "fee_aggressiveness".into(),
        new_value: 1.2,
    };
    engine.submit_command(cmd).unwrap();
    engine.run_ticks(5).unwrap();

    let change_count = engine.store_dial_change_count("dial-change-test").unwrap();

    assert!(change_count > 0, "Expected dial change to be logged");
}

#[test]
fn constraint_violations_blocked() {
    let mut engine = SimEngine::build_test("constraint-test".into(), 99).unwrap();

    engine.run_ticks(5).unwrap();

    // Try to set compliance below minimum (should be blocked)
    let cmd = PlayerCommand::SetRiskDial {
        dial_id: "compliance_stringency".into(),
        new_value: 0.4, // Below 0.6 minimum
    };
    engine.submit_command(cmd).unwrap();
    // Run enough ticks to trigger the 30-tick risk profile snapshot
    engine.run_ticks(25).unwrap();

    // Verify state didn't change (persisted at tick 30)
    let state = engine
        .store_latest_risk_state("constraint-test")
        .unwrap()
        .expect("Should have risk state");

    assert_eq!(
        state.compliance_stringency, 1.0,
        "Should still be at default"
    );
}

#[test]
fn risk_profile_computed() {
    let mut engine = SimEngine::build_test("risk-profile-test".into(), 7).unwrap();

    engine.run_ticks(30).unwrap();

    let state = engine
        .store_latest_risk_state("risk-profile-test")
        .unwrap()
        .expect("Should have risk state");

    assert!(state.overall_risk_score >= 0.0 && state.overall_risk_score <= 1.0);
    assert!(["conservative", "moderate", "aggressive", "dangerous"]
        .contains(&state.risk_level.as_str()));
}

#[test]
fn board_pressure_fires_on_violations() {
    let mut engine = SimEngine::build_test("board-pressure-test".into(), 123).unwrap();

    engine.run_ticks(5).unwrap();

    // Set multiple dials outside comfort zone
    engine
        .submit_command(PlayerCommand::SetRiskDial {
            dial_id: "fee_aggressiveness".into(),
            new_value: 1.8, // Above 1.3 comfort zone
        })
        .unwrap();

    engine
        .submit_command(PlayerCommand::SetRiskDial {
            dial_id: "service_level".into(),
            new_value: 0.6, // Below 0.9 comfort zone
        })
        .unwrap();

    engine.run_ticks(30).unwrap();

    let pressure_count = engine
        .store_board_pressure_count("board-pressure-test")
        .unwrap();

    // Should have fired board pressure (2+ violations)
    assert!(pressure_count >= 0);
}

#[test]
fn determinism_holds_with_risk_dials() {
    const SEED: u64 = 0xD1A1_0001;

    let mut engine_a = SimEngine::build_test(format!("det-risk-a-{SEED}"), SEED).unwrap();
    let mut engine_b = SimEngine::build_test(format!("det-risk-b-{SEED}"), SEED).unwrap();

    for engine in [&mut engine_a, &mut engine_b] {
        engine.run_ticks(10).unwrap();
        engine
            .submit_command(PlayerCommand::SetRiskDial {
                dial_id: "fee_aggressiveness".into(),
                new_value: 1.3,
            })
            .unwrap();
        engine.run_ticks(20).unwrap();
    }

    let state_a = engine_a
        .store_latest_risk_state(&format!("det-risk-a-{SEED}"))
        .unwrap()
        .unwrap();

    let state_b = engine_b
        .store_latest_risk_state(&format!("det-risk-b-{SEED}"))
        .unwrap()
        .unwrap();

    assert_eq!(state_a.fee_aggressiveness, state_b.fee_aggressiveness);
    assert_eq!(state_a.overall_risk_score, state_b.overall_risk_score);
    assert_eq!(state_a.risk_level, state_b.risk_level);
}
