//! Economics subsystem — quarterly P&L computation.
//!
//! This subsystem is REACTIVE. It does not generate transactions
//! or events. It observes what happened in the prior quarter and
//! computes financial KPIs.
//!
//! Execution: runs every 90 ticks (quarterly).
//! Depends on: daily_aggregate (from transaction subsystem),
//!             complaint_aggregate (from complaint subsystem),
//!             macro_state (for interest rates).

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
use std::collections::HashMap;

pub const ECONOMICS_UPDATE_INTERVAL: Tick = 90; // quarterly

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLSnapshot {
    pub tick: Tick,
    pub period: String,
    // Revenue
    pub nii: f64,
    pub fee_income: f64,
    pub gross_income: f64,
    // Costs
    pub credit_loss: f64,
    pub fraud_loss: f64,
    pub opex: f64,
    pub complaint_cost: f64,
    // Bottom line
    pub pre_tax_profit: f64,
    // KPIs
    pub nim: f64,
    pub efficiency_ratio: f64,
    // Context
    pub avg_deposits: f64,
    pub avg_loans: f64,
    pub customer_count: i64,
    pub active_accounts: i64,
}

#[derive(Debug, Clone)]
pub struct SegmentPnL {
    pub run_id: RunId,
    pub tick: Tick,
    pub segment: String,
    pub nii: f64,
    pub fee_income: f64,
    pub interchange_income: f64,
    pub gross_income: f64,
    pub acquisition_cost: f64,
    pub servicing_cost: f64,
    pub complaint_cost: f64,
    pub retention_cost: f64,
    pub churn_replacement_cost: f64,
    pub allocated_opex: f64,
    pub total_cost: f64,
    pub segment_profit: f64,
    pub customer_margin: f64,
    pub profit_per_customer: f64,
    pub active_customers: i64,
    pub avg_balance: f64,
    pub avg_revenue_per_customer: f64,
    pub avg_cost_per_customer: f64,
    pub below_target_margin: bool,
    pub cross_subsidy_recipient: bool,
}

