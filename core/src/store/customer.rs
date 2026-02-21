use super::SimStore;
use crate::{error::SimResult, types::Tick};
use rusqlite::params;

impl SimStore {
    // ── Customer ──────────────────────────────────────────────────

    pub fn insert_customer(
        &self,
        run_id: &str,
        c: &crate::customer_subsystem::CustomerRecord,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO customer (
                customer_id, run_id, name, segment, income_band, risk_band, open_tick,
                status, churn_risk, satisfaction, monthly_txn_mean, cash_intensity,
                payroll_amount, has_payroll
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &c.customer_id,
                run_id,
                &c.name,
                &c.segment,
                &c.income_band,
                &c.risk_band,
                c.open_tick as i64,
                &c.status,
                c.churn_risk,
                c.satisfaction,
                c.monthly_txn_mean,
                c.cash_intensity,
                c.payroll_amount,
                if c.has_payroll { 1 } else { 0 }
            ],
        )?;
        Ok(())
    }

    pub fn active_customers(
        &self,
        run_id: &str,
    ) -> SimResult<Vec<crate::customer_subsystem::CustomerRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT customer_id, name, segment, income_band, risk_band, open_tick,
                    status, churn_risk, satisfaction, monthly_txn_mean, cash_intensity,
                    payroll_amount, has_payroll
             FROM customer WHERE run_id = ?1 AND status = 'active'",
        )?;
        let rows = stmt.query_map(params![run_id], |row| {
            Ok(crate::customer_subsystem::CustomerRecord {
                customer_id: row.get(0)?,
                name: row.get(1)?,
                segment: row.get(2)?,
                income_band: row.get(3)?,
                risk_band: row.get(4)?,
                open_tick: row.get::<_, i64>(5)? as u64,
                status: row.get(6)?,
                churn_risk: row.get(7)?,
                satisfaction: row.get(8)?,
                monthly_txn_mean: row.get(9)?,
                cash_intensity: row.get(10)?,
                payroll_amount: row.get(11)?,
                has_payroll: row.get::<_, i32>(12)? != 0,
                product_id: String::new(), // Filled from account if needed
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn churn_customer(&self, run_id: &str, customer_id: &str, tick: Tick) -> SimResult<()> {
        self.conn.execute(
            "UPDATE customer SET status = 'churned', close_tick = ?1
             WHERE run_id = ?2 AND customer_id = ?3",
            params![tick as i64, run_id, customer_id],
        )?;
        // Also close all accounts, recording close tick
        self.conn.execute(
            "UPDATE account SET status = 'closed', close_tick = ?1
             WHERE run_id = ?2 AND customer_id = ?3 AND status = 'open'",
            params![tick as i64, run_id, customer_id],
        )?;
        Ok(())
    }
}
