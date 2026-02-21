use super::{SimStore, CustomerIdentityRow, CustomerAddressRow, CustomerPhoneRow};
use crate::{error::SimResult, types::Tick};
use rusqlite::{params, OptionalExtension};

impl SimStore {
pub fn insert_customer_identity(&self, row: &CustomerIdentityRow) -> SimResult<()> {
    self.conn.execute(
        "INSERT INTO customer_identity (
             customer_id, run_id, ssn_full, ssn_area, ssn_group, ssn_serial,
             ssn_status, identity_type, date_of_birth, age_at_open,
             ssn_shared_count, ssn_first_seen_tick
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            row.customer_id, row.run_id, row.ssn_full, row.ssn_area,
            row.ssn_group, row.ssn_serial, row.ssn_status, row.identity_type,
            row.date_of_birth, row.age_at_open, row.ssn_shared_count,
            row.ssn_first_seen_tick,
        ],
    )?;
    Ok(())
}

pub fn get_customer_identity(
    &self,
    run_id: &str,
    customer_id: &str,
) -> SimResult<Option<CustomerIdentityRow>> {
    let mut stmt = self.conn.prepare(
        "SELECT customer_id, run_id, ssn_full, ssn_area, ssn_group, ssn_serial,
                ssn_status, identity_type, date_of_birth, age_at_open,
                ssn_shared_count, ssn_first_seen_tick
         FROM customer_identity WHERE run_id = ?1 AND customer_id = ?2",
    )?;
    let row = stmt
        .query_row(params![run_id, customer_id], |r| {
            Ok(CustomerIdentityRow {
                customer_id: r.get(0)?,
                run_id: r.get(1)?,
                ssn_full: r.get(2)?,
                ssn_area: r.get(3)?,
                ssn_group: r.get(4)?,
                ssn_serial: r.get(5)?,
                ssn_status: r.get(6)?,
                identity_type: r.get(7)?,
                date_of_birth: r.get(8)?,
                age_at_open: r.get(9)?,
                ssn_shared_count: r.get(10)?,
                ssn_first_seen_tick: r.get(11)?,
            })
        })
        .optional()?;
    Ok(row)
}

/// Count total customer_identity rows for a run (test helper).
pub fn identity_count(&self, run_id: &str) -> SimResult<i64> {
    let n: i64 = self.conn.query_row(
        "SELECT COUNT(*) FROM customer_identity WHERE run_id = ?1",
        params![run_id],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// Count customers sharing the given SSN (for synthetic-identity detection).
pub fn count_ssn_sharing(&self, run_id: &str, ssn_full: &str) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COUNT(*) FROM customer_identity WHERE run_id=?1 AND ssn_full=?2",
        params![run_id, ssn_full],
        |r| r.get(0),
    )?)
}

// ── customer_address ──────────────────────────────────────────────────

pub fn insert_customer_address(&self, row: &CustomerAddressRow) -> SimResult<()> {
    self.conn.execute(
        "INSERT INTO customer_address (
             address_id, customer_id, run_id, street_address, city, state, zip_code,
             address_type, address_stability, verification_status,
             delivery_point, dwelling_type, occupant_count, first_seen_tick,
             is_high_risk, is_protected_class
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            row.address_id, row.customer_id, row.run_id,
            row.street_address, row.city, row.state, row.zip_code,
            row.address_type, row.address_stability, row.verification_status,
            row.delivery_point, row.dwelling_type, row.occupant_count,
            row.first_seen_tick, row.is_high_risk, row.is_protected_class,
        ],
    )?;
    Ok(())
}

pub fn get_customer_address(
    &self,
    run_id: &str,
    customer_id: &str,
) -> SimResult<Option<CustomerAddressRow>> {
    let mut stmt = self.conn.prepare(
        "SELECT address_id, customer_id, run_id, street_address, city, state, zip_code,
                address_type, address_stability, verification_status,
                delivery_point, dwelling_type, occupant_count, first_seen_tick,
                is_high_risk, is_protected_class
         FROM customer_address WHERE run_id = ?1 AND customer_id = ?2
         ORDER BY first_seen_tick ASC LIMIT 1",
    )?;
    let row = stmt
        .query_row(params![run_id, customer_id], |r| {
            Ok(CustomerAddressRow {
                address_id: r.get(0)?,
                customer_id: r.get(1)?,
                run_id: r.get(2)?,
                street_address: r.get(3)?,
                city: r.get(4)?,
                state: r.get(5)?,
                zip_code: r.get(6)?,
                address_type: r.get(7)?,
                address_stability: r.get(8)?,
                verification_status: r.get(9)?,
                delivery_point: r.get(10)?,
                dwelling_type: r.get(11)?,
                occupant_count: r.get(12)?,
                first_seen_tick: r.get(13)?,
                is_high_risk: r.get(14)?,
                is_protected_class: r.get(15)?,
            })
        })
        .optional()?;
    Ok(row)
}

