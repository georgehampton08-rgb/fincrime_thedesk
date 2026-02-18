use crate::{
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use uuid::Uuid;

pub struct TransactionSubsystem {
    run_id: RunId,
    store: SimStore,
}

impl TransactionSubsystem {
    pub fn new(run_id: RunId, store: SimStore) -> Self {
        Self { run_id, store }
    }

    /// Generate transactions for one account for this tick.
    #[allow(clippy::too_many_arguments)]
    fn process_account(
        &self,
        account_id: &str,
        customer_id: &str,
        monthly_txn_mean: f64,
        cash_intensity: f64,
        payroll_amount: f64,
        has_payroll: bool,
        _product_id: &str,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Payroll credit: biweekly on tick % 14 == 0
        if has_payroll && tick.is_multiple_of(14) && payroll_amount > 0.0 {
            let jitter = 1.0 + (rng.next_f64() - 0.5) * 0.05;
            let amount = payroll_amount * jitter;
            let txn_id = Uuid::new_v4().to_string();
            self.store.insert_transaction(
                &self.run_id,
                &txn_id,
                account_id,
                tick,
                amount,
                "credit",
                "payroll",
                Some("payroll-employer"),
            )?;
            self.store
                .update_account_balance(&self.run_id, account_id, amount)?;
        }

        // Daily transaction probability from monthly mean.
        // monthly_mean / 30 = daily expected count.
        let daily_prob = (monthly_txn_mean / 30.0).min(5.0);
        // Poisson approximation: floor(daily_prob) certain,
        // remainder is probabilistic.
        let certain = daily_prob.floor() as u32;
        let extra = if rng.chance(daily_prob.fract()) { 1 } else { 0 };
        let txn_count = certain + extra;

        for _ in 0..txn_count {
            let is_cash = rng.chance(cash_intensity);
            let amount = if is_cash {
                // Cash: Pareto-sampled, rounded to nearest $20
                let raw = rng.pareto(15.0, 1.6);
                let capped = raw.min(500.0);
                (capped / 20.0).round() * 20.0
            } else {
                rng.pareto(10.0, 1.4).min(2000.0)
            };

            let category = if is_cash {
                "cash_withdrawal"
            } else {
                "purchase"
            };

            // Counterparty: 80% recurring, 20% new
            let counterparty = if rng.chance(0.80) {
                // Stable recurring counterparty
                // Derive from customer_id hash for stability
                let slot = rng.next_u64_below(8);
                Some(format!("merchant-{customer_id}-{slot}"))
            } else {
                Some(format!("new-merchant-{}", rng.next_u64_below(10000)))
            };

            let txn_id = Uuid::new_v4().to_string();

            // Select payment rail for debit transactions
            let (rail_id, settlement_status) = if is_cash {
                // Cash withdrawals always go through ACH, settle immediately
                ("ACH", "settled")
            } else {
                // Purchase: select payment rail
                let rail_roll = rng.next_f64();
                if rail_roll < 0.50 {
                    ("card", "pending_authorization")
                } else if rail_roll < 0.80 {
                    ("ACH", "settled")
                } else if rail_roll < 0.90 {
                    ("wire", "settled")
                } else {
                    ("RTP", "settled")
                }
            };

            self.store.insert_transaction_with_rail(
                &self.run_id,
                &txn_id,
                account_id,
                tick,
                amount,
                "debit",
                category,
                counterparty.as_deref(),
                rail_id,
                settlement_status,
            )?;
            // For non-card rails, update balance immediately (already settled)
            // For card rails, only available_balance is affected (handled by PaymentHub)
            if rail_id != "card" {
                self.store
                    .update_account_balance(&self.run_id, account_id, -amount)?;
            }
            // Card transactions: PaymentHubSubsystem will handle auth hold on available_balance
        }

        // Overdraft check: if balance < 0 after debits
        let balance = self.store.account_balance(&self.run_id, account_id)?;
        if balance < -0.01 {
            let od_fee = 27.08;
            let fee_id = Uuid::new_v4().to_string();
            self.store.insert_transaction(
                &self.run_id,
                &fee_id,
                account_id,
                tick,
                od_fee,
                "debit",
                "overdraft_fee",
                None,
            )?;
            self.store
                .update_account_balance(&self.run_id, account_id, -od_fee)?;
            events.push(SimEvent::FeeCharged {
                tick,
                customer_id: customer_id.to_string(),
                account_id: account_id.to_string(),
                fee_type: "overdraft".to_string(),
                amount: od_fee,
            });
        }

        Ok(events)
    }
}

impl SimSubsystem for TransactionSubsystem {
    fn name(&self) -> &'static str {
        "transaction"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        // Skip tick 0 — customers are being onboarded this tick,
        // accounts aren't written yet when transaction runs.
        if tick == 0 {
            return Ok(out_events);
        }

        let accounts = self.store.active_accounts(&self.run_id)?;

        for acct in accounts {
            let events = self.process_account(
                &acct.account_id,
                &acct.customer_id,
                acct.monthly_txn_mean,
                acct.cash_intensity,
                acct.payroll_amount,
                acct.has_payroll,
                &acct.product_id,
                tick,
                rng,
            )?;
            out_events.extend(events);
        }

        // Write daily aggregate
        let agg = self.store.compute_daily_aggregate(&self.run_id, tick)?;
        self.store.save_daily_aggregate(&self.run_id, tick, &agg)?;

        log::debug!(
            "tick={tick} txn: {} txns, vol=${:.0}, fees=${:.2}",
            agg.txn_count,
            agg.txn_volume,
            agg.fee_income
        );

        Ok(out_events)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
