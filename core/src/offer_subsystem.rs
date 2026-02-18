//! Offer subsystem — customer acquisition and retention incentives.
//!
//! Manages the full offer lifecycle:
//!   offered → in_progress → completed → paid | expired
//!
//! Phase 2.2 scope:
//!   - Match new customers to eligible signup bonuses at onboarding
//!   - Track requirements progress (payroll, balance, duration)
//!   - Complete offers when requirements satisfied
//!   - Pay cash bonuses directly to customer account
//!   - Flag bonus-seekers probabilistically
//!
//! Phase 3 hook: bonus_seeker_flag feeds into AML risk scoring

use crate::{
    config::{OfferConfig, SimConfig},
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use std::collections::HashMap;

pub struct OfferSubsystem {
    run_id:        RunId,
    config:        SimConfig,
    store:         SimStore,
    initialized:   bool,
    active_offers: HashMap<String, OfferConfig>,
}

#[derive(Debug, Clone)]
pub struct CustomerOfferRecord {
    pub customer_id:       String,
    pub offer_id:          String,
    pub tick_offered:      Tick,
    pub tick_accepted:     Option<Tick>,
    pub tick_completed:    Option<Tick>,
    pub tick_paid:         Option<Tick>,
    pub status:            String,
    pub bonus_amount:      f64,
    pub bonus_paid:        f64,
    pub requirements_met:  bool,
    pub cumulative_dd:     f64,
    pub min_balance_days:  u64,
    pub ticks_in_offer:    u64,
    pub bonus_seeker_flag: bool,
    pub velocity_flag:     bool,
}

#[derive(Debug, Clone)]
pub struct CustomerSnapshot {
    pub segment:       String,
    pub churn_risk:    f64,
    pub open_tick:     Tick,
    pub product_count: usize,
}

#[derive(Debug, Clone)]
pub struct CustomerActivity {
    pub balance:               f64,
    pub has_direct_deposit:    bool,
    pub direct_deposit_amount: f64,
}

#[derive(Debug, Clone)]
pub struct OfferPerformance {
    pub offered_count:      i64,
    pub accepted_count:     i64,
    pub completed_count:    i64,
    pub expired_count:      i64,
    pub total_bonus_paid:   f64,
    pub bonus_seeker_count: i64,
}

impl OfferSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        let active_offers = config.offers.iter()
            .filter(|(_, o)| o.active)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        Self {
            run_id,
            config,
            store,
            initialized: false,
            active_offers,
        }
    }

    fn initialize_offer_config_state(&self, tick: Tick) -> SimResult<()> {
        for offer in self.config.offers.values() {
            self.store.insert_offer_config_state(
                &self.run_id,
                &offer.offer_id,
                offer.active,
                offer.start_tick,
                offer.end_tick,
                tick,
            )?;
        }
        Ok(())
    }

    fn is_customer_eligible(
        &self,
        customer: &CustomerSnapshot,
        offer: &OfferConfig,
        tick: Tick,
    ) -> bool {
        if tick < offer.start_tick {
            return false;
        }
        if let Some(end) = offer.end_tick {
            if tick > end {
                return false;
            }
        }

        // Segment check
        if !offer.eligibility.target_segments.is_empty()
            && !offer.eligibility.target_segments.contains(&customer.segment)
        {
            return false;
        }
        if offer.eligibility.exclude_segments.contains(&customer.segment) {
            return false;
        }

        // New-to-bank check — must be onboarded THIS tick
        if offer.requirements.new_to_bank_only && customer.open_tick != tick {
            return false;
        }

        // Churn risk range (retention offers)
        if let Some(min) = offer.eligibility.min_churn_risk {
            if customer.churn_risk < min {
                return false;
            }
        }
        if let Some(max) = offer.eligibility.max_churn_risk {
            if customer.churn_risk > max {
                return false;
            }
        }

        // Existing product count
        if let Some(max_p) = offer.eligibility.max_existing_products {
            if customer.product_count > max_p {
                return false;
            }
        }

        true
    }

    fn match_and_create_offer(
        &self,
        customer_id: &str,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out = Vec::new();

        let customer = match self.store.get_customer_snapshot(&self.run_id, customer_id) {
            Ok(c) => c,
            Err(_) => return Ok(out),
        };

        // Collect eligible active offers
        let mut eligible: Vec<&OfferConfig> = self.active_offers.values()
            .filter(|o| self.is_customer_eligible(&customer, o, tick))
            .collect();

        if eligible.is_empty() {
            return Ok(out);
        }

        // Pick one at random (Phase 2.2: one offer per customer at onboarding)
        let idx = rng.next_u64_below(eligible.len() as u64) as usize;
        eligible.sort_by(|a, b| a.offer_id.cmp(&b.offer_id)); // deterministic order before pick
        let offer = eligible[idx];

        let bonus_seeker_flag = rng.chance(offer.fraud_risk.bonus_seeker_probability);

        let record = CustomerOfferRecord {
            customer_id:      customer_id.to_string(),
            offer_id:         offer.offer_id.clone(),
            tick_offered:     tick,
            tick_accepted:    Some(tick), // auto-accept
            tick_completed:   None,
            tick_paid:        None,
            status:           "in_progress".to_string(),
            bonus_amount:     offer.bonus_amount,
            bonus_paid:       0.0,
            requirements_met: false,
            cumulative_dd:    0.0,
            min_balance_days: 0,
            ticks_in_offer:   0,
            bonus_seeker_flag,
            velocity_flag:    false,
        };

        self.store.insert_customer_offer(&self.run_id, &record)?;

        out.push(SimEvent::OfferMatched {
            tick,
            customer_id:  customer_id.to_string(),
            offer_id:     offer.offer_id.clone(),
            bonus_amount: offer.bonus_amount,
        });

        log::info!(
            "tick={tick} offer: matched {} to {} (bonus_seeker={})",
            customer_id, offer.offer_id, bonus_seeker_flag
        );

        Ok(out)
    }

    fn process_in_progress_offers(&mut self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut out = Vec::new();

        let offers = self.store.in_progress_offers(&self.run_id)?;

        for mut rec in offers {
            let offer_cfg = match self.active_offers.get(&rec.offer_id) {
                Some(o) => o.clone(),
                None => continue,
            };

            rec.ticks_in_offer += 7; // we run every 7 ticks

            // Get customer activity
            let activity = self.store.get_customer_activity(
                &self.run_id, &rec.customer_id, tick,
            ).unwrap_or(CustomerActivity {
                balance: 0.0,
                has_direct_deposit: false,
                direct_deposit_amount: 0.0,
            });

            // Accumulate direct deposits
            rec.cumulative_dd += activity.direct_deposit_amount;

            // Track minimum balance days
            if activity.balance >= offer_cfg.requirements.min_balance {
                rec.min_balance_days += 7;
            }

            // Check completion
            let duration_met = rec.ticks_in_offer >= offer_cfg.requirements.duration_ticks;
            let dd_met = offer_cfg.requirements.min_direct_deposit == 0.0
                || rec.cumulative_dd >= offer_cfg.requirements.min_direct_deposit;
            let balance_met = offer_cfg.requirements.min_balance == 0.0
                || rec.min_balance_days >= offer_cfg.requirements.duration_ticks / 2;

            if duration_met && dd_met && balance_met {
                rec.requirements_met = true;
                rec.status = "completed".to_string();
                rec.tick_completed = Some(tick);

                // Pay bonus if applicable
                if offer_cfg.cost_model.bonus_paid_on_completion && rec.bonus_amount > 0.0 {
                    if let Ok(acct) = self.store.customer_primary_account(
                        &self.run_id, &rec.customer_id,
                    ) {
                        let _ = self.store.update_account_balance(
                            &self.run_id, &acct, rec.bonus_amount,
                        );
                        rec.bonus_paid = rec.bonus_amount;
                        rec.tick_paid = Some(tick);
                        rec.status = "paid".to_string();

                        out.push(SimEvent::OfferBonusPaid {
                            tick,
                            customer_id:       rec.customer_id.clone(),
                            offer_id:          rec.offer_id.clone(),
                            amount:            rec.bonus_amount,
                            bonus_seeker_flag: rec.bonus_seeker_flag,
                        });

                        log::info!(
                            "tick={tick} offer: bonus ${:.0} paid to {} (seeker={})",
                            rec.bonus_amount, rec.customer_id, rec.bonus_seeker_flag
                        );
                    }
                }

                self.store.update_customer_offer(&self.run_id, &rec)?;

                out.push(SimEvent::OfferCompleted {
                    tick,
                    customer_id: rec.customer_id.clone(),
                    offer_id:    rec.offer_id.clone(),
                });
            } else if rec.ticks_in_offer > offer_cfg.requirements.duration_ticks + 30 {
                // Expired (grace period of 30 ticks)
                rec.status = "expired".to_string();
                self.store.update_customer_offer(&self.run_id, &rec)?;
            } else {
                // Still in progress — persist updated counters
                self.store.update_customer_offer(&self.run_id, &rec)?;
            }
        }

        Ok(out)
    }
}

