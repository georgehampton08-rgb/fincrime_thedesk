//! Risk appetite subsystem — player control dials and constraints.
//!
//! This subsystem:
//!   1. Maintains current dial settings
//!   2. Processes dial change commands from player
//!   3. Validates constraints (warnings and hard blocks)
//!   4. Computes aggregate risk profile score
//!   5. Fires board pressure events when dials leave comfort zone
//!   6. Tracks dial impact attribution

use crate::{
    command::PlayerCommand, config::SimConfig, error::SimResult, event::SimEvent,
    rng::SubsystemRng, store::SimStore, subsystem::SimSubsystem, types::Tick,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAppetiteState {
    pub fee_aggressiveness: f64,
    pub growth_velocity: f64,
    pub service_level: f64,
    pub retention_spend: f64,
    pub compliance_stringency: f64,
    // Risk profile
    pub overall_risk_score: f64,
    pub revenue_risk: f64,
    pub operational_risk: f64,
    pub compliance_risk: f64,
    pub financial_risk: f64,
    pub risk_level: String,
    // Violations
    pub comfort_zone_violations: u32,
}

impl RiskAppetiteState {
    fn default_from_config(config: &SimConfig) -> Self {
        let mut state = Self {
            fee_aggressiveness: 1.0,
            growth_velocity: 1.0,
            service_level: 1.0,
            retention_spend: 1.0,
            compliance_stringency: 1.0,
            overall_risk_score: 0.5,
            revenue_risk: 0.5,
            operational_risk: 0.5,
            compliance_risk: 0.5,
            financial_risk: 0.5,
            risk_level: "moderate".into(),
            comfort_zone_violations: 0,
        };

        // Set from config defaults
        for dial in &config.risk_appetite.dials {
            match dial.dial_id.as_str() {
                "fee_aggressiveness" => state.fee_aggressiveness = dial.default_value,
                "growth_velocity" => state.growth_velocity = dial.default_value,
                "service_level" => state.service_level = dial.default_value,
                "retention_spend" => state.retention_spend = dial.default_value,
                "compliance_stringency" => state.compliance_stringency = dial.default_value,
                _ => {}
            }
        }

        state
    }

    pub fn get_dial_value(&self, dial_id: &str) -> f64 {
        match dial_id {
            "fee_aggressiveness" => self.fee_aggressiveness,
            "growth_velocity" => self.growth_velocity,
            "service_level" => self.service_level,
            "retention_spend" => self.retention_spend,
            "compliance_stringency" => self.compliance_stringency,
            _ => 1.0,
        }
    }

    fn set_dial_value(&mut self, dial_id: &str, value: f64) {
        match dial_id {
            "fee_aggressiveness" => self.fee_aggressiveness = value,
            "growth_velocity" => self.growth_velocity = value,
            "service_level" => self.service_level = value,
            "retention_spend" => self.retention_spend = value,
            "compliance_stringency" => self.compliance_stringency = value,
            _ => {}
        }
    }
}

pub struct RiskAppetiteSubsystem {
    run_id: String,
    config: SimConfig,
    store: SimStore,
    state: RiskAppetiteState,
}

impl RiskAppetiteSubsystem {
    pub fn new(run_id: String, config: SimConfig, store: SimStore) -> Self {
        let state = RiskAppetiteState::default_from_config(&config);
        Self {
            run_id,
            config,
            store,
            state,
        }
    }

    fn validate_dial_change(&self, dial_id: &str, new_value: f64) -> Result<Vec<String>, String> {
        let dial_config = self
            .config
            .risk_appetite
            .dials
            .iter()
            .find(|d| d.dial_id == dial_id)
            .ok_or_else(|| format!("Unknown dial: {}", dial_id))?;

        // Check bounds
        if new_value < dial_config.min_value || new_value > dial_config.max_value {
            return Err(format!(
                "{} must be between {:.1} and {:.1}",
                dial_config.label, dial_config.min_value, dial_config.max_value
            ));
        }

        // Check constraints
        let mut warnings = Vec::new();
        let mut temp_state = self.state.clone();
        temp_state.set_dial_value(dial_id, new_value);

        for constraint in &self.config.risk_appetite.constraints {
            if Self::evaluate_constraint_condition(&constraint.condition, &temp_state) {
                match constraint.enforcement.as_str() {
                    "hard_block" => {
                        return Err(format!("BLOCKED: {}", constraint.violation_message));
                    }
                    "warning" => {
                        warnings.push(format!("WARNING: {}", constraint.violation_message));
                    }
                    _ => {}
                }
            }
        }

        Ok(warnings)
    }

    fn evaluate_constraint_condition(condition: &str, state: &RiskAppetiteState) -> bool {
        // Simplified constraint evaluation
        if condition.contains("growth_velocity > 1.5") && condition.contains("service_level < 0.8")
        {
            return state.growth_velocity > 1.5 && state.service_level < 0.8;
        }

        if condition.contains("fee_aggressiveness > 1.4")
            && condition.contains("service_level < 1.0")
        {
            return state.fee_aggressiveness > 1.4 && state.service_level < 1.0;
        }

        if condition.contains("compliance_stringency < 0.6") {
            return state.compliance_stringency < 0.6;
        }

        if condition.contains("retention_spend > 1.5")
            && condition.contains("growth_velocity > 1.5")
        {
            return state.retention_spend > 1.5 && state.growth_velocity > 1.5;
        }

        false
    }

    fn compute_risk_profile(&mut self) {
        // Revenue risk: lower fees = higher revenue risk
        self.state.revenue_risk =
            (2.0 - self.state.fee_aggressiveness) * 0.5 + (2.0 - self.state.growth_velocity) * 0.5;

        // Operational risk: poor service + high growth
        self.state.operational_risk = (2.0 - self.state.service_level) * 0.6
            + (self.state.growth_velocity - 1.0).max(0.0) * 0.4;

        // Compliance risk: low compliance stringency + aggressive fees
        self.state.compliance_risk = (2.0 - self.state.compliance_stringency) * 0.7
            + (self.state.fee_aggressiveness - 1.0).max(0.0) * 0.3;

        // Financial risk: high spend across all areas
        self.state.financial_risk = self.state.retention_spend * 0.4
            + self.state.growth_velocity * 0.3
            + self.state.service_level * 0.3;

        // Overall risk (weighted average)
        self.state.overall_risk_score = self.state.revenue_risk * 0.25
            + self.state.operational_risk * 0.20
            + self.state.compliance_risk * 0.30
            + self.state.financial_risk * 0.25;

        // Clamp to [0, 1]
        self.state.overall_risk_score = self.state.overall_risk_score.clamp(0.0, 1.0);

        // Determine risk level
        self.state.risk_level = if self.state.overall_risk_score <= 0.3 {
            "conservative".to_string()
        } else if self.state.overall_risk_score <= 0.6 {
            "moderate".to_string()
        } else if self.state.overall_risk_score <= 0.85 {
            "aggressive".to_string()
        } else {
            "dangerous".to_string()
        };
    }

    fn check_comfort_zones(&self) -> Vec<String> {
        let mut violations = Vec::new();

        for dial in &self.config.risk_appetite.dials {
            let value = self.state.get_dial_value(&dial.dial_id);

            if value < dial.comfort_zone_min {
                violations.push(format!("{}_low", dial.dial_id));
            } else if value > dial.comfort_zone_max {
                violations.push(format!("{}_high", dial.dial_id));
            }
        }

        violations
    }

    fn fire_board_pressure(&self, violations: &[String], tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        if violations.len() as u32
            >= self
                .config
                .risk_appetite
                .board_pressure
                .comfort_zone_violation_threshold
        {
            for violation_key in violations {
                if let Some(message) = self
                    .config
                    .risk_appetite
                    .board_pressure
                    .pressure_messages
                    .get(violation_key)
                {
                    let severity = if violations.len() >= 3 {
                        "high"
                    } else {
                        "medium"
                    };

                    self.store.insert_board_pressure(
                        &self.run_id,
                        tick,
                        "comfort_zone_violation",
                        violation_key,
                        message,
                        severity,
                    )?;

                    events.push(SimEvent::BoardPressureFired {
                        tick,
                        pressure_type: violation_key.clone(),
                        message: message.clone(),
                        severity: severity.to_string(),
                    });

                    log::warn!(
                        "tick={tick} BOARD PRESSURE: {} - {}",
                        violation_key,
                        message
                    );
                }
            }
        }

        Ok(events)
    }
}

impl SimSubsystem for RiskAppetiteSubsystem {
    fn name(&self) -> &'static str {
        "risk_appetite"
    }

    fn update(
        &mut self,
        tick: Tick,
        events_in: &[SimEvent],
        _rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        // Process dial change commands
        for event in events_in {
            if let SimEvent::PlayerCommandReceived { command_id, .. } = event {
                if let Ok(Some(PlayerCommand::SetRiskDial { dial_id, new_value })) =
                    self.store.get_player_command(&self.run_id, command_id)
                {
                    let old_value = self.state.get_dial_value(&dial_id);

                    // Validate
                    match self.validate_dial_change(&dial_id, new_value) {
                        Ok(warnings) => {
                            // Apply change
                            self.state.set_dial_value(&dial_id, new_value);

                            // Log change
                            self.store.log_dial_change(
                                &self.run_id,
                                tick,
                                &dial_id,
                                old_value,
                                new_value,
                                true,
                            )?;

                            out_events.push(SimEvent::RiskDialChanged {
                                tick,
                                dial_id: dial_id.clone(),
                                old_value,
                                new_value,
                                warnings: if warnings.is_empty() {
                                    None
                                } else {
                                    Some(warnings.join("; "))
                                },
                            });

                            log::info!(
                                "tick={tick} risk: {} changed {:.2} -> {:.2}",
                                dial_id,
                                old_value,
                                new_value
                            );
                        }
                        Err(error_msg) => {
                            out_events.push(SimEvent::RiskDialRejected {
                                tick,
                                dial_id: dial_id.clone(),
                                attempted_value: new_value,
                                reason: error_msg,
                            });
                        }
                    }
                }
            }
        }

        // Compute risk profile every 30 ticks
        if tick.is_multiple_of(30) {
            self.compute_risk_profile();

            // Check comfort zones
            let violations = self.check_comfort_zones();
            self.state.comfort_zone_violations = violations.len() as u32;

            // Persist state
            self.store
                .insert_risk_appetite_state(&self.run_id, tick, &self.state)?;

            // Fire board pressure if needed
            let pressure_events = self.fire_board_pressure(&violations, tick)?;
            out_events.extend(pressure_events);

            log::debug!(
                "tick={tick} risk: profile={} score={:.2} violations={}",
                self.state.risk_level,
                self.state.overall_risk_score,
                self.state.comfort_zone_violations
            );
        }

        Ok(out_events)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
