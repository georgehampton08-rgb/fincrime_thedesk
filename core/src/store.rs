//! SQLite persistence layer.
//!
//! RULE: Only store.rs talks to the database.
//! Subsystems call store methods — they never execute SQL directly.

use crate::{error::SimResult, event::EventLogEntry, types::Tick};
use rusqlite::{params, Connection, OptionalExtension};

pub struct SimStore {
    conn: Connection,
    path: Option<String>, // None for :memory:, Some(path) for file
}

impl SimStore {
    pub fn open(path: &str) -> SimResult<Self> {
        let conn = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE
                | rusqlite::OpenFlags::SQLITE_OPEN_CREATE
                | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?;
        // WAL mode only for real files (shared-memory and :memory: ignore it).
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Ok(Self {
            conn,
            path: Some(path.to_string()),
        })
    }

    /// Open an in-memory database (used in tests).
    pub fn in_memory() -> SimResult<Self> {
        let conn = Connection::open(":memory:")?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn, path: None })
    }

    /// Reopen a new connection to the same database.
    /// For in-memory databases, this returns a new in-memory database (isolated).
    /// For file-based databases, this opens the same file.
    pub fn reopen(&self) -> SimResult<Self> {
        match &self.path {
            Some(p) => Self::open(p),
            None => Self::in_memory(),
        }
    }

    /// Apply all schema migrations in order.
    pub fn migrate(&self) -> SimResult<()> {
        self.conn
            .execute_batch(include_str!("../../migrations/001_foundation.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/002_macro.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/003_customers.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/004_complaints.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/005_economics.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/006_pricing.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/007_offers.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/008_churn.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/009_customer_close_tick.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/010_segment_pnl.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/011_complaint_analytics.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/012_risk_appetite.sql"))?;
        self.conn
            .execute_batch(include_str!("../../migrations/013_payment_rails.sql"))?;
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
             ORDER BY id ASC",
        )?;
        let entries = stmt
            .query_map(params![run_id, tick as i64], |row| {
                Ok(EventLogEntry {
                    id: Some(row.get(0)?),
                    run_id: row.get(1)?,
                    tick: row.get::<_, i64>(2)? as u64,
                    subsystem: row.get(3)?,
                    event_type: row.get(4)?,
                    payload: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
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
        &self,
        run_id: &str,
        tick: Tick,
    ) -> SimResult<Option<(Tick, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT tick, state_json FROM snapshot
             WHERE run_id = ?1 AND tick <= ?2
             ORDER BY tick DESC LIMIT 1",
        )?;
        let result = stmt
            .query_row(params![run_id, tick as i64], |row| {
                Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?))
            })
            .ok();
        Ok(result)
    }

    // ── Customer ──────────────────────────────────────────────────

    pub fn insert_customer(
        &self,
        run_id: &str,
        c: &crate::customer_subsystem::CustomerRecord,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO customer (
                customer_id, run_id, segment, income_band, risk_band, open_tick,
                status, churn_risk, satisfaction, monthly_txn_mean, cash_intensity,
                payroll_amount, has_payroll
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &c.customer_id,
                run_id,
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
            "SELECT customer_id, segment, income_band, risk_band, open_tick,
                    status, churn_risk, satisfaction, monthly_txn_mean, cash_intensity,
                    payroll_amount, has_payroll
             FROM customer WHERE run_id = ?1 AND status = 'active'",
        )?;
        let rows = stmt.query_map(params![run_id], |row| {
            Ok(crate::customer_subsystem::CustomerRecord {
                customer_id: row.get(0)?,
                segment: row.get(1)?,
                income_band: row.get(2)?,
                risk_band: row.get(3)?,
                open_tick: row.get::<_, i64>(4)? as u64,
                status: row.get(5)?,
                churn_risk: row.get(6)?,
                satisfaction: row.get(7)?,
                monthly_txn_mean: row.get(8)?,
                cash_intensity: row.get(9)?,
                payroll_amount: row.get(10)?,
                has_payroll: row.get::<_, i32>(11)? != 0,
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

    // ── Churn scoring ──────────────────────────────────────────

    pub fn insert_churn_score(
        &self,
        run_id: &str,
        score: &crate::churn_subsystem::ChurnScore,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO customer_churn_score (
                run_id, customer_id, tick, churn_risk,
                base_rate, satisfaction_component, fee_burden_component,
                complaint_component, sla_breach_component, inactivity_component,
                product_depth_bonus, retention_offer_bonus, life_event_multiplier,
                predicted_churn_30d, predicted_churn_90d
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
            ON CONFLICT(run_id, customer_id, tick) DO UPDATE SET
                churn_risk = excluded.churn_risk",
            params![
                run_id,
                score.customer_id,
                score.tick as i64,
                score.churn_risk,
                score.base_rate,
                score.satisfaction_component,
                score.fee_burden_component,
                score.complaint_component,
                score.sla_breach_component,
                score.inactivity_component,
                score.product_depth_bonus,
                score.retention_offer_bonus,
                score.life_event_multiplier,
                score.predicted_churn_30d,
                score.predicted_churn_90d,
            ],
        )?;
        Ok(())
    }

    pub fn get_customer_churn_inputs(
        &self,
        run_id: &str,
        customer_id: &str,
        tick: Tick,
    ) -> SimResult<crate::churn_subsystem::CustomerChurnInputs> {
        let (segment, open_tick, satisfaction): (String, i64, f64) = self.conn.query_row(
            "SELECT segment, open_tick, satisfaction
             FROM customer
             WHERE run_id = ?1 AND customer_id = ?2",
            params![run_id, customer_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        let lookback_90 = tick.saturating_sub(90) as i64;
        let tick_i = tick as i64;

        let fee_burden_90d: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(t.amount), 0.0)
             FROM transactions t
             JOIN account a ON t.account_id = a.account_id AND a.run_id = t.run_id
             WHERE t.run_id = ?1 AND a.customer_id = ?2
               AND t.category IN ('overdraft_fee', 'nsf_fee', 'monthly_fee')
               AND t.tick >= ?3 AND t.tick <= ?4",
                params![run_id, customer_id, lookback_90, tick_i],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let complaints_90d: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
             FROM complaint
             WHERE run_id = ?1 AND customer_id = ?2
               AND tick_opened >= ?3 AND tick_opened <= ?4",
                params![run_id, customer_id, lookback_90, tick_i],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let sla_breaches_90d: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
             FROM complaint
             WHERE run_id = ?1 AND customer_id = ?2
               AND sla_breached = 1
               AND tick_opened >= ?3 AND tick_opened <= ?4",
                params![run_id, customer_id, lookback_90, tick_i],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let last_txn_tick: Option<i64> = self
            .conn
            .query_row(
                "SELECT MAX(t.tick)
             FROM transactions t
             JOIN account a ON t.account_id = a.account_id AND a.run_id = t.run_id
             WHERE t.run_id = ?1 AND a.customer_id = ?2",
                params![run_id, customer_id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();

        let ticks_since_last_txn = last_txn_tick
            .map(|t| tick.saturating_sub(t as u64))
            .unwrap_or(tick);

        let product_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
             FROM account
             WHERE run_id = ?1 AND customer_id = ?2 AND status = 'open'",
                params![run_id, customer_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let has_offer: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) > 0
             FROM customer_offer
             WHERE run_id = ?1 AND customer_id = ?2
               AND status IN ('in_progress', 'completed')
               AND offer_id LIKE '%retention%'",
                params![run_id, customer_id],
                |row| row.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);

        let life_event_delta: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(churn_risk_delta), 0.0)
             FROM life_event
             WHERE run_id = ?1 AND customer_id = ?2 AND active = 1",
                params![run_id, customer_id],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        Ok(crate::churn_subsystem::CustomerChurnInputs {
            customer_id: customer_id.to_string(),
            segment,
            open_tick: open_tick as u64,
            satisfaction,
            fee_burden_90d,
            complaints_90d,
            sla_breaches_90d,
            ticks_since_last_txn,
            product_count: product_count as usize,
            has_active_retention_offer: has_offer,
            active_life_event_delta: life_event_delta,
        })
    }

    pub fn churn_score_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM customer_churn_score WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn churn_scores_at_tick(
        &self,
        run_id: &str,
        tick: Tick,
    ) -> SimResult<Vec<crate::churn_subsystem::ChurnScore>> {
        let mut stmt = self.conn.prepare(
            "SELECT customer_id, tick, churn_risk,
                    base_rate, satisfaction_component, fee_burden_component,
                    complaint_component, sla_breach_component, inactivity_component,
                    product_depth_bonus, retention_offer_bonus, life_event_multiplier,
                    predicted_churn_30d, predicted_churn_90d
             FROM customer_churn_score
             WHERE run_id = ?1 AND tick = ?2",
        )?;

        let scores = stmt
            .query_map(params![run_id, tick as i64], |row| {
                Ok(crate::churn_subsystem::ChurnScore {
                    customer_id: row.get(0)?,
                    tick: row.get::<_, i64>(1)? as u64,
                    churn_risk: row.get(2)?,
                    base_rate: row.get(3)?,
                    satisfaction_component: row.get(4)?,
                    fee_burden_component: row.get(5)?,
                    complaint_component: row.get(6)?,
                    sla_breach_component: row.get(7)?,
                    inactivity_component: row.get(8)?,
                    product_depth_bonus: row.get(9)?,
                    retention_offer_bonus: row.get(10)?,
                    life_event_multiplier: row.get(11)?,
                    predicted_churn_30d: row.get(12)?,
                    predicted_churn_90d: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(scores)
    }

    // ── Life events ────────────────────────────────────────────

    pub fn insert_life_event(
        &self,
        run_id: &str,
        event: &crate::churn_subsystem::LifeEvent,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO life_event (
                run_id, customer_id, event_type,
                tick_occurred, tick_expires, active,
                churn_risk_delta, behavioral_changes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_id,
                event.customer_id,
                event.event_type,
                event.tick_occurred as i64,
                event.tick_expires as i64,
                if event.active { 1i32 } else { 0i32 },
                event.churn_risk_delta,
                event.behavioral_changes.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn expire_life_events(&self, run_id: &str, tick: Tick) -> SimResult<()> {
        self.conn.execute(
            "UPDATE life_event SET active = 0
             WHERE run_id = ?1 AND tick_expires <= ?2 AND active = 1",
            params![run_id, tick as i64],
        )?;
        Ok(())
    }

    pub fn life_event_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM life_event WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    // ── Churn cohorts ──────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn insert_churn_cohort(
        &self,
        run_id: &str,
        cohort_id: &str,
        tick: Tick,
        segment: &str,
        tenure: Tick,
        final_risk: f64,
        final_sat: f64,
        complaints: i64,
        fee_burden: f64,
        had_offer: bool,
        driver: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO churn_cohort (
                run_id, cohort_id, tick_churned, segment,
                tenure_ticks, final_churn_risk, final_satisfaction,
                total_complaints, total_fee_burden,
                had_retention_offer, primary_churn_driver
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                run_id,
                cohort_id,
                tick as i64,
                segment,
                tenure as i64,
                final_risk,
                final_sat,
                complaints,
                fee_burden,
                if had_offer { 1i32 } else { 0i32 },
                driver,
            ],
        )?;
        Ok(())
    }

    pub fn all_churn_cohorts(&self, run_id: &str) -> SimResult<Vec<ChurnCohortRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT cohort_id, tick_churned, segment,
                    tenure_ticks, final_churn_risk, final_satisfaction,
                    total_complaints, total_fee_burden,
                    had_retention_offer, primary_churn_driver
             FROM churn_cohort
             WHERE run_id = ?1
             ORDER BY tick_churned ASC",
        )?;

        let cohorts = stmt
            .query_map(params![run_id], |row| {
                Ok(ChurnCohortRecord {
                    cohort_id: row.get(0)?,
                    tick_churned: row.get::<_, i64>(1)? as u64,
                    segment: row.get(2)?,
                    tenure_ticks: row.get::<_, i64>(3)? as u64,
                    final_churn_risk: row.get(4)?,
                    final_satisfaction: row.get(5)?,
                    total_complaints: row.get(6)?,
                    total_fee_burden: row.get(7)?,
                    had_retention_offer: row.get::<_, i32>(8)? != 0,
                    primary_driver: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(cohorts)
    }

    // ── Churn aggregates ───────────────────────────────────────

    pub fn compute_churn_aggregate(
        &self,
        run_id: &str,
        segment: &str,
        tick: Tick,
    ) -> SimResult<crate::churn_subsystem::ChurnAggregate> {
        let active: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM customer
             WHERE run_id = ?1 AND segment = ?2 AND status = 'active'",
                params![run_id, segment],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let lookback = tick.saturating_sub(30) as i64;
        let tick_i = tick as i64;

        let churned: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM customer
             WHERE run_id = ?1 AND segment = ?2 AND status = 'churned'
               AND close_tick IS NOT NULL
               AND close_tick >= ?3 AND close_tick <= ?4",
                params![run_id, segment, lookback, tick_i],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let high_risk: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
             FROM customer_churn_score s
             JOIN customer c ON s.customer_id = c.customer_id AND s.run_id = c.run_id
             WHERE s.run_id = ?1 AND s.tick = ?2
               AND c.segment = ?3 AND c.status = 'active'
               AND s.churn_risk >= 0.85",
                params![run_id, tick_i, segment],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let avg_risk: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(AVG(s.churn_risk), 0.0)
             FROM customer_churn_score s
             JOIN customer c ON s.customer_id = c.customer_id AND s.run_id = c.run_id
             WHERE s.run_id = ?1 AND s.tick = ?2
               AND c.segment = ?3 AND c.status = 'active'",
                params![run_id, tick_i, segment],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let churn_rate = if active + churned > 0 {
            churned as f64 / (active + churned) as f64
        } else {
            0.0
        };

        let fee_driven: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM churn_cohort
             WHERE run_id = ?1 AND segment = ?2
               AND tick_churned >= ?3 AND tick_churned <= ?4
               AND primary_churn_driver = 'fee_burden'",
                params![run_id, segment, lookback, tick_i],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let service_driven: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM churn_cohort
             WHERE run_id = ?1 AND segment = ?2
               AND tick_churned >= ?3 AND tick_churned <= ?4
               AND primary_churn_driver IN ('complaints', 'sla_breach')",
                params![run_id, segment, lookback, tick_i],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let life_event_driven: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM churn_cohort
             WHERE run_id = ?1 AND segment = ?2
               AND tick_churned >= ?3 AND tick_churned <= ?4
               AND primary_churn_driver = 'life_event'",
                params![run_id, segment, lookback, tick_i],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(crate::churn_subsystem::ChurnAggregate {
            active_customers: active,
            churned_this_period: churned,
            high_risk_count: high_risk,
            churn_rate,
            avg_churn_risk: avg_risk,
            fee_driven_churn: fee_driven,
            service_driven_churn: service_driven,
            life_event_churn: life_event_driven,
        })
    }

    pub fn save_churn_aggregate(
        &self,
        run_id: &str,
        segment: &str,
        tick: Tick,
        agg: &crate::churn_subsystem::ChurnAggregate,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO churn_aggregate (
                run_id, tick, segment,
                active_customers, churned_this_period, high_risk_count,
                churn_rate, avg_churn_risk,
                fee_driven_churn, service_driven_churn, life_event_churn
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
            ON CONFLICT(run_id, tick, segment) DO UPDATE SET
                churned_this_period = excluded.churned_this_period",
            params![
                run_id,
                tick as i64,
                segment,
                agg.active_customers,
                agg.churned_this_period,
                agg.high_risk_count,
                agg.churn_rate,
                agg.avg_churn_risk,
                agg.fee_driven_churn,
                agg.service_driven_churn,
                agg.life_event_churn,
            ],
        )?;
        Ok(())
    }

    pub fn update_customer_satisfaction(
        &self,
        run_id: &str,
        customer_id: &str,
        delta: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE customer
             SET satisfaction = MAX(0.0, MIN(1.0, satisfaction + ?1))
             WHERE run_id = ?2 AND customer_id = ?3",
            params![delta, run_id, customer_id],
        )?;
        Ok(())
    }

    pub fn update_customer_churn_satisfaction(
        &self,
        run_id: &str,
        customer_id: &str,
        churn_risk: f64,
        satisfaction: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE customer SET churn_risk = ?1, satisfaction = ?2
             WHERE run_id = ?3 AND customer_id = ?4",
            params![churn_risk, satisfaction, run_id, customer_id],
        )?;
        Ok(())
    }

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

    // ── Transaction ───────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn insert_transaction(
        &self,
        run_id: &str,
        txn_id: &str,
        account_id: &str,
        tick: Tick,
        amount: f64,
        direction: &str,
        category: &str,
        counterparty: Option<&str>,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO transactions (txn_id, run_id, account_id, tick, amount, direction, category, counterparty, fraud_flag, payment_rail_id, settlement_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 'ACH', 'settled')",
            params![txn_id, run_id, account_id, tick as i64, amount, direction, category, counterparty],
        )?;
        Ok(())
    }

    /// Insert a transaction with explicit payment rail and settlement status.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_transaction_with_rail(
        &self,
        run_id: &str,
        txn_id: &str,
        account_id: &str,
        tick: Tick,
        amount: f64,
        direction: &str,
        category: &str,
        counterparty: Option<&str>,
        payment_rail_id: &str,
        settlement_status: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO transactions (txn_id, run_id, account_id, tick, amount, direction, category, counterparty, fraud_flag, payment_rail_id, settlement_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10)",
            params![txn_id, run_id, account_id, tick as i64, amount, direction, category, counterparty, payment_rail_id, settlement_status],
        )?;
        Ok(())
    }

    // ── Daily aggregate ───────────────────────────────────────────

    pub fn compute_daily_aggregate(&self, run_id: &str, tick: Tick) -> SimResult<DailyAggregate> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*), SUM(amount), SUM(CASE WHEN category = 'overdraft_fee' THEN amount ELSE 0 END)
             FROM transactions WHERE run_id = ?1 AND tick = ?2"
        )?;
        let (txn_count, txn_volume, fee_income): (i64, f64, f64) =
            stmt.query_row(params![run_id, tick as i64], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1).unwrap_or(0.0),
                    row.get(2).unwrap_or(0.0),
                ))
            })?;

        let overdraft_events: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE run_id = ?1 AND tick = ?2 AND category = 'overdraft_fee'",
            params![run_id, tick as i64],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(DailyAggregate {
            txn_count,
            txn_volume,
            fee_income,
            overdraft_events,
        })
    }

    pub fn save_daily_aggregate(
        &self,
        run_id: &str,
        tick: Tick,
        agg: &DailyAggregate,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO daily_aggregate (run_id, tick, txn_count, txn_volume, fee_income, overdraft_events)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![run_id, tick as i64, agg.txn_count, agg.txn_volume, agg.fee_income, agg.overdraft_events],
        )?;
        Ok(())
    }

    // ── Test helper methods ───────────────────────────────────────

    pub fn customer_count(&self, run_id: &str, status: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM customer WHERE run_id = ?1 AND status = ?2",
            params![run_id, status],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn txn_count_for_tick(&self, run_id: &str, tick: Tick) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE run_id = ?1 AND tick = ?2",
            params![run_id, tick as i64],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn txn_count_by_category(&self, run_id: &str, tick: Tick, cat: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE run_id = ?1 AND tick = ?2 AND category = ?3",
            params![run_id, tick as i64, cat],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn all_txn_amounts(&self, run_id: &str) -> SimResult<Vec<f64>> {
        let mut stmt = self.conn.prepare(
            "SELECT amount FROM transactions WHERE run_id = ?1 AND category = 'purchase'",
        )?;
        let amounts: Vec<f64> = stmt
            .query_map(params![run_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(amounts)
    }

    pub fn txn_count_total(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

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

    // ── Interaction ────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn insert_interaction(
        &self,
        run_id: &str,
        interaction_id: &str,
        customer_id: &str,
        tick: Tick,
        channel: &str,
        interaction_type: &str,
        complaint_id: Option<&str>,
        outcome: Option<&str>,
        satisfaction_delta: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO interaction (
                interaction_id, run_id, customer_id, tick, channel,
                interaction_type, complaint_id, outcome, satisfaction_delta
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                interaction_id,
                run_id,
                customer_id,
                tick as i64,
                channel,
                interaction_type,
                complaint_id,
                outcome,
                satisfaction_delta,
            ],
        )?;
        Ok(())
    }

    // ── Customer extensions ────────────────────────────────────────

    pub fn adjust_customer_churn_risk(
        &self,
        run_id: &str,
        customer_id: &str,
        delta: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE customer
             SET churn_risk = MAX(0.0, MIN(1.0, churn_risk + ?1))
             WHERE run_id = ?2 AND customer_id = ?3",
            params![delta, run_id, customer_id],
        )?;
        Ok(())
    }

    // ── Account helpers ────────────────────────────────────────────

    pub fn account_product(&self, run_id: &str, account_id: &str) -> SimResult<String> {
        let product: String = self.conn.query_row(
            "SELECT product_id FROM account WHERE run_id = ?1 AND account_id = ?2",
            params![run_id, account_id],
            |row| row.get(0),
        )?;
        Ok(product)
    }

    // ── Complaint aggregates ───────────────────────────────────────

    pub fn compute_complaint_aggregate(
        &self,
        run_id: &str,
        tick: Tick,
    ) -> SimResult<ComplaintAggregate> {
        let complaints_opened: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM complaint WHERE run_id = ?1 AND tick_opened = ?2",
                params![run_id, tick as i64],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let complaints_closed: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM complaint WHERE run_id = ?1 AND tick_closed = ?2",
                params![run_id, tick as i64],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let sla_breaches: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM complaint WHERE run_id = ?1 AND sla_breached = 1 AND sla_due_tick = ?2",
            params![run_id, tick as i64], |row| row.get(0),
        ).unwrap_or(0);

        let backlog_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM complaint WHERE run_id = ?1 AND status = 'open'",
                params![run_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let avg_age_days: f64 = self.conn.query_row(
            "SELECT COALESCE(AVG(?2 - tick_opened), 0.0) FROM complaint WHERE run_id = ?1 AND status = 'open'",
            params![run_id, tick as i64], |row| row.get(0),
        ).unwrap_or(0.0);

        Ok(ComplaintAggregate {
            complaints_opened,
            complaints_closed,
            sla_breaches,
            avg_age_days,
            backlog_count,
        })
    }

    pub fn save_complaint_aggregate(
        &self,
        run_id: &str,
        tick: Tick,
        agg: &ComplaintAggregate,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO complaint_aggregate
             (run_id, tick, complaints_opened, complaints_closed, sla_breaches, avg_age_days, backlog_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run_id, tick as i64,
                agg.complaints_opened, agg.complaints_closed, agg.sla_breaches,
                agg.avg_age_days, agg.backlog_count,
            ],
        )?;
        Ok(())
    }

    // ── Test / summary helpers ─────────────────────────────────────

    pub fn complaint_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM complaint WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn sla_breach_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM complaint WHERE run_id = ?1 AND sla_breached = 1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn first_open_complaint(
        &self,
        run_id: &str,
    ) -> SimResult<Option<crate::complaint_subsystem::ComplaintRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT complaint_id, customer_id, account_id, tick_opened, tick_closed,
                    product, issue, priority, status, sla_due_tick, sla_breached,
                    resolution_code, amount_refunded, udaap_flag
             FROM complaint WHERE run_id = ?1 AND status = 'open'
             ORDER BY tick_opened ASC LIMIT 1",
        )?;
        let result = stmt.query_row(params![run_id], complaint_row_mapper).ok();
        Ok(result)
    }

    pub fn customer_satisfaction(&self, run_id: &str, customer_id: &str) -> SimResult<f64> {
        let sat: f64 = self.conn.query_row(
            "SELECT satisfaction FROM customer WHERE run_id = ?1 AND customer_id = ?2",
            params![run_id, customer_id],
            |row| row.get(0),
        )?;
        Ok(sat)
    }

    pub fn complaint_backlog(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM complaint WHERE run_id = ?1 AND status = 'open'",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn fee_event_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM event_log WHERE run_id = ?1 AND event_type = 'fee_charged'",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn churned_customer_count(&self, run_id: &str) -> SimResult<i64> {
        self.customer_count(run_id, "churned")
    }

    // ── P&L ─────────────────────────────────────────────────────

    pub fn insert_pnl_snapshot(
        &self,
        run_id: &str,
        pnl: &crate::economics_subsystem::PnLSnapshot,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO pnl_snapshot (
                run_id, tick, period,
                nii, fee_income, gross_income,
                credit_loss, fraud_loss, opex, complaint_cost,
                pre_tax_profit, nim, efficiency_ratio,
                avg_deposits, avg_loans, customer_count, active_accounts
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                run_id,
                pnl.tick as i64,
                pnl.period,
                pnl.nii,
                pnl.fee_income,
                pnl.gross_income,
                pnl.credit_loss,
                pnl.fraud_loss,
                pnl.opex,
                pnl.complaint_cost,
                pnl.pre_tax_profit,
                pnl.nim,
                pnl.efficiency_ratio,
                pnl.avg_deposits,
                pnl.avg_loans,
                pnl.customer_count,
                pnl.active_accounts,
            ],
        )?;
        Ok(())
    }

    pub fn latest_pnl_snapshots(
        &self,
        run_id: &str,
        count: usize,
    ) -> SimResult<Vec<crate::economics_subsystem::PnLSnapshot>> {
        let mut stmt = self.conn.prepare(
            "SELECT tick, period, nii, fee_income, gross_income,
                    credit_loss, fraud_loss, opex, complaint_cost,
                    pre_tax_profit, nim, efficiency_ratio,
                    avg_deposits, avg_loans, customer_count, active_accounts
             FROM pnl_snapshot
             WHERE run_id = ?1
             ORDER BY tick ASC
             LIMIT ?2",
        )?;
        let snapshots = stmt
            .query_map(params![run_id, count as i64], |row| {
                Ok(crate::economics_subsystem::PnLSnapshot {
                    tick: row.get::<_, i64>(0)? as u64,
                    period: row.get(1)?,
                    nii: row.get(2)?,
                    fee_income: row.get(3)?,
                    gross_income: row.get(4)?,
                    credit_loss: row.get(5)?,
                    fraud_loss: row.get(6)?,
                    opex: row.get(7)?,
                    complaint_cost: row.get(8)?,
                    pre_tax_profit: row.get(9)?,
                    nim: row.get(10)?,
                    efficiency_ratio: row.get(11)?,
                    avg_deposits: row.get(12)?,
                    avg_loans: row.get(13)?,
                    customer_count: row.get(14)?,
                    active_accounts: row.get(15)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(snapshots)
    }

    pub fn pnl_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM pnl_snapshot WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn all_pnl_snapshots(
        &self,
        run_id: &str,
    ) -> SimResult<Vec<crate::economics_subsystem::PnLSnapshot>> {
        self.latest_pnl_snapshots(run_id, 1000)
    }

    // ── Account balance aggregates ─────────────────────────────

    pub fn avg_account_balances(
        &self,
        run_id: &str,
        _start_tick: Tick,
        _end_tick: Tick,
    ) -> SimResult<f64> {
        let sum: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(balance), 0.0)
             FROM account
             WHERE run_id = ?1 AND status = 'open'",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(sum)
    }

    pub fn active_account_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*)
             FROM account
             WHERE run_id = ?1 AND status = 'open'",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    // ── Macro state average ────────────────────────────────────

    pub fn avg_macro_base_rate(
        &self,
        run_id: &str,
        _start_tick: Tick,
        _end_tick: Tick,
    ) -> SimResult<f64> {
        let rate: f64 = self
            .conn
            .query_row(
                "SELECT base_rate FROM macro_state WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .unwrap_or(0.05);
        Ok(rate)
    }

    // ── Fee and complaint aggregates ───────────────────────────

    pub fn sum_fee_income(&self, run_id: &str, start_tick: Tick, end_tick: Tick) -> SimResult<f64> {
        let sum: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(fee_income), 0.0)
             FROM daily_aggregate
             WHERE run_id = ?1 AND tick >= ?2 AND tick <= ?3",
            params![run_id, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;
        Ok(sum)
    }

    pub fn sum_complaints_opened(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        let sum: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(complaints_opened), 0)
             FROM complaint_aggregate
             WHERE run_id = ?1 AND tick >= ?2 AND tick <= ?3",
            params![run_id, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;
        Ok(sum)
    }

    // ── Product State ──────────────────────────────────────────

    pub fn insert_product_state(
        &self,
        run_id: &str,
        state: &crate::pricing_subsystem::ProductState,
        tick: Tick,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO product_state (
                run_id, product_id,
                monthly_fee, overdraft_fee, nsf_fee, atm_fee, wire_fee,
                interest_rate, last_modified_tick
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run_id,
                state.product_id,
                state.monthly_fee,
                state.overdraft_fee,
                state.nsf_fee,
                state.atm_fee,
                state.wire_fee,
                state.interest_rate,
                tick as i64,
            ],
        )?;
        Ok(())
    }

    pub fn update_product_fee(
        &self,
        run_id: &str,
        product_id: &str,
        fee_type: &str,
        new_value: f64,
        tick: Tick,
    ) -> SimResult<()> {
        let column = match fee_type {
            "monthly_fee" => "monthly_fee",
            "overdraft_fee" => "overdraft_fee",
            "nsf_fee" => "nsf_fee",
            "atm_fee" => "atm_fee",
            "wire_fee" => "wire_fee",
            _ => return Err(anyhow::anyhow!("Invalid fee type: {fee_type}").into()),
        };

        let sql = format!(
            "UPDATE product_state
             SET {} = ?1, last_modified_tick = ?2
             WHERE run_id = ?3 AND product_id = ?4",
            column
        );

        self.conn
            .execute(&sql, params![new_value, tick as i64, run_id, product_id])?;
        Ok(())
    }

    pub fn get_product_state(
        &self,
        run_id: &str,
        product_id: &str,
    ) -> SimResult<crate::pricing_subsystem::ProductState> {
        self.conn
            .query_row(
                "SELECT product_id, monthly_fee, overdraft_fee, nsf_fee,
                    atm_fee, wire_fee, interest_rate
             FROM product_state
             WHERE run_id = ?1 AND product_id = ?2",
                params![run_id, product_id],
                |row| {
                    Ok(crate::pricing_subsystem::ProductState {
                        product_id: row.get(0)?,
                        monthly_fee: row.get(1)?,
                        overdraft_fee: row.get(2)?,
                        nsf_fee: row.get(3)?,
                        atm_fee: row.get(4)?,
                        wire_fee: row.get(5)?,
                        interest_rate: row.get(6)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    // ── Fee Change Log ─────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn log_fee_change(
        &self,
        run_id: &str,
        tick: Tick,
        product_id: &str,
        fee_type: &str,
        old_value: f64,
        new_value: f64,
        player_initiated: bool,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO fee_change_log (
                run_id, tick, product_id, fee_type,
                old_value, new_value, player_initiated
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run_id,
                tick as i64,
                product_id,
                fee_type,
                old_value,
                new_value,
                if player_initiated { 1i64 } else { 0i64 },
            ],
        )?;
        Ok(())
    }

    pub fn fee_change_history(
        &self,
        run_id: &str,
        product_id: &str,
        limit: usize,
    ) -> SimResult<Vec<FeeChangeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT tick, fee_type, old_value, new_value, player_initiated
             FROM fee_change_log
             WHERE run_id = ?1 AND product_id = ?2
             ORDER BY tick DESC
             LIMIT ?3",
        )?;

        let records = stmt
            .query_map(params![run_id, product_id, limit as i64], |row| {
                Ok(FeeChangeRecord {
                    tick: row.get::<_, i64>(0)? as u64,
                    fee_type: row.get(1)?,
                    old_value: row.get(2)?,
                    new_value: row.get(3)?,
                    player_initiated: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    // ── Regulatory Score ───────────────────────────────────────

    pub fn init_regulatory_score(&self, run_id: &str, tick: Tick) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO regulatory_score (run_id, udaap_risk_score, last_updated_tick)
             VALUES (?1, 0.0, ?2)",
            params![run_id, tick as i64],
        )?;
        Ok(())
    }

    pub fn adjust_udaap_score(&self, run_id: &str, delta: f64, tick: Tick) -> SimResult<()> {
        self.conn.execute(
            "UPDATE regulatory_score
             SET udaap_risk_score = udaap_risk_score + ?1,
                 last_updated_tick = ?2
             WHERE run_id = ?3",
            params![delta, tick as i64, run_id],
        )?;
        Ok(())
    }

    pub fn get_udaap_score(&self, run_id: &str) -> SimResult<f64> {
        self.conn
            .query_row(
                "SELECT udaap_risk_score FROM regulatory_score WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    // ── Player Command Storage ────────────────────────────────

    pub fn store_player_command(
        &self,
        run_id: &str,
        tick: Tick,
        command: &crate::command::PlayerCommand,
    ) -> SimResult<i64> {
        let cmd_type = match command {
            crate::command::PlayerCommand::Pause => "pause",
            crate::command::PlayerCommand::Resume => "resume",
            crate::command::PlayerCommand::SetSpeed { .. } => "set_speed",
            crate::command::PlayerCommand::CloseComplaint { .. } => "close_complaint",
            crate::command::PlayerCommand::SetProductFee { .. } => "set_product_fee",
            crate::command::PlayerCommand::SetRiskDial { .. } => "set_risk_dial",
        };

        let payload = serde_json::to_string(command)?;

        self.conn.execute(
            "INSERT INTO player_command (run_id, tick, cmd_type, payload)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, tick as i64, cmd_type, payload],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_player_command(
        &self,
        run_id: &str,
        command_id: &str,
    ) -> SimResult<Option<crate::command::PlayerCommand>> {
        let id: i64 = command_id
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid command_id: {command_id}"))?;

        let payload: Option<String> = self
            .conn
            .query_row(
                "SELECT payload FROM player_command WHERE id = ?1 AND run_id = ?2",
                params![id, run_id],
                |row| row.get(0),
            )
            .optional()?;

        match payload {
            Some(p) => {
                let cmd = serde_json::from_str(&p)?;
                Ok(Some(cmd))
            }
            None => Ok(None),
        }
    }

    // ── Offer tracking ─────────────────────────────────────────

    pub fn insert_offer_config_state(
        &self,
        run_id: &str,
        offer_id: &str,
        active: bool,
        start_tick: u64,
        end_tick: Option<u64>,
        tick: Tick,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO offer_config_state (
                run_id, offer_id, active, start_tick, end_tick, modified_tick
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run_id,
                offer_id,
                if active { 1i64 } else { 0i64 },
                start_tick as i64,
                end_tick.map(|t| t as i64),
                tick as i64,
            ],
        )?;
        Ok(())
    }

    pub fn insert_customer_offer(
        &self,
        run_id: &str,
        record: &crate::offer_subsystem::CustomerOfferRecord,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO customer_offer (
                run_id, customer_id, offer_id,
                tick_offered, tick_accepted, tick_completed, tick_paid,
                status, bonus_amount, bonus_paid, requirements_met,
                cumulative_dd, min_balance_days, ticks_in_offer,
                bonus_seeker_flag, velocity_flag
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                run_id,
                record.customer_id,
                record.offer_id,
                record.tick_offered as i64,
                record.tick_accepted.map(|t| t as i64),
                record.tick_completed.map(|t| t as i64),
                record.tick_paid.map(|t| t as i64),
                record.status,
                record.bonus_amount,
                record.bonus_paid,
                if record.requirements_met { 1i64 } else { 0i64 },
                record.cumulative_dd,
                record.min_balance_days as i64,
                record.ticks_in_offer as i64,
                if record.bonus_seeker_flag { 1i64 } else { 0i64 },
                if record.velocity_flag { 1i64 } else { 0i64 },
            ],
        )?;
        Ok(())
    }

    pub fn update_customer_offer(
        &self,
        run_id: &str,
        record: &crate::offer_subsystem::CustomerOfferRecord,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE customer_offer
             SET tick_completed = ?1, tick_paid = ?2, status = ?3,
                 bonus_paid = ?4, requirements_met = ?5,
                 cumulative_dd = ?6, min_balance_days = ?7, ticks_in_offer = ?8
             WHERE run_id = ?9 AND customer_id = ?10 AND offer_id = ?11
               AND status = 'in_progress'",
            params![
                record.tick_completed.map(|t| t as i64),
                record.tick_paid.map(|t| t as i64),
                record.status,
                record.bonus_paid,
                if record.requirements_met { 1i64 } else { 0i64 },
                record.cumulative_dd,
                record.min_balance_days as i64,
                record.ticks_in_offer as i64,
                run_id,
                record.customer_id,
                record.offer_id,
            ],
        )?;
        Ok(())
    }

    pub fn in_progress_offers(
        &self,
        run_id: &str,
    ) -> SimResult<Vec<crate::offer_subsystem::CustomerOfferRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT customer_id, offer_id, tick_offered, tick_accepted,
                    tick_completed, tick_paid, status, bonus_amount, bonus_paid,
                    requirements_met, cumulative_dd, min_balance_days, ticks_in_offer,
                    bonus_seeker_flag, velocity_flag
             FROM customer_offer
             WHERE run_id = ?1 AND status = 'in_progress'
             ORDER BY tick_offered ASC",
        )?;

        let records = stmt
            .query_map(params![run_id], |row| {
                Ok(crate::offer_subsystem::CustomerOfferRecord {
                    customer_id: row.get(0)?,
                    offer_id: row.get(1)?,
                    tick_offered: row.get::<_, i64>(2)? as u64,
                    tick_accepted: row.get::<_, Option<i64>>(3)?.map(|t| t as u64),
                    tick_completed: row.get::<_, Option<i64>>(4)?.map(|t| t as u64),
                    tick_paid: row.get::<_, Option<i64>>(5)?.map(|t| t as u64),
                    status: row.get(6)?,
                    bonus_amount: row.get(7)?,
                    bonus_paid: row.get(8)?,
                    requirements_met: row.get::<_, i64>(9)? != 0,
                    cumulative_dd: row.get(10)?,
                    min_balance_days: row.get::<_, i64>(11)? as u64,
                    ticks_in_offer: row.get::<_, i64>(12)? as u64,
                    bonus_seeker_flag: row.get::<_, i64>(13)? != 0,
                    velocity_flag: row.get::<_, i64>(14)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    // ── Customer snapshot and activity ─────────────────────────

    pub fn get_customer_snapshot(
        &self,
        run_id: &str,
        customer_id: &str,
    ) -> SimResult<crate::offer_subsystem::CustomerSnapshot> {
        let (segment, churn_risk, open_tick): (String, f64, i64) = self.conn.query_row(
            "SELECT segment, churn_risk, open_tick
             FROM customer
             WHERE run_id = ?1 AND customer_id = ?2",
            params![run_id, customer_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        let product_count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
             FROM account
             WHERE run_id = ?1 AND customer_id = ?2 AND status = 'open'",
                params![run_id, customer_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(crate::offer_subsystem::CustomerSnapshot {
            segment,
            churn_risk,
            open_tick: open_tick as u64,
            product_count: product_count as usize,
        })
    }

    pub fn get_customer_activity(
        &self,
        run_id: &str,
        customer_id: &str,
        tick: Tick,
    ) -> SimResult<crate::offer_subsystem::CustomerActivity> {
        let balance: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(balance), 0.0)
             FROM account
             WHERE run_id = ?1 AND customer_id = ?2 AND status = 'open'",
            params![run_id, customer_id],
            |row| row.get(0),
        )?;

        let look_back = tick.saturating_sub(30) as i64;
        let tick_i = tick as i64;

        // Check for payroll (direct deposit) in last 30 ticks
        let (has_dd, dd_amount): (i64, f64) = self
            .conn
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(t.amount), 0.0)
             FROM transactions t
             JOIN account a ON t.account_id = a.account_id AND a.run_id = t.run_id
             WHERE t.run_id = ?1 AND a.customer_id = ?2
               AND t.category = 'payroll'
               AND t.tick >= ?3 AND t.tick <= ?4",
                params![run_id, customer_id, look_back, tick_i],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap_or((0, 0.0));

        Ok(crate::offer_subsystem::CustomerActivity {
            balance,
            has_direct_deposit: has_dd > 0,
            direct_deposit_amount: dd_amount,
        })
    }

    pub fn customer_primary_account(&self, run_id: &str, customer_id: &str) -> SimResult<String> {
        self.conn
            .query_row(
                "SELECT account_id
             FROM account
             WHERE run_id = ?1 AND customer_id = ?2 AND status = 'open'
             ORDER BY open_tick ASC
             LIMIT 1",
                params![run_id, customer_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    // ── Offer performance ──────────────────────────────────────

    pub fn compute_offer_performance(
        &self,
        run_id: &str,
        offer_id: &str,
    ) -> SimResult<crate::offer_subsystem::OfferPerformance> {
        let offered: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM customer_offer
             WHERE run_id = ?1 AND offer_id = ?2",
                params![run_id, offer_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let accepted: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM customer_offer
             WHERE run_id = ?1 AND offer_id = ?2 AND tick_accepted IS NOT NULL",
                params![run_id, offer_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let completed: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM customer_offer
             WHERE run_id = ?1 AND offer_id = ?2 AND status IN ('completed', 'paid')",
                params![run_id, offer_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let expired: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM customer_offer
             WHERE run_id = ?1 AND offer_id = ?2 AND status = 'expired'",
                params![run_id, offer_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let total_paid: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(bonus_paid), 0.0) FROM customer_offer
             WHERE run_id = ?1 AND offer_id = ?2",
                params![run_id, offer_id],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        let bonus_seekers: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM customer_offer
             WHERE run_id = ?1 AND offer_id = ?2 AND bonus_seeker_flag = 1",
                params![run_id, offer_id],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(crate::offer_subsystem::OfferPerformance {
            offered_count: offered,
            accepted_count: accepted,
            completed_count: completed,
            expired_count: expired,
            total_bonus_paid: total_paid,
            bonus_seeker_count: bonus_seekers,
        })
    }

    pub fn save_offer_performance(
        &self,
        run_id: &str,
        offer_id: &str,
        tick: Tick,
        perf: &crate::offer_subsystem::OfferPerformance,
    ) -> SimResult<()> {
        let avg_bonus = if perf.completed_count > 0 {
            perf.total_bonus_paid / perf.completed_count as f64
        } else {
            0.0
        };

        self.conn.execute(
            "INSERT OR REPLACE INTO offer_performance (
                run_id, offer_id, tick,
                offered_count, accepted_count, completed_count, expired_count,
                total_bonus_paid, avg_bonus_per_completion,
                bonus_seeker_count, velocity_flag_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)",
            params![
                run_id,
                offer_id,
                tick as i64,
                perf.offered_count,
                perf.accepted_count,
                perf.completed_count,
                perf.expired_count,
                perf.total_bonus_paid,
                avg_bonus,
                perf.bonus_seeker_count,
            ],
        )?;
        Ok(())
    }

    pub fn sum_offer_bonuses_paid(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<f64> {
        let sum: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(bonus_paid), 0.0)
             FROM customer_offer
             WHERE run_id = ?1
               AND tick_paid IS NOT NULL
               AND tick_paid >= ?2 AND tick_paid <= ?3",
            params![run_id, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;
        Ok(sum)
    }

    // ── Test helpers: offers ────────────────────────────────────

    pub fn matched_offer_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM customer_offer WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn completed_offer_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM customer_offer
             WHERE run_id = ?1 AND status IN ('completed', 'paid')",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn total_bonuses_paid(&self, run_id: &str) -> SimResult<f64> {
        self.conn
            .query_row(
                "SELECT COALESCE(SUM(bonus_paid), 0.0)
             FROM customer_offer WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn bonus_seeker_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM customer_offer
             WHERE run_id = ?1 AND bonus_seeker_flag = 1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn all_account_balances(&self, run_id: &str) -> SimResult<Vec<f64>> {
        let mut stmt = self.conn.prepare(
            "SELECT balance FROM account
             WHERE run_id = ?1 AND status = 'open'",
        )?;
        let balances = stmt
            .query_map(params![run_id], |row| row.get(0))?
            .collect::<Result<Vec<f64>, _>>()?;
        Ok(balances)
    }

    // ── Segment P&L ────────────────────────────────────────────────

    pub fn insert_segment_pnl(
        &self,
        run_id: &str,
        pnl: &crate::economics_subsystem::SegmentPnL,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO segment_pnl (
                run_id, tick, segment,
                nii, fee_income, interchange_income, gross_income,
                acquisition_cost, servicing_cost, complaint_cost,
                retention_cost, churn_replacement_cost, allocated_opex, total_cost,
                segment_profit, customer_margin, profit_per_customer,
                active_customers, avg_balance,
                avg_revenue_per_customer, avg_cost_per_customer,
                below_target_margin, cross_subsidy_recipient
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23
            )",
            params![
                run_id,
                pnl.tick as i64,
                pnl.segment,
                pnl.nii,
                pnl.fee_income,
                pnl.interchange_income,
                pnl.gross_income,
                pnl.acquisition_cost,
                pnl.servicing_cost,
                pnl.complaint_cost,
                pnl.retention_cost,
                pnl.churn_replacement_cost,
                pnl.allocated_opex,
                pnl.total_cost,
                pnl.segment_profit,
                pnl.customer_margin,
                pnl.profit_per_customer,
                pnl.active_customers,
                pnl.avg_balance,
                pnl.avg_revenue_per_customer,
                pnl.avg_cost_per_customer,
                if pnl.below_target_margin { 1i64 } else { 0 },
                if pnl.cross_subsidy_recipient { 1i64 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn segment_pnl_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM segment_pnl WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn segment_pnls_at_tick(
        &self,
        run_id: &str,
        tick: Tick,
    ) -> SimResult<Vec<crate::economics_subsystem::SegmentPnL>> {
        let mut stmt = self.conn.prepare(
            "SELECT run_id, tick, segment,
                    nii, fee_income, interchange_income, gross_income,
                    acquisition_cost, servicing_cost, complaint_cost,
                    retention_cost, churn_replacement_cost, allocated_opex, total_cost,
                    segment_profit, customer_margin, profit_per_customer,
                    active_customers, avg_balance,
                    avg_revenue_per_customer, avg_cost_per_customer,
                    below_target_margin, cross_subsidy_recipient
             FROM segment_pnl
             WHERE run_id = ?1 AND tick = ?2",
        )?;
        let rows = stmt
            .query_map(params![run_id, tick as i64], |row| {
                Ok(crate::economics_subsystem::SegmentPnL {
                    run_id: row.get(0)?,
                    tick: row.get::<_, i64>(1)? as u64,
                    segment: row.get(2)?,
                    nii: row.get(3)?,
                    fee_income: row.get(4)?,
                    interchange_income: row.get(5)?,
                    gross_income: row.get(6)?,
                    acquisition_cost: row.get(7)?,
                    servicing_cost: row.get(8)?,
                    complaint_cost: row.get(9)?,
                    retention_cost: row.get(10)?,
                    churn_replacement_cost: row.get(11)?,
                    allocated_opex: row.get(12)?,
                    total_cost: row.get(13)?,
                    segment_profit: row.get(14)?,
                    customer_margin: row.get(15)?,
                    profit_per_customer: row.get(16)?,
                    active_customers: row.get(17)?,
                    avg_balance: row.get(18)?,
                    avg_revenue_per_customer: row.get(19)?,
                    avg_cost_per_customer: row.get(20)?,
                    below_target_margin: row.get::<_, i64>(21)? != 0,
                    cross_subsidy_recipient: row.get::<_, i64>(22)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn segment_customer_count(
        &self,
        run_id: &str,
        segment: &str,
        status: &str,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM customer
             WHERE run_id = ?1 AND segment = ?2 AND status = ?3",
            params![run_id, segment, status],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn segment_avg_balance(
        &self,
        run_id: &str,
        segment: &str,
        _start_tick: Tick,
        _end_tick: Tick,
    ) -> SimResult<f64> {
        let balance: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(a.balance), 0.0)
             FROM account a
             JOIN customer c ON a.customer_id = c.customer_id AND a.run_id = c.run_id
             WHERE a.run_id = ?1 AND c.segment = ?2 AND a.status = 'open'",
            params![run_id, segment],
            |row| row.get(0),
        )?;
        Ok(balance)
    }

    pub fn segment_fee_income(
        &self,
        run_id: &str,
        segment: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<f64> {
        // Fee income from transactions in the fee categories, attributed to the customer's segment
        let result: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(t.amount), 0.0)
             FROM transaction t
             JOIN account a  ON t.account_id = a.account_id AND t.run_id = a.run_id
             JOIN customer c ON a.customer_id = c.customer_id AND a.run_id = c.run_id
             WHERE t.run_id = ?1
               AND c.segment = ?2
               AND t.category IN ('overdraft_fee','nsf_fee','monthly_fee','atm_fee','wire_fee')
               AND t.tick >= ?3 AND t.tick <= ?4",
                params![run_id, segment, start_tick as i64, end_tick as i64],
                |row| row.get(0),
            )
            .unwrap_or(0.0);
        Ok(result)
    }

    pub fn segment_new_customers(
        &self,
        run_id: &str,
        segment: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM customer
             WHERE run_id = ?1 AND segment = ?2
               AND open_tick >= ?3 AND open_tick <= ?4",
            params![run_id, segment, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn segment_complaints(
        &self,
        run_id: &str,
        segment: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<crate::economics_subsystem::SegmentComplaints> {
        let standard: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
             FROM complaint c
             JOIN customer cu ON c.customer_id = cu.customer_id AND c.run_id = cu.run_id
             WHERE c.run_id = ?1 AND cu.segment = ?2
               AND c.priority = 'standard'
               AND c.tick_opened >= ?3 AND c.tick_opened <= ?4",
                params![run_id, segment, start_tick as i64, end_tick as i64],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let high: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
             FROM complaint c
             JOIN customer cu ON c.customer_id = cu.customer_id AND c.run_id = cu.run_id
             WHERE c.run_id = ?1 AND cu.segment = ?2
               AND c.priority = 'high'
               AND c.tick_opened >= ?3 AND c.tick_opened <= ?4",
                params![run_id, segment, start_tick as i64, end_tick as i64],
                |row| row.get(0),
            )
            .unwrap_or(0);

        let urgent: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*)
             FROM complaint c
             JOIN customer cu ON c.customer_id = cu.customer_id AND c.run_id = cu.run_id
             WHERE c.run_id = ?1 AND cu.segment = ?2
               AND c.priority = 'urgent'
               AND c.tick_opened >= ?3 AND c.tick_opened <= ?4",
                params![run_id, segment, start_tick as i64, end_tick as i64],
                |row| row.get(0),
            )
            .unwrap_or(0);

        Ok(crate::economics_subsystem::SegmentComplaints {
            standard,
            high,
            urgent,
        })
    }

    pub fn segment_retention_offer_cost(
        &self,
        run_id: &str,
        segment: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<f64> {
        let result: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(co.bonus_paid), 0.0)
             FROM customer_offer co
             JOIN customer c ON co.customer_id = c.customer_id AND co.run_id = c.run_id
             WHERE co.run_id = ?1 AND c.segment = ?2
               AND co.offer_id LIKE '%retention%'
               AND co.tick_paid >= ?3 AND co.tick_paid <= ?4",
                params![run_id, segment, start_tick as i64, end_tick as i64],
                |row| row.get(0),
            )
            .unwrap_or(0.0);
        Ok(result)
    }

    pub fn segment_churned_customers(
        &self,
        run_id: &str,
        segment: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM customer
             WHERE run_id = ?1 AND segment = ?2 AND status = 'churned'
               AND close_tick >= ?3 AND close_tick <= ?4",
            params![run_id, segment, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn total_active_customers(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM customer WHERE run_id = ?1 AND status = 'active'",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn insert_cross_subsidy(
        &self,
        run_id: &str,
        tick: Tick,
        provider: &str,
        recipient: &str,
        amount: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO cross_subsidy_analysis
                (run_id, tick, subsidy_provider, subsidy_recipient, subsidy_amount)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, tick as i64, provider, recipient, amount],
        )?;
        Ok(())
    }

    // ── Complaint analytics ─────────────────────────────────────────

    pub fn complaint_count_by_category(
        &self,
        run_id: &str,
        category: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM complaint
             WHERE run_id = ?1
               AND issue LIKE ?2
               AND tick_opened >= ?3 AND tick_opened <= ?4",
            params![
                run_id,
                format!("%{}%", category),
                start_tick as i64,
                end_tick as i64
            ],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn total_complaints_in_window(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM complaint
             WHERE run_id = ?1
               AND tick_opened >= ?2 AND tick_opened <= ?3",
            params![run_id, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn complaint_count_by_segment(
        &self,
        run_id: &str,
        segment: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM complaint c
             JOIN customer cu ON c.customer_id = cu.customer_id AND c.run_id = cu.run_id
             WHERE c.run_id = ?1 AND cu.segment = ?2
               AND c.tick_opened >= ?3 AND c.tick_opened <= ?4",
            params![run_id, segment, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn recent_complaints(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<Vec<RecentComplaint>> {
        let mut stmt = self.conn.prepare(
            "SELECT complaint_id, customer_id, issue, tick_opened
             FROM complaint
             WHERE run_id = ?1
               AND tick_opened >= ?2 AND tick_opened <= ?3",
        )?;
        let complaints = stmt
            .query_map(params![run_id, start_tick as i64, end_tick as i64], |row| {
                Ok(RecentComplaint {
                    complaint_id: row.get(0)?,
                    customer_id: row.get(1)?,
                    issue: row.get(2)?,
                    tick_opened: row.get::<_, i64>(3)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(complaints)
    }

    pub fn customer_recent_fee(
        &self,
        run_id: &str,
        customer_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<Option<(String, Tick)>> {
        Ok(self
            .conn
            .query_row(
                "SELECT t.category, t.tick
                 FROM transactions t
                 JOIN account a ON t.account_id = a.account_id AND t.run_id = a.run_id
                 WHERE t.run_id = ?1 AND a.customer_id = ?2
                   AND t.category IN ('overdraft_fee', 'nsf_fee', 'monthly_fee')
                   AND t.tick >= ?3 AND t.tick <= ?4
                 ORDER BY t.tick DESC
                 LIMIT 1",
                params![run_id, customer_id, start_tick as i64, end_tick as i64],
                |row| Ok((row.get(0)?, row.get::<_, i64>(1)? as u64)),
            )
            .optional()?)
    }

    pub fn customer_active_life_event(
        &self,
        run_id: &str,
        customer_id: &str,
        tick: Tick,
    ) -> SimResult<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT event_type
                 FROM life_event
                 WHERE run_id = ?1 AND customer_id = ?2 AND active = 1
                   AND tick_occurred <= ?3 AND tick_expires >= ?3
                 LIMIT 1",
                params![run_id, customer_id, tick as i64],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn complaints_resolved_with_code(
        &self,
        run_id: &str,
        resolution_code: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT complaint_id
             FROM complaint
             WHERE run_id = ?1
               AND resolution_code = ?2
               AND tick_closed >= ?3 AND tick_closed <= ?4",
        )?;
        let ids = stmt
            .query_map(
                params![run_id, resolution_code, start_tick as i64, end_tick as i64],
                |row| row.get(0),
            )?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(ids)
    }

    pub fn complaint_impact_deltas(
        &self,
        _run_id: &str,
        _complaint_id: &str,
    ) -> SimResult<ComplaintImpactDeltas> {
        // Simplified: returns representative defaults.
        // Full implementation would track pre/post satisfaction snapshots.
        Ok(ComplaintImpactDeltas {
            satisfaction_delta: 0.05,
            churn_risk_delta: -0.03,
            had_repeat_complaint: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_resolution_effectiveness(
        &self,
        run_id: &str,
        resolution_code: &str,
        tick: Tick,
        avg_sat_delta: f64,
        avg_churn_delta: f64,
        repeat_rate: f64,
        escalation_rate: f64,
        count: i64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO resolution_effectiveness (
                run_id, resolution_code, measurement_tick,
                avg_satisfaction_delta, avg_churn_risk_delta,
                repeat_complaint_rate, escalation_rate, resolution_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_id,
                resolution_code,
                tick as i64,
                avg_sat_delta,
                avg_churn_delta,
                repeat_rate,
                escalation_rate,
                count,
            ],
        )?;
        Ok(())
    }

    pub fn open_complaints_by_priority(
        &self,
        run_id: &str,
        priority: &str,
    ) -> SimResult<Vec<OpenComplaint>> {
        let mut stmt = self.conn.prepare(
            "SELECT complaint_id, tick_opened, sla_due_tick, sla_breached
             FROM complaint
             WHERE run_id = ?1 AND priority = ?2 AND status = 'open'",
        )?;
        let complaints = stmt
            .query_map(params![run_id, priority], |row| {
                Ok(OpenComplaint {
                    complaint_id: row.get(0)?,
                    tick_opened: row.get::<_, i64>(1)? as u64,
                    sla_due_tick: row.get::<_, i64>(2)? as u64,
                    sla_breached: row.get::<_, i32>(3)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(complaints)
    }

    pub fn customers_with_complaint_count_gte(
        &self,
        run_id: &str,
        threshold: i64,
    ) -> SimResult<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT customer_id, COUNT(*) as cnt
             FROM complaint
             WHERE run_id = ?1
             GROUP BY customer_id
             HAVING cnt >= ?2",
        )?;
        let results = stmt
            .query_map(params![run_id, threshold], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(results)
    }

    pub fn customer_unresolved_complaints(
        &self,
        run_id: &str,
        customer_id: &str,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM complaint
             WHERE run_id = ?1 AND customer_id = ?2 AND status = 'open'",
            params![run_id, customer_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn customer_breached_complaints(&self, run_id: &str, customer_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM complaint
             WHERE run_id = ?1 AND customer_id = ?2 AND sla_breached = 1",
            params![run_id, customer_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn customer_latest_churn_risk(&self, run_id: &str, customer_id: &str) -> SimResult<f64> {
        let risk: Option<f64> = self
            .conn
            .query_row(
                "SELECT churn_risk
                 FROM customer_churn_score
                 WHERE run_id = ?1 AND customer_id = ?2
                 ORDER BY tick DESC
                 LIMIT 1",
                params![run_id, customer_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(risk.unwrap_or(0.0))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_repeat_complainer(
        &self,
        run_id: &str,
        customer_id: &str,
        tick: Tick,
        count: i64,
        unresolved: i64,
        breached: i64,
        churn_risk: f64,
        regulatory_risk: bool,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO repeat_complainer (
                run_id, customer_id, tick_flagged, complaint_count,
                total_unresolved, total_breached, avg_severity, churn_risk,
                regulatory_risk_flag
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0.5, ?7, ?8)",
            params![
                run_id,
                customer_id,
                tick as i64,
                count,
                unresolved,
                breached,
                churn_risk,
                if regulatory_risk { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn segment_breach_rate(
        &self,
        run_id: &str,
        segment: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<f64> {
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM complaint c
             JOIN customer cu ON c.customer_id = cu.customer_id AND c.run_id = cu.run_id
             WHERE c.run_id = ?1 AND cu.segment = ?2
               AND c.tick_opened >= ?3 AND c.tick_opened <= ?4",
            params![run_id, segment, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;

        if total == 0 {
            return Ok(0.0);
        }

        let breached: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM complaint c
             JOIN customer cu ON c.customer_id = cu.customer_id AND c.run_id = cu.run_id
             WHERE c.run_id = ?1 AND cu.segment = ?2
               AND c.sla_breached = 1
               AND c.tick_opened >= ?3 AND c.tick_opened <= ?4",
            params![run_id, segment, start_tick as i64, end_tick as i64],
            |row| row.get(0),
        )?;

        Ok(breached as f64 / total as f64)
    }

    pub fn insert_complaint_pattern(
        &self,
        run_id: &str,
        tick: Tick,
        pattern: &crate::complaint_analytics_subsystem::ComplaintPattern,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO complaint_pattern (
                run_id, tick_detected, pattern_type, issue_category, segment,
                affected_count, window_start_tick, window_end_tick,
                velocity_ratio, concentration_pct, severity_score
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                run_id,
                tick as i64,
                pattern.pattern_type,
                pattern.issue_category,
                pattern.segment,
                pattern.affected_count,
                pattern.window_start_tick as i64,
                pattern.window_end_tick as i64,
                pattern.velocity_ratio,
                pattern.concentration_pct,
                pattern.severity_score,
            ],
        )?;
        Ok(())
    }

    pub fn insert_complaint_root_cause(
        &self,
        run_id: &str,
        tick: Tick,
        rc: &crate::complaint_analytics_subsystem::ComplaintRootCause,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO complaint_root_cause (
                run_id, complaint_id, root_cause_type, root_cause_id,
                confidence_score, correlation_lag_ticks, attributed_tick
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                run_id,
                rc.complaint_id,
                rc.root_cause_type,
                rc.root_cause_id,
                rc.confidence_score,
                rc.correlation_lag_ticks as i64,
                tick as i64,
            ],
        )?;
        Ok(())
    }

    pub fn insert_sla_performance(
        &self,
        run_id: &str,
        tick: Tick,
        snapshot: &crate::complaint_analytics_subsystem::SLAPerformanceSnapshot,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sla_performance_snapshot (
                run_id, tick, priority,
                aging_0_3_days, aging_4_7_days, aging_8_14_days,
                aging_15_30_days, aging_30_plus_days,
                total_open, at_risk_count, breach_count,
                breach_rate, avg_age_ticks
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                run_id,
                tick as i64,
                snapshot.priority,
                snapshot.aging_0_3_days,
                snapshot.aging_4_7_days,
                snapshot.aging_8_14_days,
                snapshot.aging_15_30_days,
                snapshot.aging_30_plus_days,
                snapshot.total_open,
                snapshot.at_risk_count,
                snapshot.breach_count,
                snapshot.breach_rate,
                snapshot.avg_age_ticks,
            ],
        )?;
        Ok(())
    }

    pub fn insert_early_warning_alert(
        &self,
        run_id: &str,
        tick: Tick,
        alert: &crate::complaint_analytics_subsystem::EarlyWarningAlert,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO early_warning_alert (
                run_id, tick_fired, alert_type, severity, segment,
                metric_name, current_value, threshold_value, delta_pct
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run_id,
                tick as i64,
                alert.alert_type,
                alert.severity,
                alert.segment,
                alert.metric_name,
                alert.current_value,
                alert.threshold_value,
                alert.delta_pct,
            ],
        )?;
        Ok(())
    }

    pub fn complaint_pattern_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM complaint_pattern WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn sla_snapshot_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sla_performance_snapshot WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn early_warning_alert_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM early_warning_alert WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn repeat_complainer_count(&self, run_id: &str) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM repeat_complainer WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // ── Risk appetite ──────────────────────────────────────────

    pub fn insert_risk_appetite_state(
        &self,
        run_id: &str,
        tick: Tick,
        state: &crate::risk_appetite_subsystem::RiskAppetiteState,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO risk_appetite_state (
                run_id, tick,
                fee_aggressiveness, growth_velocity, service_level,
                retention_spend, compliance_stringency,
                overall_risk_score, revenue_risk, operational_risk,
                compliance_risk, financial_risk, risk_level,
                comfort_zone_violations
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                run_id,
                tick as i64,
                state.fee_aggressiveness,
                state.growth_velocity,
                state.service_level,
                state.retention_spend,
                state.compliance_stringency,
                state.overall_risk_score,
                state.revenue_risk,
                state.operational_risk,
                state.compliance_risk,
                state.financial_risk,
                state.risk_level,
                state.comfort_zone_violations,
            ],
        )?;
        Ok(())
    }

    pub fn log_dial_change(
        &self,
        run_id: &str,
        tick: Tick,
        dial_id: &str,
        old_value: f64,
        new_value: f64,
        player_initiated: bool,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO dial_change_log (
                run_id, tick, dial_id, old_value, new_value, player_initiated
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run_id,
                tick as i64,
                dial_id,
                old_value,
                new_value,
                if player_initiated { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    pub fn insert_board_pressure(
        &self,
        run_id: &str,
        tick: Tick,
        pressure_type: &str,
        dial_id: &str,
        message: &str,
        severity: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO board_pressure_event (
                run_id, tick, pressure_type, dial_id, message, severity
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                run_id,
                tick as i64,
                pressure_type,
                dial_id,
                message,
                severity
            ],
        )?;
        Ok(())
    }

    pub fn latest_risk_appetite_state(
        &self,
        run_id: &str,
    ) -> SimResult<Option<crate::risk_appetite_subsystem::RiskAppetiteState>> {
        self.conn
            .query_row(
                "SELECT fee_aggressiveness, growth_velocity, service_level,
                        retention_spend, compliance_stringency,
                        overall_risk_score, revenue_risk, operational_risk,
                        compliance_risk, financial_risk, risk_level,
                        comfort_zone_violations
                 FROM risk_appetite_state
                 WHERE run_id = ?1
                 ORDER BY tick DESC
                 LIMIT 1",
                params![run_id],
                |row| {
                    Ok(crate::risk_appetite_subsystem::RiskAppetiteState {
                        fee_aggressiveness: row.get(0)?,
                        growth_velocity: row.get(1)?,
                        service_level: row.get(2)?,
                        retention_spend: row.get(3)?,
                        compliance_stringency: row.get(4)?,
                        overall_risk_score: row.get(5)?,
                        revenue_risk: row.get(6)?,
                        operational_risk: row.get(7)?,
                        compliance_risk: row.get(8)?,
                        financial_risk: row.get(9)?,
                        risk_level: row.get(10)?,
                        comfort_zone_violations: row.get(11)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn dial_change_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM dial_change_log WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn board_pressure_count(&self, run_id: &str) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM board_pressure_event WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

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

    pub fn payment_batch_count(
        &self,
        run_id: &str,
    ) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM payment_batch WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn external_statement_count(
        &self,
        run_id: &str,
    ) -> SimResult<i64> {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM external_statement WHERE run_id = ?1",
                params![run_id],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    /// Get card transactions created this tick (for authorization processing).
    pub fn get_card_transactions_at_tick(
        &self,
        run_id: &str,
        tick: Tick,
    ) -> SimResult<Vec<TransactionForSettlement>> {
        let mut stmt = self.conn.prepare(
            "SELECT txn_id, account_id, amount, direction, category
             FROM transactions
             WHERE run_id = ?1 AND payment_rail_id = 'card' AND tick = ?2
               AND settlement_status = 'pending_authorization'
             ORDER BY account_id ASC, amount ASC, rowid ASC",
        )?;
        let rows = stmt
            .query_map(params![run_id, tick as i64], |row| {
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

    pub fn update_transaction_settlement_status(
        &self,
        run_id: &str,
        txn_id: &str,
        status: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE transactions SET settlement_status = ?1
             WHERE run_id = ?2 AND txn_id = ?3",
            params![status, run_id, txn_id],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub account_id: String,
    pub customer_id: String,
    pub product_id: String,
    pub balance: f64,
    pub monthly_txn_mean: f64,
    pub cash_intensity: f64,
    pub payroll_amount: f64,
    pub has_payroll: bool,
}

#[derive(Debug, Clone)]
pub struct DailyAggregate {
    pub txn_count: i64,
    pub txn_volume: f64,
    pub fee_income: f64,
    pub overdraft_events: i64,
}

#[derive(Debug, Clone)]
pub struct ComplaintAggregate {
    pub complaints_opened: i64,
    pub complaints_closed: i64,
    pub sla_breaches: i64,
    pub avg_age_days: f64,
    pub backlog_count: i64,
}

#[derive(Debug, Clone)]
pub struct FeeChangeRecord {
    pub tick: Tick,
    pub fee_type: String,
    pub old_value: f64,
    pub new_value: f64,
    pub player_initiated: bool,
}

#[derive(Debug, Clone)]
pub struct ChurnCohortRecord {
    pub cohort_id: String,
    pub tick_churned: Tick,
    pub segment: String,
    pub tenure_ticks: Tick,
    pub final_churn_risk: f64,
    pub final_satisfaction: f64,
    pub total_complaints: i64,
    pub total_fee_burden: f64,
    pub had_retention_offer: bool,
    pub primary_driver: String,
}

#[derive(Debug, Clone)]
pub struct RecentComplaint {
    pub complaint_id: String,
    pub customer_id: String,
    pub issue: String,
    pub tick_opened: Tick,
}

#[derive(Debug, Clone)]
pub struct OpenComplaint {
    pub complaint_id: String,
    pub tick_opened: Tick,
    pub sla_due_tick: Tick,
    pub sla_breached: bool,
}

#[derive(Debug, Clone)]
pub struct ComplaintImpactDeltas {
    pub satisfaction_delta: f64,
    pub churn_risk_delta: f64,
    pub had_repeat_complaint: bool,
}

/// Row mapper for the complaint table — shared by several query methods.
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

// ── Phase 3.1: Payment hub data types ────────────────────────────

#[derive(Debug, Clone)]
pub struct AuthorizationRow {
    pub authorization_id: String,
    pub account_id: String,
    pub merchant_name: Option<String>,
    pub merchant_category: Option<String>,
    pub amount: f64,
    pub tick_authorized: Tick,
    pub status: String,
    pub tick_cleared: Option<Tick>,
    pub cleared_amount: Option<f64>,
    pub tick_settled: Option<Tick>,
    pub interchange_fee: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct PaymentBatchRow {
    pub batch_id: String,
    pub rail_id: String,
    pub tick_created: Tick,
    pub tick_processed: Option<Tick>,
    pub item_count: i64,
    pub total_amount: f64,
    pub status: String,
    pub exception_count: i64,
}

#[derive(Debug, Clone)]
pub struct ExternalStatementRow {
    pub statement_id: String,
    pub rail_id: String,
    pub tick: Tick,
    pub total_debits: f64,
    pub total_credits: f64,
    pub item_count: i64,
}

#[derive(Debug, Clone)]
pub struct TransactionForSettlement {
    pub txn_id: String,
    pub account_id: String,
    pub amount: f64,
    pub direction: String,
    pub category: String,
}
