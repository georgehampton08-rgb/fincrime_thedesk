//! Store methods for reputation management (Phase 3.6).

use crate::{error::SimResult, types::Tick};
use rusqlite::params;

use super::SimStore;

impl SimStore {
    /// Persist the daily reputation snapshot.
    pub fn insert_reputation_snapshot(
        &self,
        run_id: &str,
        tick:   Tick,
        score:  f64,
        delta:  f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO reputation_snapshot (run_id, tick, score, delta)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, tick as i64, score, delta],
        )?;
        Ok(())
    }

    /// Return the most recent reputation score for this run.
    /// Returns the configured initial score (75.0) if no snapshots exist yet.
    pub fn latest_reputation_score(&self, run_id: &str) -> SimResult<f64> {
        let score: f64 = self.conn.query_row(
            "SELECT score FROM reputation_snapshot
             WHERE run_id = ?1
             ORDER BY tick DESC LIMIT 1",
            params![run_id],
            |row| row.get(0),
        ).unwrap_or(75.0);
        Ok(score)
    }

    /// Record an individual reputation driver event.
    pub fn insert_reputation_event(
        &self,
        run_id:      &str,
        tick:        Tick,
        driver:      &str,
        delta:       f64,
        description: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO reputation_event (run_id, tick, driver, delta, description)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, tick as i64, driver, delta, description],
        )?;
        Ok(())
    }

    // ── Test / summary helpers ────────────────────────────────────────

    /// Number of daily reputation snapshots persisted (for tests).
    pub fn reputation_snapshot_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM reputation_snapshot WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Number of reputation driver events logged (for tests).
    pub fn reputation_event_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM reputation_event WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}
