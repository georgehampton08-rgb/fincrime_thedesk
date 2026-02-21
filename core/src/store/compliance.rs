//! Fraud detection, AML screening, transaction monitoring, and SAR filing queries.

use super::{
    SimStore, CustomerRow, CustomerAddressRow, CustomerPhoneRow, FraudAccountRow,
    OFACWatchlistRow, PEPRegistryRow, HighRiskJurisdictionRow, AMLScreeningResultRow,
    CustomerInternationalRow, AMLMetrics,
};
use crate::{error::SimResult, types::Tick};
use rusqlite::params;

impl SimStore {
    pub fn get_customers_onboarded_in_window(
        &self,
        run_id: &str,
        start_tick: i64,
        end_tick: i64,
    ) -> SimResult<Vec<CustomerRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT customer_id, segment, open_tick, status
             FROM customer
             WHERE run_id = ? AND open_tick >= ? AND open_tick <= ?"
        )?;

        let rows = stmt.query_map(params![run_id, start_tick, end_tick], |row| {
            Ok(CustomerRow {
                customer_id: row.get(0)?,
                segment: row.get(1)?,
                open_tick: row.get(2)?,
                status: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_customer_primary_address(
        &self,
        run_id: &str,
        customer_id: &str,
    ) -> SimResult<Option<CustomerAddressRow>> {
        // Alias for get_customer_address which already returns primary
        self.get_customer_address(run_id, customer_id)
    }

    pub fn count_customers_at_address(
        &self,
        run_id: &str,
        street_address: &str,
        city: &str,
        state: &str,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT customer_id) FROM customer_address
             WHERE run_id = ? AND street_address = ? AND city = ? AND state = ?",
            params![run_id, street_address, city, state],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_customer_primary_phone(
        &self,
        run_id: &str,
        customer_id: &str,
    ) -> SimResult<Option<CustomerPhoneRow>> {
        // Alias for get_customer_phone which already returns primary
        self.get_customer_phone(run_id, customer_id)
    }

    pub fn insert_fraud_pattern(
        &self,
        run_id: &str,
        pattern_id: &str,
        pattern_type: &str,
        detected_tick: i64,
        confidence_score: f64,
        primary_customer_id: Option<&str>,
        primary_account_id: Option<&str>,
        fraud_indicators: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO fraud_pattern
             (pattern_id, run_id, pattern_type, detected_tick, confidence_score,
              primary_customer_id, primary_account_id, fraud_indicators, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open')",
            params![
                pattern_id,
                run_id,
                pattern_type,
                detected_tick,
                confidence_score,
                primary_customer_id,
                primary_account_id,
                fraud_indicators,
            ],
        )?;
        Ok(())
    }

    pub fn get_active_accounts(&self, run_id: &str) -> SimResult<Vec<FraudAccountRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT account_id, customer_id, product_id, status, open_tick
             FROM account
             WHERE run_id = ? AND (close_tick IS NULL OR close_tick = 0) AND status = 'open'"
        )?;

        let rows = stmt.query_map(params![run_id], |row| {
            Ok(FraudAccountRow {
                account_id: row.get(0)?,
                customer_id: row.get(1)?,
                product_id: row.get(2)?,
                status: row.get(3)?,
                open_tick: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn sum_account_debits_in_window(
        &self,
        run_id: &str,
        account_id: &str,
        start_tick: i64,
        end_tick: i64,
    ) -> SimResult<f64> {
        let sum: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(ABS(amount)), 0.0) FROM transactions
             WHERE run_id = ? AND account_id = ? AND amount < 0
               AND tick >= ? AND tick <= ?",
            params![run_id, account_id, start_tick, end_tick],
            |row| row.get(0),
        )?;
        Ok(sum)
    }

    pub fn count_account_txns_in_window(
        &self,
        run_id: &str,
        account_id: &str,
        start_tick: i64,
        end_tick: i64,
    ) -> SimResult<i64> {
        // Alias for count_account_transactions_in_window
        self.count_account_transactions_in_window(run_id, account_id, start_tick, end_tick)
    }

    pub fn count_unique_counterparties_in_window(
        &self,
        run_id: &str,
        account_id: &str,
        start_tick: i64,
        end_tick: i64,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT counterparty) FROM transactions
             WHERE run_id = ? AND account_id = ?
               AND tick >= ? AND tick <= ?
               AND counterparty IS NOT NULL",
            params![run_id, account_id, start_tick, end_tick],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn insert_account_fraud_score(
        &self,
        run_id: &str,
        account_id: &str,
        tick: i64,
        overall_score: f64,
        velocity_component: f64,
        amount_component: f64,
        pattern_component: f64,
        behavioral_component: f64,
        identity_component: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO account_fraud_score
             (run_id, account_id, tick, fraud_risk, velocity_component, amount_component,
              pattern_component, behavioral_component, identity_component)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run_id,
                account_id,
                tick,
                overall_score,
                velocity_component,
                amount_component,
                pattern_component,
                behavioral_component,
                identity_component,
            ],
        )?;
        Ok(())
    }

    pub fn insert_fraud_alert(
        &self,
        run_id: &str,
        alert_id: &str,
        tick: i64,
        alert_type: &str,
        entity_type: &str,
        entity_id: &str,
        fraud_score: f64,
        severity: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO fraud_alert
             (run_id, alert_id, tick, alert_type, entity_type, entity_id,
              fraud_score, severity, investigation_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open')",
            params![
                run_id,
                alert_id,
                tick,
                alert_type,
                entity_type,
                entity_id,
                fraud_score,
                severity,
            ],
        )?;
        Ok(())
    }

    // ── Phase 3.5 Week 4: AML Screening methods ──────────────────────────────

    pub fn get_ofac_watchlist(&self) -> SimResult<Vec<OFACWatchlistRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT entity_id, entity_type, full_name, aliases, program, country_codes,
                    address_fragments, id_numbers, date_of_birth, risk_level, effective_date, remarks
             FROM ofac_watchlist"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(OFACWatchlistRow {
                entity_id: row.get(0)?,
                entity_type: row.get(1)?,
                full_name: row.get(2)?,
                aliases: row.get(3)?,
                program: row.get(4)?,
                country_codes: row.get(5)?,
                address_fragments: row.get(6)?,
                id_numbers: row.get(7)?,
                date_of_birth: row.get(8)?,
                risk_level: row.get(9)?,
                effective_date: row.get(10)?,
                remarks: row.get(11)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_pep_registry(&self) -> SimResult<Vec<PEPRegistryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT pep_id, full_name, country_code, position, position_level,
                    organization, start_date, end_date, is_current, family_members, risk_multiplier
             FROM pep_registry"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(PEPRegistryRow {
                pep_id: row.get(0)?,
                full_name: row.get(1)?,
                country_code: row.get(2)?,
                position: row.get(3)?,
                position_level: row.get(4)?,
                organization: row.get(5)?,
                start_date: row.get(6)?,
                end_date: row.get(7)?,
                is_current: row.get::<_, i32>(8)? != 0,
                family_members: row.get(9)?,
                risk_multiplier: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_high_risk_jurisdictions(&self) -> SimResult<Vec<HighRiskJurisdictionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT country_code, country_name, risk_category, risk_level, fatf_status,
                    cpi_score, enhanced_dd_required, effective_date, notes
             FROM high_risk_jurisdictions"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(HighRiskJurisdictionRow {
                country_code: row.get(0)?,
                country_name: row.get(1)?,
                risk_category: row.get(2)?,
                risk_level: row.get(3)?,
                fatf_status: row.get(4)?,
                cpi_score: row.get(5)?,
                enhanced_dd_required: row.get::<_, i32>(6)? != 0,
                effective_date: row.get(7)?,
                notes: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_international_customers_in_window(
        &self,
        run_id: &str,
        start_tick: i64,
        end_tick: i64,
    ) -> SimResult<Vec<CustomerInternationalRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT ci.customer_id, ci.run_id, ci.citizenship_country, ci.residency_country,
                    ci.is_us_person, ci.visa_status, ci.foreign_tin, ci.ofac_check_status,
                    ci.sanctions_risk, ci.pep_status, ci.source_of_funds, ci.kyc_renewal_date
             FROM customer_international ci
             JOIN customer c ON c.customer_id = ci.customer_id AND c.run_id = ci.run_id
             WHERE ci.run_id = ? AND c.open_tick >= ? AND c.open_tick <= ?"
        )?;

        let rows = stmt.query_map(params![run_id, start_tick, end_tick], |row| {
            Ok(CustomerInternationalRow {
                customer_id: row.get(0)?,
                run_id: row.get(1)?,
                citizenship_country: row.get(2)?,
                residency_country: row.get(3)?,
                is_us_person: row.get(4)?,
                visa_status: row.get(5)?,
                foreign_tin: row.get(6)?,
                ofac_check_status: row.get(7)?,
                sanctions_risk: row.get(8)?,
                pep_status: row.get(9)?,
                source_of_funds: row.get(10)?,
                kyc_renewal_date: row.get(11)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_aml_screening_result(
        &self,
        run_id: &str,
        screening_id: &str,
        customer_id: &str,
        screening_tick: i64,
        screening_type: &str,
        match_type: &str,
        match_score: f64,
        matched_entity_id: Option<&str>,
        details: &str,
        risk_impact: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO aml_screening_result
             (screening_id, run_id, customer_id, screening_tick, screening_type,
              match_type, match_score, matched_entity_id, details, status, risk_impact)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending_review', ?10)",
            params![
                screening_id,
                run_id,
                customer_id,
                screening_tick,
                screening_type,
                match_type,
                match_score,
                matched_entity_id,
                details,
                risk_impact,
            ],
        )?;
        Ok(())
    }

    pub fn insert_aml_alert(
        &self,
        run_id: &str,
        alert_id: &str,
        customer_id: &str,
        tick: i64,
        alert_type: &str,
        severity: &str,
        description: &str,
        details: &str,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO aml_alert
             (alert_id, run_id, customer_id, tick, alert_type, severity, description, details, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open')",
            params![
                alert_id,
                run_id,
                customer_id,
                tick,
                alert_type,
                severity,
                description,
                details,
            ],
        )?;
        Ok(())
    }

    pub fn get_all_active_customers(&self, run_id: &str) -> SimResult<Vec<CustomerRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT customer_id, segment, open_tick, status
             FROM customer
             WHERE run_id = ? AND status = 'active'"
        )?;

        let rows = stmt.query_map(params![run_id], |row| {
            Ok(CustomerRow {
                customer_id: row.get(0)?,
                segment: row.get(1)?,
                open_tick: row.get(2)?,
                status: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_customer_aml_screenings(
        &self,
        run_id: &str,
        customer_id: &str,
        start_tick: i64,
        end_tick: i64,
    ) -> SimResult<Vec<AMLScreeningResultRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT screening_id, screening_type, match_type, match_score, risk_impact
             FROM aml_screening_result
             WHERE run_id = ? AND customer_id = ? AND screening_tick >= ? AND screening_tick <= ?"
        )?;

        let rows = stmt.query_map(params![run_id, customer_id, start_tick, end_tick], |row| {
            Ok(AMLScreeningResultRow {
                screening_id: row.get(0)?,
                screening_type: row.get(1)?,
                match_type: row.get(2)?,
                match_score: row.get(3)?,
                risk_impact: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_customer_aml_risk(
        &self,
        run_id: &str,
        customer_id: &str,
        tick: i64,
        overall_risk_rating: &str,
        risk_score: f64,
        sanctions_risk: f64,
        pep_risk: f64,
        jurisdiction_risk: f64,
        transaction_risk: f64,
        behavioral_risk: f64,
        last_screening_tick: i64,
        requires_edd: i32,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO customer_aml_risk
             (customer_id, run_id, tick, overall_risk_rating, risk_score,
              sanctions_risk, pep_risk, jurisdiction_risk, transaction_risk, behavioral_risk,
              last_screening_tick, requires_edd)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                customer_id,
                run_id,
                tick,
                overall_risk_rating,
                risk_score,
                sanctions_risk,
                pep_risk,
                jurisdiction_risk,
                transaction_risk,
                behavioral_risk,
                last_screening_tick,
                requires_edd,
            ],
        )?;
        Ok(())
    }

    pub fn compute_aml_metrics(
        &self,
        run_id: &str,
        start_tick: i64,
        end_tick: i64,
    ) -> SimResult<AMLMetrics> {
        let screenings_performed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM aml_screening_result
             WHERE run_id = ? AND screening_tick >= ? AND screening_tick <= ?",
            params![run_id, start_tick, end_tick],
            |row| row.get(0),
        )?;

        let sanctions_hits: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM aml_screening_result
             WHERE run_id = ? AND screening_type = 'ofac_sanctions'
               AND screening_tick >= ? AND screening_tick <= ?",
            params![run_id, start_tick, end_tick],
            |row| row.get(0),
        )?;

        let pep_matches: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM aml_screening_result
             WHERE run_id = ? AND screening_type = 'pep_match'
               AND screening_tick >= ? AND screening_tick <= ?",
            params![run_id, start_tick, end_tick],
            |row| row.get(0),
        )?;

        let high_risk_customers: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT customer_id) FROM customer_aml_risk
             WHERE run_id = ? AND overall_risk_rating IN ('high', 'critical')
               AND tick >= ? AND tick <= ?",
            params![run_id, start_tick, end_tick],
            |row| row.get(0),
        )?;

        let alerts_generated: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM aml_alert
             WHERE run_id = ? AND tick >= ? AND tick <= ?",
            params![run_id, start_tick, end_tick],
            |row| row.get(0),
        )?;

        let false_positive_rate = 0.12; // Placeholder - would need review data

        Ok(AMLMetrics {
            screenings_performed,
            sanctions_hits,
            pep_matches,
            high_risk_customers,
            alerts_generated,
            false_positive_rate,
        })
    }

    pub fn insert_aml_metrics(
        &self,
        run_id: &str,
        tick: i64,
        screenings_performed: i64,
        sanctions_hits: i64,
        pep_matches: i64,
        high_risk_customers: i64,
        alerts_generated: i64,
        false_positive_rate: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO aml_metrics
             (run_id, tick, screenings_performed, sanctions_hits, pep_matches,
              high_risk_customers, alerts_generated, false_positive_rate)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run_id,
                tick,
                screenings_performed,
                sanctions_hits,
                pep_matches,
                high_risk_customers,
                alerts_generated,
                false_positive_rate,
            ],
        )?;
        Ok(())
    }

    // ── Transaction Monitoring (Phase 3.5 Week 5) ────────────────────────────

    /// Get transactions within a specific amount range and time window
    pub fn get_transactions_in_range(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
        min_amount: f64,
        max_amount: f64,
    ) -> SimResult<Vec<crate::transaction_monitoring_subsystem::TransactionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.txn_id, t.run_id, t.account_id, a.customer_id, t.tick, t.amount, t.direction, t.category
             FROM transactions t
             JOIN account a ON t.account_id = a.account_id
             WHERE t.run_id = ?1 AND t.tick BETWEEN ?2 AND ?3
               AND ABS(t.amount) >= ?4 AND ABS(t.amount) < ?5
             ORDER BY t.tick DESC",
        )?;

        let rows = stmt.query_map(
            params![run_id, start_tick as i64, end_tick as i64, min_amount, max_amount],
            |row| {
                Ok(crate::transaction_monitoring_subsystem::TransactionRow {
                    transaction_id: row.get(0)?,
                    run_id: row.get(1)?,
                    account_id: row.get(2)?,
                    customer_id: row.get(3)?,
                    tick: row.get::<_, i64>(4)? as u64,
                    amount: row.get(5)?,
                    txn_type: row.get(6)?,
                    category: row.get(7)?,
                })
            },
        )?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get all transactions in a time window
    pub fn get_all_transactions_in_window(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<Vec<crate::transaction_monitoring_subsystem::TransactionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.txn_id, t.run_id, t.account_id, a.customer_id, t.tick, t.amount, t.direction, t.category
             FROM transactions t
             JOIN account a ON t.account_id = a.account_id
             WHERE t.run_id = ?1 AND t.tick BETWEEN ?2 AND ?3
             ORDER BY t.tick DESC",
        )?;

        let rows = stmt.query_map(
            params![run_id, start_tick as i64, end_tick as i64],
            |row| {
                Ok(crate::transaction_monitoring_subsystem::TransactionRow {
                    transaction_id: row.get(0)?,
                    run_id: row.get(1)?,
                    account_id: row.get(2)?,
                    customer_id: row.get(3)?,
                    tick: row.get::<_, i64>(4)? as u64,
                    amount: row.get(5)?,
                    txn_type: row.get(6)?,
                    category: row.get(7)?,
                })
            },
        )?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get cash transactions above a threshold
    pub fn get_cash_transactions_above_threshold(
        &self,
        run_id: &str,
        tick: Tick,
        threshold: f64,
    ) -> SimResult<Vec<crate::transaction_monitoring_subsystem::TransactionRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.txn_id, t.run_id, t.account_id, a.customer_id, t.tick, t.amount, t.direction, t.category
             FROM transactions t
             JOIN account a ON t.account_id = a.account_id
             WHERE t.run_id = ?1 AND t.tick = ?2 AND t.category = 'cash_withdrawal'
               AND ABS(t.amount) >= ?3
             ORDER BY t.amount DESC",
        )?;

        let rows = stmt.query_map(
            params![run_id, tick as i64, threshold],
            |row| {
                Ok(crate::transaction_monitoring_subsystem::TransactionRow {
                    transaction_id: row.get(0)?,
                    run_id: row.get(1)?,
                    account_id: row.get(2)?,
                    customer_id: row.get(3)?,
                    tick: row.get::<_, i64>(4)? as u64,
                    amount: row.get(5)?,
                    txn_type: row.get(6)?,
                    category: row.get(7)?,
                })
            },
        )?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Insert transaction monitoring alert
    pub fn insert_transaction_monitoring_alert(
        &self,
        alert: &crate::transaction_monitoring_subsystem::AMLAlert,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO aml_alert (
                alert_id, run_id, customer_id, tick, rule_id, alert_type,
                alert_score, description, triggered_amount, transaction_count, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                alert.alert_id,
                alert.run_id,
                alert.customer_id,
                alert.tick as i64,
                alert.rule_id,
                alert.alert_type,
                alert.alert_score,
                alert.description,
                alert.triggered_amount,
                alert.transaction_count,
                alert.status,
            ],
        )?;
        Ok(())
    }

    /// Insert CTR
    pub fn insert_ctr(
        &self,
        ctr: &crate::transaction_monitoring_subsystem::CurrencyTransactionReport,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO currency_transaction_report (
                ctr_id, run_id, customer_id, account_id, transaction_id,
                filing_tick, transaction_amount, transaction_type,
                filing_deadline, filed_on_time, auto_filed
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                ctr.ctr_id,
                ctr.run_id,
                ctr.customer_id,
                ctr.account_id,
                ctr.transaction_id,
                ctr.filing_tick as i64,
                ctr.transaction_amount,
                ctr.transaction_type,
                ctr.filing_deadline as i64,
                if ctr.filed_on_time { 1 } else { 0 },
                if ctr.auto_filed { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    /// Count AML alerts in a time window
    pub fn count_aml_alerts_in_window(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM aml_alert
             WHERE run_id = ?1 AND tick BETWEEN ?2 AND ?3",
            params![run_id, start_tick as i64, end_tick as i64],
            |r| r.get(0),
        )?)
    }

    /// Count AML alerts by status
    pub fn count_aml_alerts_by_status(&self, run_id: &str, status: &str) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM aml_alert WHERE run_id = ?1 AND status = ?2",
            params![run_id, status],
            |r| r.get(0),
        )?)
    }

    /// Count CTRs filed in a time window
    pub fn count_ctrs_in_window(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM currency_transaction_report
             WHERE run_id = ?1 AND filing_tick BETWEEN ?2 AND ?3",
            params![run_id, start_tick as i64, end_tick as i64],
            |r| r.get(0),
        )?)
    }

    /// Insert transaction monitoring metrics
    pub fn insert_transaction_monitoring_metrics(
        &self,
        run_id: &str,
        tick: Tick,
        alerts_generated: i64,
        alerts_investigating: i64,
        alerts_closed: i64,
        ctrs_filed: i64,
        sars_filed: i64,
        false_positive_rate: f64,
        avg_investigation_time: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO transaction_monitoring_metrics (
                run_id, tick, alerts_generated, alerts_investigating, alerts_closed,
                ctrs_filed, sars_filed, false_positive_rate, avg_investigation_time
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                run_id,
                tick as i64,
                alerts_generated,
                alerts_investigating,
                alerts_closed,
                ctrs_filed,
                sars_filed,
                false_positive_rate,
                avg_investigation_time,
            ],
        )?;
        Ok(())
    }

    // ── Phase 3.5 Week 6: SAR Filing ─────────────────────────────────────────

    /// Insert a SAR record
    pub fn insert_sar(
        &self,
        sar: &crate::transaction_monitoring_subsystem::SuspiciousActivityReport,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT INTO suspicious_activity_report (
                sar_id, run_id, filing_tick, subject_type, subject_id,
                activity_type, suspicious_amount, narrative,
                filing_deadline, filed_on_time, filing_status,
                regulatory_fine, related_alerts
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                sar.sar_id,
                sar.run_id,
                sar.filing_tick,
                sar.subject_type,
                sar.subject_id,
                sar.activity_type,
                sar.suspicious_amount,
                sar.narrative,
                sar.filing_deadline,
                if sar.filed_on_time { 1 } else { 0 },
                sar.filing_status,
                sar.regulatory_fine,
                sar.related_alerts,
            ],
        )?;
        Ok(())
    }

    /// Get high-scoring alerts that may require SAR filing
    pub fn get_alerts_above_threshold(
        &self,
        run_id: &str,
        threshold: f64,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<Vec<crate::transaction_monitoring_subsystem::AMLAlert>> {
        let mut stmt = self.conn.prepare(
            "SELECT alert_id, run_id, customer_id, tick,
                    COALESCE(rule_id, 'UNKNOWN') as rule_id,
                    alert_type,
                    COALESCE(alert_score, 0.0) as alert_score,
                    description,
                    triggered_amount, transaction_count, status
             FROM aml_alert
             WHERE run_id = ?
               AND COALESCE(alert_score, 0.0) >= ?
               AND tick BETWEEN ? AND ?
               AND status = 'open'
             ORDER BY COALESCE(alert_score, 0.0) DESC, tick DESC",
        )?;

        let rows = stmt.query_map(params![run_id, threshold, start_tick, end_tick], |row| {
            Ok(crate::transaction_monitoring_subsystem::AMLAlert {
                alert_id: row.get(0)?,
                run_id: row.get(1)?,
                customer_id: row.get(2)?,
                tick: row.get(3)?,
                rule_id: row.get(4)?,
                alert_type: row.get(5)?,
                alert_score: row.get(6)?,
                description: row.get(7)?,
                triggered_amount: row.get(8)?,
                transaction_count: row.get(9)?,
                status: row.get(10)?,
            })
        })?;

        let mut alerts = Vec::new();
        for r in rows {
            alerts.push(r?);
        }
        Ok(alerts)
    }

    /// Count SARs filed in a time window
    pub fn count_sars_in_window(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM suspicious_activity_report
             WHERE run_id = ? AND filing_tick BETWEEN ? AND ?",
            params![run_id, start_tick, end_tick],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Count late SARs in a time window
    pub fn count_late_sars_in_window(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM suspicious_activity_report
             WHERE run_id = ? AND filing_tick BETWEEN ? AND ?
               AND filed_on_time = 0",
            params![run_id, start_tick, end_tick],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Sum regulatory fines from late SARs
    pub fn sum_sar_fines_in_window(
        &self,
        run_id: &str,
        start_tick: Tick,
        end_tick: Tick,
    ) -> SimResult<f64> {
        let total: f64 = self.conn.query_row(
            "SELECT COALESCE(SUM(regulatory_fine), 0.0)
             FROM suspicious_activity_report
             WHERE run_id = ? AND filing_tick BETWEEN ? AND ?",
            params![run_id, start_tick, end_tick],
            |row| row.get(0),
        )?;
        Ok(total)
    }

    /// Mark alert as having SAR filed
    pub fn mark_alert_sar_filed(&self, run_id: &str, alert_id: &str) -> SimResult<()> {
        self.conn.execute(
            "UPDATE aml_alert SET status = 'sar_filed'
             WHERE run_id = ? AND alert_id = ?",
            params![run_id, alert_id],
        )?;
        Ok(())
    }

    /// Insert SAR filing metrics
    pub fn insert_sar_metrics(
        &self,
        run_id: &str,
        tick: Tick,
        sars_filed: i64,
        sars_late: i64,
        total_fines: f64,
        avg_filing_time: f64,
    ) -> SimResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO sar_filing_metrics (
                run_id, tick, sars_filed, sars_late,
                total_fines, avg_filing_time
            ) VALUES (?, ?, ?, ?, ?, ?)",
            params![run_id, tick as i64, sars_filed, sars_late, total_fines, avg_filing_time],
        )?;
        Ok(())
    }
}
