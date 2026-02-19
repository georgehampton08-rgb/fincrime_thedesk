//! Complaint analytics subsystem — pattern detection and early warnings.
//!
//! This subsystem:
//!   1. Detects complaint clusters and velocity spikes
//!   2. Attributes root causes (fees, life events, products)
//!   3. Measures resolution effectiveness (monthly)
//!   4. Tracks SLA performance and aging
//!   5. Identifies repeat complainers
//!   6. Fires early warning alerts for leading indicators
//!
//! Execution: every 7 ticks.
//! Depends on: complaint, customer, transaction, churn subsystems.

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
pub struct ComplaintPattern {
    pub pattern_type: String,
    pub issue_category: String,
    pub segment: Option<String>,
    pub affected_count: i64,
    pub window_start_tick: Tick,
    pub window_end_tick: Tick,
    pub velocity_ratio: f64,
    pub concentration_pct: f64,
    pub severity_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplaintRootCause {
    pub complaint_id: String,
    pub root_cause_type: String,
    pub root_cause_id: Option<String>,
    pub confidence_score: f64,
    pub correlation_lag_ticks: Tick,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SLAPerformanceSnapshot {
    pub priority: String,
    pub aging_0_3_days: i64,
    pub aging_4_7_days: i64,
    pub aging_8_14_days: i64,
    pub aging_15_30_days: i64,
    pub aging_30_plus_days: i64,
    pub total_open: i64,
    pub at_risk_count: i64,
    pub breach_count: i64,
    pub breach_rate: f64,
    pub avg_age_ticks: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyWarningAlert {
    pub alert_type: String,
    pub severity: String,
    pub segment: Option<String>,
    pub metric_name: String,
    pub current_value: f64,
    pub threshold_value: f64,
    pub delta_pct: f64,
}

// ── Subsystem ────────────────────────────────────────────────────────────────

pub struct ComplaintAnalyticsSubsystem {
    run_id: RunId,
    config: SimConfig,
    store: SimStore,
}

impl ComplaintAnalyticsSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        Self {
            run_id,
            config,
            store,
        }
    }

    fn detect_patterns(&self, tick: Tick) -> SimResult<Vec<ComplaintPattern>> {
        let mut patterns = Vec::new();
        let pattern_config = &self.config.complaint_analytics.pattern_detection;
        let window = pattern_config.clustering_window_ticks;

        // Pattern 1: Velocity spikes by issue category
        let mut categories: Vec<&String> = pattern_config.issue_categories.keys().collect();
        categories.sort();
        for category in categories {
            let recent_count = self.store.complaint_count_by_category(
                &self.run_id,
                category,
                tick.saturating_sub(window),
                tick,
            )?;

            let prior_count = self.store.complaint_count_by_category(
                &self.run_id,
                category,
                tick.saturating_sub(window * 2),
                tick.saturating_sub(window),
            )?;

            if prior_count > 0 {
                let velocity_ratio = recent_count as f64 / prior_count as f64;

                if velocity_ratio >= pattern_config.velocity_spike_threshold
                    && recent_count >= pattern_config.cluster_threshold_count as i64
                {
                    let total_complaints = self.store.total_complaints_in_window(
                        &self.run_id,
                        tick.saturating_sub(window),
                        tick,
                    )?;

                    let concentration_pct = if total_complaints > 0 {
                        (recent_count as f64 / total_complaints as f64) * 100.0
                    } else {
                        0.0
                    };

                    let severity_score = velocity_ratio * (concentration_pct / 100.0);

                    patterns.push(ComplaintPattern {
                        pattern_type: "velocity_spike".into(),
                        issue_category: category.clone(),
                        segment: None,
                        affected_count: recent_count,
                        window_start_tick: tick.saturating_sub(window),
                        window_end_tick: tick,
                        velocity_ratio,
                        concentration_pct,
                        severity_score,
                    });
                }
            }
        }

        // Pattern 2: Segment concentration
        let mut seg_keys: Vec<&String> = self.config.segments.keys().collect();
        seg_keys.sort();
        for segment in seg_keys {
            let segment_complaints = self.store.complaint_count_by_segment(
                &self.run_id,
                segment,
                tick.saturating_sub(window),
                tick,
            )?;

            let total_complaints = self.store.total_complaints_in_window(
                &self.run_id,
                tick.saturating_sub(window),
                tick,
            )?;

            if total_complaints > 0 {
                let concentration = segment_complaints as f64 / total_complaints as f64;

                if concentration
                    >= self
                        .config
                        .complaint_analytics
                        .early_warning_indicators
                        .segment_concentration_warning
                {
                    patterns.push(ComplaintPattern {
                        pattern_type: "segment_concentration".into(),
                        issue_category: "all".into(),
                        segment: Some(segment.clone()),
                        affected_count: segment_complaints,
                        window_start_tick: tick.saturating_sub(window),
                        window_end_tick: tick,
                        velocity_ratio: 1.0,
                        concentration_pct: concentration * 100.0,
                        severity_score: concentration,
                    });
                }
            }
        }

        Ok(patterns)
    }

    fn identify_root_causes(&self, tick: Tick) -> SimResult<Vec<ComplaintRootCause>> {
        let mut root_causes = Vec::new();
        let rc_config = &self.config.complaint_analytics.root_cause_tracking;

        let recent_complaints =
            self.store
                .recent_complaints(&self.run_id, tick.saturating_sub(7), tick)?;

        for complaint in recent_complaints {
            // Check for fee-related root cause
            if complaint.issue.contains("fee") {
                let recent_fee = self.store.customer_recent_fee(
                    &self.run_id,
                    &complaint.customer_id,
                    complaint
                        .tick_opened
                        .saturating_sub(rc_config.fee_complaint_correlation_window),
                    complaint.tick_opened,
                )?;

                if let Some((fee_type, fee_tick)) = recent_fee {
                    let lag = complaint.tick_opened.saturating_sub(fee_tick);
                    let confidence = if lag <= 3 { 0.95 } else { 0.75 };

                    if confidence >= rc_config.attribution_confidence_threshold {
                        root_causes.push(ComplaintRootCause {
                            complaint_id: complaint.complaint_id.clone(),
                            root_cause_type: "fee_event".into(),
                            root_cause_id: Some(fee_type),
                            confidence_score: confidence,
                            correlation_lag_ticks: lag,
                        });
                    }
                }
            }

            // Check for life event correlation
            let life_event = self.store.customer_active_life_event(
                &self.run_id,
                &complaint.customer_id,
                complaint.tick_opened,
            )?;

            if let Some(event_type) = life_event {
                root_causes.push(ComplaintRootCause {
                    complaint_id: complaint.complaint_id.clone(),
                    root_cause_type: "life_event".into(),
                    root_cause_id: Some(event_type),
                    confidence_score: 0.80,
                    correlation_lag_ticks: 0,
                });
            }
        }

        Ok(root_causes)
    }

    fn measure_resolution_effectiveness(&self, tick: Tick) -> SimResult<()> {
        let measurement_window = self
            .config
            .complaint_analytics
            .resolution_effectiveness
            .effectiveness_measurement_window;

        let mut res_keys: Vec<&String> = self.config.resolution_codes.keys().collect();
        res_keys.sort();
        for resolution_code in res_keys {
            let resolved = self.store.complaints_resolved_with_code(
                &self.run_id,
                resolution_code,
                tick.saturating_sub(measurement_window),
                tick,
            )?;

            if resolved.len() < 5 {
                continue; // Need minimum sample size
            }

            let mut total_sat_delta = 0.0f64;
            let mut total_churn_delta = 0.0f64;
            let mut repeat_complaints = 0i64;

            for complaint_id in &resolved {
                if let Ok(deltas) = self
                    .store
                    .complaint_impact_deltas(&self.run_id, complaint_id)
                {
                    total_sat_delta += deltas.satisfaction_delta;
                    total_churn_delta += deltas.churn_risk_delta;
                    if deltas.had_repeat_complaint {
                        repeat_complaints += 1;
                    }
                }
            }

            let n = resolved.len() as f64;
            let avg_sat_delta = total_sat_delta / n;
            let avg_churn_delta = total_churn_delta / n;
            let repeat_rate = repeat_complaints as f64 / n;

            self.store.insert_resolution_effectiveness(
                &self.run_id,
                resolution_code,
                tick,
                avg_sat_delta,
                avg_churn_delta,
                repeat_rate,
                0.0, // escalation_rate (Phase 3)
                resolved.len() as i64,
            )?;
        }

        Ok(())
    }

    fn compute_sla_performance(&self, tick: Tick) -> SimResult<Vec<SLAPerformanceSnapshot>> {
        let mut snapshots = Vec::new();
        let priorities = ["low", "standard", "high", "urgent"];

        for priority in priorities {
            let open_complaints = self
                .store
                .open_complaints_by_priority(&self.run_id, priority)?;

            let mut aging_0_3: i64 = 0;
            let mut aging_4_7: i64 = 0;
            let mut aging_8_14: i64 = 0;
            let mut aging_15_30: i64 = 0;
            let mut aging_30_plus: i64 = 0;
            let mut total_age: u64 = 0;
            let mut at_risk: i64 = 0;
            let mut breached: i64 = 0;

            for complaint in &open_complaints {
                let age = tick.saturating_sub(complaint.tick_opened);
                total_age += age;

                match age {
                    0..=3 => aging_0_3 += 1,
                    4..=7 => aging_4_7 += 1,
                    8..=14 => aging_8_14 += 1,
                    15..=30 => aging_15_30 += 1,
                    _ => aging_30_plus += 1,
                }

                if complaint.sla_breached {
                    breached += 1;
                } else if complaint.sla_due_tick > 0
                    && age as f64 > complaint.sla_due_tick as f64 * 0.80
                {
                    at_risk += 1;
                }
            }

            let total = open_complaints.len() as i64;
            let breach_rate = if total > 0 {
                breached as f64 / total as f64
            } else {
                0.0
            };
            let avg_age = if total > 0 {
                total_age as f64 / total as f64
            } else {
                0.0
            };

            snapshots.push(SLAPerformanceSnapshot {
                priority: priority.to_string(),
                aging_0_3_days: aging_0_3,
                aging_4_7_days: aging_4_7,
                aging_8_14_days: aging_8_14,
                aging_15_30_days: aging_15_30,
                aging_30_plus_days: aging_30_plus,
                total_open: total,
                at_risk_count: at_risk,
                breach_count: breached,
                breach_rate,
                avg_age_ticks: avg_age,
            });
        }

        Ok(snapshots)
    }

    fn identify_repeat_complainers(&self, tick: Tick) -> SimResult<()> {
        let threshold = self
            .config
            .complaint_analytics
            .early_warning_indicators
            .repeat_complainer_threshold;

        let repeat_complainers = self
            .store
            .customers_with_complaint_count_gte(&self.run_id, threshold as i64)?;

        for (customer_id, complaint_count) in repeat_complainers {
            let unresolved = self
                .store
                .customer_unresolved_complaints(&self.run_id, &customer_id)?;
            let breached = self
                .store
                .customer_breached_complaints(&self.run_id, &customer_id)?;
            let churn_risk = self
                .store
                .customer_latest_churn_risk(&self.run_id, &customer_id)?;

            let regulatory_risk = complaint_count >= 5 || breached >= 2;

            self.store.insert_repeat_complainer(
                &self.run_id,
                &customer_id,
                tick,
                complaint_count,
                unresolved,
                breached,
                churn_risk,
                regulatory_risk,
            )?;
        }

        Ok(())
    }

    fn fire_early_warnings(&self, tick: Tick) -> SimResult<Vec<EarlyWarningAlert>> {
        let mut alerts = Vec::new();
        let ew_config = &self.config.complaint_analytics.early_warning_indicators;

        // Warning 1: Breach rate above threshold per segment
        let mut warning_seg_keys: Vec<&String> = self.config.segments.keys().collect();
        warning_seg_keys.sort();
        for segment in warning_seg_keys {
            let breach_rate = self.store.segment_breach_rate(
                &self.run_id,
                segment,
                tick.saturating_sub(30),
                tick,
            )?;

            if breach_rate >= ew_config.breach_rate_warning_threshold {
                alerts.push(EarlyWarningAlert {
                    alert_type: "breach_rate_elevated".into(),
                    severity: if breach_rate >= 0.25 {
                        "high"
                    } else {
                        "medium"
                    }
                    .into(),
                    segment: Some(segment.clone()),
                    metric_name: "sla_breach_rate".into(),
                    current_value: breach_rate,
                    threshold_value: ew_config.breach_rate_warning_threshold,
                    delta_pct: ((breach_rate - ew_config.breach_rate_warning_threshold)
                        / ew_config.breach_rate_warning_threshold)
                        * 100.0,
                });
            }
        }

        // Warning 2: Velocity spikes (reuse detect_patterns output)
        let patterns = self.detect_patterns(tick)?;
        for pattern in patterns {
            if pattern.severity_score >= 1.5 {
                alerts.push(EarlyWarningAlert {
                    alert_type: "complaint_velocity_spike".into(),
                    severity: if pattern.severity_score >= 2.5 {
                        "high"
                    } else {
                        "medium"
                    }
                    .into(),
                    segment: pattern.segment,
                    metric_name: format!("velocity_{}", pattern.issue_category),
                    current_value: pattern.velocity_ratio,
                    threshold_value: self
                        .config
                        .complaint_analytics
                        .pattern_detection
                        .velocity_spike_threshold,
                    delta_pct: (pattern.velocity_ratio - 1.0) * 100.0,
                });
            }
        }

        Ok(alerts)
    }
}

impl SimSubsystem for ComplaintAnalyticsSubsystem {
    fn name(&self) -> &'static str {
        "complaint_analytics"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        _rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        if !tick.is_multiple_of(7) || tick == 0 {
            return Ok(out_events);
        }