impl SimSubsystem for OfferSubsystem {
    fn name(&self) -> &'static str { "offer" }

    fn update(
        &mut self,
        tick: Tick,
        events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out = Vec::new();

        // First call: initialize offer config state from catalog.
        // Fall through (no early return) so CustomerOnboarded events on the
        // same tick as initialization (tick 1 initial population) are matched.
        if !self.initialized {
            self.initialize_offer_config_state(tick)?;
            self.initialized = true;
            log::info!(
                "tick={tick} offer: initialized {} offers ({} active)",
                self.config.offers.len(),
                self.active_offers.len()
            );
        }

        // Match new customers to eligible offers
        for event in events_in {
            if let SimEvent::CustomerOnboarded { customer_id, .. } = event {
                let matched = self.match_and_create_offer(customer_id, tick, rng)?;
                out.extend(matched);
            }
        }

        // Every 7 ticks: update progress on in-progress offers
        if tick % 7 == 0 {
            let progress_events = self.process_in_progress_offers(tick)?;
            out.extend(progress_events);
        }

        // Every 30 ticks: compute and save offer performance metrics
        if tick % 30 == 0 {
            for offer_id in self.active_offers.keys().cloned().collect::<Vec<_>>() {
                if let Ok(perf) = self.store.compute_offer_performance(&self.run_id, &offer_id) {
                    let _ = self.store.save_offer_performance(
                        &self.run_id, &offer_id, tick, &perf,
                    );
                }
            }
        }

        Ok(out)
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
