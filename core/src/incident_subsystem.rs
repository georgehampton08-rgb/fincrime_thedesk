//! Incident & Outage subsystem — operational backbone.
//!
//! This subsystem:
//!   1. Generates incidents based on MTBF for each system component
//!   2. Resolves incidents based on MTTR probability curves
//!   3. Monitors SLA deadlines and flags breaches
//!   4. Applies cascading impacts (stored in incident_impact table)
//!   5. Computes weekly system metrics (uptime%, incident counts)
//!
//! Downstream subsystems (Transaction, Complaint, Recon) read
//! the incident_impact table to adjust their behaviour — this keeps
//! the event-only communication rule intact.
//!
//! Execution: every tick.
//! Depends on: none (generates incidents independently).

// ── Data structs ─────────────────────────────────────────────────────────────

/// Row from the `system_component` table.
#[derive(Debug, Clone)]
pub struct SystemComponentRow {
    pub component_id: String,
    pub label: String,
    pub category: String,
    pub technology_tier: String,
    pub status: String,
    pub mtbf_days: f64,
    pub mttr_hours: f64,
    pub last_incident_tick: Option<Tick>,
    pub upgrade_in_progress: bool,
    pub upgrade_target_tier: Option<String>,
    pub upgrade_complete_tick: Option<Tick>,
}

/// Row from the `incident` table.
#[derive(Debug, Clone)]
pub struct IncidentRow {
    pub incident_id: String,
    pub run_id: String,
    pub component_id: String,
    pub tick_created: Tick,
    pub tick_resolved: Option<Tick>,
    pub severity: String,
    pub status: String,
    pub description: String,
    pub sla_deadline_tick: Tick,
    pub sla_breached: bool,
    pub estimated_revenue_impact: f64,
}

use crate::{
    config::IncidentConfig,
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};

// ── Cascading failure map ────────────────────────────────────────────────────

struct CascadeRule {
    source: &'static str,
    affected: &'static str,
    impact_type: &'static str,
    impact_value: f64,
}

const CASCADE_RULES: &[CascadeRule] = &[
    // payment_hub failures
    CascadeRule { source: "payment_hub", affected: "core_banking",   impact_type: "transaction_failure_rate",    impact_value: 0.80 },
    CascadeRule { source: "payment_hub", affected: "card_processor", impact_type: "transaction_failure_rate",    impact_value: 0.80 },
    CascadeRule { source: "payment_hub", affected: "payment_hub",    impact_type: "complaint_multiplier",        impact_value: 3.0  },
    CascadeRule { source: "payment_hub", affected: "payment_hub",    impact_type: "recon_exception_multiplier",  impact_value: 5.0  },
    // database failures
    CascadeRule { source: "database",    affected: "core_banking",   impact_type: "transaction_failure_rate",    impact_value: 1.0  },
    CascadeRule { source: "database",    affected: "payment_hub",    impact_type: "transaction_failure_rate",    impact_value: 1.0  },
    CascadeRule { source: "database",    affected: "online_banking", impact_type: "transaction_failure_rate",    impact_value: 1.0  },
    CascadeRule { source: "database",    affected: "fraud_engine",   impact_type: "fraud_detection_disabled",    impact_value: 1.0  },
    // fraud_engine failures
    CascadeRule { source: "fraud_engine", affected: "fraud_engine",  impact_type: "fraud_detection_disabled",    impact_value: 1.0  },
    // online_banking failures
    CascadeRule { source: "online_banking", affected: "customer_service", impact_type: "complaint_multiplier",   impact_value: 5.0  },
    // network failures
    CascadeRule { source: "network",     affected: "core_banking",   impact_type: "transaction_failure_rate",    impact_value: 0.50 },
    CascadeRule { source: "network",     affected: "payment_hub",    impact_type: "transaction_failure_rate",    impact_value: 0.50 },
    CascadeRule { source: "network",     affected: "online_banking", impact_type: "transaction_failure_rate",    impact_value: 0.50 },
    CascadeRule { source: "network",     affected: "fraud_engine",   impact_type: "transaction_failure_rate",    impact_value: 0.50 },
];

// ── Incident descriptions ────────────────────────────────────────────────────

