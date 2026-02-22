//! Store methods for regulatory examination (Phase 3.6).

use crate::{error::SimResult, types::Tick};
use rusqlite::params;

/// Row from the `regulatory_exam` table.
#[derive(Debug, Clone)]
pub struct RegulatoryExamRow {
    pub exam_id:        String,
    pub run_id:         String,
    pub tick_started:   Tick,
    pub tick_completed: Option<Tick>,
    pub examiner:       String,
    pub scope:          String,
    pub status:         String,
    pub finding_count:  i64,
    pub fine_total:     f64,
    pub mou_issued:     bool,
}

use super::SimStore;

impl SimStore {
    /// Open a new exam cycle.
    pub fn insert_regulatory_exam(
        &self,
        run_id:   &str,
        exam_id:  &str,
        tick:     Tick,
        examiner: &str,
        scope:    &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO regulatory_exam
             (exam_id, run_id, tick_started, examiner, scope, status,
              finding_count, fine_total, mou_issued)
             VALUES (?1, ?2, ?3, ?4, ?5, 'open', 0, 0.0, 0)",
            params![exam_id, run_id, tick as i64, examiner, scope],
        )?;
        Ok(())
    }

    /// Close an exam after inspection is complete.
    pub fn close_regulatory_exam(
        &self,
        run_id:        &str,
        exam_id:       &str,
        tick:          Tick,
        fine_total:    f64,
        finding_count: i64,
        mou_issued:    bool,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE regulatory_exam
             SET status = 'closed', tick_completed = ?1,
                 fine_total = ?2, finding_count = ?3, mou_issued = ?4
             WHERE run_id = ?5 AND exam_id = ?6",
            params![
                tick as i64, fine_total, finding_count,
                mou_issued as i64, run_id, exam_id,
            ],
        )?;
        Ok(())
    }

    /// Return the single open exam for this run (None if no exam is in progress).
    pub fn get_open_exam(&self, run_id: &str) -> SimResult<Option<RegulatoryExamRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT exam_id, run_id, tick_started, tick_completed,
                    examiner, scope, status, finding_count, fine_total, mou_issued
             FROM regulatory_exam
             WHERE run_id = ?1 AND status = 'open'
             LIMIT 1",
        )?;
        let result = stmt.query_row(params![run_id], |row| {
            Ok(RegulatoryExamRow {
                exam_id:        row.get(0)?,
                run_id:         row.get(1)?,
                tick_started:   row.get::<_, i64>(2)? as u64,
                tick_completed: row.get::<_, Option<i64>>(3)?.map(|t| t as u64),
                examiner:       row.get(4)?,
                scope:          row.get(5)?,
                status:         row.get(6)?,
                finding_count:  row.get(7)?,
                fine_total:     row.get(8)?,
                mou_issued:     row.get::<_, i64>(9)? != 0,
            })
        }).ok();
        Ok(result)
    }

    /// Record one finding within an open exam.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_exam_finding(
        &self,
        run_id:      &str,
        exam_id:     &str,
        finding_id:  &str,
        tick:        Tick,
        category:    &str,
        severity:    &str,
        description: &str,
        fine_amount: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO exam_finding
             (finding_id, exam_id, run_id, tick, category, severity,
              description, fine_amount)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                finding_id, exam_id, run_id, tick as i64,
                category, severity, description, fine_amount,
            ],
        )?;
        Ok(())
    }

    // ── Test / summary helpers ────────────────────────────────────────

    /// Total number of exam cycles (for tests).
    pub fn exam_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM regulatory_exam WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Total number of exam findings (for tests).
    pub fn exam_finding_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM exam_finding WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Sum of all fines levied across all closed exams (for tests).
    pub fn exam_fine_total(&self, run_id: &str) -> SimResult<f64> {
        let total: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(fine_total), 0.0) FROM regulatory_exam WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(total)
    }
}
