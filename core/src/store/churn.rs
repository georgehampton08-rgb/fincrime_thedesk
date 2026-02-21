use super::{SimStore, ChurnCohortRecord};
use crate::{error::SimResult, types::Tick};
use rusqlite::{params, OptionalExtension};

impl SimStore {
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
}
