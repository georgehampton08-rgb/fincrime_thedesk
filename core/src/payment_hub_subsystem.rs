//! Payment hub subsystem — manages payment rails, card authorization lifecycle,
//! batch settlement, and external statement generation.
//!
//! Execution order: After TransactionSubsystem, before ComplaintSubsystem.
//! Reads transactions already inserted by TransactionSubsystem and manages
//! the settlement lifecycle.

use crate::{
    config::PaymentHubConfig,
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::{
        AuthorizationRow, ExternalStatementRow, PaymentBatchRow, SimStore,
    },
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};

pub struct PaymentHubSubsystem {
    run_id: RunId,
    config: PaymentHubConfig,
    store: SimStore,
}

impl PaymentHubSubsystem {
    pub fn new(run_id: RunId, config: PaymentHubConfig, store: SimStore) -> Self {
        Self {
            run_id,
            config,
            store,
        }
    }

    /// Process card authorizations for transactions created this tick.
    /// For each card-rail transaction, creates an Authorization record and
    /// deducts the hold from available_balance.
    fn process_card_authorizations(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get card transactions created this tick that need authorization
        let card_txns = self.store.get_card_transactions_at_tick(&self.run_id, tick)?;

        for txn in &card_txns {
            // Only process debits (purchases) for card authorization
            if txn.direction != "debit" {
                // Credits on card rail settle immediately
                self.store.update_transaction_settlement_status(
                    &self.run_id,
                    &txn.txn_id,
                    "settled",
                )?;
                continue;
            }

            // Generate deterministic auth ID from tick + RNG to preserve determinism
            // Include tick to guarantee cross-tick uniqueness
            let auth_seq = rng.next_u64_below(1_000_000_000);
            let auth_id = format!("auth-{tick}-{auth_seq}");

            // Derive merchant name deterministically from counterparty-like data
            let merchant_slot = rng.next_u64_below(100);
            let merchant_name = format!("merchant-{merchant_slot}");

            let auth = AuthorizationRow {
                authorization_id: auth_id.clone(),
                account_id: txn.account_id.clone(),
                merchant_name: Some(merchant_name.clone()),
                merchant_category: Some(txn.category.clone()),
                amount: txn.amount,
                tick_authorized: tick,
                status: "pending".into(),
                tick_cleared: None,
                cleared_amount: None,
                tick_settled: None,
                interchange_fee: None,
            };

            self.store.insert_authorization(&self.run_id, &auth)?;

            // Deduct from available_balance (hold), not from posted balance
            self.store
                .update_available_balance(&self.run_id, &txn.account_id, -txn.amount)?;

            // Update transaction status to pending_settlement (auth has been created)
            self.store.update_transaction_settlement_status(
                &self.run_id,
                &txn.txn_id,
                "pending_settlement",
            )?;

            events.push(SimEvent::CardAuthorizationCreated {
                tick,
                authorization_id: auth_id,
                account_id: txn.account_id.clone(),
                amount: txn.amount,
                merchant_name,
            });
        }

        // Expire old authorizations
        let expired = self.store.expire_authorizations(
            &self.run_id,
            tick,
            self.config.auth_expiry_ticks,
        )?;

        // Release holds for expired authorizations
        for auth in &expired {
            self.store
                .update_available_balance(&self.run_id, &auth.account_id, auth.amount)?;
        }

        Ok(events)
    }

    /// Run the daily clearing batch for card authorizations.
    /// Matches yesterday's authorizations to clearing records.
    /// Updates authorization status to 'captured' with cleared amount.
    fn run_clearing_batch(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let events = Vec::new();

        // Get authorizations from yesterday that are ready to clear
        let auths_to_clear = self.store.get_authorizations_for_clearing(&self.run_id, tick)?;

        for auth in &auths_to_clear {
            // Cleared amount may differ slightly from authorized amount
            // ~95% of time it's the same, ~5% it's slightly different
            let cleared_amount = if rng.chance(0.05) {
                // Small adjustment: ±5% of original amount
                let adjustment = 1.0 + (rng.next_f64() - 0.5) * 0.10;
                (auth.amount * adjustment * 100.0).round() / 100.0
            } else {
                auth.amount
            };

            self.store.update_authorization_cleared(
                &self.run_id,
                &auth.authorization_id,
                tick,
                cleared_amount,
            )?;

            // If cleared amount differs from auth amount, adjust the available_balance hold.
            // The hold was for auth.amount; the actual charge is cleared_amount.
            let diff = auth.amount - cleared_amount;
            if diff.abs() > 0.001 {
                // Release the difference (positive diff = release hold, negative = increase hold)
                self.store
                    .update_available_balance(&self.run_id, &auth.account_id, diff)?;
            }
        }

        Ok(events)
    }

