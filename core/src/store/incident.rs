//! Incident and system component database queries.

use super::SimStore;
use crate::{error::SimResult, incident_subsystem::SystemComponentRow, types::Tick};
use rusqlite::params;

impl SimStore {
    pub fn list_system_components(
        &self,
        _run_id: &str,
    ) -> SimResult<Vec<crate::incident_subsystem::SystemComponentRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT component_id, label, category, technology_tier, status,
                    mtbf_days, mttr_hours, last_incident_tick,
                    upgrade_in_progress, upgrade_target_tier, upgrade_complete_tick
             FROM system_component
             ORDER BY component_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(crate::incident_subsystem::SystemComponentRow {
                component_id: r.get(0)?,
                label: r.get(1)?,
                category: r.get(2)?,
                technology_tier: r.get(3)?,
                status: r.get(4)?,
                mtbf_days: r.get(5)?,
                mttr_hours: r.get(6)?,
                last_incident_tick: r.get(7)?,
                upgrade_in_progress: r.get::<_, i32>(8)? != 0,
                upgrade_target_tier: r.get(9)?,
                upgrade_complete_tick: r.get(10)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn get_system_component(
        &self,
        component_id: &str,
    ) -> SimResult<crate::incident_subsystem::SystemComponentRow> {
        Ok(self.conn.query_row(
            "SELECT component_id, label, category, technology_tier, status,
                    mtbf_days, mttr_hours, last_incident_tick,
                    upgrade_in_progress, upgrade_target_tier, upgrade_complete_tick
             FROM system_component WHERE component_id=?1",
            params![component_id],
            |r| {
                Ok(crate::incident_subsystem::SystemComponentRow {
                    component_id: r.get(0)?,
                    label: r.get(1)?,
                    category: r.get(2)?,
                    technology_tier: r.get(3)?,
                    status: r.get(4)?,
                    mtbf_days: r.get(5)?,
                    mttr_hours: r.get(6)?,
                    last_incident_tick: r.get(7)?,
                    upgrade_in_progress: r.get::<_, i32>(8)? != 0,
                    upgrade_target_tier: r.get(9)?,
                    upgrade_complete_tick: r.get(10)?,
                })
            },
        )?)
    }

    pub fn update_component_status(
        &self,
        component_id: &str,
        new_status: &str,
        tick: Tick,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE system_component SET status=?1, last_incident_tick=?2
             WHERE component_id=?3",
            params![new_status, tick, component_id],
        )?;
        Ok(())
    }

    pub fn insert_incident(
        &self,
        run_id: &str,
        incident_id: &str,
        component_id: &str,
        tick_created: Tick,
        severity: &str,
        description: &str,
        sla_deadline_tick: Tick,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO incident (incident_id, run_id, component_id, tick_created,
                severity, description, sla_deadline_tick)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![incident_id, run_id, component_id, tick_created,
                    severity, description, sla_deadline_tick],
        )?;
        Ok(())
    }

