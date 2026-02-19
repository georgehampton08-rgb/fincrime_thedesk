use crate::{
    config::{RegionPool, SegmentConfig, SimConfig},
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::{
        AuthorizedSignerRow, BusinessEntityRow, CustodialAccountRow,
        CustomerAddressRow, CustomerBeneficiaryRow, CustomerIdentityRow,
        CustomerInternationalRow, CustomerPhoneRow, CustomerRelationshipRow,
        CustomerRiskScoreRow, DbaRegistrationRow, JointOwnershipRow, SimStore,
        TrustAccountRow, TrustBeneficiaryRow,
    },
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use serde::{Deserialize, Serialize};

pub const CHURN_THRESHOLD: f64 = 0.85;
pub const SATISFACTION_DECAY_PER_TICK: f64 = 0.0002;

/// Base simulation year — tick 0 corresponds to Jan 1 of this year
/// for deterministic date-of-birth calculation.
const SIM_BASE_YEAR: i32 = 2024;
/// Approximate ticks-per-year (365 ticks = 1 year in this sim).
const TICKS_PER_YEAR: i32 = 365;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerRecord {
    pub customer_id: String,
    pub segment: String,
    pub income_band: String,
    pub risk_band: String,
    pub open_tick: Tick,
    pub status: String, // active | churned | frozen
    pub churn_risk: f64,
    pub satisfaction: f64,
    pub monthly_txn_mean: f64,
    pub cash_intensity: f64,
    pub payroll_amount: f64,
    pub has_payroll: bool,
    pub product_id: String,
}

pub struct CustomerSubsystem {
    run_id: RunId,
    config: SimConfig,
    store: SimStore,
    initialized: bool,
}

impl CustomerSubsystem {
    pub fn new(run_id: RunId, config: SimConfig, store: SimStore) -> Self {
        Self {
            run_id,
            config,
            store,
            initialized: false,
        }
    }

    fn generate_initial_population(
        &self,
        rng: &mut SubsystemRng,
        tick: Tick,
    ) -> SimResult<Vec<(CustomerRecord, String)>> {
        let n = self.config.initial_population;
        let mut customers = Vec::with_capacity(n);

        for i in 0..n {
            let seg = self.pick_segment(rng);
            let income_band = self.pick_income_band(seg, rng);
            let has_payroll = rng.chance(seg.payroll_probability);
            let payroll_amount = if has_payroll {
                let raw = rng.pareto(seg.payroll_amount_mean * 0.5, 2.5);
                raw.min(seg.payroll_amount_mean * 3.0)
            } else {
                0.0
            };

            let mean_adj = 1.0 + (rng.next_f64() - 0.5) * 0.6;
            let monthly_txn_mean = (seg.monthly_txn_count_mean * mean_adj).max(3.0);

            let product_id =
                seg.products[rng.next_u64_below(seg.products.len() as u64) as usize].clone();

            let customer_id = format!("c-{i:06}");
            let account_id = format!("a-{i:06}");

            let record = CustomerRecord {
                customer_id: customer_id.clone(),
                segment: seg.id.clone(),
                income_band: income_band.clone(),
                risk_band: "low".into(),
                open_tick: tick,
                status: "active".into(),
                churn_risk: 0.0,
                satisfaction: 0.8,
                monthly_txn_mean,
                cash_intensity: seg.cash_intensity,
                payroll_amount,
                has_payroll,
                product_id,
            };
            customers.push((record, account_id));
        }
        Ok(customers)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 3.5-prep: Identity generation helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Weighted random pick of a region from config.
    fn pick_region<'a>(&'a self, rng: &mut SubsystemRng) -> &'a RegionPool {
        let regions = &self.config.identity_address.regions;
        let total_weight: f64 = regions.iter().map(|r| r.weight).sum();
        let roll = rng.next_f64() * total_weight;
        let mut cum = 0.0;
        for region in regions {
            cum += region.weight;
            if roll < cum {
                return region;
            }
        }
        regions.last().expect("identity_address.regions must not be empty")
    }

    /// Generate a deterministic-but-plausible fake SSN.
    ///
    /// Layout: `AAA-GG-SSSS`
    /// - AAA: area code drawn from region's ssn_area_range, stepped by customer index
    /// - GG:  01–99 (never 00), stepped by index
    /// - SSSS: 0001–9999 (never 0000), drawn from RNG
    ///
    /// ~`synthetic_identity_rate` fraction are marked as synthetic.
    fn generate_ssn(
        &self,
        idx: usize,
        region: &RegionPool,
        rng: &mut SubsystemRng,
    ) -> (String, String, String, String, &'static str, &'static str) {
        let cfg = &self.config.identity_address;

        // Area: cycle through the region's valid area range
        let range_size = (region.ssn_area_range.1 - region.ssn_area_range.0 + 1).max(1) as usize;
        let area = region.ssn_area_range.0 + ((idx % range_size) as u16);

        // Group: 01-99
        let group = (idx % 99 + 1) as u8;

        // Serial: 0001-9999
        let serial = (rng.next_u64_below(9999) + 1) as u16;

        let ssn_area   = format!("{area:03}");
        let ssn_group  = format!("{group:02}");
        let ssn_serial = format!("{serial:04}");
        let ssn_full   = format!("{ssn_area}-{ssn_group}-{ssn_serial}");

        // ~2% are synthetic identity fraudsters
        let (ssn_status, identity_type) = if rng.next_f64() < cfg.synthetic_identity_rate {
            ("synthetic", "synthetic")
        } else {
            ("valid", "natural_person")
        };

        (ssn_full, ssn_area, ssn_group, ssn_serial, ssn_status, identity_type)
    }

    /// Generate a date-of-birth string (YYYY-MM-DD) consistent with the
    /// segment's expected age range.
    fn generate_dob(
        &self,
        seg: &SegmentConfig,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> (String, i64) {
        let (age_min, age_max) = match seg.id.as_str() {
            "student"        => (17u32, 26u32),
            "mid_tier"       => (28, 65),
            "small_business" => (28, 68),
            _                => (22, 72), // mass_market default
        };

        let age = age_min + (rng.next_u64_below((age_max - age_min + 1) as u64) as u32);

        // Calculate year/month/day from SIM_BASE_YEAR and open tick
        let current_year = SIM_BASE_YEAR + (tick as i32 / TICKS_PER_YEAR);
        let birth_year   = current_year - age as i32;
        // Random birth month and day
        let month = (rng.next_u64_below(12) + 1) as u8;
        let day   = match month {
            2              => (rng.next_u64_below(28) + 1) as u8,
            4 | 6 | 9 | 11 => (rng.next_u64_below(30) + 1) as u8,
            _              => (rng.next_u64_below(31) + 1) as u8,
        };

        (format!("{birth_year:04}-{month:02}-{day:02}"), age as i64)
    }

    /// Generate a physical address row.
    ///
    /// Address type distribution:
    ///   ~1.5% homeless shelter, ~3% P.O. box, ~1% CMRA, rest residential
    fn generate_address(
        &self,
        seg: &SegmentConfig,
        region: &RegionPool,
        tick: Tick,
        customer_id: &str,
        rng: &mut SubsystemRng,
    ) -> CustomerAddressRow {
        let cfg = &self.config.identity_address;

        // Street names used in generation (20 common street names embedded for efficiency)
        const STREETS: &[&str] = &[
            "Main", "Oak", "Maple", "Cedar", "Pine", "Elm", "Washington",
            "Lake", "Hill", "Park", "River", "Union", "Lincoln", "Madison",
            "Jefferson", "Adams", "Monroe", "Jackson", "Highland", "Sunset",
        ];
        const SUFFIXES: &[&str] = &["St", "Ave", "Blvd", "Dr", "Rd", "Ln", "Way", "Pkwy"];

        let roll = rng.next_f64();
        let (address_type, address_stability, is_high_risk, is_protected_class, dwelling_type, street_address, zip_code) =
            if roll < cfg.homeless_rate {
                // Homeless shelter — shared address
                let shelters = [
                    ("110 E 3rd St", "shelter"),
                    ("1514 Elm St", "shelter"),
                    ("545 S San Pedro St", "shelter"),
                    ("250 Georgia Ave SE", "shelter"),
                ];
                let idx = rng.next_u64_below(shelters.len() as u64) as usize;
                let (street, dw_type) = shelters[idx];
                let zip = format!("{}00{}", region.zip_prefix, rng.next_u64_below(10));
                (
                    "homeless_shelter",
                    "transient",
                    1i64,
                    1i64, // protected class — fair housing concern
                    Some(dw_type.to_string()),
                    street.to_string(),
                    zip,
                )
            } else if roll < cfg.homeless_rate + cfg.po_box_rate {
                // P.O. Box
                let box_num = rng.next_u64_below(9000) + 1000;
                let zip = format!("{}1{}", region.zip_prefix, rng.next_u64_below(100));
                (
                    "po_box",
                    "stable",
                    0i64,
                    0i64,
                    None,
                    format!("PO Box {box_num}"),
                    zip,
                )
            } else if roll < cfg.homeless_rate + cfg.po_box_rate + cfg.cmra_rate {
                // CMRA (mailbox store)
                let unit_num = rng.next_u64_below(500) + 100;
                let street_idx = rng.next_u64_below(STREETS.len() as u64) as usize;
                let house_num  = rng.next_u64_below(9000) + 1000;
                let zip = format!("{}2{}", region.zip_prefix, rng.next_u64_below(100));
                (
                    "cmra",
                    "stable",
                    1i64, // CMRA is a high-risk address type
                    0i64,
                    None,
                    format!("{house_num} {} Ave, Unit {unit_num}", STREETS[street_idx]),
                    zip,
                )
            } else {
                // Standard residential
                let suffix_idx = rng.next_u64_below(SUFFIXES.len() as u64) as usize;
                let street_idx = rng.next_u64_below(STREETS.len() as u64) as usize;
                let house_num  = rng.next_u64_below(9800) + 100;
                let zip = format!("{}3{}", region.zip_prefix, rng.next_u64_below(100));

                // Apartment vs single family weighted by segment
                let is_apt = match seg.id.as_str() {
                    "student"  => rng.next_f64() < 0.72,
                    "mid_tier" => rng.next_f64() < 0.40,
                    _          => rng.next_f64() < 0.50,
                };

                let (dw_type, addr_str) = if is_apt {
                    let apt_num = rng.next_u64_below(400) + 1;
                    (
                        "apartment".to_string(),
                        format!(
                            "{house_num} {} {}, Apt {apt_num}",
                            STREETS[street_idx], SUFFIXES[suffix_idx]
                        ),
                    )
                } else {
                    (
                        "single_family".to_string(),
                        format!(
                            "{house_num} {} {}",
                            STREETS[street_idx], SUFFIXES[suffix_idx]
                        ),
                    )
                };

                (
                    "residential",
                    "stable",
                    0i64,
                    0i64,
                    Some(dw_type),
                    addr_str,
                    zip,
                )
            };

        CustomerAddressRow {
            address_id: format!("addr-{customer_id}"),
            customer_id: customer_id.to_string(),
            run_id: self.run_id.clone(),
            street_address,
            city: region.city.clone(),
            state: region.state.clone(),
            zip_code,
            address_type: address_type.to_string(),
            address_stability: address_stability.to_string(),
            verification_status: "unverified".into(),
            delivery_point: Some(address_type.to_string()),
            dwelling_type,
            occupant_count: 1,
            first_seen_tick: tick as i64,
            is_high_risk,
            is_protected_class,
        }
    }

    /// Generate a phone number row.
    fn generate_phone(
        &self,
        region: &RegionPool,
        tick: Tick,
        customer_id: &str,
        rng: &mut SubsystemRng,
    ) -> CustomerPhoneRow {
        let cfg = &self.config.identity_address;

        let area_codes = &region.area_codes;
        let area_idx = rng.next_u64_below(area_codes.len() as u64) as usize;
        let area_code = area_codes[area_idx].clone();

        // Exchange: 200-999 (NXX format — first digit must be 2-9)
        let exchange = (rng.next_u64_below(800) + 200) as u16;
        // Subscriber: 0000-9999
        let subscriber = rng.next_u64_below(10000) as u16;

        let full_number = format!("+1-{area_code}-{exchange:03}-{subscriber:04}");

        let is_voip  = rng.next_f64() < cfg.voip_rate;
        let is_ported = rng.next_f64() < 0.15; // 15% recently ported
        let burner_score = if is_ported && is_voip { 0.75 } else if is_voip { 0.45 } else { 0.0 };

        // Carrier assignment (simplified weighted pick)
        let carriers = &["Verizon", "AT&T", "T-Mobile", "Cricket", "Metro by T-Mobile"];
        let weights  = &[0.30f64, 0.27, 0.25, 0.10, 0.08];
        let roll = rng.next_f64();
        let mut cum = 0.0;
        let mut carrier_name = carriers[0];
        for (c, w) in carriers.iter().zip(weights.iter()) {
            cum += w;
            if roll < cum {
                carrier_name = c;
                break;
            }
        }

        CustomerPhoneRow {
            phone_id: format!("ph-{customer_id}"),
            customer_id: customer_id.to_string(),
            run_id: self.run_id.clone(),
            country_code: "+1".into(),
            area_code,
            exchange_code: format!("{exchange:03}"),
            subscriber_number: format!("{subscriber:04}"),
            full_number,
            phone_type: if is_voip { "voip".into() } else { "mobile".into() },
            is_primary: 1,
            is_verified: 0,
            voip_indicator: is_voip as i64,
            burner_phone_score: burner_score,
            carrier: Some(carrier_name.to_string()),
            is_ported: is_ported as i64,
            first_seen_tick: tick as i64,
            sms_failures: 0,
            customer_count: 1,
        }
    }

    fn pick_segment<'a>(&'a self, rng: &mut SubsystemRng) -> &'a SegmentConfig {
        let roll = rng.next_f64();
        let mut cumulative = 0.0;
        let segments: Vec<_> = self.config.segments.values().collect();
        for seg in &segments {
            cumulative += seg.population_share;
            if roll < cumulative {
                return seg;
            }
        }
        segments.last().unwrap()
    }

    fn pick_income_band(&self, seg: &SegmentConfig, rng: &mut SubsystemRng) -> String {
        let roll = rng.next_f64();
        let mut cum = 0.0;
        for (band, weight) in seg.income_bands.iter().zip(seg.income_band_weights.iter()) {
            cum += weight;
            if roll < cum {
                return band.clone();
            }
        }
        seg.income_bands
            .last()
            .cloned()
            .unwrap_or_else(|| "low".into())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 3.5-prep Tier 2: Business entity, EIN, demographics, beneficiary
    // ─────────────────────────────────────────────────────────────────────────

    /// Generate a deterministic EIN (Employer Identification Number).
    /// Format: XX-XXXXXXX where XX is campus prefix based on state.
    fn generate_ein(&self, state: &str, idx: usize, rng: &mut SubsystemRng) -> String {
        // IRS campus prefixes by state (simplified mapping)
        let campus_prefix = match state {
            "CA" | "HI" => 94,
            "NY" | "NJ" | "CT" => 13,
            "TX" | "NM" => 75,
            "FL" | "GA" | "SC" | "NC" => 59,
            "IL" | "WI" | "MN" | "MI" => 36,
            "PA" | "DE" | "MD" | "VA" | "WV" | "DC" => 23,
            "WA" | "OR" | "AK" => 91,
            "CO" | "UT" | "AZ" | "NV" => 84,
            "OH" | "IN" | "KY" => 31,
            "MA" | "RI" | "VT" | "NH" | "ME" => 4,
            _ => 62, // default (Philly campus)
        };
        let serial = (idx * 7919 + rng.next_u64_below(9000000) as usize) % 10_000_000;
        format!("{campus_prefix:02}-{serial:07}")
    }

    /// Generate a business entity for a small_business customer.
    fn generate_business_entity(
        &self,
        customer_id: &str,
        state: &str,
        idx: usize,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> (BusinessEntityRow, Option<DbaRegistrationRow>) {
        // Business type distribution
        let entity_types = &[
            ("sole_proprietorship", 0.35),
            ("llc", 0.30),
            ("s_corp", 0.15),
            ("c_corp", 0.05),
            ("partnership", 0.10),
            ("nonprofit", 0.05),
        ];
        let roll = rng.next_f64();
        let mut cum = 0.0;
        let mut entity_type = "llc";
        for (etype, w) in entity_types {
            cum += w;
            if roll < cum {
                entity_type = etype;
                break;
            }
        }

        // NAICS codes (top small-biz sectors)
        let naics_codes = &[
            "722511", // restaurants
            "541110", // legal services
            "531210", // real estate
            "621111", // physicians
            "236220", // construction
            "811111", // auto repair
            "812111", // barber shops
            "453110", // florists
        ];
        let naics = naics_codes[rng.next_u64_below(naics_codes.len() as u64) as usize];

        let cash_intensive_naics = ["722511", "812111", "453110", "811111"];
        let is_cash_intensive = cash_intensive_naics.contains(&naics) as i64;

        let high_risk_naics = ["531210"]; // real estate
        let is_high_risk = high_risk_naics.contains(&naics) as i64;

        let ein = self.generate_ein(state, idx, rng);

        let annual_revenue = rng.pareto(150_000.0, 1.8).min(5_000_000.0);
        let employee_count = if entity_type == "sole_proprietorship" {
            (rng.next_u64_below(3) + 1) as i64
        } else {
            (rng.next_u64_below(50) + 1) as i64
        };

        // Shell indicators: high revenue but few employees
        let shell_indicators = if annual_revenue > 500_000.0 && employee_count <= 1 {
            (rng.next_u64_below(3) + 1) as i64  // 1-3 indicators
        } else {
            0
        };

        let legal_name = format!("Business-{idx:04}");
        let entity_id = format!("ent-{customer_id}");

        let ownership_type = match entity_type {
            "sole_proprietorship" => "single",
            "partnership" => "partnership",
            _ => if rng.next_f64() < 0.7 { "single" } else { "multi" },
        };

        let owner_count = match ownership_type {
            "single" => 1,
            "partnership" => (rng.next_u64_below(3) + 2) as i64,
            _ => (rng.next_u64_below(4) + 1) as i64,
        };

        let entity_row = BusinessEntityRow {
            entity_id: entity_id.clone(),
            run_id: self.run_id.clone(),
            customer_id: customer_id.to_string(),
            legal_name: legal_name.clone(),
            dba_name: None,
            entity_type: entity_type.to_string(),
            ein,
            state_registration: state.to_string(),
            formation_date: format!("{}-01-15", SIM_BASE_YEAR - (rng.next_u64_below(10) as i32 + 1)),
            ownership_type: ownership_type.to_string(),
            owner_count,
            naics_code: naics.to_string(),
            annual_revenue: Some(annual_revenue),
            employee_count: Some(employee_count),
            is_cash_intensive,
            is_high_risk_industry: is_high_risk,
            shell_company_indicators: shell_indicators,
        };

        // ~30% get DBA names
        let dba_row = if rng.next_f64() < 0.30 {
            let dba_name = format!("DBA-{idx:04}-Services");
            // ~5% of DBAs are "potentially deceptive" (name contains bank/financial)
            let is_deceptive = rng.next_f64() < 0.05;
            Some(DbaRegistrationRow {
                dba_id: format!("dba-{customer_id}"),
                entity_id: entity_id.clone(),
                run_id: self.run_id.clone(),
                dba_name,
                state_registered: state.to_string(),
                status: "active".to_string(),
                is_potentially_deceptive: is_deceptive as i64,
            })
        } else {
            None
        };

        (entity_row, dba_row)
    }

    /// Assign marital status based on age and segment.
    fn assign_marital_status(&self, age: i64, rng: &mut SubsystemRng) -> &'static str {
        let married_rate = if age < 25 {
            0.12
        } else if age < 35 {
            0.35
        } else if age < 50 {
            0.55
        } else if age < 65 {
            0.60
        } else {
            0.45  // widowed/divorced more common
        };

        let roll = rng.next_f64();
        if roll < married_rate {
            "married"
        } else if roll < married_rate + 0.05 {
            "divorced"
        } else if roll < married_rate + 0.07 && age >= 55 {
            "widowed"
        } else {
            "single"
        }
    }

    /// Assign employment status based on segment.
    fn assign_employment(&self, seg: &SegmentConfig, rng: &mut SubsystemRng) -> (&'static str, f64, i64, &'static str) {
        // (employment_status, annual_income, credit_score, home_ownership)
        let employment = if seg.id == "small_business" {
            "self_employed"
        } else if seg.id == "premium" {
            if rng.next_f64() < 0.9 { "employed" } else { "retired" }
        } else {
            let r = rng.next_f64();
            if r < 0.70 { "employed" }
            else if r < 0.85 { "self_employed" }
            else if r < 0.92 { "retired" }
            else if r < 0.97 { "unemployed" }
            else { "student" }
        };

        // Annual income correlated with segment
        let base_income = match seg.id.as_str() {
            "premium" => 120_000.0 + rng.pareto(80_000.0, 2.0).min(400_000.0),
            "small_business" => 60_000.0 + rng.pareto(40_000.0, 1.8).min(200_000.0),
            _ => 30_000.0 + rng.pareto(20_000.0, 2.5).min(80_000.0),
        };

        // Credit score: 300-850 range, correlated with segment
        let credit_base = match seg.id.as_str() {
            "premium" => 720,
            "small_business" => 680,
            _ => 640,
        };
        let credit_jitter = (rng.next_f64() * 120.0 - 60.0) as i64;
        let credit_score = (credit_base + credit_jitter).max(300).min(850);

        // Home ownership
        let home = match seg.id.as_str() {
            "premium" => if rng.next_f64() < 0.85 { "own" } else { "rent" },
            _ => if rng.next_f64() < 0.45 { "own" } else { "rent" },
        };

        (employment, base_income, credit_score, home)
    }

    /// Generate a beneficiary for a married or POD account customer.
    fn generate_beneficiary(
        &self,
        account_id: &str,
        customer_id: &str,
        marital_status: &str,
        rng: &mut SubsystemRng,
    ) -> Option<CustomerBeneficiaryRow> {
        // Married customers: ~80% get a beneficiary
        // Single/divorced: ~20% get a beneficiary
        let bene_rate = if marital_status == "married" { 0.80 } else { 0.20 };
        if rng.next_f64() >= bene_rate {
            return None;
        }

        let (relationship, name) = if marital_status == "married" {
            ("spouse", format!("Spouse-of-{customer_id}"))
        } else {
            let choices = &[("child", "Child"), ("parent", "Parent"), ("sibling", "Sibling")];
            let idx = rng.next_u64_below(choices.len() as u64) as usize;
            (choices[idx].0, format!("{}-of-{customer_id}", choices[idx].1))
        };

        Some(CustomerBeneficiaryRow {
            beneficiary_id: format!("ben-{customer_id}"),
            account_id: account_id.to_string(),
            run_id: self.run_id.clone(),
            beneficiary_name: name,
            beneficiary_relationship: relationship.to_string(),
            beneficiary_type: "primary".to_string(),
            beneficiary_share: 1.0,
            is_per_stirpes: 0,
            trust_for_minor: 0,
            verified: if marital_status == "married" { 1 } else { 0 },
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 3.5-prep Tier 3: Custodial, trust, international
    // ─────────────────────────────────────────────────────────────────────────

    /// Generate a custodial UTMA/UGMA account for a minor.
    fn generate_custodial_account(
        &self,
        custodian_id: &str,
        custodian_ssn: &str,
        state: &str,
        idx: usize,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> CustodialAccountRow {
        let account_type = if rng.next_f64() < 0.6 { "utma" } else { "ugma" };
        // Minor age: 0-15
        let minor_age = rng.next_u64_below(16) as i32;
        let birth_year = SIM_BASE_YEAR - minor_age;
        let minor_dob = format!("{birth_year}-06-15");

        // Termination age varies by state (simplified)
        let termination_age = match state {
            "CA" | "NV" | "OR" | "WA" => 18,
            "AK" | "FL" | "NC" => 21,
            "PA" | "VA" => 25,
            _ => 21,
        };

        let relationship = if rng.next_f64() < 0.75 { "parent" }
            else if rng.next_f64() < 0.5 { "grandparent" }
            else { "guardian" };

        CustodialAccountRow {
            account_id: format!("utma-{idx:04}"),
            run_id: self.run_id.clone(),
            account_type: account_type.to_string(),
            minor_customer_id: format!("minor-{idx:04}"),
            minor_dob,
            age_of_majority: 18,
            termination_age: termination_age as i64,
            custodian_customer_id: custodian_id.to_string(),
            custodian_relationship: relationship.to_string(),
            tax_reporting_ssn: custodian_ssn.to_string(),
            state_governed: state.to_string(),
        }
    }

    /// Generate a trust account for premium customers.
    fn generate_trust_account(
        &self,
        grantor_id: &str,
        state: &str,
        idx: usize,
        rng: &mut SubsystemRng,
    ) -> (TrustAccountRow, Vec<TrustBeneficiaryRow>) {
        let trust_types = &[
            ("revocable", 0.55),
            ("irrevocable", 0.25),
            ("testamentary", 0.10),
            ("special_needs", 0.10),
        ];
        let roll = rng.next_f64();
        let mut cum = 0.0;
        let mut trust_type = "revocable";
        for (tt, w) in trust_types {
            cum += w;
            if roll < cum {
                trust_type = tt;
                break;
            }
        }

        let is_revocable = trust_type == "revocable";
        let account_id = format!("trust-{idx:04}");
        let trust_name = format!("Trust-{idx:04}-Family");

        // Irrevocable trusts need their own EIN; revocable use grantor SSN
        let (trust_ein, tax_reporting_id, tax_treatment) = if is_revocable {
            (None, format!("grantor-ssn-{idx}"), "grantor")
        } else {
            let ein = self.generate_ein(state, idx + 10000, rng);
            let tid = ein.clone();
            (Some(ein), tid, "non-grantor")
        };

        let bene_count = (rng.next_u64_below(3) + 1) as i64;

        let trust_row = TrustAccountRow {
            account_id: account_id.clone(),
            run_id: self.run_id.clone(),
            trust_type: trust_type.to_string(),
            trust_name,
            trust_ein,
            grantor_customer_id: Some(grantor_id.to_string()),
            trustee_customer_id: grantor_id.to_string(), // self-trustee for revocable
            trustee_type: if is_revocable { "individual".into() } else { "corporate".into() },
            beneficiary_count: bene_count,
            revocable: is_revocable as i64,
            tax_reporting_id,
            tax_treatment: tax_treatment.to_string(),
            spendthrift_clause: if !is_revocable { 1 } else { 0 },
            special_needs_trust: (trust_type == "special_needs") as i64,
        };

        // Generate beneficiary rows
        let mut beneficiaries = Vec::new();
        for b in 0..bene_count {
            let share = 1.0 / bene_count as f64;
            beneficiaries.push(TrustBeneficiaryRow {
                beneficiary_id: format!("tb-{idx:04}-{b}"),
                account_id: account_id.clone(),
                run_id: self.run_id.clone(),
                beneficiary_customer_id: None,
                beneficiary_name: format!("TrustBene-{idx:04}-{b}"),
                beneficiary_type: if b == 0 { "primary".into() } else { "contingent".into() },
                beneficiary_share: share,
                conditions: if trust_type == "special_needs" {
                    Some("supplemental_needs_only".into())
                } else {
                    None
                },
            });
        }

        (trust_row, beneficiaries)
    }

    /// Generate an international customer record.
    fn generate_international(
        &self,
        customer_id: &str,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> CustomerInternationalRow {
        // Country distribution (simplified)
        let countries = &[
            ("GB", "low"),
            ("CA", "low"),
            ("MX", "low"),
            ("DE", "low"),
            ("IN", "low"),
            ("CN", "medium"),
            ("BR", "medium"),
            ("RU", "high"),
            ("IR", "high"),
            ("KP", "high"),
        ];
        let idx = rng.next_u64_below(countries.len() as u64) as usize;
        let (country, risk_level) = countries[idx];

        let is_us_person = rng.next_f64() < 0.30; // 30% are US persons living abroad
        let visa_status = if !is_us_person {
            Some(match rng.next_u64_below(4) {
                0 => "H1B",
                1 => "F1",
                2 => "L1",
                _ => "B1/B2",
            }.to_string())
        } else {
            None
        };

        // OFAC screening
        let high_risk_countries = ["RU", "IR", "KP", "SY", "CU"];
        let ofac_status = if high_risk_countries.contains(&country) {
            "flagged"
        } else if rng.next_f64() < 0.05 {
            "flagged" // 5% false positive rate
        } else {
            "clear"
        };

        // PEP status (~2%)
        let is_pep = rng.next_f64() < 0.02;

        let kyc_year = SIM_BASE_YEAR + 1;
        let kyc_month = (rng.next_u64_below(12) + 1) as i32;

        CustomerInternationalRow {
            customer_id: customer_id.to_string(),
            run_id: self.run_id.clone(),
            citizenship_country: country.to_string(),
            residency_country: if is_us_person { "US".into() } else { country.to_string() },
            is_us_person: is_us_person as i64,
            visa_status,
            foreign_tin: if !is_us_person { Some(format!("FT-{customer_id}")) } else { None },
            ofac_check_status: ofac_status.to_string(),
            sanctions_risk: risk_level.to_string(),
            pep_status: is_pep as i64,
            source_of_funds: Some("employment".to_string()),
            kyc_renewal_date: format!("{kyc_year}-{kyc_month:02}-01"),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Phase 3.5-prep Tier 4: Risk scoring, signers, joint, relationships
    // ─────────────────────────────────────────────────────────────────────────

    /// Compute a BSA/AML composite risk score for a customer.
    fn compute_risk_score(
        &self,
        identity_type: &str,
        addr_type: &str,
        seg: &SegmentConfig,
        is_international: bool,
        is_cash_business: bool,
        rng: &mut SubsystemRng,
    ) -> CustomerRiskScoreRow {
        // Identity risk: synthetic identities are high risk
        let identity_risk = match identity_type {
            "synthetic" => 0.85 + rng.next_f64() * 0.15,
            _ => rng.next_f64() * 0.25,
        };

        // Geographic risk: shelter addresses, PO boxes are higher
        let geo_risk = match addr_type {
            "homeless_shelter" => 0.60 + rng.next_f64() * 0.20,
            "cmra" => 0.50 + rng.next_f64() * 0.20,
            "po_box" => 0.30 + rng.next_f64() * 0.20,
            _ => rng.next_f64() * 0.20,
        };

        // Product risk: business accounts are higher risk
        let product_risk = if seg.id == "small_business" {
            0.30 + rng.next_f64() * 0.30
        } else if seg.id == "premium" {
            0.15 + rng.next_f64() * 0.20
        } else {
            rng.next_f64() * 0.25
        };

        // Behavior risk: starts low, will evolve as txn data accumulates
        let behavior_risk = rng.next_f64() * 0.15;

        // Sanctions risk
        let sanctions_risk = if is_international { 0.30 + rng.next_f64() * 0.40 } else { 0.0 };

        // Cash business penalty
        let cash_penalty = if is_cash_business { 0.15 } else { 0.0 };

        // Composite: weighted average
        let composite = identity_risk * 0.25
            + geo_risk * 0.15
            + product_risk * 0.20
            + behavior_risk * 0.15
            + sanctions_risk * 0.15
            + cash_penalty * 0.10;

        let composite_label = if composite >= 0.70 {
            "critical"
        } else if composite >= 0.50 {
            "high"
        } else if composite >= 0.25 {
            "medium"
        } else {
            "low"
        };

        let edd = composite >= 0.50;

        CustomerRiskScoreRow {
            customer_id: String::new(), // filled by caller
            run_id: self.run_id.clone(),
            composite_risk: composite_label.to_string(),
            identity_risk_score: identity_risk,
            geographic_risk_score: geo_risk,
            product_risk_score: product_risk,
            behavior_risk_score: behavior_risk,
            sanctions_risk_score: sanctions_risk,
            edd_required: edd as i64,
            edd_last_review_tick: None,
            risk_override: None,
            risk_override_reason: None,
        }
    }

    /// Generate a joint ownership record for married customers.
    fn generate_joint_ownership(
        &self,
        account_id: &str,
        customer_id: &str,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> (JointOwnershipRow, JointOwnershipRow) {
        // Ownership type distribution
        let otype = if rng.next_f64() < 0.70 { "jtros" }
            else if rng.next_f64() < 0.60 { "community_property" }
            else { "tic" };

        let primary_pct = 0.50; // equal split for joint
        let secondary_id = format!("spouse-of-{customer_id}");

        let primary = JointOwnershipRow {
            ownership_id: format!("jo-{account_id}-0"),
            account_id: account_id.to_string(),
            run_id: self.run_id.clone(),
            owner_customer_id: customer_id.to_string(),
            ownership_percentage: primary_pct,
            ownership_type: otype.to_string(),
            survivorship_rights: if otype == "jtros" { 1 } else { 0 },
        };

        let secondary = JointOwnershipRow {
            ownership_id: format!("jo-{account_id}-1"),
            account_id: account_id.to_string(),
            run_id: self.run_id.clone(),
            owner_customer_id: secondary_id,
            ownership_percentage: 1.0 - primary_pct,
            ownership_type: otype.to_string(),
            survivorship_rights: if otype == "jtros" { 1 } else { 0 },
        };

        (primary, secondary)
    }
}

impl SimSubsystem for CustomerSubsystem {
    fn name(&self) -> &'static str {
        "customer"
    }

    fn update(
        &mut self,
        tick: Tick,
        events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut out_events = Vec::new();

        // Tick 0: generate initial population.
        if !self.initialized {
            self.initialized = true;
            let population = self.generate_initial_population(rng, tick)?;
            let mut onboarded = 0usize;

            for (customer, account_id) in population {
                self.store.insert_customer(&self.run_id, &customer)?;
                self.store.insert_account(
                    &self.run_id,
                    &account_id,
                    &customer.customer_id,
                    &customer.product_id,
                    customer.payroll_amount * 2.0,
                    tick,
                )?;

                // ── Phase 3.5-prep: Generate identity attributes ──────────
                let region = self.pick_region(rng).clone();
                let seg = self.config.segments.get(&customer.segment)
                    .expect("segment must exist in config");

                // SSN / identity
                let (ssn_full, ssn_area, ssn_group, ssn_serial, ssn_status, identity_type) =
                    self.generate_ssn(onboarded, &region, rng);

                let (date_of_birth, age_at_open) = self.generate_dob(seg, tick, rng);

                let identity_row = CustomerIdentityRow {
                    customer_id: customer.customer_id.clone(),
                    run_id: self.run_id.clone(),
                    ssn_full: ssn_full.clone(),
                    ssn_area,
                    ssn_group,
                    ssn_serial,
                    ssn_status: ssn_status.to_string(),
                    identity_type: identity_type.to_string(),
                    date_of_birth,
                    age_at_open,
                    ssn_shared_count: 0,
                    ssn_first_seen_tick: tick as i64,
                };
                self.store.insert_customer_identity(&identity_row)?;

                // Address
                let addr_row = self.generate_address(seg, &region, tick, &customer.customer_id, rng);
                let is_shelter = addr_row.address_type == "homeless_shelter";
                let state_code = addr_row.state.clone();
                self.store.insert_customer_address(&addr_row)?;

                // Phone
                let phone_row = self.generate_phone(&region, tick, &customer.customer_id, rng);
                self.store.insert_customer_phone(&phone_row)?;

                // Update customer table with state and vulnerability
                self.store.update_customer_state(&self.run_id, &customer.customer_id, &state_code)?;
                if is_shelter {
                    self.store.update_customer_vulnerability(
                        &self.run_id,
                        &customer.customer_id,
                        true,
                        Some("housing_insecure"),
                    )?;
                }

                // ── Phase 3.5-prep Tier 2: Demographics, business entity, beneficiary
                let age = identity_row.age_at_open;
                let marital_status = self.assign_marital_status(age, rng);
                let (employment_status, annual_income, credit_score, home_ownership) =
                    self.assign_employment(seg, rng);

                let dependents = if marital_status == "married" {
                    rng.next_u64_below(4) as i64
                } else {
                    rng.next_u64_below(2) as i64
                };
                let military = if rng.next_f64() < 0.06 { "veteran" }
                    else if rng.next_f64() < 0.01 { "active_duty" }
                    else { "civilian" };

                self.store.update_customer_demographics(
                    &self.run_id, &customer.customer_id,
                    marital_status, employment_status, annual_income,
                    credit_score, home_ownership, dependents, military,
                )?;

                // Account type category
                let acct_category = if seg.id == "small_business" {
                    "business_checking"
                } else {
                    "checking_individual"
                };
                let tax_id = ssn_full.clone();
                self.store.update_account_type_category(
                    &self.run_id, &account_id,
                    acct_category, "sole", "1099", &tax_id,
                )?;

                // Business entity for small_business segment
                if seg.id == "small_business" {
                    let (entity_row, dba_row) = self.generate_business_entity(
                        &customer.customer_id, &state_code, onboarded, tick, rng,
                    );
                    self.store.insert_business_entity(&entity_row)?;
                    if let Some(dba) = dba_row {
                        self.store.insert_dba_registration(&dba)?;
                    }
                }

                // Beneficiary (for married + POD-eligible customers)
                if let Some(bene) = self.generate_beneficiary(
                    &account_id, &customer.customer_id, marital_status, rng,
                ) {
                    self.store.insert_customer_beneficiary(&bene)?;
                }

                // ── Phase 3.5-prep Tier 3: Custodial, trust, international ──

                // ~2% of customers with age<50 get a custodial account for a minor
                if age < 50 && rng.next_f64() < 0.02 {
                    let custodial = self.generate_custodial_account(
                        &customer.customer_id, &ssn_full, &state_code,
                        onboarded, tick, rng,
                    );
                    self.store.insert_custodial_account(&custodial)?;
                }

                // ~3% of premium customers get a trust account
                if seg.id == "premium" && rng.next_f64() < 0.03 {
                    let (trust_row, benes) = self.generate_trust_account(
                        &customer.customer_id, &state_code, onboarded, rng,
                    );
                    self.store.insert_trust_account(&trust_row)?;
                    for bene in &benes {
                        self.store.insert_trust_beneficiary(bene)?;
                    }
                }

                // ~3% are international customers
                if rng.next_f64() < self.config.identity_address.international_customer_rate {
                    let intl = self.generate_international(
                        &customer.customer_id, tick, rng,
                    );
                    self.store.insert_customer_international(&intl)?;
                }

                // ── Phase 3.5-prep Tier 4: Risk scoring, signers, joint, relationships

                // Risk scoring for every customer
                let is_intl = rng.next_f64() < self.config.identity_address.international_customer_rate;
                let is_cash_biz = seg.id == "small_business" && rng.next_f64() < 0.50;
                let mut risk_row = self.compute_risk_score(
                    identity_type, &addr_row.address_type, seg,
                    is_intl, is_cash_biz, rng,
                );
                risk_row.customer_id = customer.customer_id.clone();
                self.store.insert_customer_risk_score(&risk_row)?;

                // Authorized signer: ~10% of accounts get an additional signer
                if rng.next_f64() < 0.10 {
                    let signer = AuthorizedSignerRow {
                        signer_id: format!("sig-{}", &customer.customer_id),
                        account_id: account_id.clone(),
                        run_id: self.run_id.clone(),
                        signer_customer_id: format!("auth-{}", &customer.customer_id),
                        signer_role: if rng.next_f64() < 0.3 { "poa".into() } else { "authorized_signer".into() },
                        authority_level: if rng.next_f64() < 0.6 { "full".into() } else { "limited".into() },
                        added_tick: tick as i64,
                        removed_tick: None,
                        is_active: 1,
                    };
                    self.store.insert_authorized_signer(&signer)?;
                }

                // Joint ownership for ~30% of married customers
                if marital_status == "married" && rng.next_f64() < 0.30 {
                    let (primary, secondary) = self.generate_joint_ownership(
                        &account_id, &customer.customer_id, tick, rng,
                    );
                    self.store.insert_joint_ownership(&primary)?;
                    self.store.insert_joint_ownership(&secondary)?;

                    // Declared spouse relationship
                    let rel = CustomerRelationshipRow {
                        relationship_id: format!("rel-sp-{}", &customer.customer_id),
                        run_id: self.run_id.clone(),
                        customer_id_a: customer.customer_id.clone(),
                        customer_id_b: format!("spouse-of-{}", &customer.customer_id),
                        relationship_type: "spouse".to_string(),
                        strength: 1.0,
                        detected_tick: tick as i64,
                        detection_method: "declared".to_string(),
                        is_suspicious: 0,
                    };
                    self.store.insert_customer_relationship(&rel)?;
                }

                // Emit identity created event
                out_events.push(SimEvent::CustomerIdentityCreated {
                    tick,
                    customer_id: customer.customer_id.clone(),
                    ssn_status: ssn_status.to_string(),
                    identity_type: identity_type.to_string(),
                });

                // Emit onboarding event
                out_events.push(SimEvent::CustomerOnboarded {
                    tick,
                    customer_id: customer.customer_id.clone(),
                    segment: customer.segment.clone(),
                    account_id,
                });

                onboarded += 1;
            }
            log::info!("tick=0 customer: onboarded {onboarded} customers with full profile");
            return Ok(out_events);
        }

        // Process fee events that affect satisfaction and churn risk.
        for event in events_in {
            if let SimEvent::FeeCharged {
                customer_id,
                fee_type,
                ..
            } = event
            {
                self.store.update_customer_satisfaction(
                    &self.run_id,
                    customer_id,
                    match fee_type.as_str() {
                        "overdraft" => -0.04,
                        "nsf"       => -0.06,
                        _           => -0.01,
                    },
                )?;
            }
        }

        // Apply satisfaction decay every 30 ticks (monthly).
        if tick.is_multiple_of(30) {
            let active = self.store.active_customers(&self.run_id)?;
            for mut c in active {
                if c.satisfaction > 0.6 {
                    c.satisfaction = (c.satisfaction - SATISFACTION_DECAY_PER_TICK * 30.0).max(0.0);
                    self.store.update_customer_churn_satisfaction(
                        &self.run_id,
                        &c.customer_id,
                        c.churn_risk,
                        c.satisfaction,
                    )?;
                }
            }
        }

        Ok(out_events)
    }


    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
