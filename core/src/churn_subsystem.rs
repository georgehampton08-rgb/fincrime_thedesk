//! Churn modeling subsystem — multi-factor customer retention prediction.
//!
//! This subsystem:
//!   1. Computes churn risk scores using a weighted formula
//!   2. Generates life events that affect behavioural patterns
//!   3. Tracks churn component contributions for analysis
//!   4. Triggers actual churn when risk exceeds threshold + coin flip
//!   5. Records churn cohorts for post-mortem analysis
//!
//! Execution: every 30 ticks (monthly).
//! Depends on: customer, complaint, transaction, offer subsystems.

use crate::{
    config::SimConfig,
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use serde::{Deserialize, Serialize};

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnScore {
    pub customer_id:            String,
    pub tick:                   Tick,
    pub churn_risk:             f64,
    // Components
    pub base_rate:              f64,
    pub satisfaction_component: f64,
    pub fee_burden_component:   f64,
    pub complaint_component:    f64,
    pub sla_breach_component:   f64,
    pub inactivity_component:   f64,
    pub product_depth_bonus:    f64,
    pub retention_offer_bonus:  f64,
    pub life_event_multiplier:  f64,
    // Predictions
    pub predicted_churn_30d:    f64,
    pub predicted_churn_90d:    f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeEvent {
    pub customer_id:       String,
    pub event_type:        String,
    pub tick_occurred:     Tick,
    pub tick_expires:      Tick,
    pub active:            bool,
    pub churn_risk_delta:  f64,
    pub behavioral_changes: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct CustomerChurnInputs {
    pub customer_id:                String,
    pub segment:                    String,
    pub open_tick:                  Tick,
    pub satisfaction:               f64,
    pub fee_burden_90d:             f64,
    pub complaints_90d:             i64,
    pub sla_breaches_90d:           i64,
    pub ticks_since_last_txn:       Tick,
    pub product_count:              usize,
    pub has_active_retention_offer: bool,
    pub active_life_event_delta:    f64,
}

#[derive(Debug, Clone)]
pub struct ChurnAggregate {
    pub active_customers:     i64,
    pub churned_this_period:  i64,
    pub high_risk_count:      i64,
    pub churn_rate:           f64,
    pub avg_churn_risk:       f64,
    pub fee_driven_churn:     i64,
    pub service_driven_churn: i64,
    pub life_event_churn:     i64,
}

// ── Subsystem ────────────────────────────────────────────────────────────────

pub struct ChurnSubsystem {
    run_id: RunId,
    config: SimConfig,
    store:  SimStore,
}

impl ChurnSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        Self { run_id, config, store }
    }

    fn compute_churn_score(
        &self,
        customer: &CustomerChurnInputs,
        tick: Tick,
    ) -> SimResult<ChurnScore> {
        let formula = &self.config.churn_model.churn_formula;

        let segment_params = self.config.churn_model.segment_base_rates
            .get(&customer.segment)
            .cloned()
            .or_else(|| {
                // Fall back to first available segment
                self.config.churn_model.segment_base_rates.values().next().cloned()
            });

        let (monthly_rate, fee_sens, svc_sens, offer_eff) = match segment_params {
            Some(p) => (p.monthly_churn_rate, p.fee_sensitivity, p.service_sensitivity, p.offer_retention_effectiveness),
            None    => (0.025, 0.80, 0.70, 0.65),
        };

        // Base rate: monthly → per-tick
        let base_rate = monthly_rate / 30.0;

        // Satisfaction component — grows when satisfaction < equilibrium
        let sat_gap = (formula.satisfaction_equilibrium - customer.satisfaction).max(0.0);
        let satisfaction_component = formula.satisfaction_weight * sat_gap * svc_sens;

        // Fee burden component
        let excess_fees = (customer.fee_burden_90d - formula.fee_burden_threshold).max(0.0);
        let fee_burden_component = formula.fee_burden_weight
            * (excess_fees / formula.fee_burden_threshold.max(1.0))
            * fee_sens;

        // Complaint component
        let complaint_component = formula.complaint_weight
            * (customer.complaints_90d as f64 / 10.0).min(1.0)
            * svc_sens;

        // SLA breach component
        let sla_breach_component = formula.sla_breach_weight
            * (customer.sla_breaches_90d as f64 / 5.0).min(1.0)
            * svc_sens;

        // Inactivity component
        let inactivity_component = if customer.ticks_since_last_txn > formula.inactivity_threshold_ticks {
            let excess = (customer.ticks_since_last_txn - formula.inactivity_threshold_ticks) as f64;
            formula.inactivity_weight * (excess / 30.0).min(1.0)
        } else {
            0.0
        };

        // Product depth bonus (negative = good, reduces churn)
        let product_depth_bonus = if customer.product_count > 1 {
            formula.product_depth_bonus * (customer.product_count - 1) as f64
        } else {
            0.0
        };

        // Retention offer bonus (negative = good, reduces churn)
        let retention_offer_bonus = if customer.has_active_retention_offer {
            formula.retention_offer_bonus * offer_eff
        } else {
            0.0
        };

        // Life event multiplier
        let life_event_multiplier = if customer.active_life_event_delta != 0.0 {
            formula.life_event_multiplier
        } else {
            1.0
        };

        let additive_risk = base_rate
            + satisfaction_component
            + fee_burden_component
            + complaint_component
            + sla_breach_component
            + inactivity_component
            + product_depth_bonus
            + retention_offer_bonus
            + customer.active_life_event_delta;

        let churn_risk = (additive_risk * life_event_multiplier).clamp(0.0, 1.0);

        // Simple forward-looking estimates
        let predicted_churn_30d = (churn_risk * 30.0).min(1.0);
        let predicted_churn_90d = (churn_risk * 90.0).min(1.0);

        Ok(ChurnScore {
            customer_id: customer.customer_id.clone(),
            tick,
            churn_risk,
            base_rate,
            satisfaction_component,
            fee_burden_component,
            complaint_component,
            sla_breach_component,
            inactivity_component,
            product_depth_bonus,
            retention_offer_bonus,
            life_event_multiplier,
            predicted_churn_30d,
            predicted_churn_90d,
        })
    }

    fn generate_life_events(
        &self,
        customer: &CustomerChurnInputs,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> Vec<LifeEvent> {
        let mut events = Vec::new();

        for event_cfg in &self.config.churn_model.life_events {
            // Segment filter
            if !event_cfg.segments.is_empty()
                && !event_cfg.segments.contains(&customer.segment)
            {
                continue;
            }

            // Annual → per-tick probability
            let prob_per_tick = event_cfg.probability_per_year / 365.0;

            if rng.chance(prob_per_tick) {
                events.push(LifeEvent {
                    customer_id:       customer.customer_id.clone(),
                    event_type:        event_cfg.event_type.clone(),
                    tick_occurred:     tick,
                    tick_expires:      tick + event_cfg.duration_ticks,
                    active:            true,
                    churn_risk_delta:  event_cfg.churn_risk_delta,
                    behavioral_changes: event_cfg.behavioral_changes.clone(),
                });
            }
        }

        events
    }

    fn should_churn(
        &self,
        score: &ChurnScore,
        rng: &mut SubsystemRng,
    ) -> bool {
        let t = &self.config.churn_model.churn_thresholds;

        if score.churn_risk < t.high_risk {
            return false;
        }

        let p = if score.churn_risk >= t.imminent_churn {
            0.95
        } else {
            let range = (t.imminent_churn - t.high_risk).max(1e-9);
            let position = score.churn_risk - t.high_risk;
            0.30 + (position / range) * 0.65
        };

        rng.chance(p)
    }

    fn record_churn_cohort(
        &self,
        customer: &CustomerChurnInputs,
        score: &ChurnScore,
        tick: Tick,
    ) -> SimResult<String> {
        let drivers = [
            ("satisfaction",  score.satisfaction_component),
            ("fee_burden",    score.fee_burden_component),
            ("complaints",    score.complaint_component),
            ("sla_breach",    score.sla_breach_component),
            ("inactivity",    score.inactivity_component),
            ("life_event",    if score.life_event_multiplier > 1.0 { 0.30 } else { 0.0 }),
        ];

        let primary_driver = drivers.iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(name, _)| *name)
            .unwrap_or("unknown");

        let cohort_id = uuid::Uuid::new_v4().to_string();
        let tenure = tick.saturating_sub(customer.open_tick);

        self.store.insert_churn_cohort(
            &self.run_id,
            &cohort_id,
            tick,
            &customer.segment,
            tenure,
            score.churn_risk,
            customer.satisfaction,
            customer.complaints_90d,
            customer.fee_burden_90d,
            customer.has_active_retention_offer,
            primary_driver,
        )?;

        Ok(cohort_id)
    }
}

impl SimSubsystem for ChurnSubsystem {
    fn name(&self) -> &'static str { "churn" }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out = Vec::new();

        if tick == 0 {
            return Ok(out);
        }

        // Expire life events whose duration has lapsed
        self.store.expire_life_events(&self.run_id, tick)?;

        // Refresh scores every `update_frequency_ticks` ticks
        if tick % self.config.churn_model.update_frequency_ticks == 0 {
            let active = self.store.active_customers(&self.run_id)?;

            for customer_record in active {
                let inputs = self.store.get_customer_churn_inputs(
                    &self.run_id,
                    &customer_record.customer_id,
                    tick,
                )?;

                let score = self.compute_churn_score(&inputs, tick)?;
                self.store.insert_churn_score(&self.run_id, &score)?;

                // Sync churn_risk back to the customer row
                self.store.update_customer_churn_satisfaction(
                    &self.run_id,
                    &customer_record.customer_id,
                    score.churn_risk,
                    inputs.satisfaction,
                )?;

                // Evaluate churn decision
                if self.should_churn(&score, rng) {
                    let cohort_id = self.record_churn_cohort(&inputs, &score, tick)?;

                    self.store.churn_customer(&self.run_id, &customer_record.customer_id, tick)?;

                    out.push(SimEvent::CustomerChurned {
                        tick,
                        customer_id: customer_record.customer_id.clone(),
                        segment:     inputs.segment.clone(),
                        churn_risk:  score.churn_risk,
                    });

                    log::info!(
                        "tick={tick} churn: {} churned (risk={:.3}, driver=cohort:{})",
                        customer_record.customer_id,
                        score.churn_risk,
                        cohort_id,
                    );

                    // Don't generate life events for churned customers
                    continue;
                }

                // Generate life events
                let life_events = self.generate_life_events(&inputs, tick, rng);
                for event in life_events {
                    let duration = event.tick_expires - event.tick_occurred;
                    self.store.insert_life_event(&self.run_id, &event)?;

                    out.push(SimEvent::LifeEventOccurred {
                        tick,
                        customer_id: event.customer_id.clone(),
                        event_type:  event.event_type.clone(),
                        duration,
                    });

                    log::debug!(
                        "tick={tick} churn: life_event={} for {} (delta={:.2})",
                        event.event_type, event.customer_id, event.churn_risk_delta,
                    );
                }
            }

            // Aggregate metrics per segment
            for segment in self.config.segments.keys().cloned().collect::<Vec<_>>() {
                if let Ok(agg) = self.store.compute_churn_aggregate(&self.run_id, &segment, tick) {
                    let _ = self.store.save_churn_aggregate(&self.run_id, &segment, tick, &agg);
                }
            }

            log::debug!("tick={tick} churn: score update complete");
        }

        Ok(out)
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
