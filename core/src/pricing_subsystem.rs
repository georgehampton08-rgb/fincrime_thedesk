//! Pricing subsystem — manages product fee configuration and
//! regulatory constraint validation.
//!
//! This subsystem owns the product_state table and processes
//! all fee change commands from the player.

use crate::{
    command::PlayerCommand,
    config::SimConfig,
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use std::collections::HashMap;

pub struct PricingSubsystem {
    run_id:        RunId,
    config:        SimConfig,
    store:         SimStore,
    initialized:   bool,
    product_state: HashMap<String, ProductState>,
}

#[derive(Debug, Clone)]
pub struct ProductState {
    pub product_id:    String,
    pub monthly_fee:   f64,
    pub overdraft_fee: f64,
    pub nsf_fee:       f64,
    pub atm_fee:       f64,
    pub wire_fee:      f64,
    pub interest_rate: f64,
}

impl PricingSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        Self {
            run_id,
            config,
            store,
            initialized: false,
            product_state: HashMap::new(),
        }
    }

    fn initialize_product_state(&mut self, tick: Tick) -> SimResult<()> {
        for product in self.config.products.values() {
            let state = ProductState {
                product_id:    product.product_id.clone(),
                monthly_fee:   product.monthly_fee,
                overdraft_fee: product.overdraft_fee,
                nsf_fee:       product.nsf_fee,
                atm_fee:       product.atm_fee,
                wire_fee:      product.wire_fee,
                interest_rate: product.interest_rate,
            };

            self.store.insert_product_state(&self.run_id, &state, tick)?;
            self.product_state.insert(product.product_id.clone(), state);
        }

        self.store.init_regulatory_score(&self.run_id, tick)?;

        Ok(())
    }

    /// Validate a fee value against constraints.
    /// Returns Ok(Some(warning)) if above soft limit,
    /// Ok(None) if fine, Err if above hard limit.
    fn validate_fee(
        &self,
        fee_type: &str,
        new_value: f64,
    ) -> Result<Option<String>, String> {
        let constraint = match self.config.fee_constraints.get(fee_type) {
            Some(c) => c,
            None => return Err(format!("Unknown fee type: {fee_type}")),
        };

        if new_value < constraint.min_value || new_value > constraint.max_value {
            return Err(format!(
                "{} must be between ${:.2} and ${:.2}. Reason: {}",
                fee_type,
                constraint.min_value,
                constraint.max_value,
                constraint.hard_limit_reason
            ));
        }

        if new_value > constraint.soft_limit {
            Ok(Some(constraint.soft_limit_warning.clone()))
        } else {
            Ok(None)
        }
    }

    fn process_fee_change(
        &mut self,
        product_id: &str,
        fee_type: &str,
        new_value: f64,
        tick: Tick,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        let warning = match self.validate_fee(fee_type, new_value) {
            Ok(w) => w,
            Err(reason) => {
                log::warn!("tick={tick} pricing: fee validation failed for {product_id}.{fee_type}: {reason}");
                events.push(SimEvent::FeeChangeRejected {
                    tick,
                    product_id: product_id.to_string(),
                    fee_type:   fee_type.to_string(),
                    reason,
                });
                return Ok(events);
            }
        };

        let state = match self.product_state.get_mut(product_id) {
            Some(s) => s,
            None => {
                let reason = format!("Unknown product: {product_id}");
                log::warn!("tick={tick} pricing: {reason}");
                events.push(SimEvent::FeeChangeRejected {
                    tick,
                    product_id: product_id.to_string(),
                    fee_type:   fee_type.to_string(),
                    reason,
                });
                return Ok(events);
            }
        };

        let old_value = match fee_type {
            "monthly_fee"   => state.monthly_fee,
            "overdraft_fee" => state.overdraft_fee,
            "nsf_fee"       => state.nsf_fee,
            "atm_fee"       => state.atm_fee,
            "wire_fee"      => state.wire_fee,
            _ => {
                let reason = format!("Invalid fee type: {fee_type}");
                log::warn!("tick={tick} pricing: {reason}");
                events.push(SimEvent::FeeChangeRejected {
                    tick,
                    product_id: product_id.to_string(),
                    fee_type:   fee_type.to_string(),
                    reason,
                });
                return Ok(events);
            }
        };

        match fee_type {
            "monthly_fee"   => state.monthly_fee   = new_value,
            "overdraft_fee" => state.overdraft_fee = new_value,
            "nsf_fee"       => state.nsf_fee       = new_value,
            "atm_fee"       => state.atm_fee       = new_value,
            "wire_fee"      => state.wire_fee      = new_value,
            _               => unreachable!(),
        }

        self.store.update_product_fee(
            &self.run_id, product_id, fee_type, new_value, tick,
        )?;
        self.store.log_fee_change(
            &self.run_id, tick, product_id, fee_type,
            old_value, new_value, true,
        )?;

        self.update_udaap_score(fee_type, new_value, tick)?;

        events.push(SimEvent::ProductFeeChanged {
            tick,
            product_id: product_id.to_string(),
            fee_type:   fee_type.to_string(),
            old_value,
            new_value,
            warning,
        });

        log::info!(
            "tick={tick} pricing: {product_id}.{fee_type} changed ${:.2} -> ${:.2}",
            old_value, new_value
        );

        Ok(events)
    }

    fn update_udaap_score(
        &self,
        fee_type: &str,
        new_value: f64,
        tick: Tick,
    ) -> SimResult<()> {
        let formula = match self.config.impact_formulas.get(fee_type) {
            Some(f) => f,
            None => return Ok(()),
        };

        let threshold = formula.parameters
            .get("udaap_risk_threshold")
            .and_then(|v| v.as_f64());
        let delta = formula.parameters
            .get("udaap_risk_delta")
            .and_then(|v| v.as_f64());

        if let (Some(thresh), Some(d)) = (threshold, delta) {
            if new_value > thresh {
                self.store.adjust_udaap_score(&self.run_id, d, tick)?;
                log::debug!(
                    "tick={tick} pricing: UDAAP risk increased by {:.2} ({fee_type} above threshold)",
                    d
                );
            }
        }

        Ok(())
    }
}

impl SimSubsystem for PricingSubsystem {
    fn name(&self) -> &'static str { "pricing" }

    fn update(
        &mut self,
        tick: Tick,
        events_in: &[SimEvent],
        _rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        // First call: initialize product state from catalog
        if !self.initialized {
            self.initialize_product_state(tick)?;
            self.initialized = true;
            log::info!(
                "tick={tick} pricing: initialized {} products from catalog",
                self.product_state.len()
            );
            return Ok(out_events);
        }

        // Process player commands
        for event in events_in {
            if let SimEvent::PlayerCommandReceived { command_id, .. } = event {
                match self.store.get_player_command(&self.run_id, command_id) {
                    Ok(Some(PlayerCommand::SetProductFee { product_id, fee_type, new_value })) => {
                        let events = self.process_fee_change(
                            &product_id, &fee_type, new_value, tick,
                        )?;
                        out_events.extend(events);
                    }
                    Ok(Some(_)) => {} // Other commands handled elsewhere
                    Ok(None) => {
                        log::warn!("tick={tick} pricing: command {command_id} not found in store");
                    }
                    Err(e) => {
                        log::warn!("tick={tick} pricing: error fetching command {command_id}: {e}");
                    }
                }
            }
        }

        Ok(out_events)
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