/// Count total customer_address rows for a run (test helper).
pub fn address_count(&self, run_id: &str) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COUNT(*) FROM customer_address WHERE run_id = ?1",
        params![run_id],
        |r| r.get(0),
    )?)
}

/// Count customers sharing the same normalized address string.
pub fn count_address_sharing(
    &self,
    run_id: &str,
    street_address: &str,
    city: &str,
    state: &str,
    zip_code: &str,
) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COUNT(*) FROM customer_address
         WHERE run_id=?1 AND street_address=?2 AND city=?3 AND state=?4 AND zip_code=?5",
        params![run_id, street_address, city, state, zip_code],
        |r| r.get(0),
    )?)
}

/// Max number of addresses in a run where >1 customers share the exact location.
pub fn max_address_occupant_count(&self, run_id: &str) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COALESCE(MAX(sub.cnt), 0) FROM (
             SELECT COUNT(*) AS cnt FROM customer_address
             WHERE run_id = ?1
             GROUP BY street_address, city, state, zip_code
         ) sub",
        params![run_id],
        |r| r.get(0),
    )?)
}

// ── customer_phone ────────────────────────────────────────────────────

pub fn insert_customer_phone(&self, row: &CustomerPhoneRow) -> SimResult<()> {
    self.conn.execute(
        "INSERT INTO customer_phone (
             phone_id, customer_id, run_id, country_code, area_code, exchange_code,
             subscriber_number, full_number, phone_type, is_primary, is_verified,
             voip_indicator, burner_phone_score, carrier, is_ported,
             first_seen_tick, sms_failures, customer_count
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
        params![
            row.phone_id, row.customer_id, row.run_id,
            row.country_code, row.area_code, row.exchange_code,
            row.subscriber_number, row.full_number, row.phone_type,
            row.is_primary, row.is_verified, row.voip_indicator,
            row.burner_phone_score, row.carrier, row.is_ported,
            row.first_seen_tick, row.sms_failures, row.customer_count,
        ],
    )?;
    Ok(())
}

pub fn get_customer_phone(
    &self,
    run_id: &str,
    customer_id: &str,
) -> SimResult<Option<CustomerPhoneRow>> {
    let mut stmt = self.conn.prepare(
        "SELECT phone_id, customer_id, run_id, country_code, area_code, exchange_code,
                subscriber_number, full_number, phone_type, is_primary, is_verified,
                voip_indicator, burner_phone_score, carrier, is_ported,
                first_seen_tick, sms_failures, customer_count
         FROM customer_phone WHERE run_id = ?1 AND customer_id = ?2
         ORDER BY is_primary DESC, first_seen_tick ASC LIMIT 1",
    )?;
    let row = stmt
        .query_row(params![run_id, customer_id], |r| {
            Ok(CustomerPhoneRow {
                phone_id: r.get(0)?,
                customer_id: r.get(1)?,
                run_id: r.get(2)?,
                country_code: r.get(3)?,
                area_code: r.get(4)?,
                exchange_code: r.get(5)?,
                subscriber_number: r.get(6)?,
                full_number: r.get(7)?,
                phone_type: r.get(8)?,
                is_primary: r.get(9)?,
                is_verified: r.get(10)?,
                voip_indicator: r.get(11)?,
                burner_phone_score: r.get(12)?,
                carrier: r.get(13)?,
                is_ported: r.get(14)?,
                first_seen_tick: r.get(15)?,
                sms_failures: r.get(16)?,
                customer_count: r.get(17)?,
            })
        })
        .optional()?;
    Ok(row)
}

/// Count total customer_phone rows for a run (test helper).
pub fn phone_count(&self, run_id: &str) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COUNT(*) FROM customer_phone WHERE run_id = ?1",
        params![run_id],
        |r| r.get(0),
    )?)
}

/// Count how many customers share the same phone number.
pub fn count_phone_sharing(&self, run_id: &str, full_number: &str) -> SimResult<i64> {
    Ok(self.conn.query_row(
        "SELECT COUNT(*) FROM customer_phone WHERE run_id=?1 AND full_number=?2",
        params![run_id, full_number],
        |r| r.get(0),
    )?)
}

}
