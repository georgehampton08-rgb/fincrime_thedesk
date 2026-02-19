//! Reconciliation subsystem — compares internal ledger entries against
//! external statements, manages exceptions, and applies exam pressure.
//!
//! Execution order: After PaymentHubSubsystem (slot 14).
//! Runs on day N+1 to reconcile day N's settled items.
//!
//! Design:
//!   - Internal total = sum(ledger_entry.amount) for rail/tick
//!   - External total = external_statement.total_debits + total_credits for rail/tick
//!   - Delta > tolerance → create recon_exception
//!   - Auto-clear on day 2+ if delta < auto_clear_threshold AND cause = 'timing'
//!   - SLA breach at sla_days, escalation at escalation_age_days
//!   - Write-offs drive UDAAP penalty via regulatory_score_component

use crate::{
    config::ReconciliationConfig,
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::{ReconExceptionRow, ReconMetricsRow, SimStore},
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};

pub struct ReconciliationSubsystem {
    run_id: RunId,
    config: ReconciliationConfig,
    store: SimStore,
}

impl ReconciliationSubsystem {
    pub fn new(run_id: RunId, config: ReconciliationConfig, store: SimStore) -> Self {
        Self {
            run_id,
            config,
            store,
        }
    }

    // ── Per-rail reconciliation ────────────────────────────────────

    /// Reconcile a single rail for yesterday's tick.
    /// Compares internal ledger total vs external statement total.
    /// Returns events emitted (exception created or nothing).
    fn reconcile_rail(
        &self,
        rail_id: &str,
        settle_tick: Tick,
        current_tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Fetch external statement for yesterday
        let stmt = self
            .store
            .get_external_statement_for_tick(&self.run_id, rail_id, settle_tick);

        // Spec: if no external statement exists, skip quietly (normal for new/zero-volume rails)
        let stmt = match stmt {
            Ok(Some(s)) => s,
            Ok(None) | Err(_) => return Ok(events),
        };

        // Internal: sum of all ledger_entry rows for this rail/tick
        let internal_total =
            self.store
                .sum_settled_for_rail(&self.run_id, rail_id, settle_tick)?;

        // External: total from the external statement
        let external_total = stmt.total_debits + stmt.total_credits;

        let delta = (internal_total - external_total).abs();

        // Load config for this rail (falls back to defaults if not seeded)
        let cfg = self.store.get_recon_queue_config(rail_id)?;

        // Delta within tolerance → no exception needed
        if delta <= cfg.tolerance_amount {
            return Ok(events);
        }

        // Generate deterministic exception ID: tick + rail + rng sequence
        let seq = rng.next_u64_below(1_000_000_000);
        let exception_id = format!("RECON-{current_tick}-{rail_id}-{seq}");

        let cause = self.infer_cause(delta, &cfg);

        let ex = ReconExceptionRow {
            exception_id: exception_id.clone(),
            run_id: self.run_id.clone(),
            rail_id: rail_id.to_string(),
            tick_detected: current_tick,
            tick_resolved: None,
            status: "open".into(),
            delta_amount: delta,
            internal_total,
            external_total,
            item_count_delta: None,
            suspected_cause: Some(cause),
            assigned_to: None,
            resolution_notes: None,
            resolution_type: None,
            write_off_amount: 0.0,
        };

        self.store.insert_recon_exception(&ex)?;

        events.push(SimEvent::ReconExceptionCreated {
            tick: current_tick,
            exception_id,
            rail_id: rail_id.to_string(),
            delta_amount: delta,
        });

        Ok(events)
    }

    /// Heuristic for suspected cause based on delta characteristics.
    fn infer_cause(&self, delta: f64, cfg: &crate::store::ReconQueueConfigRow) -> String {
        if delta < cfg.auto_clear_threshold {
            // Small delta → likely a timing difference (T+0 vs T+1 statement cut)
            "timing".into()
        } else if (delta - delta.round()).abs() < 0.001 {
            // Whole-dollar delta → likely a missing item
            "missing_item".into()
        } else {
            "unknown".into()
        }
    }

