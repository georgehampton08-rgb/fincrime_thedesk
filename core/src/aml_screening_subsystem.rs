//! AML Screening subsystem — Phase 3.5 Week 4.
//!
//! This subsystem:
//!   1. Screens new customers against OFAC sanctions lists
//!   2. Identifies Politically Exposed Persons (PEPs)
//!   3. Assesses jurisdiction risk for international customers
//!   4. Calculates customer AML risk ratings
//!   5. Generates AML alerts for high-risk matches
//!   6. Computes weekly AML metrics

use crate::{
    error::SimResult,
    event::SimEvent,
    rng::SubsystemRng,
    store::SimStore,
    subsystem::SimSubsystem,
    types::{RunId, Tick},
};
use std::any::Any;

// ── Constants ────────────────────────────────────────────────────────────────

const OFAC_EXACT_MATCH_THRESHOLD: f64 = 0.95;
const OFAC_FUZZY_MATCH_THRESHOLD: f64 = 0.80;
const PEP_NAME_MATCH_THRESHOLD: f64 = 0.85;
const RISK_RATING_THRESHOLD_HIGH: f64 = 0.60;
const RISK_RATING_THRESHOLD_CRITICAL: f64 = 0.80;
const SCREENING_INTERVAL_TICKS: u64 = 30; // Monthly rescreening
const METRICS_INTERVAL: u64 = 7; // Weekly

// ── Subsystem ────────────────────────────────────────────────────────────────

pub struct AMLScreeningSubsystem {
    run_id: RunId,
    store: SimStore,
}

impl AMLScreeningSubsystem {
    pub fn new(run_id: RunId, store: SimStore) -> Self {
        Self { run_id, store }
    }

    /// Screen new customers against OFAC sanctions list.
    fn screen_ofac(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get customers onboarded in last 7 days (new customers)
        let recent_customers = self.store.get_customers_onboarded_in_window(
            &self.run_id,
            tick.saturating_sub(7) as i64,
            tick as i64,
        )?;

        for customer in recent_customers {
            // Generate synthetic customer name for matching (name not stored in DB)
            let customer_name = self.generate_customer_name(&customer.customer_id, rng);

            // Check against OFAC watchlist
            let watchlist = self.store.get_ofac_watchlist()?;

            for entry in &watchlist {
                // Calculate name match score (simplified fuzzy matching)
                let match_score = self.calculate_name_match_score(
                    &customer_name,
                    &entry.full_name,
                    rng,
                );

                if match_score >= OFAC_FUZZY_MATCH_THRESHOLD {
                    let match_type = if match_score >= OFAC_EXACT_MATCH_THRESHOLD {
                        "exact_match"
                    } else {
                        "fuzzy_match"
                    };

                    let screening_id = format!(
                        "aml-ofac-{}-{}-{}",
                        customer.customer_id,
                        entry.entity_id,
                        tick
                    );

                    let details = serde_json::json!({
                        "customer_name": customer_name,
                        "watchlist_name": entry.full_name,
                        "program": entry.program,
                        "match_score": match_score,
                    }).to_string();

                    self.store.insert_aml_screening_result(
                        &self.run_id,
                        &screening_id,
                        &customer.customer_id,
                        tick as i64,
                        "ofac_sanctions",
                        match_type,
                        match_score,
                        Some(&entry.entity_id),
                        &details,
                        if entry.program == "SDN" { 0.50 } else { 0.30 },
                    )?;

                    // Generate critical alert for SDN matches
                    if entry.program == "SDN" && match_score >= OFAC_EXACT_MATCH_THRESHOLD {
                        let alert_id = format!("aml-alert-ofac-{}-{}", customer.customer_id, tick);

                        self.store.insert_aml_alert(
                            &self.run_id,
                            &alert_id,
                            &customer.customer_id,
                            tick as i64,
                            "sanctions_hit",
                            "critical",
                            &format!("OFAC SDN exact match: {}", entry.full_name),
                            &details,
                        )?;

                        events.push(SimEvent::AMLAlertGenerated {
                            tick,
                            alert_id,
                            alert_type: "sanctions_hit".to_string(),
                            customer_id: customer.customer_id.clone(),
                            severity: "critical".to_string(),
                            risk_score: match_score,
                        });
                    }

                    events.push(SimEvent::AMLScreeningHit {
                        tick,
                        screening_id,
                        screening_type: "ofac_sanctions".to_string(),
                        customer_id: customer.customer_id.clone(),
                        match_type: match_type.to_string(),
                        match_score,
                    });

                    break; // Only record first match per customer
                }
            }
        }

        Ok(events)
    }

