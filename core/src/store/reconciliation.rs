use super::{SimStore, ReconExceptionRow, ReconQueueConfigRow, ReconMetricsRow};
use crate::{error::SimResult, types::Tick};
use rusqlite::params;

impl SimStore {
    pub fn insert_ledger_entry(
        &self,
        run_id: &str,
        entry_id: &str,
        rail_id: &str,
        tick: Tick,
        amount: f64,
        direction: &str,
        source_txn_id: Option<&str>,
        source_auth_id: Option<&str>,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO ledger_entry
             (entry_id, run_id, rail_id, tick, amount, direction, source_txn_id, source_auth_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                entry_id,
                run_id,
                rail_id,
                tick as i64,
                amount,
                direction,
                source_txn_id,
                source_auth_id,
            ],
        )?;
        Ok(())
    }

    /// Sum all settled ledger amounts for a given rail on a given tick.
    /// This is the internal total used for reconciliation.
    pub fn sum_settled_for_rail(
        &self,
        run_id: &str,
        rail_id: &str,
        tick: Tick,
    ) -> SimResult<f64> {
        let total: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(amount), 0.0)
             FROM ledger_entry
             WHERE run_id = ?1 AND rail_id = ?2 AND tick = ?3",
            params![run_id, rail_id, tick as i64],
            |row| row.get(0),
        )?;
        Ok(total)
    }

    // ─────────────────────────────────────────────────────────────────
    // Phase 3.2: Reconciliation exceptions
    // ─────────────────────────────────────────────────────────────────

    pub fn insert_recon_exception(&self, ex: &ReconExceptionRow) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO recon_exception
             (exception_id, run_id, rail_id, tick_detected, tick_resolved,
              status, delta_amount, internal_total, external_total,
              item_count_delta, suspected_cause, assigned_to,
              resolution_notes, resolution_type, write_off_amount)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                ex.exception_id,
                ex.run_id,
                ex.rail_id,
                ex.tick_detected as i64,
                ex.tick_resolved.map(|t| t as i64),
                ex.status,
                ex.delta_amount,
                ex.internal_total,
                ex.external_total,
                ex.item_count_delta,
                ex.suspected_cause,
                ex.assigned_to,
                ex.resolution_notes,
                ex.resolution_type,
                ex.write_off_amount,
            ],
        )?;
        Ok(())
    }

    pub fn get_open_recon_exceptions(&self, run_id: &str) -> SimResult<Vec<ReconExceptionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT exception_id, run_id, rail_id, tick_detected, tick_resolved,
                    status, delta_amount, internal_total, external_total,
                    item_count_delta, suspected_cause, assigned_to,
                    resolution_notes, resolution_type, write_off_amount
             FROM recon_exception
             WHERE run_id = ?1 AND status IN ('open','investigating')
             ORDER BY tick_detected ASC, delta_amount DESC",
        )?;
        let rows = stmt
            .query_map(params![run_id], |row| Self::map_recon_exception_row(row))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn get_recon_exceptions_by_rail(
        &self,
        run_id: &str,
        rail_id: &str,
    ) -> SimResult<Vec<ReconExceptionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT exception_id, run_id, rail_id, tick_detected, tick_resolved,
                    status, delta_amount, internal_total, external_total,
                    item_count_delta, suspected_cause, assigned_to,
                    resolution_notes, resolution_type, write_off_amount
             FROM recon_exception
             WHERE run_id = ?1 AND rail_id = ?2
             ORDER BY tick_detected ASC",
        )?;
        let rows = stmt
            .query_map(params![run_id, rail_id], |row| {
                Self::map_recon_exception_row(row)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn update_recon_exception_status(
        &self,
        run_id: &str,
        exception_id: &str,
        status: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE recon_exception SET status = ?1
             WHERE run_id = ?2 AND exception_id = ?3",
            params![status, run_id, exception_id],
        )?;
        Ok(())
    }

    pub fn resolve_recon_exception(
        &self,
        run_id: &str,
        exception_id: &str,
        tick: Tick,
        resolution_type: &str,
        notes: &str,
        write_off_amount: f64,
    ) -> SimResult<()> {
        let status = if resolution_type == "write_off" {
            "written_off"
        } else {
            "resolved"
        };
        self.conn.execute(
            "UPDATE recon_exception
             SET status = ?1, tick_resolved = ?2,
                 resolution_type = ?3, resolution_notes = ?4, write_off_amount = ?5
             WHERE run_id = ?6 AND exception_id = ?7",
            params![
                status,
                tick as i64,
                resolution_type,
                notes,
                write_off_amount,
                run_id,
                exception_id,
            ],
        )?;
        Ok(())
    }

    /// Count open exceptions older than `age_days` ticks for a run.
    pub fn count_exceptions_aged_over(&self, run_id: &str, age_days: i64) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM recon_exception
             WHERE run_id = ?1
               AND status IN ('open','investigating')
               AND tick_detected <= (SELECT MAX(tick_detected) FROM recon_exception WHERE run_id = ?1) - ?2",
            params![run_id, age_days],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn sum_write_offs(&self, run_id: &str, start_tick: Tick, end_tick: Tick) -> SimResult<f64> {
        let total: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(write_off_amount), 0.0)
             FROM recon_exception
             WHERE run_id = ?1
               AND status = 'written_off'
               AND tick_resolved >= ?2 AND tick_resolved <= ?3",
            params![run_id, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;
        Ok(total)
    }

    pub fn get_recon_queue_backlog(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM recon_exception
             WHERE run_id = ?1 AND status = 'open'",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_avg_exception_age(&self, run_id: &str, current_tick: Tick) -> SimResult<f64> {
        let avg: f64 = self.conn.query_row(
            "SELECT COALESCE(AVG(?1 - tick_detected), 0.0)
             FROM recon_exception
             WHERE run_id = ?2 AND status IN ('open','investigating')",
            params![current_tick as i64, run_id],
            |row| row.get(0),
        )?;
        Ok(avg)
    }

    // ─────────────────────────────────────────────────────────────────
    // Phase 3.2: Recon queue configuration
    // ─────────────────────────────────────────────────────────────────

    /// Look up the per-rail tolerance / SLA config from the DB.
    /// Falls back to a safe default if not seeded.
    pub fn get_recon_queue_config(
        &self,
        rail_id: &str,
    ) -> SimResult<ReconQueueConfigRow> {
        let row = self.conn.query_row(
            "SELECT rail_id, tolerance_amount, auto_clear_threshold,
                    sla_days, escalation_threshold, escalation_age_days
             FROM recon_queue_config
             WHERE rail_id = ?1",
            params![rail_id],
            |row| {
                Ok(ReconQueueConfigRow {
                    rail_id: row.get(0)?,
                    tolerance_amount: row.get(1)?,
                    auto_clear_threshold: row.get(2)?,
                    sla_days: row.get(3)?,
                    escalation_threshold: row.get(4)?,
                    escalation_age_days: row.get(5)?,
                })
            },
        );
        // If not configured, return a conservative default
        Ok(row.unwrap_or_else(|_| ReconQueueConfigRow {
            rail_id: rail_id.to_string(),
            tolerance_amount: 0.01,
            auto_clear_threshold: 1.00,
            sla_days: 3,
            escalation_threshold: 100.00,
            escalation_age_days: 7,
        }))
    }

    // ─────────────────────────────────────────────────────────────────
    // Phase 3.2: Recon metrics
    // ─────────────────────────────────────────────────────────────────

    pub fn insert_recon_metrics(&self, m: &ReconMetricsRow) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO recon_metrics
             (run_id, tick, rail_id,
              total_exceptions, open_exceptions,
              aged_exceptions_7d, aged_exceptions_14d, aged_exceptions_30d,
              total_delta_amount, unresolved_amount, write_off_amount,
              auto_cleared, manually_resolved, written_off,
              avg_resolution_days, sla_compliance_pct)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            params![
                m.run_id,
                m.tick as i64,
                m.rail_id,
                m.total_exceptions,
                m.open_exceptions,
                m.aged_exceptions_7d,
                m.aged_exceptions_14d,
                m.aged_exceptions_30d,
                m.total_delta_amount,
                m.unresolved_amount,
                m.write_off_amount,
                m.auto_cleared,
                m.manually_resolved,
                m.written_off,
                m.avg_resolution_days,
                m.sla_compliance_pct,
            ],
        )?;
        Ok(())
    }

    pub fn get_recon_metrics(
        &self,
        run_id: &str,
        rail_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<Vec<ReconMetricsRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT run_id, tick, rail_id,
                    total_exceptions, open_exceptions,
                    aged_exceptions_7d, aged_exceptions_14d, aged_exceptions_30d,
                    total_delta_amount, unresolved_amount, write_off_amount,
                    auto_cleared, manually_resolved, written_off,
                    avg_resolution_days, sla_compliance_pct
             FROM recon_metrics
             WHERE run_id = ?1 AND rail_id = ?2
               AND tick >= ?3 AND tick <= ?4
             ORDER BY tick ASC",
        )?;
        let rows = stmt
            .query_map(
                params![run_id, rail_id, start_tick as i64, end_tick as i64],
                |row| {
                    Ok(ReconMetricsRow {
                        run_id: row.get(0)?,
                        tick: row.get::<_, i64>(1)? as Tick,
                        rail_id: row.get(2)?,
                        total_exceptions: row.get(3)?,
                        open_exceptions: row.get(4)?,
                        aged_exceptions_7d: row.get(5)?,
                        aged_exceptions_14d: row.get(6)?,
                        aged_exceptions_30d: row.get(7)?,
                        total_delta_amount: row.get(8)?,
                        unresolved_amount: row.get(9)?,
                        write_off_amount: row.get(10)?,
                        auto_cleared: row.get(11)?,
                        manually_resolved: row.get(12)?,
                        written_off: row.get(13)?,
                        avg_resolution_days: row.get(14)?,
                        sla_compliance_pct: row.get(15)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // ─────────────────────────────────────────────────────────────────
    // Phase 3.2: Regulatory score components
    // ─────────────────────────────────────────────────────────────────

    pub fn insert_regulatory_score_component(
        &self,
        run_id: &str,
        tick: Tick,
        component: &str,
        score_delta: f64,
        findings: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO regulatory_score_component
             (run_id, tick, component, score_delta, findings)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, tick as i64, component, score_delta, findings],
        )?;
        Ok(())
    }

    pub fn sum_recon_score_deltas(&self, run_id: &str, tick: Tick) -> SimResult<f64> {
        let total: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(score_delta), 0.0)
             FROM regulatory_score_component
             WHERE run_id = ?1 AND tick = ?2",
            params![run_id, tick as i64],
            |row| row.get(0),
        )?;
        Ok(total)
    }

    // ─────────────────────────────────────────────────────────────────
    // Phase 3.2: Test helper methods
    // ─────────────────────────────────────────────────────────────────

    pub fn recon_exception_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM recon_exception WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn recon_metrics_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM recon_metrics WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn ledger_entry_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM ledger_entry WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn map_recon_exception_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<ReconExceptionRow> {
        Ok(ReconExceptionRow {
            exception_id: row.get(0)?,
            run_id: row.get(1)?,
            rail_id: row.get(2)?,
            tick_detected: row.get::<_, i64>(3)? as Tick,
            tick_resolved: row.get::<_, Option<i64>>(4)?.map(|t| t as Tick),
            status: row.get(5)?,
            delta_amount: row.get(6)?,
            internal_total: row.get(7)?,
            external_total: row.get(8)?,
            item_count_delta: row.get(9)?,
            suspected_cause: row.get(10)?,
            assigned_to: row.get(11)?,
            resolution_notes: row.get(12)?,
            resolution_type: row.get(13)?,
            write_off_amount: row.get(14)?,
        })
    }
}
