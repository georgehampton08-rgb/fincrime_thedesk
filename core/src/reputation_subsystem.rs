//! Reputation Management subsystem — Phase 3.6.
//!
//! Maintains a composite daily reputation score [0.0, 100.0] that:
//!   - Decays every tick when negative signal events occur (SLA breaches,
//!     SAR late filings, regulatory fines, MOUs).
//!   - Recovers passively at a configurable rate when the score is below 80.
//!
//! The score gates onboarding in CustomerSubsystem (future hook) and is
//! surfaced to the UI via the event stream for the PlayerKPI panel.
//!
//! Execution: every tick, last in registration order (reads all
//!   signals from the current tick's event list).
//! Depends on: reads events_in for MOUReceived, RegulatoryExamClosed,
//!   SLABreached, IncidentSLABreach, SARLateFiling.

use crate::{
    config::ReputationConfig,
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};

pub struct ReputationSubsystem {
    run_id: RunId,
    config: ReputationConfig,
    store:  SimStore,
}

impl ReputationSubsystem {
    pub fn new(run_id: RunId, config: ReputationConfig, store: SimStore) -> Self {
        Self { run_id, config, store }
    }

    /// Scan this tick's events and compute the total reputation delta.
    fn compute_delta(
        &self,
        tick:      Tick,
        events_in: &[SimEvent],
    ) -> SimResult<(f64, Vec<(String, f64, String)>)> {
        let mut total_delta = 0.0f64;
        let mut drivers: Vec<(String, f64, String)> = Vec::new(); // (driver, delta, desc)

        for event in events_in {
            match event {
                // Complaint SLA breach
                SimEvent::SLABreached { complaint_id, .. } => {
                    let d = -self.config.sla_breach_impact;
                    total_delta += d;
                    drivers.push((
                        "sla_breach".into(), d,
                        format!("Complaint SLA breach: {complaint_id}"),
                    ));
                }

                // Incident SLA breach
                SimEvent::IncidentSLABreach { incident_id, .. } => {
                    let d = -self.config.sla_breach_impact;
                    total_delta += d;
                    drivers.push((
                        "sla_breach".into(), d,
                        format!("Incident SLA breach: {incident_id}"),
                    ));
                }

                // SAR late filing
                SimEvent::SARLateFiling { sar_id, .. } => {
                    let d = -self.config.sar_late_impact;
                    total_delta += d;
                    drivers.push((
                        "sar_late".into(), d,
                        format!("SAR late filing: {sar_id}"),
                    ));
                }

                // MOU received
                SimEvent::MOUReceived { exam_id, examiner, .. } => {
                    let d = -self.config.mou_impact;
                    total_delta += d;
                    drivers.push((
                        "mou".into(), d,
                        format!("MOU issued by {examiner} (exam {exam_id})"),
                    ));
                }

                // Regulatory exam closed (fine-based impact even without MOU)
                SimEvent::RegulatoryExamClosed { fine_total, mou_issued, .. } => {
                    if !mou_issued && *fine_total > 0.0 {
                        let d = -(fine_total / 1000.0) * self.config.fine_impact_per_1k;
                        total_delta += d;
                        drivers.push((
                            "exam_fine".into(), d,
                            format!("Regulatory fine: ${fine_total:.0}"),
                        ));
                    }
                }

                _ => {}
            }
        }

        // Passive recovery: only when score < 80
        let current = self.store.latest_reputation_score(&self.run_id)?;
        if current < 80.0 {
            let recovery = self.config.recovery_per_tick;
            total_delta += recovery;
            if recovery > 0.01 {
                drivers.push(("recovery".into(), recovery, format!("tick={tick} passive recovery")));
            }
        }

        Ok((total_delta, drivers))
    }
}

impl SimSubsystem for ReputationSubsystem {
    fn name(&self) -> &'static str {
        "reputation"
    }

    fn update(
        &mut self,
        tick:       Tick,
        events_in:  &[SimEvent],
        _rng:       &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out = Vec::new();

        if !self.config.enabled {
            return Ok(out);
        }

        // Tick 0: seed the initial score snapshot.
        if tick == 0 {
            self.store.insert_reputation_snapshot(
                &self.run_id, 0, self.config.initial_score, 0.0,
            )?;
            return Ok(out);
        }

        let (total_delta, drivers) = self.compute_delta(tick, events_in)?;

        // Apply delta and clamp.
        let prev = self.store.latest_reputation_score(&self.run_id)?;
        let new_score = (prev + total_delta).clamp(0.0, 100.0);
        let actual_delta = new_score - prev;

        // Persist the snapshot.
        self.store.insert_reputation_snapshot(&self.run_id, tick, new_score, actual_delta)?;

        // Persist individual driver events.
        for (driver, delta, desc) in &drivers {
            self.store.insert_reputation_event(&self.run_id, tick, driver, *delta, desc)?;
        }

        // Determine primary driver for the event payload.
        let primary_driver = drivers
            .iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(d, _, _)| d.clone())
            .unwrap_or_else(|| "recovery".into());

        out.push(SimEvent::ReputationUpdated {
            tick,
            score: new_score,
            delta: actual_delta,
            primary_driver,
        });

        if actual_delta < -2.0 {
            log::warn!(
                "tick={tick} reputation drop: {prev:.1} -> {new_score:.1} ({actual_delta:+.2})"
            );
        } else {
            log::debug!(
                "tick={tick} reputation: {new_score:.1} ({actual_delta:+.2})"
            );
        }

        Ok(out)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