    /// Identify Politically Exposed Persons (PEPs).
    fn screen_pep(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get recently onboarded customers
        let recent_customers = self.store.get_customers_onboarded_in_window(
            &self.run_id,
            tick.saturating_sub(7) as i64,
            tick as i64,
        )?;

        for customer in recent_customers {
            // Generate synthetic customer name for matching
            let customer_name = self.generate_customer_name(&customer.customer_id, rng);

            // Check against PEP registry
            let pep_registry = self.store.get_pep_registry()?;

            for pep in &pep_registry {
                let match_score = self.calculate_name_match_score(
                    &customer_name,
                    &pep.full_name,
                    rng,
                );

                if match_score >= PEP_NAME_MATCH_THRESHOLD {
                    let screening_id = format!(
                        "aml-pep-{}-{}-{}",
                        customer.customer_id,
                        pep.pep_id,
                        tick
                    );

                    let details = serde_json::json!({
                        "customer_name": customer_name,
                        "pep_name": pep.full_name,
                        "position": pep.position,
                        "country": pep.country_code,
                        "match_score": match_score,
                    }).to_string();

                    let risk_impact = 0.20 * pep.risk_multiplier;

                    self.store.insert_aml_screening_result(
                        &self.run_id,
                        &screening_id,
                        &customer.customer_id,
                        tick as i64,
                        "pep_match",
                        if match_score >= 0.95 { "exact_match" } else { "fuzzy_match" },
                        match_score,
                        Some(&pep.pep_id),
                        &details,
                        risk_impact,
                    )?;

                    // Generate alert for Tier 1 PEPs
                    if pep.position_level == "tier_1_national" {
                        let alert_id = format!("aml-alert-pep-{}-{}", customer.customer_id, tick);

                        self.store.insert_aml_alert(
                            &self.run_id,
                            &alert_id,
                            &customer.customer_id,
                            tick as i64,
                            "pep_identified",
                            "high",
                            &format!("PEP match - {}: {}", pep.position, pep.full_name),
                            &details,
                        )?;

                        events.push(SimEvent::AMLAlertGenerated {
                            tick,
                            alert_id,
                            alert_type: "pep_identified".to_string(),
                            customer_id: customer.customer_id.clone(),
                            severity: "high".to_string(),
                            risk_score: match_score,
                        });
                    }

                    events.push(SimEvent::AMLScreeningHit {
                        tick,
                        screening_id,
                        screening_type: "pep_match".to_string(),
                        customer_id: customer.customer_id.clone(),
                        match_type: if match_score >= 0.95 { "exact_match" } else { "fuzzy_match" }.to_string(),
                        match_score,
                    });

                    break; // Only record first PEP match
                }
            }
        }

        Ok(events)
    }

    /// Assess jurisdiction risk for international customers.
    fn assess_jurisdiction_risk(
        &self,
        tick: Tick,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Get international customers onboarded recently
        let recent_international = self.store.get_international_customers_in_window(
            &self.run_id,
            tick.saturating_sub(7) as i64,
            tick as i64,
        )?;

        for customer_intl in recent_international {
            // Check if citizenship or country of residence is high-risk
            let high_risk_jurisdictions = self.store.get_high_risk_jurisdictions()?;

            for jurisdiction in &high_risk_jurisdictions {
                if customer_intl.citizenship_country == jurisdiction.country_code
                    || customer_intl.residency_country == jurisdiction.country_code
                {
                    let screening_id = format!(
                        "aml-jurisdiction-{}-{}-{}",
                        customer_intl.customer_id,
                        jurisdiction.country_code,
                        tick
                    );

                    let details = serde_json::json!({
                        "country": jurisdiction.country_name,
                        "country_code": jurisdiction.country_code,
                        "risk_category": jurisdiction.risk_category,
                        "risk_level": jurisdiction.risk_level,
                    }).to_string();

                    let risk_impact = match jurisdiction.risk_level.as_str() {
                        "critical" => 0.40,
                        "high" => 0.25,
                        "elevated" => 0.15,
                        _ => 0.10,
                    };

                    self.store.insert_aml_screening_result(
                        &self.run_id,
                        &screening_id,
                        &customer_intl.customer_id,
                        tick as i64,
                        "jurisdiction_risk",
                        "exact_match",
                        1.0,
                        Some(&jurisdiction.country_code),
                        &details,
                        risk_impact,
                    )?;

                    // Alert for critical jurisdictions
                    if jurisdiction.risk_level == "critical" {
                        let alert_id = format!("aml-alert-jurisdiction-{}-{}", customer_intl.customer_id, tick);

                        self.store.insert_aml_alert(
                            &self.run_id,
                            &alert_id,
                            &customer_intl.customer_id,
                            tick as i64,
                            "high_risk_jurisdiction",
                            "high",
                            &format!("Customer from high-risk jurisdiction: {}", jurisdiction.country_name),
                            &details,
                        )?;

                        events.push(SimEvent::AMLAlertGenerated {
                            tick,
                            alert_id,
                            alert_type: "high_risk_jurisdiction".to_string(),
                            customer_id: customer_intl.customer_id.clone(),
                            severity: "high".to_string(),
                            risk_score: risk_impact,
                        });
                    }

                    events.push(SimEvent::AMLScreeningHit {
                        tick,
                        screening_id,
                        screening_type: "jurisdiction_risk".to_string(),
                        customer_id: customer_intl.customer_id.clone(),
                        match_type: "exact_match".to_string(),
                        match_score: 1.0,
                    });
                }
            }
        }

        Ok(events)
    }

