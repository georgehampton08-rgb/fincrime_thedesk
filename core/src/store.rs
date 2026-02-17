//! SQLite persistence layer.
//!
//! RULE: Only store.rs talks to the database.
//! Subsystems call store methods — they never execute SQL directly.

use rusqlite::{Connection, params};
use crate::{
    error::SimResult,
    event::EventLogEntry,
    types::Tick,
};

pub struct SimStore {
    conn: Connection,
}

impl SimStore {
    /// Open (or create) the simulation database at `path`.
    pub fn open(path: &str) -> SimResult<Self> {
        let conn = Connection::open(path)?;
        // WAL mode: better concurrent read performance.
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (used in tests).
    pub fn in_memory() -> SimResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    /// Apply all schema migrations in order.
    pub fn migrate(&self) -> SimResult<()> {
        self.conn.execute_batch(include_str!("../../migrations/001_foundation.sql"))?;
        Ok(())
    }

    // ── Run ────────────────────────────────────────────────────

    pub fn insert_run(&self, run_id: &str, seed: u64, version: &str) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO run (run_id, seed, version, started_at) VALUES (?1, ?2, ?3, ?4)",
            params![run_id, seed as i64, version, 0i64],
        )?;
        Ok(())
    }

    // ── Event log ──────────────────────────────────────────────

    pub fn append_event(&self, entry: &EventLogEntry) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO event_log (run_id, tick, subsystem, event_type, payload, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entry.run_id,
                entry.tick as i64,
                entry.subsystem,
                entry.event_type,
                entry.payload,
                entry.tick as i64,
            ],
        )?;
        Ok(())
    }

    pub fn events_for_tick(&self, run_id: &str, tick: Tick) -> SimResult<Vec<EventLogEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, tick, subsystem, event_type, payload
             FROM event_log WHERE run_id = ?1 AND tick = ?2
             ORDER BY id ASC"
        )?;
        let entries = stmt.query_map(params![run_id, tick as i64], |row| {
            Ok(EventLogEntry {
                id:         Some(row.get(0)?),
                run_id:     row.get(1)?,
                tick:       row.get::<_, i64>(2)? as u64,
                subsystem:  row.get(3)?,
                event_type: row.get(4)?,
                payload:    row.get(5)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    // ── Snapshot ───────────────────────────────────────────────

    pub fn save_snapshot(&self, run_id: &str, tick: Tick, state_json: &str) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO snapshot (run_id, tick, state_json) VALUES (?1, ?2, ?3)",
            params![run_id, tick as i64, state_json],
        )?;
        Ok(())
    }

    pub fn latest_snapshot_before(
        &self, run_id: &str, tick: Tick
    ) -> SimResult<Option<(Tick, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT tick, state_json FROM snapshot
             WHERE run_id = ?1 AND tick <= ?2
             ORDER BY tick DESC LIMIT 1"
        )?;
        let result = stmt.query_row(params![run_id, tick as i64], |row| {
            Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?))
        }).ok();
        Ok(result)
    }
}
