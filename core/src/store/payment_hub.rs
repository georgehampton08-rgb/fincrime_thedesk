use super::{SimStore, AuthorizationRow, PaymentBatchRow, ExternalStatementRow, TransactionForSettlement};
use crate::{error::SimResult, types::Tick};
use rusqlite::{params, OptionalExtension};

impl SimStore {
    // ── Payment hub: authorization lifecycle ────────────────────────

    pub fn insert_authorization(
        &self,
        run_id: &str,
        auth: &AuthorizationRow,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO authorization (
                authorization_id, run_id, account_id, merchant_name, merchant_category,
                amount, tick_authorized, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                auth.authorization_id,
                run_id,
                auth.account_id,
                auth.merchant_name,
                auth.merchant_category,
                auth.amount,
                auth.tick_authorized as i64,
                auth.status,
            ],
        )?;
        Ok(())
    }

    pub fn get_authorization(
        &self,
        run_id: &str,
        auth_id: &str,
    ) -> SimResult<AuthorizationRow> {
        let row = self.conn.query_row(
            "SELECT authorization_id, account_id, merchant_name, merchant_category,
                    amount, tick_authorized, status, tick_cleared, cleared_amount,
                    tick_settled, interchange_fee
             FROM authorization
             WHERE run_id = ?1 AND authorization_id = ?2",
            params![run_id, auth_id],
            |row| {
                Ok(AuthorizationRow {
                    authorization_id: row.get(0)?,
                    account_id: row.get(1)?,
                    merchant_name: row.get(2)?,
                    merchant_category: row.get(3)?,
                    amount: row.get(4)?,
                    tick_authorized: row.get::<_, i64>(5)? as u64,
                    status: row.get(6)?,
                    tick_cleared: row.get::<_, Option<i64>>(7)?.map(|t| t as u64),
                    cleared_amount: row.get(8)?,
                    tick_settled: row.get::<_, Option<i64>>(9)?.map(|t| t as u64),
                    interchange_fee: row.get(10)?,
                })
            },
        )?;
        Ok(row)
    }

    pub fn update_authorization_cleared(
        &self,
        run_id: &str,
        auth_id: &str,
        tick: Tick,
        cleared_amount: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE authorization SET status = 'captured', tick_cleared = ?1, cleared_amount = ?2
             WHERE run_id = ?3 AND authorization_id = ?4 AND status = 'pending'",
            params![tick as i64, cleared_amount, run_id, auth_id],
        )?;
        Ok(())
    }

    pub fn update_authorization_settled(
        &self,
        run_id: &str,
        auth_id: &str,
        tick: Tick,
        interchange_fee: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE authorization SET status = 'settled', tick_settled = ?1, interchange_fee = ?2
             WHERE run_id = ?3 AND authorization_id = ?4 AND status = 'captured'",
            params![tick as i64, interchange_fee, run_id, auth_id],
        )?;
        Ok(())
    }

    pub fn expire_authorizations(
        &self,
        run_id: &str,
        tick: Tick,
        expiry_ticks: Tick,
    ) -> SimResult<Vec<AuthorizationRow>> {
        let cutoff = tick.saturating_sub(expiry_ticks) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT authorization_id, account_id, merchant_name, merchant_category,
                    amount, tick_authorized, status, tick_cleared, cleared_amount,
                    tick_settled, interchange_fee
             FROM authorization
             WHERE run_id = ?1 AND status = 'pending' AND tick_authorized <= ?2",
        )?;
        let rows: Vec<AuthorizationRow> = stmt
            .query_map(params![run_id, cutoff], |row| {
                Ok(AuthorizationRow {
                    authorization_id: row.get(0)?,
                    account_id: row.get(1)?,
                    merchant_name: row.get(2)?,
                    merchant_category: row.get(3)?,
                    amount: row.get(4)?,
                    tick_authorized: row.get::<_, i64>(5)? as u64,
                    status: row.get(6)?,
                    tick_cleared: row.get::<_, Option<i64>>(7)?.map(|t| t as u64),
                    cleared_amount: row.get(8)?,
                    tick_settled: row.get::<_, Option<i64>>(9)?.map(|t| t as u64),
                    interchange_fee: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Mark them expired
        self.conn.execute(
            "UPDATE authorization SET status = 'expired'
             WHERE run_id = ?1 AND status = 'pending' AND tick_authorized <= ?2",
            params![run_id, cutoff],
        )?;

        Ok(rows)
    }

    pub fn get_pending_authorizations(
        &self,
        run_id: &str,
        account_id: &str,
    ) -> SimResult<Vec<AuthorizationRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT authorization_id, account_id, merchant_name, merchant_category,
                    amount, tick_authorized, status, tick_cleared, cleared_amount,
                    tick_settled, interchange_fee
             FROM authorization
             WHERE run_id = ?1 AND account_id = ?2 AND status = 'pending'",
        )?;
        let rows = stmt
            .query_map(params![run_id, account_id], |row| {
                Ok(AuthorizationRow {
                    authorization_id: row.get(0)?,
                    account_id: row.get(1)?,
                    merchant_name: row.get(2)?,
                    merchant_category: row.get(3)?,
                    amount: row.get(4)?,
                    tick_authorized: row.get::<_, i64>(5)? as u64,
                    status: row.get(6)?,
                    tick_cleared: row.get::<_, Option<i64>>(7)?.map(|t| t as u64),
                    cleared_amount: row.get(8)?,
                    tick_settled: row.get::<_, Option<i64>>(9)?.map(|t| t as u64),
                    interchange_fee: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Get authorizations that were authorized at `tick - 1` and are ready to clear.
    pub fn get_authorizations_for_clearing(
        &self,
        run_id: &str,
        tick: Tick,
    ) -> SimResult<Vec<AuthorizationRow>> {
        let clearing_tick = tick.saturating_sub(1) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT authorization_id, account_id, merchant_name, merchant_category,
                    amount, tick_authorized, status, tick_cleared, cleared_amount,
                    tick_settled, interchange_fee
             FROM authorization
             WHERE run_id = ?1 AND status = 'pending' AND tick_authorized = ?2
             ORDER BY authorization_id ASC",
        )?;
        let rows = stmt
            .query_map(params![run_id, clearing_tick], |row| {
                Ok(AuthorizationRow {
                    authorization_id: row.get(0)?,
                    account_id: row.get(1)?,
                    merchant_name: row.get(2)?,
                    merchant_category: row.get(3)?,
                    amount: row.get(4)?,
                    tick_authorized: row.get::<_, i64>(5)? as u64,
                    status: row.get(6)?,
                    tick_cleared: row.get::<_, Option<i64>>(7)?.map(|t| t as u64),
                    cleared_amount: row.get(8)?,
                    tick_settled: row.get::<_, Option<i64>>(9)?.map(|t| t as u64),
                    interchange_fee: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Get captured authorizations ready to settle (cleared at `tick - 1`).
    pub fn get_authorizations_for_settlement(
        &self,
        run_id: &str,
        tick: Tick,
    ) -> SimResult<Vec<AuthorizationRow>> {
        let settle_tick = tick.saturating_sub(1) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT authorization_id, account_id, merchant_name, merchant_category,
                    amount, tick_authorized, status, tick_cleared, cleared_amount,
                    tick_settled, interchange_fee
             FROM authorization
             WHERE run_id = ?1 AND status = 'captured' AND tick_cleared = ?2
             ORDER BY authorization_id ASC",
        )?;
        let rows = stmt
            .query_map(params![run_id, settle_tick], |row| {
                Ok(AuthorizationRow {
                    authorization_id: row.get(0)?,
                    account_id: row.get(1)?,
                    merchant_name: row.get(2)?,
                    merchant_category: row.get(3)?,
                    amount: row.get(4)?,
                    tick_authorized: row.get::<_, i64>(5)? as u64,
                    status: row.get(6)?,
                    tick_cleared: row.get::<_, Option<i64>>(7)?.map(|t| t as u64),
                    cleared_amount: row.get(8)?,
                    tick_settled: row.get::<_, Option<i64>>(9)?.map(|t| t as u64),
                    interchange_fee: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // ── Payment hub: available balance ─────────────────────────────

    /// Adjust available_balance only (not posted/balance). Used for card auth holds.
    pub fn update_available_balance(
        &self,
        run_id: &str,
        account_id: &str,
        delta: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE account SET available_balance = available_balance + ?1
             WHERE run_id = ?2 AND account_id = ?3",
            params![delta, run_id, account_id],
        )?;
        Ok(())
    }

    /// Adjust posted balance only (not available_balance). Used for card settlement.
    pub fn update_posted_balance(
        &self,
        run_id: &str,
        account_id: &str,
        delta: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE account SET balance = balance + ?1
             WHERE run_id = ?2 AND account_id = ?3",
            params![delta, run_id, account_id],
        )?;
        Ok(())
    }

    /// Get (posted_balance, available_balance) for an account.
    pub fn get_account_balances(
        &self,
        run_id: &str,
        account_id: &str,
    ) -> SimResult<(f64, f64)> {
        let result = self.conn.query_row(
            "SELECT balance, available_balance FROM account
             WHERE run_id = ?1 AND account_id = ?2",
            params![run_id, account_id],
            |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
        )?;
        Ok(result)
    }

    // ── Payment hub: batch tracking ───────────────────────────────

    pub fn insert_payment_batch(
        &self,
        run_id: &str,
        batch: &PaymentBatchRow,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO payment_batch (
                batch_id, run_id, rail_id, tick_created, tick_processed,
                item_count, total_amount, status, exception_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                batch.batch_id,
                run_id,
                batch.rail_id,
                batch.tick_created as i64,
                batch.tick_processed.map(|t| t as i64),
                batch.item_count,
                batch.total_amount,
                batch.status,
                batch.exception_count,
            ],
        )?;
        Ok(())
    }

    pub fn update_batch_status(
        &self,
        run_id: &str,
        batch_id: &str,
        status: &str,
        tick_processed: Tick,
        exception_count: i64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE payment_batch SET status = ?1, tick_processed = ?2, exception_count = ?3
             WHERE run_id = ?4 AND batch_id = ?5",
            params![status, tick_processed as i64, exception_count, run_id, batch_id],
        )?;
        Ok(())
    }

    // ── Payment hub: external statements ──────────────────────────

    pub fn insert_external_statement(
        &self,
        run_id: &str,
        stmt_row: &ExternalStatementRow,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO external_statement (
                statement_id, run_id, rail_id, tick, total_debits, total_credits, item_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                stmt_row.statement_id,
                run_id,
                stmt_row.rail_id,
                stmt_row.tick as i64,
                stmt_row.total_debits,
                stmt_row.total_credits,
                stmt_row.item_count,
            ],
        )?;
        Ok(())
    }

    pub fn get_external_statement(
        &self,
        run_id: &str,
        rail_id: &str,
        tick: Tick,
    ) -> SimResult<Option<ExternalStatementRow>> {
        let result = self
            .conn
            .query_row(
                "SELECT statement_id, rail_id, tick, total_debits, total_credits, item_count
                 FROM external_statement
                 WHERE run_id = ?1 AND rail_id = ?2 AND tick = ?3",
                params![run_id, rail_id, tick as i64],
                |row| {
                    Ok(ExternalStatementRow {
                        statement_id: row.get(0)?,
                        rail_id: row.get(1)?,
                        tick: row.get::<_, i64>(2)? as u64,
                        total_debits: row.get(3)?,
                        total_credits: row.get(4)?,
                        item_count: row.get(5)?,
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    /// Alias used by ReconciliationSubsystem. Identical to `get_external_statement`.
    pub fn get_external_statement_for_tick(
        &self,
        run_id: &str,
        rail_id: &str,
        tick: Tick,
    ) -> SimResult<Option<ExternalStatementRow>> {
        self.get_external_statement(run_id, rail_id, tick)
    }

    // ── Payment hub: transaction settlement queries ───────────────

    /// Get unsettled transactions ready to settle for a specific rail.
    /// For ACH: transactions created at tick - settlement_delay_ticks.
    pub fn get_transactions_for_settlement(
        &self,
        run_id: &str,
        rail_id: &str,
        created_at_tick: Tick,
    ) -> SimResult<Vec<TransactionForSettlement>> {
        let mut stmt = self.conn.prepare(
            "SELECT txn_id, account_id, amount, direction, category
             FROM transactions
             WHERE run_id = ?1 AND payment_rail_id = ?2 AND tick = ?3
               AND settlement_status = 'pending_settlement'
             ORDER BY account_id ASC, amount ASC",
        )?;
        let rows = stmt
            .query_map(params![run_id, rail_id, created_at_tick as i64], |row| {
                Ok(TransactionForSettlement {
                    txn_id: row.get(0)?,
                    account_id: row.get(1)?,
                    amount: row.get(2)?,
                    direction: row.get(3)?,
                    category: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn mark_transaction_settled(
        &self,
        run_id: &str,
        txn_id: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE transactions SET settlement_status = 'settled'
             WHERE run_id = ?1 AND txn_id = ?2",
            params![run_id, txn_id],
        )?;
        Ok(())
    }

    /// Sum settled transaction volume for a rail at a specific tick, broken out by direction.
    pub fn settlement_totals_for_tick(
        &self,
        run_id: &str,
        rail_id: &str,
        tick: Tick,
    ) -> SimResult<(f64, f64, i64)> {
        // Sum debits and credits separately
        let result = self.conn.query_row(
            "SELECT
                 COALESCE(SUM(CASE WHEN direction = 'debit' THEN amount ELSE 0 END), 0.0),
                 COALESCE(SUM(CASE WHEN direction = 'credit' THEN amount ELSE 0 END), 0.0),
                 COUNT(*)
             FROM transactions
             WHERE run_id = ?1 AND payment_rail_id = ?2 AND tick = ?3
               AND settlement_status = 'settled'",
            params![run_id, rail_id, tick as i64],
            |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?, row.get::<_, i64>(2)?)),
        )?;
        Ok(result)
    }

    // ── Payment hub: query helpers ────────────────────────────────

    pub fn authorization_count(
        &self,
        run_id: &str,
        status: &str,
    ) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM authorization WHERE run_id = ?1 AND status = ?2",
                params![run_id, status],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }
}