fn incident_description(component: &str, severity: &str) -> String {
    match (component, severity) {
        ("core_banking", "P0")     => "Core banking system complete outage".into(),
        ("core_banking", _)        => "Core banking system degraded performance".into(),
        ("payment_hub", "P0")      => "Payment hub total failure — all rails down".into(),
        ("payment_hub", _)         => "Payment hub partial failure — intermittent errors".into(),
        ("card_processor", "P0")   => "Card processor unresponsive — all card txns failing".into(),
        ("card_processor", _)      => "Card processor elevated latency".into(),
        ("fraud_engine", "P0")     => "Fraud engine offline — detection disabled".into(),
        ("fraud_engine", _)        => "Fraud engine scoring delays".into(),
        ("online_banking", "P0")   => "Online banking portal down — customers locked out".into(),
        ("online_banking", _)      => "Online banking intermittent errors".into(),
        ("mobile_banking", "P0")   => "Mobile app crashed — all users affected".into(),
        ("mobile_banking", _)      => "Mobile app slow response times".into(),
        ("network", "P0")          => "Network backbone failure — campus-wide outage".into(),
        ("network", _)             => "Network congestion — elevated packet loss".into(),
        ("data_warehouse", _)      => "Data warehouse query failures".into(),
        ("aml_screening", _)       => "AML screening queue backlog".into(),
        ("customer_service", _)    => "Customer service platform degraded".into(),
        _                          => format!("{component} incident (severity {severity})"),
    }
}

// ── Subsystem ────────────────────────────────────────────────────────────────

pub struct IncidentSubsystem {
    run_id: RunId,
    config: IncidentConfig,
    store: SimStore,
}

impl IncidentSubsystem {
    pub fn new(run_id: RunId, config: IncidentConfig, store: SimStore) -> Self {
        Self { run_id, config, store }
    }

    /// Pick severity from ordered weighted distribution.
    fn pick_severity(&self, rng: &mut SubsystemRng) -> String {
        let roll = rng.next_f64();
        let mut cumulative = 0.0;
        for (sev, weight) in &self.config.severity_weights {
            cumulative += weight;
            if roll < cumulative {
                return sev.clone();
            }
        }
        self.config.severity_weights.last()
            .map(|(s, _)| s.clone())
            .unwrap_or_else(|| "P3".into())
    }

    /// Get SLA deadline in ticks for a severity.
    fn sla_deadline_ticks(&self, severity: &str) -> Tick {
        for (sev, deadline) in &self.config.sla_deadlines {
            if sev == severity {
                return *deadline;
            }
        }
        3 // default P3
    }

    /// Check if new incidents should be generated.
    fn generate_incidents(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        let components = self.store.list_system_components(&self.run_id)?;

        for comp in &components {
            // Skip components already in incident or under upgrade
            if comp.status != "operational" {
                continue;
            }

            // Daily failure probability: 1 - exp(-1/mtbf)
            let daily_prob = 1.0 - (-1.0 / comp.mtbf_days).exp();
            if !rng.chance(daily_prob) {
                continue;
            }

            let severity = self.pick_severity(rng);
            let sla_ticks = self.sla_deadline_ticks(&severity);
            let incident_id = format!("inc-{}-{}-{tick}", comp.component_id, rng.next_u64() % 100000);
            let description = incident_description(&comp.component_id, &severity);

            // Determine new component status
            let new_status = if severity == "P0" || severity == "P1" {
                "down"
            } else {
                "degraded"
            };

            self.store.insert_incident(
                &self.run_id,
                &incident_id,
                &comp.component_id,
                tick,
                &severity,
                &description,
                tick + sla_ticks,
            )?;

            self.store.update_component_status(
                &comp.component_id,
                new_status,
                tick,
            )?;

            events.push(SimEvent::IncidentCreated {
                tick,
                incident_id: incident_id.clone(),
                component: comp.component_id.clone(),
                severity: severity.clone(),
                description: description.clone(),
            });

            events.push(SimEvent::ComponentStatusChanged {
                tick,
                component_id: comp.component_id.clone(),
                old_status: "operational".into(),
                new_status: new_status.into(),
                reason: format!("Incident {incident_id}"),
            });

            log::info!(
                "tick={tick} incident: {} on {} — {}",
                severity, comp.component_id, description
            );
        }

        Ok(events)
    }