    // ── Aging and lifecycle management ────────────────────────────

    /// Process all open exceptions: emit SLA breach / escalation events as needed.
    fn process_aging_exceptions(
        &self,
        current_tick: Tick,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        let open = self.store.get_open_recon_exceptions(&self.run_id)?;

        for ex in &open {
            let age_days = (current_tick as i64) - (ex.tick_detected as i64);
            let cfg = self.store.get_recon_queue_config(&ex.rail_id)?;

            // SLA breach
            if age_days >= cfg.sla_days && ex.status == "open" {
                self.store.update_recon_exception_status(
                    &self.run_id,
                    &ex.exception_id,
                    "investigating",
                )?;
                events.push(SimEvent::ReconExceptionSLABreach {
                    tick: current_tick,
                    exception_id: ex.exception_id.clone(),
                    age_days: age_days as Tick,
                });
            }

            // Escalation (by age or amount)
            let should_escalate_by_age = age_days >= cfg.escalation_age_days;
            let should_escalate_by_amount = ex.delta_amount >= cfg.escalation_threshold;

            if should_escalate_by_age || should_escalate_by_amount {
                let reason = if should_escalate_by_age && should_escalate_by_amount {
                    "age_and_amount".into()
                } else if should_escalate_by_age {
                    "age".into()
                } else {
                    "amount".into()
                };

                events.push(SimEvent::ReconExceptionEscalated {
                    tick: current_tick,
                    exception_id: ex.exception_id.clone(),
                    reason,
                });
            }
        }

        Ok(events)
    }

    /// Auto-clear timing exceptions after at least 1 day (per spec).
    fn auto_clear_exceptions(&self, current_tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        if !self.config.enable_auto_clear {
            return Ok(events);
        }

        let open = self.store.get_open_recon_exceptions(&self.run_id)?;

        for ex in &open {
            let age_days = (current_tick as i64) - (ex.tick_detected as i64);
            let cfg = self.store.get_recon_queue_config(&ex.rail_id)?;

            let is_timing = ex
                .suspected_cause
                .as_deref()
                .map_or(false, |c| c == "timing");
            let matured = age_days >= 1; // Must wait at least 1 day per spec
            let within_threshold = ex.delta_amount < cfg.auto_clear_threshold;

            if is_timing && matured && within_threshold {
                self.store.resolve_recon_exception(
                    &self.run_id,
                    &ex.exception_id,
                    current_tick,
                    "auto_clear",
                    "Auto-cleared: timing difference resolved by next-day statement",
                    0.0,
                )?;

                events.push(SimEvent::ReconExceptionAutoCleared {
                    tick: current_tick,
                    exception_id: ex.exception_id.clone(),
                    delta_amount: ex.delta_amount,
                });

                events.push(SimEvent::ReconExceptionResolved {
                    tick: current_tick,
                    exception_id: ex.exception_id.clone(),
                    resolution_type: "auto_clear".into(),
                    write_off_amount: 0.0,
                });
            }
        }

        Ok(events)
    }

    // ── Weekly metrics snapshot ────────────────────────────────────

