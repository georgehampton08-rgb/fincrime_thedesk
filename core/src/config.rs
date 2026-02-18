use crate::types::Tick;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductConfig {
    pub product_id:        String,
    pub product_type:      String,
    pub tier:              String,
    pub label:             String,
    pub monthly_fee:       f64,
    pub overdraft_fee:     f64,
    pub nsf_fee:           f64,
    pub atm_fee:           f64,
    pub wire_fee:          f64,
    pub interest_rate:     f64,
    pub min_balance_waive: Option<f64>,
    pub min_dd_waive:      Option<f64>,
    pub target_segment:    String,
}

#[derive(Debug, Clone, Deserialize)]
struct ProductCatalogFile {
    products: Vec<ProductConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentConfig {
    pub id:                         String,
    pub label:                      String,
    pub population_share:           f64,
    pub income_bands:               Vec<String>,
    pub income_band_weights:        Vec<f64>,
    pub monthly_txn_count_mean:     f64,
    pub monthly_txn_count_std:      f64,
    pub txn_amount_pareto_xmin:     f64,
    pub txn_amount_pareto_alpha:    f64,
    pub cash_intensity:             f64,
    pub payroll_probability:        f64,
    pub payroll_amount_mean:        f64,
    pub payroll_amount_std:         f64,
    pub overdraft_probability_per_tick: f64,
    pub nsf_probability_per_tick:   f64,
    pub base_churn_rate_per_tick:   f64,
    pub fee_sensitivity:            f64,
    pub products:                   Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplaintTrigger {
    pub event_type:           String,
    #[serde(default)]
    pub fee_type:             Option<String>,
    #[serde(default)]
    pub amount_threshold:     Option<f64>,
    #[serde(default)]
    pub prior_breach:         bool,
    pub probability:          f64,
    pub issue_category:       String,
    pub priority:             String,
    pub sla_acknowledge_days: u64,
    pub sla_resolve_days:     u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionCode {
    pub code:                String,
    pub satisfaction_delta:  f64,
    pub churn_risk_delta:    f64,
    pub avg_amount_refunded: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct SegmentsFile {
    segments: Vec<SegmentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct ComplaintConfigFile {
    triggers:         Vec<ComplaintTrigger>,
    resolution_codes: Vec<ResolutionCode>,
}

// ── Phase 2.2: Offer catalog ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferConfig {
    pub offer_id:     String,
    pub offer_type:   String,
    pub label:        String,
    pub product_id:   Option<String>,
    pub bonus_amount: f64,
    pub requirements: OfferRequirements,
    pub eligibility:  OfferEligibility,
    pub cost_model:   OfferCostModel,
    pub fraud_risk:   OfferFraudRisk,
    pub active:       bool,
    pub start_tick:   u64,
    pub end_tick:     Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferRequirements {
    pub min_direct_deposit: f64,
    pub min_balance:        f64,
    pub duration_ticks:     u64,
    pub new_to_bank_only:   bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferEligibility {
    pub target_segments:      Vec<String>,
    pub exclude_segments:     Vec<String>,
    pub min_credit_score:     Option<f64>,
    pub max_existing_products: Option<usize>,
    #[serde(default)]
    pub min_churn_risk:       Option<f64>,
    #[serde(default)]
    pub max_churn_risk:       Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferCostModel {
    pub bonus_paid_on_completion: bool,
    pub promo_rate_duration:      u64,
    pub promo_rate_delta:         f64,
    #[serde(default)]
    pub fee_waiver_duration:      Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferFraudRisk {
    pub bonus_seeker_probability: f64,
    pub velocity_flag_threshold:  usize,
}

#[derive(Debug, Clone, Deserialize)]
struct OfferCatalogFile {
    offers: Vec<OfferConfig>,
}

// ── Phase 2.3: Churn model ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnModelConfig {
    pub model_version:           String,
    pub update_frequency_ticks:  Tick,
    pub segment_base_rates:      HashMap<String, SegmentChurnParams>,
    pub churn_formula:           ChurnFormulaWeights,
    pub life_events:             Vec<LifeEventConfig>,
    pub churn_thresholds:        ChurnThresholds,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentChurnParams {
    pub monthly_churn_rate:            f64,
    pub annual_churn_rate:             f64,
    pub fee_sensitivity:               f64,
    pub service_sensitivity:           f64,
    pub offer_retention_effectiveness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnFormulaWeights {
    pub base_rate_weight:           f64,
    pub satisfaction_weight:        f64,
    pub satisfaction_equilibrium:   f64,
    pub fee_burden_weight:          f64,
    pub fee_burden_threshold:       f64,
    pub complaint_weight:           f64,
    pub complaint_lookback_ticks:   Tick,
    pub sla_breach_weight:          f64,
    pub sla_breach_lookback_ticks:  Tick,
    pub inactivity_weight:          f64,
    pub inactivity_threshold_ticks: Tick,
    pub product_depth_bonus:        f64,
    pub retention_offer_bonus:      f64,
    pub life_event_multiplier:      f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifeEventConfig {
    pub event_type:           String,
    pub probability_per_year: f64,
    #[serde(default)]
    pub segments:             Vec<String>,
    pub churn_risk_delta:     f64,
    pub duration_ticks:       Tick,
    pub behavioral_changes:   serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChurnThresholds {
    pub low_risk:       f64,
    pub medium_risk:    f64,
    pub high_risk:      f64,
    pub imminent_churn: f64,
}

// ── Phase 2.1: Fee constraints ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeConstraint {
    pub fee_type:            String,
    pub min_value:           f64,
    pub max_value:           f64,
    pub soft_limit:          f64,
    pub soft_limit_warning:  String,
    pub hard_limit_reason:   String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeImpactFormula {
    #[serde(flatten)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct FeeConstraintsFile {
    fee_types:       Vec<FeeConstraint>,
    impact_formulas: HashMap<String, FeeImpactFormula>,
}

#[derive(Debug, Clone)]
pub struct SimConfig {
    pub segments:            HashMap<String, SegmentConfig>,
    pub initial_population:  usize,
    pub complaint_triggers:  Vec<ComplaintTrigger>,
    pub resolution_codes:    HashMap<String, ResolutionCode>,
    pub products:            HashMap<String, ProductConfig>,
    pub fee_constraints:     HashMap<String, FeeConstraint>,
    pub impact_formulas:     HashMap<String, FeeImpactFormula>,
    pub offers:              HashMap<String, OfferConfig>,
    pub churn_model:         ChurnModelConfig,
}

impl SimConfig {
    /// Load from the data/ directory.
    /// In tests, use SimConfig::default_test().
    pub fn load(data_dir: &str) -> anyhow::Result<Self> {
        let path = format!("{data_dir}/segments/segments.json");
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Cannot read {path}: {e}"))?;
        let file: SegmentsFile = serde_json::from_str(&content)?;
        let segments = file.segments.into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();

        let complaint_path = format!("{data_dir}/complaints/complaint_triggers.json");
        let complaint_content = std::fs::read_to_string(&complaint_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {complaint_path}: {e}"))?;
        let complaint_file: ComplaintConfigFile = serde_json::from_str(&complaint_content)?;
        let resolution_codes = complaint_file.resolution_codes.into_iter()
            .map(|r| (r.code.clone(), r))
            .collect();

        let product_path = format!("{data_dir}/products/product_catalog.json");
        let product_content = std::fs::read_to_string(&product_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {product_path}: {e}"))?;
        let product_file: ProductCatalogFile = serde_json::from_str(&product_content)?;
        let products = product_file.products.into_iter()
            .map(|p| (p.product_id.clone(), p))
            .collect();

        let fee_path = format!("{data_dir}/products/fee_constraints.json");
        let fee_content = std::fs::read_to_string(&fee_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {fee_path}: {e}"))?;
        let fee_file: FeeConstraintsFile = serde_json::from_str(&fee_content)?;
        let fee_constraints = fee_file.fee_types.into_iter()
            .map(|f| (f.fee_type.clone(), f))
            .collect();

        let offer_path = format!("{data_dir}/offers/offer_catalog.json");
        let offer_content = std::fs::read_to_string(&offer_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {offer_path}: {e}"))?;
        let offer_file: OfferCatalogFile = serde_json::from_str(&offer_content)?;
        let offers = offer_file.offers.into_iter()
            .map(|o| (o.offer_id.clone(), o))
            .collect();

        let churn_path = format!("{data_dir}/churn/churn_model_config.json");
        let churn_content = std::fs::read_to_string(&churn_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {churn_path}: {e}"))?;
        let churn_model: ChurnModelConfig = serde_json::from_str(&churn_content)?;

        Ok(Self {
            segments,
            initial_population: 500,
            complaint_triggers: complaint_file.triggers,
            resolution_codes,
            products,
            fee_constraints,
            impact_formulas: fee_file.impact_formulas,
            offers,
            churn_model,
        })
    }

    /// Config with hardcoded defaults for use in unit tests.
    pub fn default_test() -> Self {
        let seg = SegmentConfig {
            id:                          "mass_market".into(),
            label:                       "Mass Market".into(),
            population_share:            1.0,
            income_bands:                vec!["low".into()],
            income_band_weights:         vec![1.0],
            monthly_txn_count_mean:      20.0,
            monthly_txn_count_std:       4.0,
            txn_amount_pareto_xmin:      15.0,
            txn_amount_pareto_alpha:     1.8,
            cash_intensity:              0.35,
            payroll_probability:         0.5,
            payroll_amount_mean:         2000.0,
            payroll_amount_std:          400.0,
            overdraft_probability_per_tick: 0.005,
            nsf_probability_per_tick:    0.002,
            base_churn_rate_per_tick:    0.001,
            fee_sensitivity:             0.8,
            products:                    vec!["basic_checking".into()],
        };

        let triggers = vec![
            ComplaintTrigger {
                event_type:           "fee_charged".into(),
                fee_type:             Some("overdraft".into()),
                amount_threshold:     None,
                prior_breach:         false,
                probability:          0.12,
                issue_category:       "fee_dispute".into(),
                priority:             "standard".into(),
                sla_acknowledge_days: 2,
                sla_resolve_days:     15,
            },
        ];

        let resolution_codes = [
            (
                "explanation_only".into(),
                ResolutionCode {
                    code:                "explanation_only".into(),
                    satisfaction_delta:  -0.02,
                    churn_risk_delta:    0.03,
                    avg_amount_refunded: 0.0,
                },
            ),
            (
                "monetary_relief".into(),
                ResolutionCode {
                    code:                "monetary_relief".into(),
                    satisfaction_delta:  0.15,
                    churn_risk_delta:    -0.10,
                    avg_amount_refunded: 27.08,
                },
            ),
        ].into();

        let products = [(
            "basic_checking".into(),
            ProductConfig {
                product_id:        "basic_checking".into(),
                product_type:      "checking".into(),
                tier:              "basic".into(),
                label:             "Basic Checking".into(),
                monthly_fee:       0.0,
                overdraft_fee:     27.08,
                nsf_fee:           17.72,
                atm_fee:           2.50,
                wire_fee:          25.0,
                interest_rate:     0.0,
                min_balance_waive: None,
                min_dd_waive:      None,
                target_segment:    "mass_market".into(),
            },
        )].into();

        let fee_constraints = [
            (
                "overdraft_fee".into(),
                FeeConstraint {
                    fee_type:           "overdraft_fee".into(),
                    min_value:          0.0,
                    max_value:          35.0,
                    soft_limit:         29.0,
                    soft_limit_warning: "Overdraft fees above $29 add +0.10 to UDAAP risk score".into(),
                    hard_limit_reason:  "FDIC guidance ceiling of $35 per overdraft event".into(),
                },
            ),
            (
                "monthly_fee".into(),
                FeeConstraint {
                    fee_type:           "monthly_fee".into(),
                    min_value:          0.0,
                    max_value:          30.0,
                    soft_limit:         20.0,
                    soft_limit_warning: "Fees above $20/month trigger 1.4x complaint rate multiplier".into(),
                    hard_limit_reason:  "Federal disclosure requirements limit monthly fees to $30".into(),
                },
            ),
            (
                "nsf_fee".into(),
                FeeConstraint {
                    fee_type:           "nsf_fee".into(),
                    min_value:          0.0,
                    max_value:          25.0,
                    soft_limit:         20.0,
                    soft_limit_warning: "NSF fees above $20 add +0.08 to UDAAP risk score".into(),
                    hard_limit_reason:  "Industry best practice ceiling of $25".into(),
                },
            ),
            (
                "atm_fee".into(),
                FeeConstraint {
                    fee_type:           "atm_fee".into(),
                    min_value:          0.0,
                    max_value:          8.0,
                    soft_limit:         5.0,
                    soft_limit_warning: "ATM fees above $5 trigger satisfaction delta -0.05".into(),
                    hard_limit_reason:  "No regulatory ceiling, competitive pressure limits to ~$8".into(),
                },
            ),
            (
                "wire_fee".into(),
                FeeConstraint {
                    fee_type:           "wire_fee".into(),
                    min_value:          0.0,
                    max_value:          50.0,
                    soft_limit:         35.0,
                    soft_limit_warning: "Wire fees above $35 increase premium segment churn sensitivity +0.20".into(),
                    hard_limit_reason:  "No regulatory ceiling, market-driven limit".into(),
                },
            ),
        ].into();

        let impact_formulas = [
            (
                "overdraft_fee".into(),
                FeeImpactFormula {
                    parameters: serde_json::json!({
                        "udaap_risk_threshold": 29.0,
                        "udaap_risk_delta": 0.10
                    }),
                },
            ),
            (
                "nsf_fee".into(),
                FeeImpactFormula {
                    parameters: serde_json::json!({
                        "udaap_risk_threshold": 20.0,
                        "udaap_risk_delta": 0.08
                    }),
                },
            ),
        ].into();

        let offers = [(
            "signup_bonus_100".into(),
            OfferConfig {
                offer_id:     "signup_bonus_100".into(),
                offer_type:   "signup_cash_bonus".into(),
                label:        "$100 Sign-Up Bonus".into(),
                product_id:   Some("basic_checking".into()),
                bonus_amount: 100.0,
                requirements: OfferRequirements {
                    min_direct_deposit: 500.0,
                    min_balance:        100.0,
                    duration_ticks:     60,
                    new_to_bank_only:   true,
                },
                eligibility: OfferEligibility {
                    target_segments:      vec!["mass_market".into()],
                    exclude_segments:     vec![],
                    min_credit_score:     None,
                    max_existing_products: None, // allow customers with their first account
                    min_churn_risk:       None,
                    max_churn_risk:       None,
                },
                cost_model: OfferCostModel {
                    bonus_paid_on_completion: true,
                    promo_rate_duration:      0,
                    promo_rate_delta:         0.0,
                    fee_waiver_duration:      None,
                },
                fraud_risk: OfferFraudRisk {
                    bonus_seeker_probability: 0.15,
                    velocity_flag_threshold:  3,
                },
                active:     true,
                start_tick: 0,
                end_tick:   None,
            },
        )].into();

        let churn_model = ChurnModelConfig {
            model_version: "2.3.0-test".into(),
            update_frequency_ticks: 30,
            segment_base_rates: [(
                "mass_market".into(),
                SegmentChurnParams {
                    monthly_churn_rate:            0.025,
                    annual_churn_rate:             0.26,
                    fee_sensitivity:               0.80,
                    service_sensitivity:           0.70,
                    offer_retention_effectiveness: 0.65,
                },
            )].into(),
            churn_formula: ChurnFormulaWeights {
                base_rate_weight:           1.0,
                satisfaction_weight:        0.40,
                satisfaction_equilibrium:   0.65,
                fee_burden_weight:          0.25,
                fee_burden_threshold:       50.0,
                complaint_weight:           0.20,
                complaint_lookback_ticks:   90,
                sla_breach_weight:          0.35,
                sla_breach_lookback_ticks:  90,
                inactivity_weight:          0.15,
                inactivity_threshold_ticks: 60,
                product_depth_bonus:        -0.08,
                retention_offer_bonus:      -0.15,
                life_event_multiplier:      1.25,
            },
            life_events: vec![
                LifeEventConfig {
                    event_type:           "job_change".into(),
                    probability_per_year: 0.15,
                    segments:             vec![],
                    churn_risk_delta:     0.12,
                    duration_ticks:       90,
                    behavioral_changes:   serde_json::json!({}),
                },
            ],
            churn_thresholds: ChurnThresholds {
                low_risk:       0.30,
                medium_risk:    0.60,
                high_risk:      0.85,
                imminent_churn: 0.95,
            },
        };

        Self {
            segments: [("mass_market".into(), seg)].into(),
            initial_population: 50,
            complaint_triggers: triggers,
            resolution_codes,
            products,
            fee_constraints,
            impact_formulas,
            offers,
            churn_model,
        }
    }
}
