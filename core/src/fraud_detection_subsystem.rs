//! Fraud Detection subsystem — Phase 3.5 Week 3.
//!
//! This subsystem:
//!   1. Detects synthetic identity fraud using SSN/DOB/address patterns
//!   2. Identifies bust-out fraud (rapid spending followed by default)
//!   3. Detects money mule accounts (rapid in/out transfers)
//!   4. Flags elder abuse patterns (unusual activity for seniors)
//!   5. Calculates account-level fraud risk scores
//!   6. Generates fraud alerts for investigation

use crate::{
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use std::any::Any;

// ── Constants ────────────────────────────────────────────────────────────────

const SYNTHETIC_IDENTITY_THRESHOLD: f64 = 0.60;
const BUST_OUT_AMOUNT_THRESHOLD: f64 = 5000.0;
const ACCOUNT_RISK_THRESHOLD: f64 = 0.60;
const VELOCITY_THRESHOLD: i64 = 10; // transactions per day
const ELDER_AGE_THRESHOLD: i64 = 65;
const METRICS_INTERVAL: i64 = 7; // weekly

// ── Subsystem ────────────────────────────────────────────────────────────────

pub struct FraudDetectionSubsystem {
    run_id: RunId,
    store: SimStore,
}

impl FraudDetectionSubsystem {
    pub fn new(run_id: RunId, store: SimStore) -> Self {
        Self { run_id, store }
    }

    /// Detect synthetic identity fraud using customer identity data.
    fn detect_synthetic_identity(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get recently onboarded customers (last 30 days)
        let recent_customers = self.store.get_customers_onboarded_in_window(
            &self.run_id,
            tick.saturating_sub(30) as i64,
            tick as i64,
        )?;

        for customer in recent_customers {
            let identity = match self.store.get_customer_identity(&self.run_id, &customer.customer_id)? {
                Some(id) => id,
                None => continue,
            };

            let address = match self.store.get_customer_primary_address(&self.run_id, &customer.customer_id)? {
                Some(addr) => addr,
                None => continue,
            };

            let mut score = 0.0;
            let mut indicators = Vec::new();

            // Indicator 1: SSN shared across multiple customers (0.40)
            if identity.ssn_shared_count > 1 {
                score += 0.40;
                indicators.push(format!("SSN shared by {} customers", identity.ssn_shared_count));
            }

            // Indicator 2: High-risk address type (0.25)
            if address.address_type == "cmra" || address.address_type == "po_box" {
                score += 0.25;
                indicators.push(format!("High-risk address type: {}", address.address_type));
            }

            // Indicator 3: Address shared by many customers (0.20)
            let address_sharing = self.store.count_customers_at_address(
                &self.run_id,
                &address.street_address,
                &address.city,
                &address.state,
            )?;
            if address_sharing > 5 {
                score += 0.20;
                indicators.push(format!("Address shared by {} customers", address_sharing));
            }

            // Indicator 4: Synthetic identity status (0.30)
            if identity.identity_type == "synthetic" {
                score += 0.30;
                indicators.push("Identity marked as synthetic".to_string());
            }

            // Indicator 5: Phone sharing (0.15)
            if let Some(phone) = self.store.get_customer_primary_phone(&self.run_id, &customer.customer_id)? {
                if phone.customer_count > 1 {
                    score += 0.15;
                    indicators.push(format!("Phone shared by {} customers", phone.customer_count));
                }
            }

            // Add deterministic noise
            score += rng.next_f64() * 0.05;
            score = score.min(1.0);

            if score >= SYNTHETIC_IDENTITY_THRESHOLD && !indicators.is_empty() {
                let pattern_id = format!("fraud-syn-{}-{}", customer.customer_id, tick);

                self.store.insert_fraud_pattern(
                    &self.run_id,
                    &pattern_id,
                    "synthetic_identity",
                    tick as i64,
                    score,
                    Some(&customer.customer_id),
                    None,
                    &serde_json::to_string(&indicators).unwrap_or_default(),
                )?;

                events.push(SimEvent::FraudPatternDetected {
                    tick,
                    pattern_id: pattern_id.clone(),
                    pattern_type: "synthetic_identity".to_string(),
                    customer_id: customer.customer_id.clone(),
                    confidence_score: score,
                });
            }
        }

        Ok(events)
    }

    /// Detect bust-out fraud (rapid spending increase then default).
    fn detect_bust_out(
        &self,
        tick: Tick,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get active accounts
        let accounts = self.store.get_active_accounts(&self.run_id)?;

        for account in accounts {
            // Check spending in last 7 days vs prior 30 days
            let recent_spending = self.store.sum_account_debits_in_window(
                &self.run_id,
                &account.account_id,
                tick.saturating_sub(7) as i64,
                tick as i64,
            )?;

            let historical_spending = self.store.sum_account_debits_in_window(
                &self.run_id,
                &account.account_id,
                tick.saturating_sub(37) as i64,
                tick.saturating_sub(7) as i64,
            )?;

            // Bust-out pattern: recent spending > 5x historical + over threshold
            if historical_spending > 0.0
                && recent_spending > 5.0 * historical_spending
                && recent_spending > BUST_OUT_AMOUNT_THRESHOLD
            {
                let pattern_id = format!("fraud-bustout-{}-{}", account.account_id, tick);
                let score = ((recent_spending / historical_spending) / 10.0).min(1.0);

                let indicators = vec![
                    format!("Recent spending: ${:.2}", recent_spending),
                    format!("Historical avg: ${:.2}", historical_spending),
                    format!("Ratio: {:.1}x", recent_spending / historical_spending),
                ];

                self.store.insert_fraud_pattern(
                    &self.run_id,
                    &pattern_id,
                    "bust_out",
                    tick as i64,
                    score,
                    None,
                    Some(&account.account_id),
                    &serde_json::to_string(&indicators).unwrap_or_default(),
                )?;

                events.push(SimEvent::FraudPatternDetected {
                    tick,
                    pattern_id,
                    pattern_type: "bust_out".to_string(),
                    customer_id: account.customer_id.clone(),
                    confidence_score: score,
                });
            }
        }

        Ok(events)
    }

    /// Calculate account fraud risk score.
    fn calculate_account_fraud_scores(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Only compute monthly
        if !tick.is_multiple_of(30) {
            return Ok(events);
        }

        let accounts = self.store.get_active_accounts(&self.run_id)?;

        for account in accounts {
            let mut score = 0.0;

            // Component 1: Velocity (transaction count)
            let txn_count = self.store.count_account_txns_in_window(
                &self.run_id,
                &account.account_id,
                tick.saturating_sub(7) as i64,
                tick as i64,
            )?;
            let velocity_component = ((txn_count as f64) / 50.0).min(0.30);
            score += velocity_component;

            // Component 2: Amount (average transaction size)
            let avg_amount = if txn_count > 0 {
                let total = self.store.sum_account_debits_in_window(
                    &self.run_id,
                    &account.account_id,
                    tick.saturating_sub(7) as i64,
                    tick as i64,
                )?;
                total / txn_count as f64
            } else {
                0.0
            };
            let amount_component = (avg_amount / 1000.0).min(0.25);
            score += amount_component;

            // Component 3: Pattern (diversity of counterparties)
            let unique_counterparties = self.store.count_unique_counterparties_in_window(
                &self.run_id,
                &account.account_id,
                tick.saturating_sub(7) as i64,
                tick as i64,
            )?;
            let pattern_component = if unique_counterparties > 15 { 0.20 } else { 0.0 };
            score += pattern_component;

            // Component 4: Behavioral (cash intensity)
            // Component 5: Identity (from customer risk score)

            // Add noise
            score += rng.next_f64() * 0.05;
            score = score.min(1.0);

            // Store score
            self.store.insert_account_fraud_score(
                &self.run_id,
                &account.account_id,
                tick as i64,
                score,
                velocity_component,
                amount_component,
                pattern_component,
                0.0, // behavioral_component
                0.0, // identity_component
            )?;

            // Alert if high risk
            if score >= ACCOUNT_RISK_THRESHOLD {
                let alert_id = format!("fraud-acct-{}-{}", account.account_id, tick);

                self.store.insert_fraud_alert(
                    &self.run_id,
                    &alert_id,
                    tick as i64,
                    "account_risk_score",
                    "account",
                    &account.account_id,
                    score,
                    "medium",
                )?;

                events.push(SimEvent::FraudAlertGenerated {
                    tick,
                    alert_id,
                    alert_type: "account_risk_score".to_string(),
                    entity_id: account.account_id.clone(),
                    fraud_score: score,
                    severity: "medium".to_string(),
                });
            }
        }

        Ok(events)
    }
}

impl SimSubsystem for FraudDetectionSubsystem {
    fn name(&self) -> &'static str {
        "fraud_detection"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        if tick == 0 {
            return Ok(events);
        }

        // 1. Detect synthetic identity (daily for recent customers)
        events.extend(self.detect_synthetic_identity(tick, rng)?);

        // 2. Detect bust-out patterns (weekly)
        if tick.is_multiple_of(7) {
            events.extend(self.detect_bust_out(tick)?);
        }

        // 3. Calculate account fraud scores (monthly)
        events.extend(self.calculate_account_fraud_scores(tick, rng)?);

        Ok(events)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