impl SegmentPnL {
    pub fn zero(run_id: &str, segment: &str, tick: Tick) -> Self {
        Self {
            run_id: run_id.into(),
            tick,
            segment: segment.to_string(),
            nii: 0.0,
            fee_income: 0.0,
            interchange_income: 0.0,
            gross_income: 0.0,
            acquisition_cost: 0.0,
            servicing_cost: 0.0,
            complaint_cost: 0.0,
            retention_cost: 0.0,
            churn_replacement_cost: 0.0,
            allocated_opex: 0.0,
            total_cost: 0.0,
            segment_profit: 0.0,
            customer_margin: 0.0,
            profit_per_customer: 0.0,
            active_customers: 0,
            avg_balance: 0.0,
            avg_revenue_per_customer: 0.0,
            avg_cost_per_customer: 0.0,
            below_target_margin: false,
            cross_subsidy_recipient: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SegmentComplaints {
    pub standard: i64,
    pub high: i64,
    pub urgent: i64,
}

pub struct EconomicsSubsystem {
    run_id: RunId,
    config: SimConfig,
    store: SimStore,
    quarter_number: u32,
}

impl EconomicsSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        Self {
            run_id,
            config,
            store,
            quarter_number: 0,
        }
    }

    fn compute_pnl(&mut self, tick: Tick) -> SimResult<PnLSnapshot> {
        self.quarter_number += 1;
        let period = format!(
            "Q{}-Y{}",
            ((self.quarter_number - 1) % 4) + 1,
            ((self.quarter_number - 1) / 4) + 1
        );

        // Quarter range: ticks [tick - 89, tick] inclusive
        let quarter_start = tick.saturating_sub(89);
        let quarter_end = tick;

        // ── Revenue ────────────────────────────────────────────

        // Net Interest Income (NII)
        let avg_deposits =
            self.store
                .avg_account_balances(&self.run_id, quarter_start, quarter_end)?;

        let avg_rate = self
            .store
            .avg_macro_base_rate(&self.run_id, quarter_start, quarter_end)?;

        // Simplified NII: deposits earn 0.5× base rate as spread
        let deposit_rate = avg_rate * 0.5;
        let nii = avg_deposits * deposit_rate * (90.0 / 365.0);

        // Fee Income from daily_aggregate
        let fee_income = self
            .store
            .sum_fee_income(&self.run_id, quarter_start, quarter_end)?;

        let gross_income = nii + fee_income;

        // ── Costs ──────────────────────────────────────────────

        let credit_loss = 0.0; // Phase 2+
        let fraud_loss = 0.0; // Phase 3

        // Operating expenses
        let staff_count = 20;
        let loaded_cost = 85000.0;
        let overhead_multiplier = 1.8;
        let quarterly_staff_cost = (staff_count as f64 * loaded_cost * overhead_multiplier) / 4.0;

        // Complaint handling cost: $50 per complaint
        let complaint_count =
            self.store
                .sum_complaints_opened(&self.run_id, quarter_start, quarter_end)?;
        let complaint_cost = complaint_count as f64 * 50.0;

        // Offer acquisition / retention cost: bonuses paid this quarter
        let offer_bonus_cost =
            self.store
                .sum_offer_bonuses_paid(&self.run_id, quarter_start, quarter_end)?;

        let opex = quarterly_staff_cost + complaint_cost + offer_bonus_cost;

        // ── Bottom Line ────────────────────────────────────────

        let pre_tax_profit = gross_income - credit_loss - fraud_loss - opex;

        // ── KPIs ───────────────────────────────────────────────

        // NIM: annualized
        let nim = if avg_deposits > 0.0 {
            (nii / avg_deposits) * 4.0 * 100.0
        } else {
            0.0
        };

        // Efficiency Ratio: opex / gross_income × 100%
        let efficiency_ratio = if gross_income > 0.0 {
            (opex / gross_income) * 100.0
        } else {
            0.0
        };

        // ── Context ────────────────────────────────────────────

        let customer_count = self.store.customer_count(&self.run_id, "active")?;
        let active_accounts = self.store.active_account_count(&self.run_id)?;

        Ok(PnLSnapshot {
            tick,
            period,
            nii,
            fee_income,
            gross_income,
            credit_loss,
            fraud_loss,
            opex,
            complaint_cost,
            pre_tax_profit,
            nim,
            efficiency_ratio,
            avg_deposits,
            avg_loans: 0.0, // Phase 2+
            customer_count,
            active_accounts,
        })
    }

    fn compute_segment_pnl(
        &self,
        tick: Tick,
        quarter_start: Tick,
        quarter_end: Tick,
        total_nii: f64,
    ) -> SimResult<HashMap<String, SegmentPnL>> {
        let mut result = HashMap::new();

        for segment_name in self.config.segments.keys() {
            let pnl = self.compute_single_segment_pnl(
                segment_name,
                tick,
                quarter_start,
                quarter_end,
                total_nii,
            )?;
            result.insert(segment_name.clone(), pnl);
        }

        Ok(result)
    }

    fn compute_single_segment_pnl(
        &self,
        segment: &str,
        tick: Tick,
        quarter_start: Tick,
        quarter_end: Tick,
        total_nii: f64,
    ) -> SimResult<SegmentPnL> {
        let econ = &self.config.segment_economics;

        let active_customers =
            self.store
                .segment_customer_count(&self.run_id, segment, "active")?;

        if active_customers == 0 {
            return Ok(SegmentPnL::zero(&self.run_id, segment, tick));
        }

        // ── Revenue ────────────────────────────────────────────

        // NII: attributed by balance share
        let segment_balance =
            self.store
                .segment_avg_balance(&self.run_id, segment, quarter_start, quarter_end)?;
        let total_balance =
            self.store
                .avg_account_balances(&self.run_id, quarter_start, quarter_end)?;
        let balance_share = if total_balance > 0.0 {
            segment_balance / total_balance
        } else {
            0.0
        };
        let nii = total_nii * balance_share;

        // Fee income: direct attribution
        let fee_income =
            self.store
                .segment_fee_income(&self.run_id, segment, quarter_start, quarter_end)?;

        // Interchange income
        let avg_swipes = econ
            .revenue_attribution
            .average_card_swipes_per_month
            .get(segment)
            .copied()
            .unwrap_or(0) as f64;
        let interchange_income = active_customers as f64
            * avg_swipes
            * 3.0
            * econ.revenue_attribution.interchange_revenue_per_swipe;

        let gross_income = nii + fee_income + interchange_income;

        // ── Costs ──────────────────────────────────────────────

        let acq_cost_per = econ
            .cost_allocation_model
            .acquisition_cost_per_customer
            .get(segment)
            .copied()
            .unwrap_or(85.0);

        let new_customers =
            self.store
                .segment_new_customers(&self.run_id, segment, quarter_start, quarter_end)?;
        let acquisition_cost = new_customers as f64 * acq_cost_per;

        let svc_cost_per = econ
            .cost_allocation_model
            .monthly_servicing_cost_per_customer
            .get(segment)
            .copied()
            .unwrap_or(4.50);
        let servicing_cost = active_customers as f64 * svc_cost_per * 3.0;

        let complaints =
            self.store
                .segment_complaints(&self.run_id, segment, quarter_start, quarter_end)?;
        let complaint_cost = self.compute_segment_complaint_cost(
            &complaints,
            &econ
                .cost_allocation_model
                .complaint_handling_cost_per_complaint,
        );

        let retention_cost = self.store.segment_retention_offer_cost(
            &self.run_id,
            segment,
            quarter_start,
            quarter_end,
        )?;

        let churned = self.store.segment_churned_customers(
            &self.run_id,
            segment,
            quarter_start,
            quarter_end,
        )?;
        let churn_replacement_cost = churned as f64
            * acq_cost_per
            * econ.cost_allocation_model.churn_replacement_cost_multiplier;

        let total_active = self.store.total_active_customers(&self.run_id)?.max(1);
        let customer_share = active_customers as f64 / total_active as f64;
        let allocated_opex = 45_000.0 * customer_share;

        let total_cost = acquisition_cost
            + servicing_cost
            + complaint_cost
            + retention_cost
            + churn_replacement_cost
            + allocated_opex;

        // ── Bottom Line ────────────────────────────────────────

        let segment_profit = gross_income - total_cost;
        let customer_margin = if gross_income > 0.0 {
            segment_profit / gross_income
        } else {
            0.0
        };
        let profit_per_customer = segment_profit / active_customers as f64;
        let avg_revenue_per_customer = gross_income / active_customers as f64;
        let avg_cost_per_customer = total_cost / active_customers as f64;

        let target_margin = econ
            .profitability_metrics
            .target_customer_margin
            .get(segment)
            .copied()
            .unwrap_or(0.20);
        let below_target_margin = customer_margin
            < target_margin + econ.profitability_metrics.warning_threshold_below_target;

        Ok(SegmentPnL {
            run_id: self.run_id.clone(),
            tick,
            segment: segment.to_string(),
            nii,
            fee_income,
            interchange_income,
            gross_income,
            acquisition_cost,
            servicing_cost,
            complaint_cost,
            retention_cost,
            churn_replacement_cost,
            allocated_opex,
            total_cost,
            segment_profit,
            customer_margin,
            profit_per_customer,
            active_customers,
            avg_balance: segment_balance,
            avg_revenue_per_customer,
            avg_cost_per_customer,
            below_target_margin,
            cross_subsidy_recipient: false, // set by analyze_cross_subsidies
        })
    }

    fn compute_segment_complaint_cost(
        &self,
        complaints: &SegmentComplaints,
        cost_per_priority: &HashMap<String, f64>,
    ) -> f64 {
        let standard = cost_per_priority.get("standard").copied().unwrap_or(50.0);
        let high = cost_per_priority.get("high").copied().unwrap_or(120.0);
        let urgent = cost_per_priority.get("urgent").copied().unwrap_or(280.0);

        (complaints.standard as f64 * standard)
            + (complaints.high as f64 * high)
            + (complaints.urgent as f64 * urgent)
    }

    fn analyze_cross_subsidies(
        &self,
        tick: Tick,
        segment_pnls: &HashMap<String, SegmentPnL>,
    ) -> SimResult<()> {
        let threshold = self
            .config
            .segment_economics
            .profitability_metrics
            .cross_subsidy_flag_threshold;

        let profitable: Vec<(&String, f64)> = segment_pnls
            .iter()
            .filter(|(_, pnl)| pnl.segment_profit > 0.0)
            .map(|(name, pnl)| (name, pnl.segment_profit))
            .collect();

        let unprofitable: Vec<(&String, f64)> = segment_pnls
            .iter()
            .filter(|(_, pnl)| pnl.segment_profit < 0.0)
            .map(|(name, pnl)| (name, pnl.segment_profit.abs()))
            .collect();

        let total_loss: f64 = unprofitable.iter().map(|(_, l)| l).sum();

        for (provider, provider_profit) in &profitable {
            for (recipient, recipient_loss) in &unprofitable {
                let subsidy_share = if total_loss > 0.0 {
                    recipient_loss / total_loss
                } else {
                    0.0
                };
                let subsidy_amount = provider_profit * subsidy_share;
                if subsidy_amount > threshold * provider_profit {
                    self.store.insert_cross_subsidy(
                        &self.run_id,
                        tick,
                        provider,
                        recipient,
                        subsidy_amount,
                    )?;
                }
            }
        }

        Ok(())
    }
}

impl SimSubsystem for EconomicsSubsystem {
    fn name(&self) -> &'static str {
        "economics"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        _rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        if !tick.is_multiple_of(ECONOMICS_UPDATE_INTERVAL) || tick == 0 {
            return Ok(vec![]);
        }

