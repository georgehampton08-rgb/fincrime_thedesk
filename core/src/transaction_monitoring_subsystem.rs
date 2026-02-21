//! Transaction Monitoring subsystem — Week 5 of Phase 3.5.
//!
//! Monitors transactions for suspicious patterns and regulatory triggers:
//! - Structuring (multiple transactions just under $10k CTR threshold)
//! - Velocity (high transaction volume in short period)
//! - CTR Auto-Filing (>$10k cash transactions)
//! - Geographic Anomalies (unexpected locations)
//! - Rapid Money Movement (immediate withdrawal after deposit)
//!
//! Execution: Every tick, monitors recent transactions and generates alerts.

use crate::{
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use std::collections::HashMap;

// ── Constants ────────────────────────────────────────────────────────────────

const CTR_THRESHOLD: f64 = 10000.0; // $10k cash transaction triggers CTR
const STRUCTURING_THRESHOLD: f64 = 9000.0; // Transactions just under $10k
const STRUCTURING_COUNT_THRESHOLD: usize = 3; // 3+ transactions in 7 days
const STRUCTURING_LOOKBACK_DAYS: u64 = 7;

const HIGH_VELOCITY_AMOUNT_7D: f64 = 50000.0; // $50k in 7 days
const HIGH_VELOCITY_COUNT_7D: usize = 10; // 10+ transactions in 7 days

const RAPID_MOVEMENT_THRESHOLD: f64 = 5000.0; // $5k+ immediate withdrawal after deposit
const RAPID_MOVEMENT_WINDOW_DAYS: u64 = 1; // Within 1 day

const METRICS_INTERVAL: u64 = 7; // Compute metrics every 7 ticks (weekly)

// ── Data Structures ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TransactionMonitoringRule {
    pub rule_id: String,
    pub rule_name: String,
    pub rule_type: String,
    pub threshold_amount: Option<f64>,
    pub threshold_count: Option<i64>,
    pub lookback_days: i64,
    pub base_alert_score: f64,
    pub auto_file_sar: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct AMLAlert {
    pub alert_id: String,
    pub run_id: String,
    pub customer_id: String,
    pub tick: Tick,
    pub rule_id: String,
    pub alert_type: String,
    pub alert_score: f64,
    pub description: String,
    pub triggered_amount: Option<f64>,
    pub transaction_count: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct TransactionRow {
    pub transaction_id: String,
    pub run_id: String,
    pub account_id: String,
    pub customer_id: String,
    pub tick: Tick,
    pub amount: f64,
    pub txn_type: String,
    pub category: String,
}

#[derive(Debug, Clone)]
pub struct CurrencyTransactionReport {
    pub ctr_id: String,
    pub run_id: String,
    pub customer_id: String,
    pub account_id: String,
    pub transaction_id: String,
    pub filing_tick: Tick,
    pub transaction_amount: f64,
    pub transaction_type: String,
    pub filing_deadline: Tick,
    pub filed_on_time: bool,
    pub auto_filed: bool,
}

#[derive(Debug, Clone)]
pub struct SuspiciousActivityReport {
    pub sar_id: String,
    pub run_id: String,
    pub filing_tick: Tick,
    pub subject_type: String,
    pub subject_id: String,
    pub activity_type: String,
    pub suspicious_amount: f64,
    pub narrative: String,
    pub filing_deadline: Tick,
    pub filed_on_time: bool,
    pub filing_status: String,
    pub regulatory_fine: f64,
    pub related_alerts: Option<String>, // JSON array of alert IDs
}

// ── Subsystem ────────────────────────────────────────────────────────────────

pub struct TransactionMonitoringSubsystem {
    run_id: RunId,
    store: SimStore,
}

impl TransactionMonitoringSubsystem {
    pub fn new(run_id: RunId, store: SimStore) -> Self {
        Self { run_id, store }
    }

    /// Detect structuring: Multiple transactions just under $10k threshold
    fn detect_structuring(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get transactions in last 7 days that are just under $10k
        let lookback_start = tick.saturating_sub(STRUCTURING_LOOKBACK_DAYS);
        let suspicious_txns = self.store.get_transactions_in_range(
            &self.run_id,
            lookback_start,
            tick,
            STRUCTURING_THRESHOLD,
            CTR_THRESHOLD,
        )?;

        // Group by customer
        let mut customer_txns: HashMap<String, Vec<_>> = HashMap::new();
        for txn in suspicious_txns {
            customer_txns
                .entry(txn.customer_id.clone())
                .or_insert_with(Vec::new)
                .push(txn);
        }

        for (customer_id, txns) in customer_txns {
            if txns.len() >= STRUCTURING_COUNT_THRESHOLD {
                let total_amount: f64 = txns.iter().map(|t| t.amount).sum();
                let alert_id = format!("STRUCT-{}-{}", customer_id, rng.next_u64_below(100000));

                let description = format!(
                    "{} transactions totaling ${:.2} just under $10k threshold in {} days",
                    txns.len(),
                    total_amount,
                    STRUCTURING_LOOKBACK_DAYS
                );

                let alert = AMLAlert {
                    alert_id: alert_id.clone(),
                    run_id: self.run_id.clone(),
                    customer_id: customer_id.clone(),
                    tick,
                    rule_id: "STRUCT_9K".into(),
                    alert_type: "structuring".into(),
                    alert_score: 90.0, // High confidence
                    description: description.clone(),
                    triggered_amount: Some(total_amount),
                    transaction_count: Some(txns.len() as i64),
                    status: "open".into(),
                };

                self.store.insert_transaction_monitoring_alert(&alert)?;

                events.push(SimEvent::TransactionMonitoringAlert {
                    tick,
                    alert_id,
                    alert_type: "structuring".into(),
                    customer_id: customer_id.clone(),
                    alert_score: 90.0,
                    description,
                });

                log::warn!(
                    "tick={} Structuring detected: {} ({} transactions, ${:.2})",
                    tick,
                    customer_id,
                    txns.len(),
                    total_amount
                );
            }
        }

        Ok(events)
    }

    /// Detect high velocity: Too many transactions or too much volume in short period
    fn detect_velocity(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        let lookback_start = tick.saturating_sub(7); // 7-day window
        let all_txns = self.store.get_all_transactions_in_window(
            &self.run_id,
            lookback_start,
            tick,
        )?;

        // Group by customer
        let mut customer_txns: HashMap<String, Vec<_>> = HashMap::new();
        for txn in all_txns {
            customer_txns
                .entry(txn.customer_id.clone())
                .or_insert_with(Vec::new)
                .push(txn);
        }

        for (customer_id, txns) in customer_txns {
            let total_amount: f64 = txns.iter().map(|t| t.amount.abs()).sum();
            let txn_count = txns.len();

            // Check if exceeds thresholds
            if total_amount > HIGH_VELOCITY_AMOUNT_7D && txn_count > HIGH_VELOCITY_COUNT_7D {
                let alert_id = format!("VEL-{}-{}", customer_id, rng.next_u64_below(100000));

                let description = format!(
                    "{} transactions totaling ${:.2} in 7 days (threshold: ${})",
                    txn_count, total_amount, HIGH_VELOCITY_AMOUNT_7D
                );

                let alert = AMLAlert {
                    alert_id: alert_id.clone(),
                    run_id: self.run_id.clone(),
                    customer_id: customer_id.clone(),
                    tick,
                    rule_id: "VEL_50K_7D".into(),
                    alert_type: "velocity".into(),
                    alert_score: 75.0,
                    description: description.clone(),
                    triggered_amount: Some(total_amount),
                    transaction_count: Some(txn_count as i64),
                    status: "open".into(),
                };

                self.store.insert_transaction_monitoring_alert(&alert)?;

                events.push(SimEvent::TransactionMonitoringAlert {
                    tick,
                    alert_id,
                    alert_type: "velocity".into(),
                    customer_id: customer_id.clone(),
                    alert_score: 75.0,
                    description,
                });

                log::info!(
                    "tick={} High velocity detected: {} ({} txns, ${:.2})",
                    tick,
                    customer_id,
                    txn_count,
                    total_amount
                );
            }
        }

        Ok(events)
    }

    /// Auto-file CTRs for cash transactions >= $10k
    fn file_ctrs(&self, tick: Tick, rng: &mut SubsystemRng) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get cash transactions >= $10k from this tick
        let large_cash_txns = self.store.get_cash_transactions_above_threshold(
            &self.run_id,
            tick,
            CTR_THRESHOLD,
        )?;

        for txn in large_cash_txns {
            let ctr_id = format!("CTR-{}-{}", txn.transaction_id, rng.next_u64_below(1000));

            // CTR must be filed within 15 days
            let filing_deadline = tick + 15;

            let ctr = CurrencyTransactionReport {
                ctr_id: ctr_id.clone(),
                run_id: self.run_id.clone(),
                customer_id: txn.customer_id.clone(),
                account_id: txn.account_id.clone(),
                transaction_id: txn.transaction_id.clone(),
                filing_tick: tick,
                transaction_amount: txn.amount,
                transaction_type: if txn.txn_type == "credit" {
                    "cash_deposit".into()
                } else {
                    "cash_withdrawal".into()
                },
                filing_deadline,
                filed_on_time: true, // Auto-filed immediately
                auto_filed: true,
            };

            self.store.insert_ctr(&ctr)?;

            events.push(SimEvent::CTRFiled {
                tick,
                ctr_id,
                customer_id: txn.customer_id,
                amount: txn.amount,
                transaction_type: ctr.transaction_type,
            });

            log::info!(
                "tick={} CTR filed: ${:.2} {} transaction",
                tick,
                txn.amount,
                txn.txn_type
            );
        }

        Ok(events)
    }

    /// Detect rapid money movement: Immediate withdrawal after large deposit
    fn detect_rapid_movement(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get transactions from last day
        let lookback_start = tick.saturating_sub(RAPID_MOVEMENT_WINDOW_DAYS);
        let recent_txns = self.store.get_all_transactions_in_window(
            &self.run_id,
            lookback_start,
            tick,
        )?;

        // Group by account
        let mut account_txns: HashMap<String, Vec<_>> = HashMap::new();
        for txn in recent_txns {
            account_txns
                .entry(txn.account_id.clone())
                .or_insert_with(Vec::new)
                .push(txn);
        }

        for (account_id, txns) in account_txns {
            // Sort by tick
            let mut sorted_txns = txns.clone();
            sorted_txns.sort_by_key(|t| t.tick);

            // Look for deposit followed by withdrawal
            for i in 0..sorted_txns.len().saturating_sub(1) {
                let deposit = &sorted_txns[i];
                let withdrawal = &sorted_txns[i + 1];

                if deposit.txn_type == "credit"
                    && withdrawal.txn_type == "debit"
                    && deposit.amount >= RAPID_MOVEMENT_THRESHOLD
                    && withdrawal.amount >= RAPID_MOVEMENT_THRESHOLD
                    && withdrawal.tick <= deposit.tick + 1
                // Within 1 day
                {
                    let alert_id = format!(
                        "RAPID-{}-{}",
                        deposit.customer_id,
                        rng.next_u64_below(100000)
                    );

                    let description = format!(
                        "Deposit of ${:.2} followed by withdrawal of ${:.2} within 1 day",
                        deposit.amount, withdrawal.amount
                    );

                    let alert = AMLAlert {
                        alert_id: alert_id.clone(),
                        run_id: self.run_id.clone(),
                        customer_id: deposit.customer_id.clone(),
                        tick,
                        rule_id: "RAPID_MOVE".into(),
                        alert_type: "rapid_movement".into(),
                        alert_score: 80.0,
                        description: description.clone(),
                        triggered_amount: Some(deposit.amount),
                        transaction_count: Some(2),
                        status: "open".into(),
                    };

                    self.store.insert_transaction_monitoring_alert(&alert)?;

                    events.push(SimEvent::TransactionMonitoringAlert {
                        tick,
                        alert_id,
                        alert_type: "rapid_movement".into(),
                        customer_id: deposit.customer_id.clone(),
                        alert_score: 80.0,
                        description,
                    });

                    log::info!(
                        "tick={} Rapid movement detected: account {} (deposit ${:.2}, withdrawal ${:.2})",
                        tick,
                        account_id,
                        deposit.amount,
                        withdrawal.amount
                    );

                    break; // Only alert once per account per tick
                }
            }
        }

        Ok(events)
    }

    /// Compute weekly monitoring metrics
    fn compute_metrics(&self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        let window_start = tick.saturating_sub(METRICS_INTERVAL);

        let alerts_generated = self
            .store
            .count_aml_alerts_in_window(&self.run_id, window_start, tick)?;
        let alerts_investigating =
            self.store
                .count_aml_alerts_by_status(&self.run_id, "investigating")?;
        let alerts_closed = self
            .store
            .count_aml_alerts_by_status(&self.run_id, "closed_benign")?
            + self
                .store
                .count_aml_alerts_by_status(&self.run_id, "closed_no_action")?;
        let ctrs_filed = self
            .store
            .count_ctrs_in_window(&self.run_id, window_start, tick)?;

        // Compute false positive rate (closed_benign / total closed)
        let total_closed = alerts_closed as f64;
        let false_positives = self
            .store
            .count_aml_alerts_by_status(&self.run_id, "closed_benign")? as f64;
        let false_positive_rate = if total_closed > 0.0 {
            false_positives / total_closed
        } else {
            0.0
        };

        self.store.insert_transaction_monitoring_metrics(
            &self.run_id,
            tick,
            alerts_generated,
            alerts_investigating,
            alerts_closed,
            ctrs_filed,
            0, // SARs (will be added in Week 6)
            false_positive_rate,
            0.0, // avg_investigation_time (placeholder)
        )?;

        events.push(SimEvent::TransactionMonitoringMetricsComputed {
            tick,
            alerts_generated,
            ctrs_filed,
        });

        Ok(events)
    }

    /// File SARs for high-scoring alerts (Week 6)
    fn file_sars(&self, tick: Tick, rng: &mut SubsystemRng) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get high-scoring alerts from last 30 days that haven't had SAR filed
        let lookback_start = tick.saturating_sub(30);
        let high_score_alerts = self.store.get_alerts_above_threshold(
            &self.run_id,
            85.0, // SAR threshold: 85+ alert score
            lookback_start,
            tick,
        )?;

        for alert in high_score_alerts {
            // Generate SAR ID
            let sar_id = format!("SAR-{}-{}", alert.customer_id, rng.next_u64_below(1000000));

            // Generate narrative based on alert type
            let narrative = match alert.alert_type.as_str() {
                "structuring" => format!(
                    "Customer {} engaged in potential structuring activity. {}",
                    alert.customer_id, alert.description
                ),
                "velocity" => format!(
                    "Customer {} exhibited high-velocity transaction patterns. {}",
                    alert.customer_id, alert.description
                ),
                "rapid_movement" => format!(
                    "Customer {} demonstrated rapid money movement patterns. {}",
                    alert.customer_id, alert.description
                ),
                _ => format!(
                    "Suspicious activity detected for customer {}. Type: {}. {}",
                    alert.customer_id, alert.alert_type, alert.description
                ),
            };

            // SAR must be filed within 30 days of detection
            let filing_deadline = alert.tick + 30;
            let days_elapsed = tick.saturating_sub(alert.tick);
            let filed_on_time = days_elapsed <= 30;

            // Calculate regulatory fine for late filing: $25,000 base + $1,000 per day late
            let regulatory_fine = if !filed_on_time {
                let days_late = days_elapsed - 30;
                25000.0 + (days_late as f64 * 1000.0)
            } else {
                0.0
            };

            let sar = SuspiciousActivityReport {
                sar_id: sar_id.clone(),
                run_id: self.run_id.clone(),
                filing_tick: tick,
                subject_type: "customer".into(),
                subject_id: alert.customer_id.clone(),
                activity_type: alert.alert_type.clone(),
                suspicious_amount: alert.triggered_amount.unwrap_or(0.0),
                narrative,
                filing_deadline,
                filed_on_time,
                filing_status: if filed_on_time {
                    "filed".into()
                } else {
                    "late".into()
                },
                regulatory_fine,
                related_alerts: Some(format!("[\"{}]", alert.alert_id)),
            };

            self.store.insert_sar(&sar)?;
            self.store.mark_alert_sar_filed(&self.run_id, &alert.alert_id)?;

            events.push(SimEvent::SARFiled {
                tick,
                sar_id: sar_id.clone(),
                customer_id: alert.customer_id.clone(),
                activity_type: sar.activity_type.clone(),
                suspicious_amount: sar.suspicious_amount,
            });

            if !filed_on_time {
                let days_late = (days_elapsed - 30) as i64;
                events.push(SimEvent::SARLateFiling {
                    tick,
                    sar_id,
                    customer_id: alert.customer_id,
                    days_late,
                    regulatory_fine,
                });

                log::warn!(
                    "tick={} SAR filed LATE: {} ({} days late, fine: ${:.2})",
                    tick,
                    sar.sar_id,
                    days_late,
                    regulatory_fine
                );
            } else {
                log::info!(
                    "tick={} SAR filed: {} for {} ({})",
                    tick,
                    sar.sar_id,
                    alert.customer_id,
                    sar.activity_type
                );
            }
        }

        Ok(events)
    }

    /// Compute monthly SAR filing metrics
    fn compute_sar_metrics(&self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Monthly interval (30 ticks)
        let window_start = tick.saturating_sub(30);

        let sars_filed = self
            .store
            .count_sars_in_window(&self.run_id, window_start, tick)?;
        let sars_late = self
            .store
            .count_late_sars_in_window(&self.run_id, window_start, tick)?;
        let total_fines = self
            .store
            .sum_sar_fines_in_window(&self.run_id, window_start, tick)?;

        self.store.insert_sar_metrics(
            &self.run_id,
            tick,
            sars_filed,
            sars_late,
            total_fines,
            0.0, // avg_filing_time (placeholder)
        )?;

        events.push(SimEvent::SARMetricsComputed {
            tick,
            sars_filed,
            sars_late,
            total_fines,
        });

        log::info!(
            "tick={} SAR metrics: {} filed, {} late, ${:.2} fines",
            tick,
            sars_filed,
            sars_late,
            total_fines
        );

        Ok(events)
    }
}

impl SimSubsystem for TransactionMonitoringSubsystem {
    fn name(&self) -> &'static str {
        "transaction_monitoring"
    }

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

        // 1. Detect structuring (every tick)
        out.extend(self.detect_structuring(tick, rng)?);

        // 2. Detect high velocity (every tick)
        out.extend(self.detect_velocity(tick, rng)?);

        // 3. Auto-file CTRs for large cash transactions (every tick)
        out.extend(self.file_ctrs(tick, rng)?);

        // 4. Detect rapid money movement (every tick)
        out.extend(self.detect_rapid_movement(tick, rng)?);

        // 5. Compute metrics (weekly)
        if tick.is_multiple_of(METRICS_INTERVAL) {
            out.extend(self.compute_metrics(tick)?);
        }

        // 6. File SARs for high-scoring alerts (weekly)
        if tick.is_multiple_of(METRICS_INTERVAL) {
            out.extend(self.file_sars(tick, rng)?);
        }

        // 7. Compute SAR metrics (monthly - every 30 ticks)
        if tick.is_multiple_of(30) {
            out.extend(self.compute_sar_metrics(tick)?);
        }

        Ok(out)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