    fn compute_recon_metrics(&self, current_tick: Tick) -> SimResult<()> {
        let rail_ids = ["ACH", "wire", "RTP", "card"];

        for rail_id in &rail_ids {
            let all = self
                .store
                .get_recon_exceptions_by_rail(&self.run_id, rail_id)?;

            if all.is_empty() {
                continue;
            }

            let total_exceptions = all.len() as i64;
            let open_exceptions = all
                .iter()
                .filter(|e| e.status == "open" || e.status == "investigating")
                .count() as i64;
            let aged_7d = all
                .iter()
                .filter(|e| {
                    (e.status == "open" || e.status == "investigating")
                        && (current_tick as i64) - (e.tick_detected as i64) >= 7
                })
                .count() as i64;
            let aged_14d = all
                .iter()
                .filter(|e| {
                    (e.status == "open" || e.status == "investigating")
                        && (current_tick as i64) - (e.tick_detected as i64) >= 14
                })
                .count() as i64;
            let aged_30d = all
                .iter()
                .filter(|e| {
                    (e.status == "open" || e.status == "investigating")
                        && (current_tick as i64) - (e.tick_detected as i64) >= 30
                })
                .count() as i64;
            let total_delta: f64 = all.iter().map(|e| e.delta_amount).sum();
            let unresolved: f64 = all
                .iter()
                .filter(|e| e.status == "open" || e.status == "investigating")
                .map(|e| e.delta_amount)
                .sum();
            let write_off: f64 = all.iter().map(|e| e.write_off_amount).sum();
            let auto_cleared = all
                .iter()
                .filter(|e| e.resolution_type.as_deref() == Some("auto_clear"))
                .count() as i64;
            let manually_resolved = all
                .iter()
                .filter(|e| e.resolution_type.as_deref() == Some("manual_adjustment"))
                .count() as i64;
            let written_off = all
                .iter()
                .filter(|e| e.status == "written_off")
                .count() as i64;

            let row = ReconMetricsRow {
                run_id: self.run_id.clone(),
                tick: current_tick,
                rail_id: rail_id.to_string(),
                total_exceptions,
                open_exceptions,
                aged_exceptions_7d: aged_7d,
                aged_exceptions_14d: aged_14d,
                aged_exceptions_30d: aged_30d,
                total_delta_amount: total_delta,
                unresolved_amount: unresolved,
                write_off_amount: write_off,
                auto_cleared,
                manually_resolved,
                written_off,
                avg_resolution_days: None,
                sla_compliance_pct: None,
            };

            self.store.insert_recon_metrics(&row)?;
        }

        Ok(())
    }

    // ── Regulatory exam pressure ───────────────────────────────────

    /// Apply a UDAAP regulatory penalty when exceptions are aged or write-offs are high.
    fn apply_recon_exam_pressure(&self, tick: Tick) -> SimResult<()> {
        let aged_exceptions = self
            .store
            .count_exceptions_aged_over(&self.run_id, 30)?;
        // Sum write-offs over the rolling year
        let start_tick = tick.saturating_sub(365);
        let write_offs = self
            .store
            .sum_write_offs(&self.run_id, start_tick, tick)?;

        if aged_exceptions > 50 {
            self.store.insert_regulatory_score_component(
                &self.run_id,
                tick,
                "recon_controls",
                -20.0,
                &format!("{aged_exceptions} exceptions aged >30 days"),
            )?;
        }

        if write_offs > 10_000.0 {
            self.store.insert_regulatory_score_component(
                &self.run_id,
                tick,
                "recon_write_offs",
                -15.0,
                &format!("${write_offs:.2} in write-offs"),
            )?;
        }

        Ok(())
    }
}

impl SimSubsystem for ReconciliationSubsystem {
    fn name(&self) -> &'static str {
        "reconciliation"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        // Ticks 0 and 1 have no prior day to reconcile
        if tick < 2 {
            return Ok(out_events);
        }

        // Reconcile day N-1 (yesterday's settlements)
        let settle_tick = tick - 1;
        let rail_ids = ["ACH", "wire", "RTP", "card"];

        for rail_id in &rail_ids {
            out_events.extend(self.reconcile_rail(rail_id, settle_tick, tick, rng)?);
        }

        // Auto-clear timing exceptions from previous days
        out_events.extend(self.auto_clear_exceptions(tick)?);

        // Process aging: SLA breaches and escalations
        out_events.extend(self.process_aging_exceptions(tick)?);

        // Compute weekly metrics snapshot
        if tick as i64 % self.config.metrics_frequency_ticks == 0 {
            self.compute_recon_metrics(tick)?;
        }

        // Regulatory exam pressure (runs every day, no-op unless thresholds hit)
        self.apply_recon_exam_pressure(tick)?;

        log::debug!(
            "tick={tick} reconciliation: {} events",
            out_events.len()
        );

        Ok(out_events)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
