use crate::{
    config::{SegmentConfig, SimConfig},
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use serde::{Deserialize, Serialize};

pub const CHURN_THRESHOLD: f64 = 0.85;
pub const SATISFACTION_DECAY_PER_TICK: f64 = 0.0002;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerRecord {
    pub customer_id: String,
    pub segment: String,
    pub income_band: String,
    pub risk_band: String,
    pub open_tick: Tick,
    pub status: String, // active | churned | frozen
    pub churn_risk: f64,
    pub satisfaction: f64,
    pub monthly_txn_mean: f64,
    pub cash_intensity: f64,
    pub payroll_amount: f64,
    pub has_payroll: bool,
    pub product_id: String,
}

pub struct CustomerSubsystem {
    run_id: RunId,
    config: SimConfig,
    store: SimStore,
    initialized: bool,
}

impl CustomerSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        Self {
            run_id,
            config,
            store,
            initialized: false,
        }
    }

    fn generate_initial_population(
        &self,
        rng: &mut SubsystemRng,
        tick: Tick,
    ) -> SimResult<Vec<(CustomerRecord, String)>> {
        // Returns Vec<(customer, account_id)>
        let n = self.config.initial_population;
        let mut customers = Vec::with_capacity(n);

        for i in 0..n {
            let seg = self.pick_segment(rng);
            let income_band = self.pick_income_band(seg, rng);
            let has_payroll = rng.chance(seg.payroll_probability);
            let payroll_amount = if has_payroll {
                let raw = rng.pareto(seg.payroll_amount_mean * 0.5, 2.5);
                raw.min(seg.payroll_amount_mean * 3.0)
            } else {
                0.0
            };

            // Individual transaction mean: segment mean ± 30%
            let mean_adj = 1.0 + (rng.next_f64() - 0.5) * 0.6;
            let monthly_txn_mean = (seg.monthly_txn_count_mean * mean_adj).max(3.0);

            let product_id =
                seg.products[rng.next_u64_below(seg.products.len() as u64) as usize].clone();

            let customer_id = format!("c-{i:06}");
            let account_id = format!("a-{i:06}");

            let record = CustomerRecord {
                customer_id: customer_id.clone(),
                segment: seg.id.clone(),
                income_band: income_band.clone(),
                risk_band: "low".into(),
                open_tick: tick,
                status: "active".into(),
                churn_risk: 0.0,
                satisfaction: 0.8,
                monthly_txn_mean,
                cash_intensity: seg.cash_intensity,
                payroll_amount,
                has_payroll,
                product_id,
            };
            customers.push((record, account_id));
        }
        Ok(customers)
    }

    fn pick_segment<'a>(&'a self, rng: &mut SubsystemRng) -> &'a SegmentConfig {
        let roll = rng.next_f64();
        let mut cumulative = 0.0;
        let segments: Vec<_> = self.config.segments.values().collect();
        for seg in &segments {
            cumulative += seg.population_share;
            if roll < cumulative {
                return seg;
            }
        }
        segments.last().unwrap()
    }

    fn pick_income_band(&self, seg: &SegmentConfig, rng: &mut SubsystemRng) -> String {
        let roll = rng.next_f64();
        let mut cum = 0.0;
        for (band, weight) in seg.income_bands.iter().zip(seg.income_band_weights.iter()) {
            cum += weight;
            if roll < cum {
                return band.clone();
            }
        }
        seg.income_bands
            .last()
            .cloned()
            .unwrap_or_else(|| "low".into())
    }
}

impl SimSubsystem for CustomerSubsystem {
    fn name(&self) -> &'static str {
        "customer"
    }

    fn update(
        &mut self,
        tick: Tick,
        events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        // Tick 0: generate initial population.
        if !self.initialized {
            self.initialized = true;
            let population = self.generate_initial_population(rng, tick)?;
            let mut onboarded = 0usize;
            for (customer, account_id) in population {
                self.store.insert_customer(&self.run_id, &customer)?;
                self.store.insert_account(
                    &self.run_id,
                    &account_id,
                    &customer.customer_id,
                    &customer.product_id,
                    customer.payroll_amount * 2.0, // seed balance
                    tick,
                )?;
                out_events.push(SimEvent::CustomerOnboarded {
                    tick,
                    customer_id: customer.customer_id.clone(),
                    segment: customer.segment.clone(),
                    account_id,
                });
                onboarded += 1;
            }
            log::info!("tick=0 customer: onboarded {onboarded} customers");
            return Ok(out_events);
        }

        // Process fee events from the transaction subsystem
        // that affect satisfaction and churn risk.
        for event in events_in {
            if let SimEvent::FeeCharged {
                customer_id,
                fee_type,
                ..
            } = event
            {
                self.store.update_customer_satisfaction(
                    &self.run_id,
                    customer_id,
                    match fee_type.as_str() {
                        "overdraft" => -0.04,
                        "nsf" => -0.06,
                        _ => -0.01,
                    },
                )?;
            }
        }

        // Apply satisfaction decay every 30 ticks (monthly).
        // ChurnSubsystem (Phase 2.3) now owns churn evaluation and decision.
        if tick.is_multiple_of(30) {
            let active = self.store.active_customers(&self.run_id)?;
            for mut c in active {
                // Gentle satisfaction decay toward equilibrium
                if c.satisfaction > 0.6 {
                    c.satisfaction = (c.satisfaction - SATISFACTION_DECAY_PER_TICK * 30.0).max(0.0);
                    self.store.update_customer_churn_satisfaction(
                        &self.run_id,
                        &c.customer_id,
                        c.churn_risk, // leave churn_risk unchanged; ChurnSubsystem will update
                        c.satisfaction,
                    )?;
                }
            }
        }

        Ok(out_events)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