    /// Process ongoing incidents: resolve or SLA-check.
    fn process_ongoing(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();
        let open = self.store.get_open_incidents(&self.run_id)?;

        for inc in &open {
            let elapsed = tick.saturating_sub(inc.tick_created);

            // Resolution probability increases with time: P = 1 - exp(-elapsed / mttr)
            let comp = self.store.get_system_component(&inc.component_id)?;
            // mttr_hours → ticks (1 tick = 1 day = 24 hours)
            let mttr_ticks = comp.mttr_hours / 24.0;
            let resolve_prob = if mttr_ticks > 0.0 {
                1.0 - (-(elapsed as f64) / mttr_ticks).exp()
            } else {
                1.0
            };

            if rng.chance(resolve_prob) {
                // Resolve
                self.store.resolve_incident(
                    &self.run_id,
                    &inc.incident_id,
                    tick,
                )?;

                self.store.update_component_status(
                    &inc.component_id,
                    "operational",
                    tick,
                )?;

                events.push(SimEvent::IncidentResolved {
                    tick,
                    incident_id: inc.incident_id.clone(),
                    component: inc.component_id.clone(),
                    duration_ticks: elapsed,
                });

                events.push(SimEvent::ComponentStatusChanged {
                    tick,
                    component_id: inc.component_id.clone(),
                    old_status: comp.status.clone(),
                    new_status: "operational".into(),
                    reason: format!("Incident {} resolved", inc.incident_id),
                });

                log::info!(
                    "tick={tick} incident: {} resolved after {elapsed} ticks",
                    inc.incident_id
                );
            } else {
                // Check SLA breach
                if !inc.sla_breached && tick > inc.sla_deadline_tick {
                    let ticks_overdue = tick - inc.sla_deadline_tick;
                    self.store.mark_incident_sla_breached(
                        &self.run_id,
                        &inc.incident_id,
                    )?;

                    events.push(SimEvent::IncidentSLABreach {
                        tick,
                        incident_id: inc.incident_id.clone(),
                        severity: inc.severity.clone(),
                        ticks_overdue,
                    });

                    log::warn!(
                        "tick={tick} incident SLA BREACH: {} ({}) overdue by {ticks_overdue} ticks",
                        inc.incident_id, inc.severity
                    );
                }
            }
        }

        Ok(events)
    }

    /// Write cascading impact rows for active P0/P1 incidents.
    fn apply_cascading_impacts(&self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        if !self.config.cascading_failures_enabled {
            return Ok(events);
        }

        let active = self.store.get_active_incidents(&self.run_id)?;

        for inc in &active {
            // Only P0 and P1 cascade
            if inc.severity != "P0" && inc.severity != "P1" {
                continue;
            }

            for rule in CASCADE_RULES {
                if rule.source != inc.component_id {
                    continue;
                }

                self.store.insert_incident_impact(
                    &self.run_id,
                    &inc.incident_id,
                    tick,
                    rule.impact_type,
                    rule.affected,
                    rule.impact_value,
                )?;

                events.push(SimEvent::CascadingImpactApplied {
                    tick,
                    incident_id: inc.incident_id.clone(),
                    impact_type: rule.impact_type.into(),
                    affected_component: rule.affected.into(),
                    impact_value: rule.impact_value,
                });
            }
        }

        Ok(events)
    }

    /// Compute weekly system metrics per component.
    fn compute_metrics(&self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();
        let components = self.store.list_system_components(&self.run_id)?;
        let window = 30; // 30-day rolling window

        for comp in &components {
            let (uptime_pct, incident_count, avg_mttr, sla_breaches) =
                self.store.compute_component_uptime(
                    &self.run_id,
                    &comp.component_id,
                    tick.saturating_sub(window),
                    tick,
                )?;

            self.store.insert_system_metrics(
                &self.run_id,
                tick,
                &comp.component_id,
                uptime_pct,
                incident_count,
                avg_mttr,
                sla_breaches,
            )?;

            events.push(SimEvent::SystemMetricsComputed {
                tick,
                component_id: comp.component_id.clone(),
                uptime_pct_30d: uptime_pct,
                total_incidents_30d: incident_count as i64,
            });
        }

        Ok(events)
    }
}

impl SimSubsystem for IncidentSubsystem {
    fn name(&self) -> &'static str {
        "incident"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out = Vec::new();

        if tick == 0 || !self.config.enabled {
            return Ok(out);
        }

        // 1. Generate new incidents
        out.extend(self.generate_incidents(tick, rng)?);

        // 2. Process ongoing incidents (resolve or SLA check)
        out.extend(self.process_ongoing(tick, rng)?);

        // 3. Apply cascading impacts for active P0/P1 incidents
        out.extend(self.apply_cascading_impacts(tick)?);

        // 4. Weekly system metrics
        if tick.is_multiple_of(self.config.metrics_interval_ticks) {
            out.extend(self.compute_metrics(tick)?);
        }

        Ok(out)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