    /// Run daily settlement batch for card authorizations.
    /// Settles captured authorizations: updates posted_balance, releases holds,
    /// calculates interchange fees.
    fn run_card_settlement_batch(
        &self,
        tick: Tick,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get captured authorizations ready to settle (cleared yesterday)
        let auths_to_settle = self.store.get_authorizations_for_settlement(&self.run_id, tick)?;

        if auths_to_settle.is_empty() {
            return Ok(events);
        }

        let mut total_amount = 0.0;
        let mut item_count = 0i64;

        for auth in &auths_to_settle {
            let settled_amount = auth.cleared_amount.unwrap_or(auth.amount);
            let interchange_fee = settled_amount * self.config.interchange_fee_rate;

            // Update posted balance (deduct the settled amount)
            self.store
                .update_posted_balance(&self.run_id, &auth.account_id, -settled_amount)?;

            // Release the available_balance hold and adjust for the actual settled amount.
            // The hold was already adjusted during clearing, so now we release the remaining hold
            // and deduct the posted amount. Since available_balance already had the hold,
            // and posted_balance is now decremented, available_balance is already correct.
            // No additional available_balance change needed — the hold accounted for it.

            // Mark authorization as settled
            self.store.update_authorization_settled(
                &self.run_id,
                &auth.authorization_id,
                tick,
                interchange_fee,
            )?;

            total_amount += settled_amount;
            item_count += 1;

            events.push(SimEvent::CardSettled {
                tick,
                authorization_id: auth.authorization_id.clone(),
                original_auth_amount: auth.amount,
                settled_amount,
            });
        }

        // Create payment batch record for card settlement
        if item_count > 0 {
            let batch_id = format!("batch-card-{tick}");
            let batch = PaymentBatchRow {
                batch_id: batch_id.clone(),
                rail_id: "card".into(),
                tick_created: tick,
                tick_processed: Some(tick),
                item_count,
                total_amount,
                status: "settled".into(),
                exception_count: 0,
            };
            self.store.insert_payment_batch(&self.run_id, &batch)?;

            events.push(SimEvent::PaymentBatchSettled {
                tick,
                batch_id,
                rail_id: "card".into(),
                exceptions: 0,
            });
        }

        Ok(events)
    }

    /// Run settlement for non-card rails (ACH, wire, RTP).
    /// ACH: settles transactions from tick - 1 (T+1)
    /// Wire/RTP: settles transactions from this tick (T+0)
    fn run_non_card_settlement(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        for rail in &self.config.rails {
            if rail.rail_type == "card" {
                continue; // Card is handled separately
            }

            let settlement_tick = tick.saturating_sub(rail.settlement_delay_ticks);
            let txns = self.store.get_transactions_for_settlement(
                &self.run_id,
                &rail.rail_id,
                settlement_tick,
            )?;

            if txns.is_empty() {
                continue;
            }

            let mut total_amount = 0.0;
            let mut item_count = 0i64;
            let mut exception_count = 0i64;

            for txn in &txns {
                // Operational risk: small chance of processing failure
                if rng.chance(rail.operational_risk_base) {
                    exception_count += 1;
                    continue;
                }

                // Mark transaction as settled
                self.store
                    .mark_transaction_settled(&self.run_id, &txn.txn_id)?;

                total_amount += txn.amount;
                item_count += 1;
            }

            if item_count > 0 || exception_count > 0 {
                let batch_id = format!("batch-{}-{tick}", rail.rail_id);
                let batch = PaymentBatchRow {
                    batch_id: batch_id.clone(),
                    rail_id: rail.rail_id.clone(),
                    tick_created: tick,
                    tick_processed: Some(tick),
                    item_count,
                    total_amount,
                    status: if exception_count > 0 {
                        "settled".into()
                    } else {
                        "settled".into()
                    },
                    exception_count,
                };
                self.store.insert_payment_batch(&self.run_id, &batch)?;

                events.push(SimEvent::PaymentBatchCreated {
                    tick,
                    batch_id: batch_id.clone(),
                    rail_id: rail.rail_id.clone(),
                    item_count,
                    total_amount,
                });

                events.push(SimEvent::PaymentBatchSettled {
                    tick,
                    batch_id,
                    rail_id: rail.rail_id.clone(),
                    exceptions: exception_count,
                });
            }
        }

        Ok(events)
    }

    /// Generate external settlement statements for each rail.
    /// These represent what an external clearinghouse/network would report.
    fn generate_external_statements(
        &self,
        tick: Tick,
    ) -> SimResult<Vec<SimEvent>> {
        let events = Vec::new();

        for rail in &self.config.rails {
            // Get settlement totals for this rail at this tick
            let (total_debits, total_credits, item_count) =
                self.store
                    .settlement_totals_for_tick(&self.run_id, &rail.rail_id, tick)?;

            // Only create statement if there's activity
            if item_count > 0 {
                let stmt_id = format!("stmt-{}-{tick}", rail.rail_id);
                let stmt = ExternalStatementRow {
                    statement_id: stmt_id,
                    rail_id: rail.rail_id.clone(),
                    tick,
                    total_debits,
                    total_credits,
                    item_count,
                };
                self.store.insert_external_statement(&self.run_id, &stmt)?;
            }
        }

        Ok(events)
    }
}

impl SimSubsystem for PaymentHubSubsystem {
    fn name(&self) -> &'static str {
        "payment_hub"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        // Skip tick 0 — no transactions exist yet
        if tick == 0 {
            return Ok(out_events);
        }

        // 1. Process card authorizations for this tick's card transactions
        out_events.extend(self.process_card_authorizations(tick, rng)?);

        // 2. Run clearing batch (captures yesterday's pending auths)
        out_events.extend(self.run_clearing_batch(tick, rng)?);

        // 3. Run card settlement batch (settles yesterday's captured auths)
        out_events.extend(self.run_card_settlement_batch(tick)?);

        // 4. Run non-card rail settlement (ACH T+1, wire/RTP T+0)
        out_events.extend(self.run_non_card_settlement(tick, rng)?);

        // 5. Generate external statements for reconciliation
        out_events.extend(self.generate_external_statements(tick)?);

        log::debug!(
            "tick={tick} payment_hub: {} events",
            out_events.len()
        );

        Ok(out_events)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