        let quarter_start = tick.saturating_sub(89);
        let quarter_end = tick;

        let pnl = self.compute_pnl(tick)?;
        self.store.insert_pnl_snapshot(&self.run_id, &pnl)?;

        log::info!(
            "{}: NII=${:.0} Fees=${:.0} OPEX=${:.0} Profit=${:.0} NIM={:.2}% Eff={:.1}%",
            pnl.period,
            pnl.nii,
            pnl.fee_income,
            pnl.opex,
            pnl.pre_tax_profit,
            pnl.nim,
            pnl.efficiency_ratio
        );

        // Segment P&L uses the bank-level NII we just computed
        let segment_pnls = self.compute_segment_pnl(tick, quarter_start, quarter_end, pnl.nii)?;

        self.analyze_cross_subsidies(tick, &segment_pnls)?;

        for (segment, seg_pnl) in &segment_pnls {
            self.store.insert_segment_pnl(&self.run_id, seg_pnl)?;

            if seg_pnl.below_target_margin {
                log::warn!(
                    "{} {}: margin {:.1}% below target",
                    pnl.period,
                    segment,
                    seg_pnl.customer_margin * 100.0
                );
            }
        }

        Ok(vec![SimEvent::QuarterlyPnLComputed {
            tick,
            period: pnl.period.clone(),
            gross_income: pnl.gross_income,
            pre_tax_profit: pnl.pre_tax_profit,
            nim: pnl.nim,
            efficiency_ratio: pnl.efficiency_ratio,
        }])
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
