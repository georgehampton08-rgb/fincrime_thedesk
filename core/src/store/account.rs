use super::{SimStore, AccountRow};
use crate::{error::SimResult, types::Tick};
use rusqlite::params;

impl SimStore {
    // ── Account ───────────────────────────────────────────────────

    pub fn insert_account(
        &self,
        run_id: &str,
        account_id: &str,
        customer_id: &str,
        product_id: &str,
        initial_balance: f64,
        tick: Tick,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO account (account_id, run_id, customer_id, product_id, balance, available_balance, open_tick, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open')",
            params![account_id, run_id, customer_id, product_id, initial_balance, initial_balance, tick as i64],
        )?;
        Ok(())
    }

    pub fn active_accounts(&self, run_id: &str) -> SimResult<Vec<AccountRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.account_id, a.customer_id, a.product_id, a.balance,
                    c.monthly_txn_mean, c.cash_intensity, c.payroll_amount, c.has_payroll
             FROM account a
             JOIN customer c ON a.customer_id = c.customer_id AND a.run_id = c.run_id
             WHERE a.run_id = ?1 AND a.status = 'open' AND c.status = 'active'",
        )?;
        let rows = stmt.query_map(params![run_id], |row| {
            Ok(AccountRow {
                account_id: row.get(0)?,
                customer_id: row.get(1)?,
                product_id: row.get(2)?,
                balance: row.get(3)?,
                monthly_txn_mean: row.get(4)?,
                cash_intensity: row.get(5)?,
                payroll_amount: row.get(6)?,
                has_payroll: row.get::<_, i32>(7)? != 0,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn account_balance(&self, run_id: &str, account_id: &str) -> SimResult<f64> {
        let balance: f64 = self.conn.query_row(
            "SELECT balance FROM account WHERE run_id = ?1 AND account_id = ?2",
            params![run_id, account_id],
            |row| row.get(0),
        )?;
        Ok(balance)
    }

    pub fn update_account_balance(
        &self,
        run_id: &str,
        account_id: &str,
        delta: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE account SET balance = balance + ?1, available_balance = available_balance + ?1 WHERE run_id = ?2 AND account_id = ?3",
            params![delta, run_id, account_id],
        )?;
        Ok(())
    }
}