    /// Calculate customer AML risk rating (monthly).
    fn calculate_risk_ratings(
        &self,
        tick: Tick,
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        // Only run monthly
        if !tick.is_multiple_of(30) {
            return Ok(events);
        }

        // Get all active customers
        let customers = self.store.get_all_active_customers(&self.run_id)?;

        for customer in customers {
            let mut sanctions_risk = 0.0;
            let mut pep_risk = 0.0;
            let mut jurisdiction_risk = 0.0;

            // Aggregate screening results from last 90 days
            let screenings = self.store.get_customer_aml_screenings(
                &self.run_id,
                &customer.customer_id,
                tick.saturating_sub(90) as i64,
                tick as i64,
            )?;

            for screening in screenings {
                match screening.screening_type.as_str() {
                    "ofac_sanctions" => {
                        sanctions_risk = f64::max(sanctions_risk, screening.risk_impact);
                    }
                    "pep_match" => {
                        pep_risk = f64::max(pep_risk, screening.risk_impact);
                    }
                    "jurisdiction_risk" => {
                        jurisdiction_risk = f64::max(jurisdiction_risk, screening.risk_impact);
                    }
                    _ => {}
                }
            }

            // Behavioral risk component (simplified - could integrate with fraud detection)
            let behavioral_risk = rng.next_f64() * 0.10;

            // Transaction risk component (placeholder)
            let transaction_risk = 0.05;

            // Calculate overall risk score
            let mut risk_score = sanctions_risk * 2.0  // Sanctions are most critical
                + pep_risk * 1.5
                + jurisdiction_risk * 1.2
                + transaction_risk * 0.8
                + behavioral_risk * 0.5;

            // Add noise and clamp
            risk_score += rng.next_f64() * 0.05;
            risk_score = risk_score.min(1.0);

            // Determine risk rating
            let overall_risk_rating = if risk_score >= RISK_RATING_THRESHOLD_CRITICAL {
                "critical"
            } else if risk_score >= RISK_RATING_THRESHOLD_HIGH {
                "high"
            } else if risk_score >= 0.30 {
                "medium"
            } else {
                "low"
            };

            let requires_edd = if risk_score >= RISK_RATING_THRESHOLD_HIGH { 1 } else { 0 };

            self.store.insert_customer_aml_risk(
                &self.run_id,
                &customer.customer_id,
                tick as i64,
                overall_risk_rating,
                risk_score,
                sanctions_risk,
                pep_risk,
                jurisdiction_risk,
                transaction_risk,
                behavioral_risk,
                tick as i64,
                requires_edd,
            )?;

            // Generate alert for risk rating elevation
            if risk_score >= RISK_RATING_THRESHOLD_HIGH {
                let alert_id = format!("aml-alert-risk-{}-{}", customer.customer_id, tick);

                self.store.insert_aml_alert(
                    &self.run_id,
                    &alert_id,
                    &customer.customer_id,
                    tick as i64,
                    "risk_rating_elevated",
                    if risk_score >= RISK_RATING_THRESHOLD_CRITICAL { "critical" } else { "high" },
                    &format!("Customer AML risk rating elevated to {}", overall_risk_rating),
                    &serde_json::json!({
                        "risk_score": risk_score,
                        "sanctions_risk": sanctions_risk,
                        "pep_risk": pep_risk,
                        "jurisdiction_risk": jurisdiction_risk,
                    }).to_string(),
                )?;

                events.push(SimEvent::AMLAlertGenerated {
                    tick,
                    alert_id,
                    alert_type: "risk_rating_elevated".to_string(),
                    customer_id: customer.customer_id.clone(),
                    severity: if risk_score >= RISK_RATING_THRESHOLD_CRITICAL { "critical" } else { "high" }.to_string(),
                    risk_score,
                });
            }

            events.push(SimEvent::AMLRiskRatingComputed {
                tick,
                customer_id: customer.customer_id.clone(),
                risk_rating: overall_risk_rating.to_string(),
                risk_score,
                requires_edd: requires_edd != 0,
            });
        }

        Ok(events)
    }

