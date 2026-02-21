use super::SimStore;
use crate::{error::SimResult, types::Tick};
use rusqlite::params;

impl SimStore {
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

}