    pub fn get_open_incidents(
        &self,
        run_id: &str,
    ) -> SimResult<Vec<crate::incident_subsystem::IncidentRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT incident_id, run_id, component_id, tick_created, tick_resolved,
                    severity, status, description, sla_deadline_tick, sla_breached,
                    estimated_revenue_impact
             FROM incident
             WHERE run_id=?1 AND status='open'
             ORDER BY incident_id",
        )?;
        let rows = stmt.query_map(params![run_id], |r| {
            Ok(crate::incident_subsystem::IncidentRow {
                incident_id: r.get(0)?,
                run_id: r.get(1)?,
                component_id: r.get(2)?,
                tick_created: r.get(3)?,
                tick_resolved: r.get(4)?,
                severity: r.get(5)?,
                status: r.get(6)?,
                description: r.get(7)?,
                sla_deadline_tick: r.get(8)?,
                sla_breached: r.get::<_, i32>(9)? != 0,
                estimated_revenue_impact: r.get(10)?,
            })
        })?;
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    pub fn get_active_incidents(
        &self,
        run_id: &str,
    ) -> SimResult<Vec<crate::incident_subsystem::IncidentRow>> {
        // Same as open but could include sla_breached too
        self.get_open_incidents(run_id)
    }

    pub fn resolve_incident(
        &self,
        run_id: &str,
        incident_id: &str,
        tick_resolved: Tick,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE incident SET status='resolved', tick_resolved=?1
             WHERE run_id=?2 AND incident_id=?3",
            params![tick_resolved, run_id, incident_id],
        )?;
        Ok(())
    }

    pub fn mark_incident_sla_breached(
        &self,
        run_id: &str,
        incident_id: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "UPDATE incident SET sla_breached=1, status='sla_breached'
             WHERE run_id=?1 AND incident_id=?2",
            params![run_id, incident_id],
        )?;
        Ok(())
    }

    pub fn insert_incident_impact(
        &self,
        run_id: &str,
        incident_id: &str,
        tick: Tick,
        impact_type: &str,
        affected_component: &str,
        impact_value: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO incident_impact (run_id, incident_id, tick, impact_type,
                affected_component, impact_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![run_id, incident_id, tick, impact_type,
                    affected_component, impact_value],
        )?;
        Ok(())
    }

    /// Downstream query: get the maximum impact value for a given type at a tick.
    pub fn get_impact_value(
        &self,
        run_id: &str,
        tick: Tick,
        impact_type: &str,
    ) -> SimResult<f64> {
        let val: f64 = self.conn.query_row(
            "SELECT COALESCE(MAX(impact_value), 0.0)
             FROM incident_impact
             WHERE run_id=?1 AND tick=?2 AND impact_type=?3",
            params![run_id, tick, impact_type],
            |r| r.get(0),
        )?;
        Ok(val)
    }

    /// Downstream query: check if any impact of the given type is active at tick.
    pub fn has_active_impact(
        &self,
        run_id: &str,
        tick: Tick,
        impact_type: &str,
    ) -> SimResult<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*)
             FROM incident_impact
             WHERE run_id=?1 AND tick=?2 AND impact_type=?3",
            params![run_id, tick, impact_type],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn insert_system_metrics(
        &self,
        run_id: &str,
        tick: Tick,
        component_id: &str,
        uptime_pct: f64,
        incident_count: i32,
        avg_mttr_hours: f64,
        sla_breach_count: i32,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO system_metrics
                (run_id, tick, component_id, uptime_pct, incident_count,
                 avg_mttr_hours, sla_breach_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![run_id, tick, component_id, uptime_pct,
                    incident_count, avg_mttr_hours, sla_breach_count],
        )?;
        Ok(())
    }

    /// Compute uptime % and incident stats for a component over a tick window.
    pub fn compute_component_uptime(
        &self,
        run_id: &str,
        component_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<(f64, i32, f64, i32)> {
        let window = (end_tick - start_tick).max(1) as f64;

        // Count incidents in window
        let incident_count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM incident
             WHERE run_id=?1 AND component_id=?2
             AND tick_created >= ?3 AND tick_created <= ?4",
            params![run_id, component_id, start_tick, end_tick],
            |r| r.get(0),
        )?;

        // Total down ticks (ticks where incident was open)
        let down_ticks: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(
                CASE
                    WHEN tick_resolved IS NOT NULL THEN MIN(tick_resolved, ?4) - MAX(tick_created, ?3)
                    ELSE ?4 - MAX(tick_created, ?3)
                END
             ), 0.0)
             FROM incident
             WHERE run_id=?1 AND component_id=?2
             AND tick_created <= ?4
             AND (tick_resolved IS NULL OR tick_resolved >= ?3)",
            params![run_id, component_id, start_tick, end_tick],
            |r| r.get(0),
        )?;

        let uptime_pct = ((window - down_ticks) / window * 100.0).max(0.0).min(100.0);

        // Average MTTR (resolved only)
        let avg_mttr: f64 = self.conn.query_row(
            "SELECT COALESCE(AVG(tick_resolved - tick_created), 0.0)
             FROM incident
             WHERE run_id=?1 AND component_id=?2
             AND tick_resolved IS NOT NULL
             AND tick_created >= ?3 AND tick_created <= ?4",
            params![run_id, component_id, start_tick, end_tick],
            |r| r.get(0),
        )?;

        // SLA breaches
        let sla_breaches: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM incident
             WHERE run_id=?1 AND component_id=?2
             AND sla_breached=1
             AND tick_created >= ?3 AND tick_created <= ?4",
            params![run_id, component_id, start_tick, end_tick],
            |r| r.get(0),
        )?;

        Ok((uptime_pct, incident_count, avg_mttr, sla_breaches))
    }

    // ── Incident test helpers ────────────────────────────────────────────────

    pub fn incident_count(&self, run_id: &str) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM incident WHERE run_id=?1",
            params![run_id],
            |r| r.get(0),
        )?)
    }

    pub fn resolved_incident_count(&self, run_id: &str) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM incident WHERE run_id=?1 AND status='resolved'",
            params![run_id],
            |r| r.get(0),
        )?)
    }

    pub fn sla_breached_count(&self, run_id: &str) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM incident WHERE run_id=?1 AND sla_breached=1",
            params![run_id],
            |r| r.get(0),
        )?)
    }

    pub fn incident_impact_count(&self, run_id: &str) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM incident_impact WHERE run_id=?1",
            params![run_id],
            |r| r.get(0),
        )?)
    }

    pub fn system_metrics_count(&self, run_id: &str) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM system_metrics WHERE run_id=?1",
            params![run_id],
            |r| r.get(0),
        )?)
    }

    pub fn system_component_count(&self) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM system_component",
            [],
            |r| r.get(0),
        )?)
    }

}
