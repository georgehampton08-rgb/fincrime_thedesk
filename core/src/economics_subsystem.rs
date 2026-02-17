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

pub const ECONOMICS_UPDATE_INTERVAL: Tick = 90; // quarterly

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLSnapshot {
    pub tick:             Tick,
    pub period:           String,
    // Revenue
    pub nii:              f64,
    pub fee_income:       f64,
    pub gross_income:     f64,
    // Costs
    pub credit_loss:      f64,
    pub fraud_loss:       f64,
    pub opex:             f64,
    pub complaint_cost:   f64,
    // Bottom line
    pub pre_tax_profit:   f64,
    // KPIs
    pub nim:              f64,
    pub efficiency_ratio: f64,
    // Context
    pub avg_deposits:     f64,
    pub avg_loans:        f64,
    pub customer_count:   i64,
    pub active_accounts:  i64,
}

pub struct EconomicsSubsystem {
    run_id:         RunId,
    #[allow(dead_code)]
    config:         SimConfig,
    store:          SimStore,
    quarter_number: u32,
}

impl EconomicsSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        Self { run_id, config, store, quarter_number: 0 }
    }

    fn compute_pnl(&mut self, tick: Tick) -> SimResult<PnLSnapshot> {
        self.quarter_number += 1;
        let period = format!("Q{}-Y{}",
            ((self.quarter_number - 1) % 4) + 1,
            ((self.quarter_number - 1) / 4) + 1
        );

        // Quarter range: ticks [tick - 89, tick] inclusive
        let quarter_start = tick.saturating_sub(89);
        let quarter_end   = tick;

        // ── Revenue ────────────────────────────────────────────

        // Net Interest Income (NII)
        let avg_deposits = self.store.avg_account_balances(
            &self.run_id, quarter_start, quarter_end
        )?;

        let avg_rate = self.store.avg_macro_base_rate(
            &self.run_id, quarter_start, quarter_end
        )?;

        // Simplified NII: deposits earn 0.5× base rate as spread
        let deposit_rate = avg_rate * 0.5;
        let nii = avg_deposits * deposit_rate * (90.0 / 365.0);

        // Fee Income from daily_aggregate
        let fee_income = self.store.sum_fee_income(
            &self.run_id, quarter_start, quarter_end
        )?;

        let gross_income = nii + fee_income;

        // ── Costs ──────────────────────────────────────────────

        let credit_loss = 0.0; // Phase 2+
        let fraud_loss  = 0.0; // Phase 3

        // Operating expenses
        let staff_count          = 20;
        let loaded_cost          = 85000.0;
        let overhead_multiplier  = 1.8;
        let quarterly_staff_cost = (staff_count as f64 * loaded_cost * overhead_multiplier) / 4.0;

        // Complaint handling cost: $50 per complaint
        let complaint_count = self.store.sum_complaints_opened(
            &self.run_id, quarter_start, quarter_end
        )?;
        let complaint_cost = complaint_count as f64 * 50.0;

        let opex = quarterly_staff_cost + complaint_cost;

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

        let customer_count  = self.store.customer_count(&self.run_id, "active")?;
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
}

impl SimSubsystem for EconomicsSubsystem {
    fn name(&self) -> &'static str { "economics" }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        _rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        if !tick.is_multiple_of(ECONOMICS_UPDATE_INTERVAL) || tick == 0 {
            return Ok(vec![]);
        }

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

        Ok(vec![SimEvent::QuarterlyPnLComputed {
            tick,
            period:           pnl.period.clone(),
            gross_income:     pnl.gross_income,
            pre_tax_profit:   pnl.pre_tax_profit,
            nim:              pnl.nim,
            efficiency_ratio: pnl.efficiency_ratio,
        }])
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}
