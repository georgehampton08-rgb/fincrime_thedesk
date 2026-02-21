use super::SimStore;
use crate::{error::SimResult, types::Tick};
use rusqlite::params;

// Helper function for mapping complaint rows
fn complaint_row_mapper(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<crate::complaint_subsystem::ComplaintRecord> {
    Ok(crate::complaint_subsystem::ComplaintRecord {
        complaint_id: row.get(0)?,
        customer_id: row.get(1)?,
        account_id: row.get(2)?,
        tick_opened: row.get::<_, i64>(3)? as u64,
        tick_closed: row.get::<_, Option<i64>>(4)?.map(|t| t as u64),
        product: row.get(5)?,
        issue: row.get(6)?,
        priority: row.get(7)?,
        status: row.get(8)?,
        sla_due_tick: row.get::<_, i64>(9)? as u64,
        sla_breached: row.get::<_, i32>(10)? != 0,
        resolution_code: row.get(11)?,
        amount_refunded: row.get(12)?,
        udaap_flag: row.get::<_, i32>(13)? != 0,
    })
}

impl SimStore {
    // ── Complaint ──────────────────────────────────────────────────

    pub fn insert_complaint(
        &self,
        run_id: &str,
        c: &crate::complaint_subsystem::ComplaintRecord,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO complaint (
                complaint_id, run_id, customer_id, account_id, tick_opened, tick_closed,
                product, issue, priority, status, sla_due_tick, sla_breached,
                resolution_code, amount_refunded, udaap_flag
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                &c.complaint_id,
                run_id,
                &c.customer_id,
                c.account_id.as_deref(),
                c.tick_opened as i64,
                c.tick_closed.map(|t| t as i64),
                &c.product,
                &c.issue,
                &c.priority,
                &c.status,
                c.sla_due_tick as i64,
                if c.sla_breached { 1i32 } else { 0i32 },
                c.resolution_code.as_deref(),
                c.amount_refunded,
                if c.udaap_flag { 1i32 } else { 0i32 },
            ],
        )?;
        Ok(())
    }

    pub fn get_complaint(
        &self,
        run_id: &str,
        complaint_id: &str,
    ) -> SimResult<crate::complaint_subsystem::ComplaintRecord> {
        self.conn
            .query_row(
                "SELECT complaint_id, customer_id, account_id, tick_opened, tick_closed,
                    product, issue, priority, status, sla_due_tick, sla_breached,
                    resolution_code, amount_refunded, udaap_flag
             FROM complaint WHERE run_id = ?1 AND complaint_id = ?2",
                params![run_id, complaint_id],
                complaint_row_mapper,
            )
            .map_err(Into::into)
    }

    pub fn open_complaints(
        &self,
        run_id: &str,
    ) -> SimResult<Vec<crate::complaint_subsystem::ComplaintRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT complaint_id, customer_id, account_id, tick_opened, tick_closed,
                    product, issue, priority, status, sla_due_tick, sla_breached,
                    resolution_code, amount_refunded, udaap_flag
             FROM complaint WHERE run_id = ?1 AND status = 'open'
             ORDER BY tick_opened ASC",
        )?;
        let rows = stmt.query_map(params![run_id], complaint_row_mapper)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn close_complaint(
        &self,
        run_id: &str,
        complaint_id: &str,
        tick: Tick,
        resolution_code: &str,
        amount_refunded: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE complaint SET status = 'closed', tick_closed = ?1,
             resolution_code = ?2, amount_refunded = ?3
             WHERE run_id = ?4 AND complaint_id = ?5",
            params![
                tick as i64,
                resolution_code,
                amount_refunded,
                run_id,
                complaint_id
            ],
        )?;
        Ok(())
    }

    pub fn mark_complaint_sla_breach(&self, run_id: &str, complaint_id: &str) -> SimResult<()> {
        self.conn.execute(
            "UPDATE complaint SET sla_breached = 1 WHERE run_id = ?1 AND complaint_id = ?2",
            params![run_id, complaint_id],
        )?;
        Ok(())
    }
}
