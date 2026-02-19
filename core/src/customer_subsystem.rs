use crate::{
    config::{RegionPool, SegmentConfig, SimConfig},
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::{
        CustomerAddressRow, CustomerIdentityRow, CustomerPhoneRow, SimStore,
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
            log::info!("tick=0 customer: onboarded {onboarded} customers with identity data");
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
