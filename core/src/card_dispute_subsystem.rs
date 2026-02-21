//! Card Dispute & Chargeback subsystem — Phase 3.4.
//!
//! Generates disputes from settled card authorizations, models the complete
//! dispute lifecycle, detects friendly fraud, and integrates with Economics
//! (chargeback losses) and Complaint (rejected disputes) subsystems.
//!
//! Execution order: After ReconciliationSubsystem, before ComplaintSubsystem.

use crate::{
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use std::any::Any;

// Constants
const DISPUTE_GENERATION_RATE: f64 = 0.008; // 0.8% of settled auths
const PROVISIONAL_CREDIT_THRESHOLD_DAYS: i64 = 10; // Issue credit after 10 days
const METRICS_INTERVAL: i64 = 7; // Weekly chargeback metrics
const FRIENDLY_FRAUD_THRESHOLD: f64 = 0.70; // Auto-reject above 70%

pub struct CardDisputeSubsystem {
    run_id: RunId,
    store: SimStore,
}

impl CardDisputeSubsystem {
    pub fn new(run_id: RunId, store: SimStore) -> Self {
        Self { run_id, store }
    }

    /// Generate new disputes from settled authorizations.
    /// Queries auths settled 30-60 days ago (typical dispute filing window).
    fn generate_disputes(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Query settled auths in 30-60 day window
        let window_start = tick.saturating_sub(60) as i64;
        let window_end = tick.saturating_sub(30) as i64;

        let settled_auths = self.store.get_settled_authorizations_in_window(
            &self.run_id,
            window_start,
            window_end,
        )?;

        for auth in settled_auths {
            // Probabilistic dispute generation
            if !rng.chance(DISPUTE_GENERATION_RATE) {
                continue;
            }

            // Map merchant category (fallback to "retail" if missing)
            let merchant_category = auth
                .merchant_category
                .as_deref()
                .unwrap_or("retail");

            // Sample dispute reason based on category
            let reason = self.sample_dispute_reason(rng, merchant_category);

            // Generate deterministic dispute ID
            let seq = rng.next_u64_below(1_000_000);
            let dispute_id = format!("disp-{}-{}", tick, seq);

            // Calculate friendly fraud score
            let fraud_score = self.calculate_friendly_fraud_score(&auth.account_id, tick, rng)?;

            // Get customer_id from account
            let customer_id = self
                .store
                .get_account_customer_id(&self.run_id, &auth.account_id)?;

            // Use cleared amount if available, otherwise original amount
            let settled_amount = auth.cleared_amount.unwrap_or(auth.amount);

            // Create dispute
            self.store.insert_dispute(
                &self.run_id,
                &dispute_id,
                &auth.authorization_id,
                &auth.account_id,
                &customer_id,
                tick as i64,
                settled_amount,
                auth.merchant_name.as_deref().unwrap_or("unknown"),
                merchant_category,
                &reason,
                fraud_score,
            )?;

            events.push(SimEvent::DisputeFiled {
                tick,
                dispute_id: dispute_id.clone(),
                authorization_id: auth.authorization_id.clone(),
                customer_id: customer_id.clone(),
                amount: settled_amount,
                reason: reason.clone(),
            });

            // Immediately transition to investigating
            self.store
                .update_dispute_status(&self.run_id, &dispute_id, "investigating")?;

            events.push(SimEvent::DisputeStatusChanged {
                tick,
                dispute_id,
                old_status: "open".to_string(),
                new_status: "investigating".to_string(),
            });
        }

        Ok(events)
    }

    /// Sample dispute reason deterministically based on merchant category.
    fn sample_dispute_reason(&self, rng: &mut SubsystemRng, category: &str) -> String {
        let roll = rng.next_f64();

        match category {
            "digital_goods" | "online_gaming" | "subscription" => {
                if roll < 0.25 {
                    "unauthorized_charge"
                } else if roll < 0.45 {
                    "service_not_rendered"
                } else if roll < 0.60 {
                    "not_as_described"
                } else if roll < 0.75 {
                    "cancelled_subscription"
                } else if roll < 0.85 {
                    "duplicate_charge"
                } else {
                    "merchant_fraud"
                }
            }
            "cash_advance" | "atm" | "cash_withdrawal" => {
                if roll < 0.60 {
                    "atm_error"
                } else if roll < 0.80 {
                    "incorrect_amount"
                } else {
                    "unauthorized_charge"
                }
            }
            _ => {
                // retail, travel, grocery, purchase, or default
                if roll < 0.20 {
                    "unauthorized_charge"
                } else if roll < 0.35 {
                    "duplicate_charge"
                } else if roll < 0.50 {
                    "defective_product"
                } else if roll < 0.65 {
                    "not_as_described"
                } else if roll < 0.80 {
                    "incorrect_amount"
                } else {
                    "credit_not_received"
                }
            }
        }
        .to_string()
    }

    /// Calculate friendly fraud score using 5 indicators.
    /// Score range: [0.0, 1.0], threshold 0.70 triggers auto-rejection.
    fn calculate_friendly_fraud_score(
        &self,
        account_id: &str,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<f64> {
        let mut score = 0.0;

        // Indicator 1: Dispute frequency (>3 in 90 days = +0.30)
        let recent_disputes = self.store.count_disputes_in_window(
            &self.run_id,
            account_id,
            tick.saturating_sub(90) as i64,
            tick as i64,
        )?;
        if recent_disputes >= 3 {
            score += 0.30;
        } else if recent_disputes == 2 {
            score += 0.15;
        }

        // Indicator 2: High-value disputes (>$500, 2+ occurrences = +0.25)
        let high_value = self
            .store
            .count_high_value_disputes(&self.run_id, account_id, 500.0)?;
        if high_value >= 2 {
            score += 0.25;
        }

        // Indicator 3: Dispute-to-transaction ratio (>10% = +0.20)
        let total_txns = self.store.count_account_transactions_in_window(
            &self.run_id,
            account_id,
            tick.saturating_sub(90) as i64,
            tick as i64,
        )?;
        if total_txns > 0 {
            let ratio = recent_disputes as f64 / total_txns as f64;
            if ratio > 0.10 {
                score += 0.20;
            }
        }

        // Indicator 4: Repeat merchant disputes (same merchant 2+ times = +0.15)
        let repeat_merchant = self
            .store
            .count_repeat_merchant_disputes(&self.run_id, account_id)?;
        if repeat_merchant >= 2 {
            score += 0.15;
        }

        // Indicator 5: Account age (<90 days = new account = +0.10)
        let account_age = self
            .store
            .get_account_age(&self.run_id, account_id, tick as i64)?;
        if account_age < 90 {
            score += 0.10;
        }

        // Add deterministic noise [0.0, 0.05]
        score += rng.next_f64() * 0.05;

        Ok(score.min(1.0))
    }

    /// Issue provisional credits for disputes in investigating status >10 days.
    fn issue_provisional_credits(&self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        let disputes = self.store.get_disputes_needing_provisional_credit(
            &self.run_id,
            tick as i64,
            PROVISIONAL_CREDIT_THRESHOLD_DAYS,
        )?;

        for dispute in disputes {
            // Credit account balance (positive = credit)
            self.store.update_posted_balance(
                &self.run_id,
                &dispute.account_id,
                dispute.amount,
            )?;

            // Mark dispute as having provisional credit
            self.store.mark_provisional_credit_issued(
                &self.run_id,
                &dispute.dispute_id,
                dispute.amount,
            )?;

            events.push(SimEvent::ProvisionalCreditIssued {
                tick,
                dispute_id: dispute.dispute_id.clone(),
                account_id: dispute.account_id.clone(),
                amount: dispute.amount,
            });
        }

        Ok(events)
    }

    /// Progress dispute lifecycle through states.
    fn progress_dispute_lifecycle(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        let active = self.store.get_active_disputes(&self.run_id)?;

        for dispute in active {
            let config = self.store.get_dispute_config(&dispute.reason)?;
            let days_since_filed = (tick as i64) - dispute.tick_filed;

            // investigating → resolved_rejected (if high fraud score)
            if dispute.status == "investigating"
                && dispute.friendly_fraud_score > FRIENDLY_FRAUD_THRESHOLD
            {
                if days_since_filed >= config.investigation_duration_ticks {
                    self.resolve_dispute(&dispute.dispute_id, tick, "rejected", false, &mut events)?;

                    events.push(SimEvent::FriendlyFraudDetected {
                        tick,
                        dispute_id: dispute.dispute_id.clone(),
                        customer_id: dispute.customer_id.clone(),
                        fraud_score: dispute.friendly_fraud_score,
                    });
                }
            }
            // investigating → awaiting_merchant (normal path)
            else if dispute.status == "investigating"
                && days_since_filed >= config.investigation_duration_ticks
            {
                self.store
                    .update_dispute_status(&self.run_id, &dispute.dispute_id, "awaiting_merchant")?;
                events.push(SimEvent::DisputeStatusChanged {
                    tick,
                    dispute_id: dispute.dispute_id.clone(),
                    old_status: "investigating".to_string(),
                    new_status: "awaiting_merchant".to_string(),
                });
            }
            // awaiting_merchant → under_review (after 7 days)
            else if dispute.status == "awaiting_merchant"
                && days_since_filed >= config.investigation_duration_ticks + 7
            {
                self.store
                    .update_dispute_status(&self.run_id, &dispute.dispute_id, "under_review")?;
                events.push(SimEvent::DisputeStatusChanged {
                    tick,
                    dispute_id: dispute.dispute_id.clone(),
                    old_status: "awaiting_merchant".to_string(),
                    new_status: "under_review".to_string(),
                });
            }
            // under_review → resolved (use win probability)
            else if dispute.status == "under_review"
                && days_since_filed >= config.investigation_duration_ticks + 10
            {
                let customer_won = rng.chance(config.win_probability);
                let outcome = if customer_won { "accepted" } else { "rejected" };
                self.resolve_dispute(&dispute.dispute_id, tick, outcome, customer_won, &mut events)?;
            }
        }

        Ok(events)
    }

    /// Resolve dispute with final outcome.
    fn resolve_dispute(
        &self,
        dispute_id: &str,
        tick: Tick,
        outcome: &str,
        customer_won: bool,
        events: &mut Vec<SimEvent>,
    ) -> SimResult<()> {
        let dispute = self.store.get_dispute(&self.run_id, dispute_id)?;

        // Update status and resolution
        let new_status = format!("resolved_{}", outcome);
        self.store
            .update_dispute_status(&self.run_id, dispute_id, &new_status)?;
        self.store
            .mark_dispute_resolved(&self.run_id, dispute_id, tick as i64, outcome)?;

        events.push(SimEvent::DisputeResolved {
            tick,
            dispute_id: dispute_id.to_string(),
            outcome: outcome.to_string(),
            customer_won,
        });

        if customer_won {
            // Issue chargeback
            self.store
                .mark_chargeback_issued(&self.run_id, dispute_id)?;

            events.push(SimEvent::ChargebackIssued {
                tick,
                dispute_id: dispute_id.to_string(),
                amount: dispute.amount,
                merchant_name: dispute.merchant_name.clone(),
            });

            // If provisional credit NOT issued, credit account now
            if !dispute.provisional_credit_issued {
                self.store.update_posted_balance(
                    &self.run_id,
                    &dispute.account_id,
                    dispute.amount,
                )?;
            }
        } else {
            // Customer lost - reverse provisional credit if issued
            if dispute.provisional_credit_issued {
                self.store.update_posted_balance(
                    &self.run_id,
                    &dispute.account_id,
                    -dispute.amount,
                )?;
            }
        }

        Ok(())
    }

    /// Compute weekly chargeback metrics.
    fn compute_metrics(&self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        if (tick as i64) % METRICS_INTERVAL != 0 {
            return Ok(events);
        }

        let window_start = (tick as i64).saturating_sub(7);
        let metrics = self
            .store
            .compute_chargeback_metrics(&self.run_id, window_start, tick as i64)?;

        self.store
            .insert_chargeback_metrics(&self.run_id, tick as i64, &metrics)?;

        events.push(SimEvent::ChargebackMetricsComputed {
            tick,
            disputes_filed_7d: metrics.disputes_filed,
            win_rate_7d: metrics.win_rate,
            chargeback_amount_7d: metrics.total_chargeback_amount,
        });

        Ok(events)
    }
}

impl SimSubsystem for CardDisputeSubsystem {
    fn name(&self) -> &'static str {
        "card_dispute"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // 1. Generate new disputes from settled auths
        events.extend(self.generate_disputes(tick, rng)?);

        // 2. Issue provisional credits for long-running investigations
        events.extend(self.issue_provisional_credits(tick)?);

        // 3. Progress dispute lifecycle
        events.extend(self.progress_dispute_lifecycle(tick, rng)?);

        // 4. Compute weekly metrics
        events.extend(self.compute_metrics(tick)?);

        Ok(events)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
