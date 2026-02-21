use super::{SimStore, BusinessEntityRow, DbaRegistrationRow, CustomerBeneficiaryRow};
use crate::{error::SimResult, types::Tick};
use rusqlite::{params, OptionalExtension};

impl SimStore {

pub fn insert_business_entity(&self, row: &BusinessEntityRow) -> SimResult<()> {
    self.conn.execute(
        "INSERT INTO business_entity (
             entity_id, run_id, customer_id, legal_name, dba_name, entity_type,
             ein, state_registration, formation_date, ownership_type, owner_count,
             naics_code, annual_revenue, employee_count,
             is_cash_intensive, is_high_risk_industry, shell_company_indicators
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
        params![
            row.entity_id, row.run_id, row.customer_id, row.legal_name,
            row.dba_name, row.entity_type, row.ein, row.state_registration,
            row.formation_date, row.ownership_type, row.owner_count,
            row.naics_code, row.annual_revenue, row.employee_count,
            row.is_cash_intensive, row.is_high_risk_industry, row.shell_company_indicators,
        ],
    )?;
    Ok(())
}

pub fn get_business_entity(
    &self,
    run_id: &str,
    customer_id: &str,
) -> SimResult<Option<BusinessEntityRow>> {
    let mut stmt = self.conn.prepare(
        "SELECT entity_id, run_id, customer_id, legal_name, dba_name, entity_type,
                ein, state_registration, formation_date, ownership_type, owner_count,
                naics_code, annual_revenue, employee_count,
                is_cash_intensive, is_high_risk_industry, shell_company_indicators
         FROM business_entity WHERE run_id = ?1 AND customer_id = ?2",
    )?;
    let row = stmt
        .query_row(params![run_id, customer_id], |r| {
            Ok(BusinessEntityRow {
                entity_id: r.get(0)?,
                run_id: r.get(1)?,
                customer_id: r.get(2)?,
                legal_name: r.get(3)?,
                dba_name: r.get(4)?,
                entity_type: r.get(5)?,
                ein: r.get(6)?,
                state_registration: r.get(7)?,
                formation_date: r.get(8)?,
                ownership_type: r.get(9)?,
                owner_count: r.get(10)?,
                naics_code: r.get(11)?,
                annual_revenue: r.get(12)?,
                employee_count: r.get(13)?,
                is_cash_intensive: r.get(14)?,
                is_high_risk_industry: r.get(15)?,
                shell_company_indicators: r.get(16)?,
            })
        })
        .optional()?;
    Ok(row)
}

pub fn business_entity_count(&self, run_id: &str) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COUNT(*) FROM business_entity WHERE run_id = ?1",
        params![run_id],
        |r| r.get(0),
    )?)
}

// ── dba_registration ──────────────────────────────────────────────────

pub fn insert_dba_registration(&self, row: &DbaRegistrationRow) -> SimResult<()> {
    self.conn.execute(
        "INSERT INTO dba_registration (
             dba_id, entity_id, run_id, dba_name, state_registered,
             status, is_potentially_deceptive
         ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            row.dba_id, row.entity_id, row.run_id, row.dba_name,
            row.state_registered, row.status, row.is_potentially_deceptive,
        ],
    )?;
    Ok(())
}

pub fn dba_count(&self, run_id: &str) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COUNT(*) FROM dba_registration WHERE run_id = ?1",
        params![run_id],
        |r| r.get(0),
    )?)
}

// ── customer_beneficiary ──────────────────────────────────────────────

pub fn insert_customer_beneficiary(&self, row: &CustomerBeneficiaryRow) -> SimResult<()> {
    self.conn.execute(
        "INSERT INTO customer_beneficiary (
             beneficiary_id, account_id, run_id, beneficiary_name,
             beneficiary_relationship, beneficiary_type, beneficiary_share,
             is_per_stirpes, trust_for_minor, verified
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            row.beneficiary_id, row.account_id, row.run_id, row.beneficiary_name,
            row.beneficiary_relationship, row.beneficiary_type, row.beneficiary_share,
            row.is_per_stirpes, row.trust_for_minor, row.verified,
        ],
    )?;
    Ok(())
}

pub fn beneficiary_count(&self, run_id: &str) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COUNT(*) FROM customer_beneficiary WHERE run_id = ?1",
        params![run_id],
        |r| r.get(0),
    )?)
}

}
