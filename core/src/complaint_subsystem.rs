//! Complaint & Service subsystem — Phase 1C.
//!
//! Listens for FeeCharged and SLABreached events, generates complaints
//! probabilistically, tracks SLA aging, and processes player resolutions.
//!
//! DESIGN RULE: Complaints are LEADING indicators. They fire BEFORE churn.
//! A high complaint rate this quarter predicts high churn next quarter.

use crate::{
    config::{ComplaintTrigger, ResolutionCode, SimConfig},
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplaintRecord {
    pub complaint_id: String,
    pub customer_id: String,
    pub account_id: Option<String>,
    pub tick_opened: Tick,
    pub tick_closed: Option<Tick>,
    pub product: String,
    pub issue: String,
    pub priority: String,
    pub status: String,
    pub sla_due_tick: Tick,
    pub sla_breached: bool,
    pub resolution_code: Option<String>,
    pub amount_refunded: f64,
    pub udaap_flag: bool,
}

pub struct ComplaintSubsystem {
    run_id: RunId,
    store: SimStore,
    trigger_map: HashMap<String, Vec<ComplaintTrigger>>,
    // Retained for Phase 1E player-command wiring.
    #[allow(dead_code)]
    resolution_codes: HashMap<String, ResolutionCode>,
}

impl ComplaintSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        let mut trigger_map: HashMap<String, Vec<ComplaintTrigger>> = HashMap::new();
        for trigger in config.complaint_triggers {
            trigger_map
                .entry(trigger.event_type.clone())
                .or_default()
                .push(trigger);
        }
        let resolution_codes = config.resolution_codes;
        Self {
            run_id,
            store,
            trigger_map,
            resolution_codes,
        }
    }

    /// Returns a cloned trigger if a complaint should fire for this event.
    /// Returns owned (not reference) to avoid borrow conflicts in update().
    fn should_trigger_complaint(
        &self,
        event: &SimEvent,
        rng: &mut SubsystemRng,
    ) -> Option<ComplaintTrigger> {
        match event {
            SimEvent::FeeCharged { fee_type, .. } => {
                let triggers = self.trigger_map.get("fee_charged")?;
                for trigger in triggers {
                    if let Some(ref ft) = trigger.fee_type {
                        if ft == fee_type && rng.chance(trigger.probability) {
                            return Some(trigger.clone());
                        }
                    }
                }
                None
            }
            SimEvent::SLABreached { .. } => {
                let triggers = self.trigger_map.get("sla_breach")?;
                for trigger in triggers {
                    if trigger.prior_breach && rng.chance(trigger.probability) {
                        return Some(trigger.clone());
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn make_complaint(
        complaint_id: String,
        customer_id: &str,
        account_id: Option<&str>,
        product: &str,
        trigger: &ComplaintTrigger,
        tick: Tick,
    ) -> ComplaintRecord {
        ComplaintRecord {
            complaint_id,
            customer_id: customer_id.to_string(),
            account_id: account_id.map(String::from),
            tick_opened: tick,
            tick_closed: None,
            product: product.to_string(),
            issue: trigger.issue_category.clone(),
            priority: trigger.priority.clone(),
            status: "open".to_string(),
            sla_due_tick: tick + trigger.sla_resolve_days,
            sla_breached: false,
            resolution_code: None,
            amount_refunded: 0.0,
            udaap_flag: trigger.issue_category == "fee_dispute",
        }
    }

    fn process_sla_aging(&self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();
        for complaint in self.store.open_complaints(&self.run_id)? {
            if !complaint.sla_breached && tick >= complaint.sla_due_tick {
                self.store
                    .mark_complaint_sla_breach(&self.run_id, &complaint.complaint_id)?;
                self.store.update_customer_satisfaction(
                    &self.run_id,
                    &complaint.customer_id,
                    -0.15,
                )?;
                events.push(SimEvent::SLABreached {
                    tick,
                    complaint_id: complaint.complaint_id.clone(),
                    customer_id: complaint.customer_id.clone(),
                    days_overdue: (tick.saturating_sub(complaint.sla_due_tick)) as i32,
                });
            }
        }
        Ok(events)
    }

    /// Resolve a complaint via player command. Wired in Phase 1E.
    #[allow(dead_code)]
    fn process_resolution(
        &self,
        complaint_id: &str,
        resolution_code: &str,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let complaint = self.store.get_complaint(&self.run_id, complaint_id)?;
        if complaint.status != "open" {
            log::warn!("Attempted to resolve non-open complaint {complaint_id}");
            return Ok(vec![]);
        }

        let resolution = self
            .resolution_codes
            .get(resolution_code)
            .ok_or_else(|| anyhow::anyhow!("Unknown resolution code: {resolution_code}"))?;

        let refund = resolution.avg_amount_refunded;
        self.store
            .close_complaint(&self.run_id, complaint_id, tick, resolution_code, refund)?;

        self.store.update_customer_satisfaction(
            &self.run_id,
            &complaint.customer_id,
            resolution.satisfaction_delta,
        )?;
        self.store.adjust_customer_churn_risk(
            &self.run_id,
            &complaint.customer_id,
            resolution.churn_risk_delta,
        )?;

        let interaction_id = format!("int-{:016x}", rng.next_u64());
        self.store.insert_interaction(
            &self.run_id,
            &interaction_id,
            &complaint.customer_id,
            tick,
            "system",
            "complaint_resolution",
            Some(complaint_id),
            Some(resolution_code),
            resolution.satisfaction_delta,
        )?;

        Ok(vec![SimEvent::ComplaintResolved {
            tick,
            complaint_id: complaint_id.to_string(),
            customer_id: complaint.customer_id.clone(),
            resolution_code: resolution_code.to_string(),
            satisfaction_delta: resolution.satisfaction_delta,
        }])
    }
}

impl SimSubsystem for ComplaintSubsystem {
    fn name(&self) -> &'static str {
        "complaint"
    }

    fn update(
        &mut self,
        tick: Tick,
        events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        // 1. Generate complaints from triggering events.
        for event in events_in {
            let Some(trigger) = self.should_trigger_complaint(event, rng) else {
                continue;
            };

            let (customer_id, account_id, product) = match event {
                SimEvent::FeeCharged {
                    customer_id,
                    account_id,
                    ..
                } => {
                    let prod = self.store.account_product(&self.run_id, account_id)?;
                    (customer_id.clone(), Some(account_id.clone()), prod)
                }
                SimEvent::SLABreached {
                    customer_id,
                    complaint_id,
                    ..
                } => {
                    let c = self.store.get_complaint(&self.run_id, complaint_id)?;
                    (customer_id.clone(), None, c.product)
                }
                _ => continue,
            };

            let complaint_id = format!("cmp-{tick:08x}-{:016x}", rng.next_u64());
            let complaint = Self::make_complaint(
                complaint_id,
                &customer_id,
                account_id.as_deref(),
                &product,
                &trigger,
                tick,
            );

            self.store.insert_complaint(&self.run_id, &complaint)?;
            self.store
                .update_customer_satisfaction(&self.run_id, &customer_id, -0.03)?;

            out_events.push(SimEvent::ComplaintFiled {
                tick,
                complaint_id: complaint.complaint_id,
                customer_id,
                issue: trigger.issue_category,
                priority: trigger.priority,
            });
        }

        // 2. SLA aging and breach detection.
        out_events.extend(self.process_sla_aging(tick)?);

        // 3. Player resolution commands (wired in Phase 1E; stub here).
        for event in events_in {
            if let SimEvent::PlayerCommandReceived { command_type, .. } = event {
                if command_type == "close_complaint" {
                    // Full wiring happens in Phase 1E with the UI layer.
                }
            }
        }

        // 4. Periodic complaint aggregate (every 7 ticks).
        if tick.is_multiple_of(7) {
            let agg = self.store.compute_complaint_aggregate(&self.run_id, tick)?;
            self.store
                .save_complaint_aggregate(&self.run_id, tick, &agg)?;
            log::debug!(
                "tick={tick} complaint: opened={} closed={} breached={} backlog={}",
                agg.complaints_opened,
                agg.complaints_closed,
                agg.sla_breaches,
                agg.backlog_count,
            );
        }

        Ok(out_events)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