    /// Compute weekly AML metrics.
    fn compute_metrics(&self, tick: Tick) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        if !tick.is_multiple_of(METRICS_INTERVAL) {
            return Ok(events);
        }

        let start_tick = tick.saturating_sub(7);
        let metrics = self.store.compute_aml_metrics(&self.run_id, start_tick as i64, tick as i64)?;

        self.store.insert_aml_metrics(
            &self.run_id,
            tick as i64,
            metrics.screenings_performed,
            metrics.sanctions_hits,
            metrics.pep_matches,
            metrics.high_risk_customers,
            metrics.alerts_generated,
            metrics.false_positive_rate,
        )?;

        events.push(SimEvent::AMLMetricsComputed {
            tick,
            screenings_7d: metrics.screenings_performed,
            sanctions_hits_7d: metrics.sanctions_hits,
            pep_matches_7d: metrics.pep_matches,
            alerts_generated_7d: metrics.alerts_generated,
        });

        Ok(events)
    }

    /// Generate a synthetic customer name from customer_id for screening.
    /// Uses deterministic RNG to ensure reproducibility.
    fn generate_customer_name(&self, customer_id: &str, rng: &mut SubsystemRng) -> String {
        const FIRST_NAMES: &[&str] = &[
            "John", "Maria", "Chen", "Ahmed", "Sofia", "Nikolai", "Elena", "Jean",
            "Alexander", "Li", "Carlos", "Anna", "David", "Yuki", "Mohammed",
        ];
        const LAST_NAMES: &[&str] = &[
            "Smith", "Garcia", "Wang", "Al-Mansoori", "Martinez", "Petrov", "Dubois",
            "Chen", "Santos", "Volkov", "Rodriguez", "Kim", "Johnson", "Oliveira",
        ];

        let hash = customer_id.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        let first_idx = (hash % FIRST_NAMES.len() as u64) as usize;
        let last_idx = ((hash / 100) % LAST_NAMES.len() as u64) as usize;

        format!("{} {}", FIRST_NAMES[first_idx], LAST_NAMES[last_idx])
    }

    /// Simplified fuzzy name matching with deterministic noise.
    fn calculate_name_match_score(
        &self,
        name1: &str,
        name2: &str,
        rng: &mut SubsystemRng,
    ) -> f64 {
        let n1 = name1.to_lowercase();
        let n2 = name2.to_lowercase();

        // Exact match
        if n1 == n2 {
            return 1.0;
        }

        // Simple heuristic: count matching words
        let words1: Vec<&str> = n1.split_whitespace().collect();
        let words2: Vec<&str> = n2.split_whitespace().collect();

        if words1.is_empty() || words2.is_empty() {
            return 0.0;
        }

        let mut matching_words = 0;
        for w1 in &words1 {
            for w2 in &words2 {
                if w1 == w2 || w1.starts_with(w2) || w2.starts_with(w1) {
                    matching_words += 1;
                    break;
                }
            }
        }

        let base_score = matching_words as f64 / words1.len().max(words2.len()) as f64;

        // Add deterministic noise
        let noise = rng.next_f64() * 0.05;
        (base_score + noise).min(1.0)
    }
}

impl SimSubsystem for AMLScreeningSubsystem {
    fn name(&self) -> &'static str {
        "aml_screening"
    }

    fn update(
        &mut self,
        tick: Tick,
        _events_in: &[SimEvent],
        rng: &mut SubsystemRng,
    ) -> SimResult<Vec<SimEvent>> {
        let mut events = Vec::new();

        if tick == 0 {
            return Ok(events);
        }

        // 1. OFAC sanctions screening (daily for new customers)
        events.extend(self.screen_ofac(tick, rng)?);

        // 2. PEP screening (daily for new customers)
        events.extend(self.screen_pep(tick, rng)?);

        // 3. Jurisdiction risk assessment (daily for new international customers)
        events.extend(self.assess_jurisdiction_risk(tick)?);

        // 4. Calculate customer risk ratings (monthly)
        events.extend(self.calculate_risk_ratings(tick, rng)?);

        // 5. Compute metrics (weekly)
        events.extend(self.compute_metrics(tick)?);

        Ok(events)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