        // 1. Detect patterns and persist
        let patterns = self.detect_patterns(tick)?;
        for pattern in patterns {
            self.store
                .insert_complaint_pattern(&self.run_id, tick, &pattern)?;
            log::info!(
                "tick={tick} analytics: detected {} in {} (severity={:.2})",
                pattern.pattern_type,
                pattern.issue_category,
                pattern.severity_score,
            );
        }

        // 2. Root cause attribution
        let root_causes = self.identify_root_causes(tick)?;
        for rc in root_causes {
            self.store
                .insert_complaint_root_cause(&self.run_id, tick, &rc)?;
        }

        // 3. Resolution effectiveness (monthly)
        if tick.is_multiple_of(30) {
            self.measure_resolution_effectiveness(tick)?;
        }

        // 4. SLA performance snapshots
        let sla_snapshots = self.compute_sla_performance(tick)?;
        for snapshot in sla_snapshots {
            self.store
                .insert_sla_performance(&self.run_id, tick, &snapshot)?;
        }

        // 5. Repeat complainer identification
        self.identify_repeat_complainers(tick)?;

        // 6. Early warning alerts
        let alerts = self.fire_early_warnings(tick)?;
        for alert in alerts {
            self.store
                .insert_early_warning_alert(&self.run_id, tick, &alert)?;

            out_events.push(SimEvent::ComplaintWarningFired {
                tick,
                alert_type: alert.alert_type.clone(),
                severity: alert.severity.clone(),
                segment: alert.segment.clone(),
            });

            log::warn!(
                "tick={tick} WARNING: {} ({}) - {} = {:.2} (threshold {:.2})",
                alert.alert_type,
                alert.severity,
                alert.metric_name,
                alert.current_value,
                alert.threshold_value,
            );
        }

        Ok(out_events)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
